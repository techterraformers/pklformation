use anyhow::{bail, Context};
use aws_sdk_cloudformation::{
    error::SdkError,
    types::{ChangeSetType, StackStatus},
    Client,
};
use chrono::Utc;
use std::{
    io::{self, Read},
    path::PathBuf,
    process::Command,
};
use tracing::{debug, info};

pub struct UpCommand {
    client: Client,
    stack: String,
    template: PathBuf,
}

pub enum StackOperation {
    Wait,
    Stop(String),
    Delete,
    Create,
    Update,
}

impl UpCommand {
    pub fn new(client: Client, stack: String, template: PathBuf) -> Self {
        Self {
            client,
            stack,
            template,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
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

        let template = String::from_utf8(template_eval_result.stdout)?;

        let stack_operation = self.stack_operation().await?;

        match stack_operation {
            StackOperation::Wait => print!("wait"),
            StackOperation::Stop(_) => print!("stop"),
            StackOperation::Delete => print!("delete"),
            StackOperation::Create => print!("create"),
            StackOperation::Update => print!("update"),
        }

        let change_set_name = format!("{}-{}", self.stack, Utc::now().format("%Y%m%d-%H%M%S-%f"));
        info!("Crate Change Set: {change_set_name}");

        let changeset = self
            .client
            .create_change_set()
            .stack_name(self.stack.clone())
            .change_set_name(change_set_name)
            // .change_set_type(change_set_type)
            .template_body(template)
            .send()
            .await?;

        debug!("New changeset: {changeset:?}");
        print!("{changeset:?}");

        if self.ask_confirm() {
            self.client
                .execute_change_set()
                .change_set_name(changeset.id.context("Empty changeset ID")?)
                .send()
                .await?;
            info!("Change Set applied");
        }

        Ok(())
    }

    fn ask_confirm(&self) -> bool {
        println!("Do you want procede [y/N]?");
        let mut input = [0];
        io::stdin().read_exact(&mut input).unwrap();
        match input[0] as char {
            'y' | 'Y' => return true,
            'n' | 'N' => return false,
            _ => false,
        }
    }

    async fn stack_operation(&self) -> anyhow::Result<StackOperation> {
        let describe_stack = self
            .client
            .describe_stacks()
            .stack_name(self.stack.clone())
            .send()
            .await;

        debug!("Stack description: {describe_stack:?}");
        if describe_stack.is_err() {
            if let Err(SdkError::ServiceError(_)) = describe_stack {
                Ok(StackOperation::Create)
            } else {
                bail!("Error while retreiving the status description: {describe_stack:?}")
            }
        } else {
            let stacks = describe_stack?.stacks.context("No stacks list")?;

            let stack = stacks.get(0).context("Empty stacks list")?;

            let stack_status = stack
                .stack_status
                .as_ref()
                .context("Stack without status")?;
            let status_reason = stack
                .stack_status_reason()
                .unwrap_or("Unknow: check the aws cosole")
                .to_owned();
            let op = match stack_status {
                StackStatus::CreateComplete => StackOperation::Update,
                StackStatus::CreateFailed => StackOperation::Delete,
                StackStatus::CreateInProgress => StackOperation::Wait,
                StackStatus::DeleteComplete => StackOperation::Create,
                StackStatus::DeleteFailed => StackOperation::Stop(status_reason),
                StackStatus::DeleteInProgress => StackOperation::Wait,
                StackStatus::ImportComplete => StackOperation::Update,
                StackStatus::ImportInProgress => StackOperation::Wait,
                StackStatus::ImportRollbackComplete => StackOperation::Delete,
                StackStatus::ImportRollbackFailed => StackOperation::Stop(status_reason),
                StackStatus::ImportRollbackInProgress => StackOperation::Wait,
                StackStatus::ReviewInProgress => StackOperation::Wait,
                StackStatus::RollbackComplete => StackOperation::Update,
                StackStatus::RollbackFailed => StackOperation::Delete,
                StackStatus::RollbackInProgress => StackOperation::Wait,
                StackStatus::UpdateComplete => StackOperation::Update,
                StackStatus::UpdateCompleteCleanupInProgress => StackOperation::Wait,
                StackStatus::UpdateFailed => StackOperation::Stop(status_reason),
                StackStatus::UpdateInProgress => StackOperation::Wait,
                StackStatus::UpdateRollbackComplete => StackOperation::Update,
                StackStatus::UpdateRollbackCompleteCleanupInProgress => StackOperation::Wait,
                StackStatus::UpdateRollbackFailed => StackOperation::Stop(status_reason),
                StackStatus::UpdateRollbackInProgress => StackOperation::Wait,
                _ => StackOperation::Stop(status_reason),
            };

            Ok(op)
        }
    }
}
