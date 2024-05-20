use aws_sdk_cloudformation::types::StackStatus;

use crate::{aws_client::AwsClient, display::Display};

pub struct ListCommand {
    client: AwsClient,
    display: Display,
    status_filter: Option<Vec<StackStatus>>,
}

impl ListCommand {
    pub fn new(client: AwsClient, status_filter: Option<Vec<StackStatus>>) -> Self {
        Self {
            client,
            display: Display::new(),
            status_filter,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let status_filter = self.status_filter.unwrap_or(vec![
            StackStatus::CreateComplete,
            StackStatus::CreateInProgress,
            StackStatus::ImportComplete,
            StackStatus::ImportInProgress,
        ]);
        let stacks = self.client.list_stacks(&status_filter).await?;
        self.display.print_stack_summaries(&stacks);
        Ok(())
    }
}
