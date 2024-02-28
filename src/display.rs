use aws_sdk_cloudformation::{
    operation::describe_change_set::DescribeChangeSetOutput,
    types::{ChangeAction, ResourceStatus, StackEvent},
};
use colored::Colorize;
use dialoguer::Confirm;
use std::io::Write;

pub fn ask_confirm(msg: &str) -> bool {
    Confirm::new()
        .with_prompt(msg)
        .default(false)
        .interact()
        .unwrap()
}

const UNKNOWN_REASON: &str = "UNKNOW REASON";
const UNKNOWN_RESOURCE_TYPE: &str = "UNKNOW RESOURCE TYPE";
const UNKNOWN_RESOURCE_LOGICAL_ID: &str = "UNKNOW RESOURCE LOGICAL ID";
pub fn print_change_set(change_set: &DescribeChangeSetOutput) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();

    writeln!(lock, "The following changes will performed to the stack:").unwrap();
    writeln!(
        lock,
        "Stack: {}",
        change_set.stack_name.as_deref().unwrap_or("UNKOWN STACK")
    )
    .unwrap();
    writeln!(
        lock,
        "Change set: {}",
        change_set
            .change_set_name
            .as_deref()
            .unwrap_or("UNKOWN CHANGE SET")
    )
    .unwrap();

    change_set
        .changes()
        .iter()
        .filter_map(|c| c.resource_change.as_ref())
        .for_each(|rc| {
            let header = format!(
                "{}: {} ",
                rc.resource_type.as_deref().unwrap_or(UNKNOWN_RESOURCE_TYPE),
                rc.logical_resource_id
                    .as_deref()
                    .unwrap_or(UNKNOWN_RESOURCE_LOGICAL_ID)
            );
            match rc.action.as_ref().unwrap() {
                ChangeAction::Add => writeln!(lock, "{} {}", "+".green(), header.green()).unwrap(),
                ChangeAction::Modify => {
                    writeln!(lock, "{} {}", "~".yellow(), header.yellow()).unwrap()
                }
                ChangeAction::Remove => writeln!(lock, "{} {}", "-".red(), header.red()).unwrap(),
                _ => writeln!(lock, "? {}", "Unknown Change Type".purple()).unwrap(),
            }
        })
}

pub fn print_resources_errors(events: impl Iterator<Item = StackEvent>) {
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
