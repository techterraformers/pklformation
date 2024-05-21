mod aws_client;
mod commands;
mod display;

use std::path::PathBuf;

use crate::commands::describe::DescribeCommand;
use crate::commands::destroy::DestroyCommand;
use crate::commands::list::ListCommand;
use crate::commands::preview::PreviewCommand;
use crate::commands::up::UpCommand;

use aws_sdk_cloudformation::types::StackStatus;
use clap::{Parser, Subcommand};
use std::time::Duration;
use tracing::{span, Level};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "5", value_parser = parse_duration)]
    pool_interval: Duration,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Up {
        #[arg(short, long)]
        stack: String,
        #[arg(short, long)]
        template: PathBuf,
    },

    Preview {
        #[arg(short, long)]
        stack: String,
        #[arg(short, long)]
        template: PathBuf,
    },

    Destroy {
        #[arg(short, long)]
        stack: String,
    },

    List {
        #[arg(short, long)]
        status_filter: Option<Vec<StackStatus>>,
    },

    Describe {
        #[arg(short, long)]
        stack: String,
    },
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

#[::tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = aws_client::AwsClient::new().await;

    tracing_subscriber::fmt().init();

    match &cli.command {
        Commands::Up { stack, template } => {
            let span = span!(
                Level::INFO,
                "up",
                stack = stack,
                template = template.to_str()
            );
            let _enter = span.enter();
            UpCommand::new(
                client,
                stack.to_string(),
                template.to_path_buf(),
                cli.pool_interval.to_owned(),
            )
            .run()
            .await?;
        }
        Commands::Preview { stack, template } => {
            let span = span!(Level::DEBUG, "preview", stack = stack);
            let _enter = span.enter();
            PreviewCommand::new(
                client,
                stack.to_string(),
                template.to_path_buf(),
                cli.pool_interval.to_owned(),
            )
            .run()
            .await?;
        }
        Commands::Destroy { stack } => {
            let span = span!(Level::DEBUG, "destroy", stack = stack);
            let _enter = span.enter();
            DestroyCommand::new(client, stack.to_string(), cli.pool_interval.to_owned())
                .run()
                .await?;
        }
        Commands::List { status_filter } => {
            let span = span!(Level::DEBUG, "list");
            let _entr = span.enter();
            ListCommand::new(client, status_filter.clone())
                .run()
                .await?;
        }
        Commands::Describe { stack } => {
            let span = span!(Level::DEBUG, "describe", stack = stack);
            let _enter = span.enter();
            DescribeCommand::new(client, stack.to_string(), cli.pool_interval.to_owned())
                .run()
                .await?;
        }
    }

    Ok(())
}
