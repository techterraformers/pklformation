use aws_sdk_cloudformation::{
    operation::{
        describe_change_set::DescribeChangeSetOutput,
        list_stack_resources::ListStackResourcesOutput,
    },
    types::{
        ChangeAction, ChangeSetStatus, Parameter, Replacement, RequiresRecreation, ResourceStatus,
        Stack, StackEvent, StackStatus, StackSummary,
    },
};
use colored::Colorize;
use dialoguer::Confirm;
use std::io::Write;

const UNKNOWN_RESOURCE_TYPE: &str = "UNKNOW RESOURCE TYPE";
const UNKNOWN_REASON: &str = "UNKNOW REASON";
const UNKNOWN_RESOURCE_LOGICAL_ID: &str = "UNKNOW RESOURCE LOGICAL ID";

struct ChangeActionSimbol(ChangeAction);

impl std::fmt::Display for ChangeActionSimbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ChangeAction::Add => write!(f, "+"),
            ChangeAction::Dynamic => write!(f, "~/+"),
            ChangeAction::Modify => write!(f, "~"),
            ChangeAction::Remove => write!(f, "-"),
            _ => write!(f, "?"),
        }
    }
}

enum TextColor {
    Green,
    Yellow,
    Red,
    Purple,
    Default,
}

impl TextColor {
    pub fn from_stack_status(stack_status: &StackStatus) -> Self {
        match stack_status {
            StackStatus::CreateComplete => TextColor::Green,
            StackStatus::CreateFailed => TextColor::Red,
            StackStatus::CreateInProgress => TextColor::Yellow,
            StackStatus::DeleteComplete => TextColor::Green,
            StackStatus::DeleteFailed => TextColor::Red,
            StackStatus::DeleteInProgress => TextColor::Yellow,
            StackStatus::ImportComplete => TextColor::Green,
            StackStatus::ImportInProgress => TextColor::Yellow,
            StackStatus::ImportRollbackComplete => TextColor::Green,
            StackStatus::ImportRollbackFailed => TextColor::Red,
            StackStatus::ImportRollbackInProgress => TextColor::Yellow,
            StackStatus::ReviewInProgress => TextColor::Yellow,
            StackStatus::RollbackComplete => TextColor::Green,
            StackStatus::RollbackFailed => TextColor::Red,
            StackStatus::RollbackInProgress => TextColor::Yellow,
            StackStatus::UpdateComplete => TextColor::Green,
            StackStatus::UpdateCompleteCleanupInProgress => TextColor::Green,
            StackStatus::UpdateFailed => TextColor::Red,
            StackStatus::UpdateInProgress => TextColor::Yellow,
            StackStatus::UpdateRollbackComplete => TextColor::Green,
            StackStatus::UpdateRollbackCompleteCleanupInProgress => TextColor::Yellow,
            StackStatus::UpdateRollbackFailed => TextColor::Red,
            StackStatus::UpdateRollbackInProgress => TextColor::Yellow,
            _ => TextColor::Red,
        }
    }

    pub fn from_change_set_status(change_set_status: &ChangeSetStatus) -> Self {
        match change_set_status {
            ChangeSetStatus::CreateComplete => TextColor::Green,
            ChangeSetStatus::CreateInProgress => TextColor::Yellow,
            ChangeSetStatus::CreatePending => TextColor::Yellow,
            ChangeSetStatus::DeleteComplete => TextColor::Green,
            ChangeSetStatus::DeleteFailed => TextColor::Red,
            ChangeSetStatus::DeleteInProgress => TextColor::Yellow,
            ChangeSetStatus::DeletePending => TextColor::Yellow,
            ChangeSetStatus::Failed => TextColor::Red,
            _ => TextColor::Red,
        }
    }

    pub fn from_change_action(change_action: &ChangeAction) -> Self {
        match change_action {
            ChangeAction::Add => TextColor::Green,
            ChangeAction::Dynamic => TextColor::Purple,
            ChangeAction::Import => TextColor::Green,
            ChangeAction::Modify => TextColor::Yellow,
            ChangeAction::Remove => TextColor::Red,
            _ => TextColor::Red,
        }
    }

    pub fn from_replacement(replacement: &Replacement) -> Self {
        match replacement {
            Replacement::Conditional => TextColor::Yellow,
            Replacement::False => TextColor::Green,
            Replacement::True => TextColor::Red,
            _ => TextColor::Red,
        }
    }

    pub fn from_requires_recreation(requires_recreation: &RequiresRecreation) -> Self {
        match requires_recreation {
            RequiresRecreation::Always => TextColor::Red,
            RequiresRecreation::Conditionally => TextColor::Yellow,
            RequiresRecreation::Never => TextColor::Green,
            _ => TextColor::Red,
        }
    }

