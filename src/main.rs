mod cli;
mod error;

use clap::{CommandFactory, Parser};
use cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Auto-enable --no-input when stdin is not a TTY (AI agents, pipes, scripts)
    let mut cli = cli;
    if !cli.no_input {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            cli.no_input = true;
        }
    }

    let output_format = cli.output.clone();
    let result = run(cli).await;
    if let Err(e) = result {
        let exit_code = e.downcast_ref::<error::JrError>()
            .map(|je| je.exit_code())
            .unwrap_or(1);

        // Structured JSON errors when --output json is set
        match output_format {
            cli::OutputFormat::Json => {
                eprintln!("{}", serde_json::json!({
                    "error": e.to_string(),
                    "code": exit_code
                }));
            }
            _ => {
                eprintln!("Error: {e}");
            }
        }

        std::process::exit(exit_code);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Handle completion before anything else (no config/auth needed)
    if let cli::Command::Completion { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "jr", &mut std::io::stdout());
        return Ok(());
    }

    // Set up Ctrl+C handler
    let main_task = async {
        match cli.command {
            cli::Command::Completion { .. } => unreachable!(),
            cli::Command::Init => todo!("init"),
            cli::Command::Auth { command: _ } => todo!("auth"),
            cli::Command::Me => todo!("me"),
            cli::Command::Project { command: _ } => todo!("project"),
            cli::Command::Issue { command: _ } => todo!("issue"),
            cli::Command::Board { command: _ } => todo!("board"),
            cli::Command::Sprint { command: _ } => todo!("sprint"),
            cli::Command::Worklog { command: _ } => todo!("worklog"),
        }
    };

    tokio::select! {
        result = main_task => result,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nInterrupted");
            std::process::exit(130);
        }
    }
}
