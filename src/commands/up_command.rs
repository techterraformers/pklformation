use anyhow::{bail, Context};

use spinners::{Spinner, Spinners};

use aws_sdk_cloudformation::{
    types::{ChangeSetType, StackStatus},
    Client,
};
use chrono::Utc;
use core::str;
use std::{
    io::{self, Read},
    path::PathBuf,
    process::Command,
    thread,
    time::Duration,
};
use tracing::{debug, info};

pub struct UpCommand {
    client: Client,
    stack: String,
    template: PathBuf,
    pool_interval: Duration,
}

fn ask_confirm() -> bool {
    println!("Do you want procede [y/N]?");
    let mut input = [0];
    io::stdin().read_exact(&mut input).unwrap();
    match input[0] as char {
        'y' | 'Y' => true,
        'n' | 'N' => false,
        _ => false,
    }
}

async fn stack_status(client: &Client, stack_name: &str) -> anyhow::Result<(StackStatus, String)> {
    let describe_stacks_output = client
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

async fn create_or_update(
    client: &Client,
    stack_name: &str,
    template: &str,
    change_set_type: ChangeSetType,
) -> anyhow::Result<()> {
    info!("{change_set_type:?} stack {stack_name}...");
    let change_set_name = format!("{}-{}", stack_name, Utc::now().format("%Y%m%d-%H%M%S-%f"));
    info!("Create change set {change_set_name}...");
    let changeset = client
        .create_change_set()
        .stack_name(stack_name)
        .change_set_name(change_set_name.clone())
        .change_set_type(change_set_type)
        .template_body(template)
        .send()
        .await?;
    debug!("Create change set {changeset:?}");
    info!("Change set {change_set_name} created!");

    if ask_confirm() {
        info!("Apply change set {change_set_name}!");
        let execution_result = client
            .execute_change_set()
            .change_set_name(changeset.id.context("Empty changeset ID")?)
            .send()
            .await?;

        debug!("Execution result: {execution_result:?}");
        info!("Change Set {change_set_name} applied!");
    }

    Ok(())
}

async fn delete(client: &Client, stack_name: &str) -> anyhow::Result<()> {
    info!("Delete stack {stack_name}...");
    let deletation_result = client.delete_stack().stack_name(stack_name).send().await?;
    debug!("Deletation result: {deletation_result:?}");

    info!("Stack {stack_name} deleted!");
    Ok(())
}
async fn recreate(
    client: &Client,
    stack_name: &str,
    template: &str,
    pool_interval: Duration,
) -> anyhow::Result<()> {
    info!("Re-create stack {stack_name}...");
    delete(client, stack_name).await?;
    let _ = wait_until_op_in_progress(client, stack_name, pool_interval).await;
    create_or_update(client, stack_name, template, ChangeSetType::Create).await?;
    info!("Stack {stack_name} re-created!");
    Ok(())
}

fn op_in_progres(status: &StackStatus) -> bool {
    matches!(
        status,
        StackStatus::CreateInProgress
            | StackStatus::DeleteInProgress
            | StackStatus::ImportInProgress
            | StackStatus::ImportRollbackInProgress
            | StackStatus::ReviewInProgress
            | StackStatus::RollbackInProgress
            | StackStatus::UpdateCompleteCleanupInProgress
            | StackStatus::UpdateInProgress
            | StackStatus::UpdateRollbackCompleteCleanupInProgress
            | StackStatus::UpdateRollbackInProgress
    )
}

async fn wait_until_op_in_progress(
    client: &Client,
    stack_name: &str,
    pool_interval: Duration,
) -> anyhow::Result<(StackStatus, String)> {
    let (status, reason) = stack_status(client, stack_name).await?;

    if op_in_progres(&status) {
        let mut sp = Spinner::new(Spinners::Dots9, format!("Waiting for {status:?}"));
        loop {
            let (status, reason) = stack_status(client, stack_name).await?;
            thread::sleep(pool_interval);
            if !op_in_progres(&status) {
                sp.stop();
                return Ok((status, reason));
            }
        }
    }

    Ok((status, reason))
}

impl UpCommand {
    pub fn new(client: Client, stack: String, template: PathBuf, pool_interval: Duration) -> Self {
        Self {
            client,
            stack,
            template,
            pool_interval,
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

        let wait_result =
            wait_until_op_in_progress(&self.client, &self.stack, self.pool_interval).await;

        if wait_result.is_err() {
            create_or_update(&self.client, &self.stack, &template, ChangeSetType::Create).await?;
        } else {
            let (last_status, reason) = wait_result?;
            match last_status {
                StackStatus::DeleteComplete => {
                    create_or_update(&self.client, &self.stack, &template, ChangeSetType::Create)
                        .await?;
                }
                StackStatus::CreateComplete
                | StackStatus::ImportComplete
                | StackStatus::UpdateComplete
                | StackStatus::UpdateRollbackComplete => {
                    create_or_update(&self.client, &self.stack, &template, ChangeSetType::Update)
                        .await?;
                }
                StackStatus::CreateFailed | StackStatus::RollbackComplete => {
                    recreate(&self.client, &self.stack, &template, self.pool_interval).await?;
                }
                _ => {
                    tracing::error!("Up failed with status: {last_status:?}, reason: {reason:?}. Check the AWS Console");
                    return Ok(());
                }
            }
        }

        let (op_status, reason) =
            wait_until_op_in_progress(&self.client, &self.stack, self.pool_interval).await?;

        match op_status {
            StackStatus::CreateComplete | StackStatus::UpdateComplete => {
                info!("Up compleated successfully!")
            }
            _ => tracing::error!("Up failed with status: {op_status:?}, reason: {reason:?}"),
        }
        Ok(())
    }
}
