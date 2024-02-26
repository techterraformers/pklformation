use anyhow::{anyhow, bail, Context};

use aws_sdk_cloudformation::types::{ChangeSetType, StackStatus};

use std::{path::PathBuf, process::Command, time::Duration};
use tracing::{debug, info};

use crate::{
    aws_client::AwsClient,
    display::{ask_confirm, print_change_set},
};

pub struct UpCommand {
    client: AwsClient,
    stack: String,
    template: PathBuf,
    pool_interval: Duration,
}

impl UpCommand {
    pub fn new(
        client: AwsClient,
        stack: String,
        template: PathBuf,
        pool_interval: Duration,
    ) -> Self {
        Self {
            client,
            stack,
            template,
            pool_interval,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let wait_result = self
            .client
            .wait_until_op_in_progress(&self.stack, self.pool_interval)
            .await;

        if wait_result.is_err() {
            self.create_or_update(ChangeSetType::Create).await?;
        } else {
            let (last_status, reason) = wait_result?;
            match last_status {
                StackStatus::DeleteComplete => {
                    self.create_or_update(ChangeSetType::Create).await?;
                }
                StackStatus::CreateComplete
                | StackStatus::ImportComplete
                | StackStatus::UpdateComplete
                | StackStatus::UpdateRollbackComplete => {
                    self.create_or_update(ChangeSetType::Update).await?;
                }
                StackStatus::CreateFailed | StackStatus::RollbackComplete => {
                    self.recreate().await?;
                }
                StackStatus::ReviewInProgress => {
                    self.continue_pending_change_set().await?;
                }
                _ => {
                    tracing::error!("Up failed with status: {last_status:?}, reason: {reason:?}. Check the AWS Console");
                    return Ok(());
                }
            }
        }

        let (op_status, reason) = self
            .client
            .wait_until_op_in_progress(&self.stack, self.pool_interval)
            .await?;

        match op_status {
            StackStatus::CreateComplete | StackStatus::UpdateComplete => {
                info!("Up compleated successfully!")
            }
            _ => tracing::error!("Up failed with status: {op_status:?}, reason: {reason:?}"),
        }
        Ok(())
    }

    pub async fn eval_template(&self) -> anyhow::Result<String> {
        let template_eval_result = Command::new("pkl")
            .args([
                "eval",
                self.template.to_str().unwrap(),
                "--project-dir",
                self.template.parent().unwrap().to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()?;

        debug!("pkl eval result: {template_eval_result:?}");
        if !template_eval_result.status.success() {
            bail!(String::from_utf8(template_eval_result.stderr)?);
        }

        Ok(String::from_utf8(template_eval_result.stdout)?)
    }

    async fn create_or_update(&self, change_set_type: ChangeSetType) -> anyhow::Result<()> {
        let template = self.eval_template().await?;
        let change_set = self
            .client
            .create_or_update_change_set(&self.stack, &template, change_set_type)
            .await?;
        let change_set_id = change_set.id().context("Empty change set id")?;
        let change_set_description = self.client.describe_change_set(change_set_id).await?;
        print_change_set(&change_set_description);

        if ask_confirm("Do you want to continue?") {
            self.client.execute_change_set(change_set_id).await?;
        } else {
            self.client.delete_change_set(change_set_id).await?;
        }

        Ok(())
    }

    async fn recreate(&self) -> anyhow::Result<()> {
        info!("Past creation of the {} failed", self.stack);
        if ask_confirm("Do you want to re-create the stack?") {
            info!("Re-create stack {}...", self.stack);
            self.client.delete(&self.stack).await?;
            let _ = self
                .client
                .wait_until_op_in_progress(&self.stack, self.pool_interval)
                .await;
            self.create_or_update(ChangeSetType::Create).await?;
            info!("Stack {} re-created!", self.stack);
        }
        Ok(())
    }

    async fn continue_pending_change_set(&self) -> anyhow::Result<()> {
        print!("Found a pending change set:");
        let pending_change_set = self
            .client
            .pending_change_set(&self.stack)
            .await?
            .ok_or(anyhow!("Pending change set not found"))?;

        let change_set_id = pending_change_set
            .change_set_id
            .as_deref()
            .context("Empty change set id")?;
        let pending_change_set_description = self.client.describe_change_set(change_set_id).await?;
        print_change_set(&pending_change_set_description);
        if ask_confirm("Do you want to apply this change set?") {
            self.client.execute_change_set(change_set_id).await?;
        } else if ask_confirm("Do you want to delete this change set?") {
            self.client.delete_change_set(change_set_id).await?;
        }

        Ok(())
    }
}
