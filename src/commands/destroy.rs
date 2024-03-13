use aws_sdk_cloudformation::types::StackStatus;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use crate::{aws_client::AwsClient, display::Display};

pub struct DestroyCommand {
    client: AwsClient,
    stack: String,
    pool_interval: Duration,
    display: Display,
}

impl DestroyCommand {
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

        let start_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        let stack = self.client.describe_stack(&self.stack).await?;
        self.display.print_stack(&stack);
        if let Some(change_set_id) = stack.change_set_id() {
            let change_set = self.client.describe_change_set(change_set_id).await?;
            self.display.print_change_set(&change_set);
        }

        self.display.ask_confirm("Do you want to continue?");
        self.client.delete_stack(stack.stack_id().unwrap()).await?;

        let (op_status, _reason) = self
            .client
            .wait_until_stack_op_in_progress(&self.stack, self.pool_interval)
            .await;

        match op_status {
            StackStatus::DeleteComplete => {
                info!("Destroy compleated successfully!")
            }
            _ => {
                error!("Up failed with status: {op_status:?}");
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
}