    pub fn from_resource_status(resource_status: &ResourceStatus) -> Self {
        match resource_status {
            ResourceStatus::CreateComplete => TextColor::Green,
            ResourceStatus::CreateFailed => TextColor::Red,
            ResourceStatus::CreateInProgress => TextColor::Yellow,
            ResourceStatus::DeleteComplete => TextColor::Green,
            ResourceStatus::DeleteFailed => TextColor::Red,
            ResourceStatus::DeleteInProgress => TextColor::Yellow,
            ResourceStatus::ImportComplete => TextColor::Green,
            ResourceStatus::ImportInProgress => TextColor::Yellow,
            ResourceStatus::ImportRollbackComplete => TextColor::Green,
            ResourceStatus::ImportRollbackFailed => TextColor::Red,
            ResourceStatus::ImportRollbackInProgress => TextColor::Yellow,
            ResourceStatus::RollbackComplete => TextColor::Green,
            ResourceStatus::RollbackFailed => TextColor::Red,
            ResourceStatus::RollbackInProgress => TextColor::Yellow,
            ResourceStatus::UpdateComplete => TextColor::Green,
            ResourceStatus::UpdateFailed => TextColor::Red,
            ResourceStatus::UpdateInProgress => TextColor::Yellow,
            ResourceStatus::UpdateRollbackComplete => TextColor::Green,
            ResourceStatus::UpdateRollbackFailed => TextColor::Red,
            ResourceStatus::UpdateRollbackInProgress => TextColor::Yellow,
            _ => TextColor::Red,
        }
    }

    pub fn colorize(&self, str: &str) -> String {
        match self {
            TextColor::Green => str.green().to_string(),
            TextColor::Yellow => str.yellow().to_string(),
            TextColor::Red => str.red().to_string(),
            TextColor::Purple => str.purple().to_string(),
            TextColor::Default => str.to_string(),
        }
    }
}

macro_rules! str_repeat {
    ($str:literal, $times:literal) => {{
        const A: &[u8] = unsafe { std::mem::transmute::<&str, &[u8]>($str) };
        let mut out = [A[0]; { A.len() * $times }];
        let mut i = 0;
        while i < out.len() {
            let a = i % A.len();
            out[i] = A[a];
            i += 1;
        }
        #[allow(clippy::transmute_bytes_to_str)]
        unsafe {
            std::mem::transmute::<&[u8], &str>(&out)
        }
    }};
}

macro_rules! pformat {
    ($fmt_str:literal, $identation:expr, $color:expr) => {{
        let ident = str_repeat!(" ", $identation);
        let str_format = format!($fmt_str);
        $color.colorize(&format!("{} {}", ident, str_format))
    }};
    ($fmt_str:literal, $identation:expr, $color:expr, $($args:tt)* ) => {{
        let ident = str_repeat!(" ", $identation);
        let str_format = format!($fmt_str, $($args)*);
        $color.colorize(&format!("{} {}", ident, str_format))
    }};
}

macro_rules! pprintln {
    ($lock:expr, $fmt_str:literal, $identation:expr, $color:expr) => {{
        let str = pformat!($fmt_str, $identation, $color);
        writeln!($lock,"{}", str).unwrap()
    }};
    ($lock:expr, $fmt_str:literal, $identation:expr, $color:expr, $($args:tt)* ) => {{
        let str = pformat!($fmt_str, $identation, $color, $($args)*);
        writeln!($lock,"{}", str).unwrap()
    }};
}

pub struct Display {}
impl Display {
    pub fn new() -> Self {
        Self {}
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

        pprintln!(
            lock,
            "Change set: {}",
            0,
            TextColor::Default,
            change_set
                .change_set_name
                .as_deref()
                .unwrap_or("UNKOWN CHANGE SET")
        );

        if let Some(status) = change_set.status.as_ref() {
            pprintln!(
                lock,
                "Change set status: {status:?}",
                0,
                TextColor::from_change_set_status(status)
            );
            if let Some(status_reason) = change_set.status_reason.as_ref() {
                pprintln!(
                    lock,
                    "reason: {status_reason:?}",
                    1,
                    TextColor::from_change_set_status(status)
                )
            }
        }

        change_set
            .changes()
            .iter()
            .filter_map(|c| c.resource_change.as_ref())
            .for_each(|rc| {
                pprintln!(
                    lock,
                    "{} {} ({})",
                    2,
                    TextColor::from_change_action(rc.action().unwrap()),
                    ChangeActionSimbol(rc.action().unwrap().clone()),
                    rc.logical_resource_id
                        .as_deref()
                        .unwrap_or(UNKNOWN_RESOURCE_LOGICAL_ID),
                    rc.resource_type.as_deref().unwrap_or(UNKNOWN_RESOURCE_TYPE),
                );

                pprintln!(
                    lock,
                    "Action: {:?}",
                    4,
                    TextColor::from_change_action(rc.action().unwrap()),
                    rc.action().unwrap()
                );

                if let Some(replacement) = rc.replacement() {
                    pprintln!(
                        lock,
                        "Replacement: {replacement:?}",
                        4,
                        TextColor::from_replacement(replacement)
                    );
                }

                if let Some(change_res_id) = rc.change_set_id() {
                    pprintln!(
                        lock,
                        "Physical Resource: {change_res_id}",
                        4,
                        TextColor::Default
                    );
                }

                if !rc.scope().is_empty() {
                    let scope = &rc
                        .scope()
                        .iter()
                        .map(|s| format!("{s:?}"))
                        .collect::<Vec<String>>()
                        .join(", ");
                    pprintln!(lock, "Change Scope: {scope}", 4, TextColor::Default);
                }

                if !rc.details().is_empty() {
                    pprintln!(lock, "Changed Properties", 4, TextColor::Default);
                    for detail in rc.details() {
                        if let Some(target) = detail.target() {
                            pprintln!(
                                lock,
                                "{} {}",
                                6,
                                TextColor::Default,
                                target
                                    .attribute()
                                    .map(|a| format! {"{a:?}"})
                                    .unwrap_or_else(|| "".to_string()),
                                target.name().unwrap_or_default()
                            );
                            if let Some(requires_recreation) = target.requires_recreation() {
                                pprintln!(
                                    lock,
                                    "{:?}",
                                    8,
                                    TextColor::from_requires_recreation(requires_recreation),
                                    requires_recreation
                                )
                            }
                        }

                        if let Some(causing_eentity) = detail.causing_entity() {
                            pprintln!(
                                lock,
                                "Causing entity: {causing_eentity}",
                                8,
                                TextColor::Default
                            );
                        }
                        if let Some(change_source) = detail.change_source() {
                            pprintln!(
                                lock,
                                "Causing entity: {change_source:?}",
                                8,
                                TextColor::Default
                            );
                        }
                    }
                }
            })
    }

