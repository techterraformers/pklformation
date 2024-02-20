mod commands;

use std::path::PathBuf;

use crate::commands::up_command::UpCommand;
use aws_config::BehaviorVersion;
use clap::{Parser, Subcommand};
use std::time::Duration;
use tracing::{span, Level};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
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
        #[arg(short, long, default_value = "30", value_parser = parse_duration)]
        pool_interval: Duration,
    },

    Preview {
        #[arg(short, long)]
        stack: String,
    },

    Destroy {
        stack: String,
    },

    List {},
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
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_cloudformation::Client::new(&config);

    tracing_subscriber::fmt().init();

    match &cli.command {
        Commands::Up {
            stack,
            template,
            pool_interval,
        } => {
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
                pool_interval.to_owned(),
            )
            .run()
            .await?;
        }
        Commands::Preview { stack } => {
            span!(Level::DEBUG, "preview", stack = stack);
        }
        Commands::Destroy { stack } => {
            span!(Level::DEBUG, "destroy", stack = stack);
        }
        Commands::List {} => {
            span!(Level::DEBUG, "list");
        }
        Commands::Describe { stack } => {
            span!(Level::DEBUG, "describe", stack = stack);
        }
    }

    Ok(())
}
