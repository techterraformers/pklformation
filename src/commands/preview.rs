use anyhow::{anyhow, bail, Context};

use aws_sdk_cloudformation::types::{ChangeSetType, StackStatus};

use std::{path::PathBuf, process::Command, time::Duration};
use tracing::{debug, info};

use crate::{aws_client::AwsClient, display::Display};

pub struct PreviewCommand {
    client: AwsClient,
    stack: String,
    template: PathBuf,
    pool_interval: Duration,
    display: Display,
}

impl PreviewCommand {
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

        if wait_result.is_err() {
            self.preview_new_change_set(ChangeSetType::Create).await?;
        } else {
            let (last_status, reason) = wait_result?;
            match last_status {
                StackStatus::DeleteComplete => {
                    self.preview_new_change_set(ChangeSetType::Create).await?;
                }
                StackStatus::CreateComplete
                | StackStatus::ImportComplete
                | StackStatus::UpdateComplete
                | StackStatus::UpdateRollbackComplete => {
                    self.preview_new_change_set(ChangeSetType::Update).await?;
                }
                StackStatus::ReviewInProgress => {
                    self.preview_exisint_change_set().await?;
                }
                _ => {
                    tracing::error!("Preview failed with status: {last_status:?}, reason: {reason:?}. Check the AWS Console");
                    return Ok(());
                }
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

    async fn preview_new_change_set(&self, change_set_type: ChangeSetType) -> anyhow::Result<()> {
        info!("Preview stack {} ...", self.stack);
        let template = self.eval_template().await?;
        let change_set = self
            .client
            .create_or_update_change_set(&self.stack, &template, change_set_type)
            .await?;
        let change_set_id = change_set.id().context("Empty change set id")?;
        self.client
            .wait_until_change_set_op_in_progress(change_set_id, self.pool_interval)
            .await?;
        let change_set_description = self.client.describe_change_set(change_set_id).await?;
        self.display.print_change_set(&change_set_description);

        Ok(())
    }

    async fn preview_exisint_change_set(&self) -> anyhow::Result<()> {
        print!("Found a pending change set:");
        let pending_change_set = self
            .client
            .pending_change_set(&self.stack)
            .await?
            .ok_or(anyhow!("Pending changeset not found"))?;

        let change_set_id = pending_change_set
            .change_set_id
            .as_deref()
            .context("Empty change set id")?;
        let pending_change_set_description = self.client.describe_change_set(change_set_id).await?;
        self.display
            .print_change_set(&pending_change_set_description);

        Ok(())
    }
}
