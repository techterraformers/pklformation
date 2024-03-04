use std::{thread, time::Duration};

use anyhow::Context;
use aws_config::BehaviorVersion;
use aws_sdk_cloudformation::{
    operation::{
        create_change_set::CreateChangeSetOutput, describe_change_set::DescribeChangeSetOutput,
    },
    types::{ChangeSetStatus, ChangeSetSummary, ChangeSetType, ExecutionStatus, StackEvent, StackStatus},
    Client,
};
use chrono::Utc;
use spinners::{Spinner, Spinners};
use tracing::{debug, info};

pub struct AwsClient {
    inner: Client,
}

impl AwsClient {
    pub async fn new() -> Self {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        Self {
            inner: aws_sdk_cloudformation::Client::new(&config),
        }
    }

    pub async fn describe_change_set(
        &self,
        change_set_id: &str,
    ) -> anyhow::Result<DescribeChangeSetOutput> {
        let describe_change_set = self
            .inner
            .describe_change_set()
            .change_set_name(change_set_id)
            .send()
            .await?;
        debug!("Change set desription: {:?}", &describe_change_set);
        Ok(describe_change_set)
    }

    pub async fn change_set_status(
        &self,
        change_set_id: &str,
    ) -> anyhow::Result<(ChangeSetStatus, String)> {
        let describe_change_set_output = self.describe_change_set(change_set_id).await?;
        Ok((
            describe_change_set_output
                .status
                .as_ref()
                .context("Stack without status")?
                .clone(),
            describe_change_set_output
                .status_reason()
                .unwrap_or("Unknown reason")
                .to_owned(),
        ))
    }

    pub async fn delete_change_set(&self, change_set_id: &str) -> anyhow::Result<()> {
        let delete_change_set_result = self
            .inner
            .delete_change_set()
            .change_set_name(change_set_id)
            .send()
            .await?;
        debug!("Delete Change set resul: {:?}", &delete_change_set_result);
        Ok(())
    }

    pub async fn stack_status(&self, stack_name: &str) -> anyhow::Result<(StackStatus, String)> {
        let describe_stacks_output = self
            .inner
            .describe_stacks()
            .stack_name(stack_name)
            .send()
            .await?;
        let stacks = describe_stacks_output.stacks.context("No stacks list")?;
        let stack = stacks.first().context("Empty stacks list")?;

        Ok((
            stack
                .stack_status
                .as_ref()
                .context("Stack without status")?
                .clone(),
            stack
                .stack_status_reason()
                .unwrap_or("Unknown reason")
                .to_owned(),
        ))
    }

    pub async fn create_or_update_change_set(
        &self,
        stack_name: &str,
        template: &str,
        change_set_type: ChangeSetType,
    ) -> anyhow::Result<CreateChangeSetOutput> {
        info!("{change_set_type:?} stack {stack_name}...");
        let change_set_name = format!("{}-{}", stack_name, Utc::now().format("%Y%m%d-%H%M%S-%f"));
        info!("Create change set {change_set_name}...");
        let changeset = self
            .inner
            .create_change_set()
            .stack_name(stack_name)
            .change_set_name(change_set_name.clone())
            .change_set_type(change_set_type.clone())
            .template_body(template)
            .send()
            .await?;

        info!("{change_set_type:?} change set {stack_name} done!");
        Ok(changeset)
    }

    pub async fn execute_change_set(&self, change_set_id: &str) -> anyhow::Result<()> {
        info!("Apply change set {change_set_id}!",);
        let execution_result = self
            .inner
            .execute_change_set()
            .change_set_name(change_set_id)
            .send()
            .await?;

        debug!("Execution result: {execution_result:?}");
        info!("Change Set {change_set_id} applied!");
        Ok(())
    }

    pub async fn describe_stack_events(
        &self,
        stack: &str,
    ) -> anyhow::Result<Vec<StackEvent>> {
        info!("Describe stack events {stack}!",);
        let stack_events: Vec<_> = self
            .inner
            .describe_stack_events()
            .stack_name(stack)
            .into_paginator()
            .items()
            .send()
            .collect::<Result<Vec<_>,_>>()
            .await?;

        debug!("Describe stack events result: {stack_events:?}");
        Ok(stack_events)
    }

    pub async fn delete_stack(&self, stack_name: &str) -> anyhow::Result<()> {
        info!("Delete stack {stack_name}...");
        let deletation_result = self
            .inner
            .delete_stack()
            .stack_name(stack_name)
            .send()
            .await?;
        debug!("Deletation result: {deletation_result:?}");

        info!("Stack {stack_name} deleted!");
        Ok(())
    }

    fn stack_op_in_progres(status: &StackStatus) -> bool {
        matches!(
            status,
            StackStatus::CreateInProgress
                | StackStatus::DeleteInProgress
                | StackStatus::ImportInProgress
                | StackStatus::ImportRollbackInProgress
                | StackStatus::RollbackInProgress
                | StackStatus::UpdateCompleteCleanupInProgress
                | StackStatus::UpdateInProgress
                | StackStatus::UpdateRollbackCompleteCleanupInProgress
                | StackStatus::UpdateRollbackInProgress
        )
    }

    fn change_set_op_in_progres(status: &ChangeSetStatus) -> bool {
        matches!(
            status,
            ChangeSetStatus::CreateInProgress
                | ChangeSetStatus::CreatePending
                | ChangeSetStatus::DeleteInProgress
                | ChangeSetStatus::DeletePending
        )
    }

    pub async fn wait_until_stack_op_in_progress(
        &self,
        stack_name: &str,
        pool_interval: Duration,
    ) -> anyhow::Result<(StackStatus, String)> {
        let (status, reason) = self.stack_status(stack_name).await?;

        if Self::stack_op_in_progres(&status) {
            let mut sp = Spinner::new(Spinners::Dots9, format!("Waiting for {status:?}"));
            loop {
                let (status, reason) = self.stack_status(stack_name).await?;
                thread::sleep(pool_interval);
                if !Self::stack_op_in_progres(&status) {
                    sp.stop();
                    return Ok((status, reason));
                }
            }
        }

        Ok((status, reason))
    }

    pub async fn wait_until_change_set_op_in_progress(
        &self,
        change_set_id: &str,
        pool_interval: Duration,
    ) -> anyhow::Result<(ChangeSetStatus, String)> {
        let (status, reason) = self.change_set_status(change_set_id).await?;

        if Self::change_set_op_in_progres(&status) {
            let mut sp = Spinner::new(Spinners::Dots9, format!("Waiting for {status:?}"));
            loop {
                let (status, reason) = self.change_set_status(change_set_id).await?;
                thread::sleep(pool_interval);
                if !Self::change_set_op_in_progres(&status) {
                    sp.stop();
                    return Ok((status, reason));
                }
            }
        }

        Ok((status, reason))
    }

    pub async fn pending_change_set(
        &self,
        stack_name: &str,
    ) -> anyhow::Result<Option<ChangeSetSummary>> {
        let list_change_set = self
            .inner
            .list_change_sets()
            .stack_name(stack_name)
            .send()
            .await?;
        Ok(list_change_set
            .summaries()
            .iter()
            .find(|cs| matches!(cs.execution_status, Some(ExecutionStatus::Available)))
            .cloned())
    }
}
