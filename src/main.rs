mod commands;

use std::path::PathBuf;

use aws_config::BehaviorVersion;
use clap::{Parser, Subcommand};
use tracing::{span, Level};

use crate::commands::up_command::UpCommand;

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

#[::tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_cloudformation::Client::new(&config);

    tracing_subscriber::fmt()
        .with_target(false)
        .try_init()
        .unwrap();

    match &cli.command {
        Commands::Up {
            stack,
            template,
        } => {
            span!(
                Level::INFO,
                "up",
                stack = stack,
                template = template.to_str()
            );
            UpCommand::new(client, stack.to_string(), template.to_path_buf())
                .run()
                .await?;
        }
        Commands::Preview { stack } => {
            span!(Level::INFO, "preview", stack = stack);
        }
        Commands::Destroy { stack } => {
            span!(Level::INFO, "destroy", stack = stack);
        }
        Commands::List {} => {
            span!(Level::INFO, "list");
        }
        Commands::Describe { stack } => {
            span!(Level::INFO, "describe", stack = stack);
        }
    }

    Ok(())
}
