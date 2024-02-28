use aws_sdk_cloudformation::{
    operation::describe_change_set::DescribeChangeSetOutput,
    types::{ChangeAction, ChangeSetStatus, Replacement, ResourceStatus, StackEvent, StackStatus},
};
use colored::Colorize;
use dialoguer::Confirm;
use std::io::Write;

const UNKNOWN_RESOURCE_TYPE: &str = "UNKNOW RESOURCE TYPE";
const UNKNOWN_REASON: &str = "UNKNOW REASON";
const UNKNOWN_RESOURCE_LOGICAL_ID: &str = "UNKNOW RESOURCE LOGICAL ID";

pub struct Display {}
impl Display {
    pub fn new() -> Self {
        Self {}
    }

    pub fn colorize_by_stack_status(self, stack_status: &StackStatus, str: &str) -> String {
        match stack_status {
            StackStatus::CreateComplete => str.green().to_string(),
            StackStatus::CreateFailed => str.red().to_string(),
            StackStatus::CreateInProgress => str.yellow().to_string(),
            StackStatus::DeleteComplete => str.green().to_string(),
            StackStatus::DeleteFailed => str.red().to_string(),
            StackStatus::DeleteInProgress => str.yellow().to_string(),
            StackStatus::ImportComplete => str.green().to_string(),
            StackStatus::ImportInProgress => str.yellow().to_string(),
            StackStatus::ImportRollbackComplete => str.green().to_string(),
            StackStatus::ImportRollbackFailed => str.red().to_string(),
            StackStatus::ImportRollbackInProgress => str.yellow().to_string(),
            StackStatus::ReviewInProgress => str.yellow().to_string(),
            StackStatus::RollbackComplete => str.green().to_string(),
            StackStatus::RollbackFailed => str.red().to_string(),
            StackStatus::RollbackInProgress => str.yellow().to_string(),
            StackStatus::UpdateComplete => str.green().to_string(),
            StackStatus::UpdateCompleteCleanupInProgress => str.green().to_string(),
            StackStatus::UpdateFailed => str.red().to_string(),
            StackStatus::UpdateInProgress => str.yellow().to_string(),
            StackStatus::UpdateRollbackComplete => str.green().to_string(),
            StackStatus::UpdateRollbackCompleteCleanupInProgress => str.yellow().to_string(),
            StackStatus::UpdateRollbackFailed => str.red().to_string(),
            StackStatus::UpdateRollbackInProgress => str.yellow().to_string(),
            _ => str.red().to_string(),
        }
    }

    pub fn colorize_by_change_set_status(
        &self,
        change_set_status: &ChangeSetStatus,
        str: String,
    ) -> String {
        match change_set_status {
            ChangeSetStatus::CreateComplete => str.green().to_string(),
            ChangeSetStatus::CreateInProgress => str.yellow().to_string(),
            ChangeSetStatus::CreatePending => str.yellow().to_string(),
            ChangeSetStatus::DeleteComplete => str.green().to_string(),
            ChangeSetStatus::DeleteFailed => str.red().to_string(),
            ChangeSetStatus::DeleteInProgress => str.yellow().to_string(),
            ChangeSetStatus::DeletePending => str.yellow().to_string(),
            ChangeSetStatus::Failed => str.red().to_string(),
            _ => str.red().to_string(),
        }
    }

    pub fn colorize_by_change_action(&self, change_action: &ChangeAction, str: String) -> String {
        match change_action {
            ChangeAction::Add => str.green().to_string(),
            ChangeAction::Dynamic => str.purple().to_string(),
            ChangeAction::Import => str.green().to_string(),
            ChangeAction::Modify => str.yellow().to_string(),
            ChangeAction::Remove => str.red().to_string(),
            _ => str.red().to_string(),
        }
    }

    pub fn colorize_by_replacement(&self, replacement: &Replacement, str: String) -> String {
        match replacement {
            Replacement::Conditional => str.yellow().to_string(),
            Replacement::False => str.green().to_string(),
            Replacement::True => str.red().to_string(),
            _ => str.red().to_string(),
        }
    }

    pub fn change_action_simbol(&self, action: &ChangeAction) -> &'static str {
        match action {
            ChangeAction::Add => "+",
            ChangeAction::Dynamic => "~/+",
            ChangeAction::Modify => "~",
            ChangeAction::Remove => "-",
            _ => "?",
        }
    }

    pub fn ask_confirm(&self, msg: &str) -> bool {
        Confirm::new()
            .with_prompt(msg)
            .default(false)
            .interact()
            .unwrap()
    }

    pub fn print_change_set(&self, change_set: &DescribeChangeSetOutput) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();

        writeln!(
            lock,
            "Change set: {}",
            change_set
                .change_set_name
                .as_deref()
                .unwrap_or("UNKOWN CHANGE SET")
        )
        .unwrap();

        if let Some(status) = change_set.status.as_ref() {
            writeln!(
                lock,
                "{}",
                self.colorize_by_change_set_status(
                    &status,
                    format!("Change set status: {status:?}")
                )
            )
            .unwrap();
        }

        change_set
            .changes()
            .iter()
            .filter_map(|c| c.resource_change.as_ref())
            .for_each(|rc| {
                let header = format!(
                    "{} ({})",
                    rc.logical_resource_id
                        .as_deref()
                        .unwrap_or(UNKNOWN_RESOURCE_LOGICAL_ID),
                    rc.resource_type.as_deref().unwrap_or(UNKNOWN_RESOURCE_TYPE),
                );

                writeln!(
                    lock,
                    "{} {}",
                    self.change_action_simbol(rc.action().unwrap()),
                    self.colorize_by_change_action(rc.action().unwrap(), format!("+ {}", header))
                )
                .unwrap();

                writeln!(
                    lock,
                    "Action: {}",
                    self.colorize_by_change_action(
                        rc.action().unwrap(),
                        format!("{:?}", rc.action().unwrap())
                    )
                )
                .unwrap();

                if let Some(replacement) = rc.replacement() {
                    writeln!(
                        lock,
                        "Replacement: {}",
                        self.colorize_by_replacement(replacement, format!("{:?}", replacement))
                    )
                    .unwrap();
                }
            })
    }

    pub fn print_resources_errors(&self, events: impl Iterator<Item = StackEvent>) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        events
            .filter(|p| {
                matches!(
                    p.resource_status(),
                    Some(ResourceStatus::UpdateFailed) | Some(ResourceStatus::CreateFailed)
                )
            })
            .for_each(|error| {
                writeln!(
                    lock,
                    "{}: {}",
                    error.resource_type().unwrap_or(UNKNOWN_RESOURCE_TYPE).red(),
                    error
                        .logical_resource_id()
                        .unwrap_or(UNKNOWN_RESOURCE_LOGICAL_ID)
                        .red()
                )
                .unwrap();
                writeln!(
                    lock,
                    "reason: {}",
                    error
                        .resource_status_reason()
                        .unwrap_or(UNKNOWN_REASON)
                        .red(),
                )
                .unwrap();
                writeln!(
                    lock,
                    "properties: {}",
                    error.resource_properties().unwrap_or("").red(),
                )
                .unwrap();
            });
    }
}
