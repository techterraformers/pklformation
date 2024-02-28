use anyhow::{anyhow, bail, Context};

use aws_sdk_cloudformation::types::{ChangeSetStatus, ChangeSetType, ResourceStatus, StackStatus};

use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::{debug, info};

use crate::{aws_client::AwsClient, display::Display};

pub struct UpCommand {
    client: AwsClient,
    stack: String,
    template: PathBuf,
    pool_interval: Duration,
    display: Display,
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
            display: Display::new(),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let wait_result = self
            .client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await;

        let start_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
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

        let (op_status, _reason) = self
            .client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await?;

        match op_status {
            StackStatus::CreateComplete | StackStatus::UpdateComplete => {
                info!("Up compleated successfully!")
            }
            _ => {
                tracing::error!("Up failed with status: {op_status:?}");
                let events = self
                    .client
                    .describe_stack_events(&self.stack)
                    .await?
                    .into_iter()
                    .filter(|p| {
                        p.timestamp().map(|t| t.as_secs_f64()).unwrap_or_default() > start_time
                    });
                self.display.print_resources_errors(events);
            }
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
        info!("Create stack {} ...", self.stack);
        let template = self.eval_template().await?;
        let change_set = self
            .client
            .create_or_update_change_set(&self.stack, &template, change_set_type)
            .await?;
        let change_set_id = change_set.id().context("Empty change set id")?;
        self.client
            .wait_until_change_set_op_in_progress(&change_set_id, self.pool_interval)
            .await?;
        let change_set_description = self.client.describe_change_set(change_set_id).await?;
        self.display.print_change_set(&change_set_description);

        if self.display.ask_confirm("Do you want to continue?") {
            self.client.execute_change_set(change_set_id).await?;
        } else {
            self.client.delete_change_set(change_set_id).await?;
        }
        self.client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await?;

        Ok(())
    }

    async fn recreate(&self) -> anyhow::Result<()> {
        info!(
            "Past creation of the stack {} failed, re-create stack...",
            self.stack
        );
        info!("Re-create stack {}...", self.stack);
        self.client.delete(&self.stack).await?;
        let _ = self
            .client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await;
        self.create_or_update(ChangeSetType::Create).await?;
        info!("Stack {} re-created!", self.stack);
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
        self.display
            .print_change_set(&pending_change_set_description);
        if self
            .display
            .ask_confirm("Do you want to apply this change set?")
        {
            self.client.execute_change_set(change_set_id).await?;
            self.client
                .wait_until_change_set_op_in_progress(change_set_id, self.pool_interval)
                .await?;
        } else if self
            .display
            .ask_confirm("Do you want to create a new change set?")
        {
            self.client.delete_change_set(change_set_id).await?;
            let (status, reason) = self
                .client
                .wait_until_change_set_op_in_progress(change_set_id, self.pool_interval)
                .await?;
            if status == ChangeSetStatus::DeleteComplete {
                self.create_or_update(ChangeSetType::Update).await?;
            } else {
                bail!(
                    "Unable to delete the change set {}: {}",
                    change_set_id,
                    reason
                );
            }
        }

        Ok(())
    }
}