    pub fn print_stack_summaries(&self, stacks: &[StackSummary]) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        for stack in stacks {
            pprintln!(
                lock,
                "Stack name: {}",
                0,
                TextColor::Default,
                stack.stack_name().unwrap_or_default(),
            );

            if let Some(status) = stack.stack_status() {
                pprintln!(
                    lock,
                    "Status: {status:?}",
                    1,
                    TextColor::from_stack_status(status),
                )
            }
        }
    }

    pub fn print_stack(&self, stack: &Stack) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        pprintln!(
            lock,
            "Stack name: {}",
            0,
            TextColor::Default,
            stack.stack_name().unwrap_or_default()
        );
        if let Some(parent) = stack.parent_id() {
            pprintln!(lock, "Parent: {parent}", 0, TextColor::Default);
        }
        if let Some(description) = stack.description() {
            pprintln!(lock, "Description: {description}", 0, TextColor::Default);
        }
        if let Some(creation_date) = stack.creation_time() {
            pprintln!(
                lock,
                "Creation time: {creation_date}",
                0,
                TextColor::Default
            );
        }
        if let Some(last_updated_time) = stack.last_updated_time() {
            pprintln!(
                lock,
                "Last update time: {last_updated_time}",
                0,
                TextColor::Default
            );
        }
        if let Some(stack_status) = stack.stack_status() {
            let color = TextColor::from_stack_status(stack_status);
            pprintln!(lock, "Status: {stack_status:?}", 0, color);
            if let Some(stack_status_reason) = stack.stack_status_reason() {
                pprintln!(lock, "Status reason: {stack_status_reason}", 0, color);
            }
        }

        if !stack.parameters().is_empty() {
            pprintln!(lock, "Parameters:", 0, TextColor::Default);
            for Parameter {
                parameter_key: key,
                parameter_value: value,
                ..
            } in stack.parameters()
            {
                let key = key.clone().unwrap_or_default();
                let value = value.clone().unwrap_or_default();
                pprintln!(lock, "{key}:{value}", 0, TextColor::Default);
            }
        }
    }

    pub fn print_stack_resources(&self, resources: &ListStackResourcesOutput) {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        pprintln!(lock, "Stack resources:", 0, TextColor::Default);
        for resource in resources.stack_resource_summaries() {
            if let Some(logical_id) = resource.physical_resource_id() {
                pprintln!(
                    lock,
                    "{logical_id} ({:?})",
                    4,
                    TextColor::Default,
                    resource.resource_type().unwrap_or_default()
                );
            }

            if let Some(physical_id) = resource.physical_resource_id() {
                pprintln!(lock, "Physical ID: {physical_id}", 6, TextColor::Default);
            }

            if let Some(last_updated_timestamp) = resource.last_updated_timestamp() {
                pprintln!(
                    lock,
                    "Last updated timestamp: {last_updated_timestamp}",
                    6,
                    TextColor::Default
                );
            }

            if let Some(resource_status) = resource.resource_status() {
                pprintln!(
                    lock,
                    "Status: {resource_status:?}",
                    6,
                    TextColor::from_resource_status(resource_status)
                );
            }
        }
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
                pprintln!(
                    lock,
                    "{}: {}",
                    0,
                    TextColor::Red,
                    error.resource_type().unwrap_or(UNKNOWN_RESOURCE_TYPE),
                    error
                        .logical_resource_id()
                        .unwrap_or(UNKNOWN_RESOURCE_LOGICAL_ID)
                );
                pprintln!(
                    lock,
                    "reason: {}",
                    0,
                    TextColor::Red,
                    error.resource_status_reason().unwrap_or(UNKNOWN_REASON)
                );
                pprintln!(
                    lock,
                    "properties: {}",
                    0,
                    TextColor::Red,
                    error.resource_properties().unwrap_or(""),
                );
            });
    }
}
