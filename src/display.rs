use aws_sdk_cloudformation::{
    operation::describe_change_set::DescribeChangeSetOutput, types::ChangeAction,
};
use colored::Colorize;
use dialoguer::Confirm;

pub fn ask_confirm(msg: &str) -> bool {
    Confirm::new()
        .with_prompt(msg)
        .default(false)
        .interact()
        .unwrap()
}

pub fn print_change_set(change_set: &DescribeChangeSetOutput) {
    println!("The following changes will performed to the stack:");
    println!(
        "Stack: {}",
        change_set.stack_name.as_deref().unwrap_or("UNKOWN STACK")
    );
    println!(
        "Change set: {}",
        change_set
            .change_set_name
            .as_deref()
            .unwrap_or("UNKOWN CHANGE SET")
    );

    change_set
        .changes()
        .iter()
        .filter_map(|c| c.resource_change.as_ref())
        .for_each(|rc| {
            let header = format!(
                "{}: {} ",
                rc.resource_type.as_deref().unwrap_or("UNKNOW RESURCE TYPE"),
                rc.logical_resource_id
                    .as_deref()
                    .unwrap_or("UNKNOW RESOUCE LOGICAL ID")
            );
            match rc.action.as_ref().unwrap() {
                ChangeAction::Add => println!("{} {}", "+".green(), header.green()),
                ChangeAction::Modify => println!("{} {}", "~".yellow(), header.yellow()),
                ChangeAction::Remove => println!("{} {}", "-".red(), header.red()),
                _ => println!("? {}", "Unknown Change Type".purple()),
            }
        })
}
