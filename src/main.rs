use clap::{CommandFactory, Parser};
use jr::api;
use jr::cli;
use jr::cli::Cli;
use jr::config;
use jr::error;
use jr::output;

#[tokio::main]
async fn main() {
    let mut cli = Cli::parse();

    // Validate --profile early so a bad name fails before any work runs.
    // The validated value is threaded into `Config::load_with` rather than
    // through an env-var seam, since `unsafe { std::env::set_var(...) }` is
    // unsound under #[tokio::main] (worker threads already exist).
    if let Some(p) = cli.profile.as_deref() {
        if let Err(e) = config::validate_profile_name(p) {
            eprintln!("Error: {e}");
            std::process::exit(e.exit_code());
        }
    }

    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Auto-enable --no-input when stdin is not a TTY (AI agents, pipes, scripts)
    if !cli.no_input {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            cli.no_input = true;
        }
    }

    let output_format = cli.output;
    let result = run(cli).await;
    if let Err(e) = result {
        let exit_code = e
            .chain()
            .find_map(|cause| cause.downcast_ref::<error::JrError>())
            .map(|je| je.exit_code())
            .unwrap_or(1);

        // Structured JSON errors when --output json is set
        match output_format {
            cli::OutputFormat::Json => {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": e.to_string(),
                        "code": exit_code
                    })
                );
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
            cli::Command::Init => cli::init::handle().await,
            cli::Command::Assets { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::assets::handle(command, &cli.output, &client).await
            }
            cli::Command::Auth { command } => match command {
                // For each subcommand that takes its own `--profile` arg, we
                // compose an "effective profile" by falling back to the
                // global `--profile` (`cli.profile`) when the subcommand-level
                // value is `None`. Without this, `jr --profile sandbox auth
                // <subcmd>` would silently drop the global flag because each
                // handler reloads config internally and only sees the
                // subcommand-level arg.
                cli::AuthCommand::Login {
                    profile,
                    url,
                    oauth,
                    email,
                    token,
                    client_id,
                    client_secret,
                } => {
                    let effective_profile = profile.or_else(|| cli.profile.clone());
                    cli::auth::handle_login(cli::auth::LoginArgs {
                        profile: effective_profile,
                        url,
                        oauth,
                        email,
                        token,
                        client_id,
                        client_secret,
                        no_input: cli.no_input,
                    })
                    .await
                }
                cli::AuthCommand::Status { profile } => {
                    let effective_profile = profile.or_else(|| cli.profile.clone());
                    cli::auth::status(effective_profile.as_deref()).await
                }
                cli::AuthCommand::Refresh {
                    profile,
                    oauth,
                    email,
                    token,
                    client_id,
                    client_secret,
                } => {
                    let effective_profile = profile.or_else(|| cli.profile.clone());
                    cli::auth::refresh_credentials(cli::auth::RefreshArgs {
                        profile: effective_profile.as_deref(),
                        oauth,
                        email,
                        token,
                        client_id,
                        client_secret,
                        no_input: cli.no_input,
                        output: &cli.output,
                    })
                    .await
                }
                cli::AuthCommand::Switch { name } => {
                    cli::auth::handle_switch(&name, cli.profile.as_deref()).await
                }
                cli::AuthCommand::List => {
                    cli::auth::handle_list(&cli.output, cli.profile.as_deref()).await
                }
                cli::AuthCommand::Logout { profile } => {
                    let effective_profile = profile.or_else(|| cli.profile.clone());
                    cli::auth::handle_logout(effective_profile.as_deref()).await
                }
                cli::AuthCommand::Remove { name } => {
                    cli::auth::handle_remove(&name, cli.no_input, cli.profile.as_deref()).await
                }
            },
            cli::Command::Me => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                let user = client.get_myself().await?;
                output::print_output(
                    &cli.output,
                    &["Field", "Value"],
                    &[
                        vec!["Name".into(), user.display_name.clone()],
                        vec![
                            "Email".into(),
                            user.email_address.clone().unwrap_or_default(),
                        ],
                        vec!["Account ID".into(), user.account_id.clone()],
                    ],
                    &user,
                )?;
                Ok(())
            }
            cli::Command::Project { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::project::handle(
                    command,
                    &config,
                    &client,
                    &cli.output,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Issue { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::issue::handle(
                    *command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                    cli.no_input,
                )
                .await?;
                Ok(())
            }
            cli::Command::Board { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::board::handle(
                    command,
                    &config,
                    &client,
                    &cli.output,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Sprint { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::sprint::handle(
                    command,
                    &config,
                    &client,
                    &cli.output,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Worklog { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::worklog::handle(command, &client, &cli.output).await
            }
            cli::Command::Team { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::team::handle(command, &cli.output, &config, &client).await
            }
            cli::Command::User { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::user::handle(command, &cli.output, &client).await
            }
            cli::Command::Queue { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::queue::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Api {
                path,
                method,
                data,
                header,
            } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client = api::client::JiraClient::from_config(&config, cli.verbose)?;
                cli::api::handle_api(path, method, data, header, &client).await
            }
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
