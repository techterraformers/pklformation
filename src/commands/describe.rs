use aws_sdk_cloudformation::types::StackStatus;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use crate::{aws_client::AwsClient, display::Display};

pub struct DescribeCommand {
    client: AwsClient,
    stack: String,
    pool_interval: Duration,
    display: Display,
}

impl DescribeCommand {
    pub fn new(client: AwsClient, stack: String, pool_interval: Duration) -> Self {
        Self {
            client,
            stack,
            pool_interval,
            display: Display::new(),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let _wait_result = self
            .client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await;

        let stack = self.client.describe_stack(&self.stack).await?;
        self.display.print_stack(&stack);
        if let Some(stack_id) = stack.stack_id() {
            let stack_resources = self.client.list_stack_resources(stack_id).await?;
            self.display.print_stack_resources(&stack_resources);
        }
        Ok(())
    }
}
