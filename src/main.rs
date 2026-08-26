use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{CommandFactory, Parser};
use jr::api;
use jr::cli;
use jr::cli::Cli;
use jr::config;
use jr::error;
use jr::output;

/// Initialize the tracing subscriber based on CLI verbosity flags.
///
/// - Default (no verbose flags): WARN level — silent in normal use.
/// - `--verbose`: DEBUG level — shows request method+URL, response status.
/// - `--verbose-bodies`: TRACE level — shows full request/response bodies.
/// - `RUST_LOG` env var overrides the CLI-derived level (via `EnvFilter`).
///
/// Initialized with `.try_init().ok()` instead of `.init()` so that calling
/// this multiple times in the same process (e.g., integration tests that spawn
/// a subprocess but also call lib code) does not panic on double-init.
fn init_tracing(cli: &Cli) {
    use tracing::Level;
    use tracing_subscriber::{EnvFilter, fmt};

    let default_level = if cli.verbose_bodies {
        Level::TRACE
    } else if cli.verbose {
        Level::DEBUG
    } else {
        Level::WARN
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level.to_string()));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init()
        .ok();
}

#[tokio::main]
async fn main() {
    let mut cli = Cli::try_parse().unwrap_or_else(|err| {
        // Intercept `jr issue comment <TOKEN> …` when TOKEN is not a valid
        // CommentSubcommand.  Walk err.context() to find the attempted token so
        // the intercept works even when global flags precede the subcommand
        // (e.g., `jr --output json issue comment KEY "text"`).
        //
        // Only `InvalidSubcommand` under the `issue comment` context is handled
        // here.  All other ErrorKinds pass through to clap's own renderer via
        // `err.exit()` — this preserves byte-identical output for
        // `DisplayHelp`/`DisplayVersion` (stdout + exit 0) and all usage-error
        // kinds (stderr + exit 2).  AC-011 / BC-3.5.012.
        if err.kind() == ErrorKind::InvalidSubcommand {
            // Detect that the error is scoped to `jr issue comment` by inspecting
            // the Usage context entry — it contains the full command path as plain
            // text (e.g. "Usage: jr issue comment <COMMAND>").  Using Usage is
            // robust against global flag reordering (`jr --output json issue comment
            // KEY "text"` shifts argv positions, but Usage always reflects the actual
            // parse path).  AC-013 / BC-3.5.012.
            let under_issue_comment = err.context().any(|(kind, value)| {
                kind == ContextKind::Usage && value.to_string().contains("issue comment")
            });
            if under_issue_comment {
                // Pull the attempted token from the InvalidSubcommand context entry.
                let attempted_token = err.context().find_map(|(kind, value)| {
                    if kind == ContextKind::InvalidSubcommand {
                        if let ContextValue::String(s) = value {
                            return Some(s.clone());
                        }
                    }
                    None
                });
                if let Some(ref token) = attempted_token {
                    if token.eq_ignore_ascii_case("list") || token.eq_ignore_ascii_case("ls") {
                        eprintln!("error: to list all comments, use `jr issue comments` (plural)");
                        std::process::exit(2);
                    }
                }
                eprintln!("error: use `jr issue comment add` instead");
                eprintln!(
                    "       `jr issue comment KEY \"text\"` is no longer valid; \
                     the comment command is now a subcommand group."
                );
                std::process::exit(2);
            }
        }
        // All non-intercepted errors pass through clap's own renderer.
        err.exit()
    });

    if cli.no_color || std::env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Auto-enable --no-input when stdin is not a TTY (AI agents, pipes, scripts).
    // Exception: when JR_OAUTH_CODE is set the caller is a test harness that
    // injects an auth code via env var and may also pipe stdin to simulate
    // interactive selection — do not override the explicit no_input value in
    // that case. See tests/multi_cloudid_disambiguation.rs.
    if !cli.no_input {
        use std::io::IsTerminal;
        let oauth_code_test_mode = std::env::var("JR_OAUTH_CODE").is_ok();
        #[cfg(debug_assertions)]
        let stdin_is_tty_forced = std::env::var("JR_STDIN_IS_TTY")
            .map(|v| v == "1")
            .unwrap_or(false);
        #[cfg(not(debug_assertions))]
        let stdin_is_tty_forced = false;
        if !stdin_is_tty_forced && !std::io::stdin().is_terminal() && !oauth_code_test_mode {
            cli.no_input = true;
        }
    }

    // Initialize structured logging before any command dispatch.
    // Output goes to stderr; stdout is reserved for data output.
    // RUST_LOG env var overrides the CLI-derived log level.
    init_tracing(&cli);

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

/// Outcome of racing a work future against a shutdown-signal future via
/// [`run_until_shutdown`]. See BC-X.3.006 (`run()`'s Ctrl+C/SIGINT contract).
pub(crate) enum RunOutcome<T> {
    /// `work` resolved first; carries its result.
    Completed(T),
    /// `shutdown` resolved first; `work` was abandoned mid-flight.
    Interrupted,
}

/// Races `work` against `shutdown`, returning whichever resolves first.
///
/// Deliberately contains NO side effects (no `eprintln!`, no
/// `std::process::exit`) — those stay at the `run()` call site so this
/// function can be unit-tested in-process via injected
/// `std::future::{ready, pending}` without tearing down the test harness
/// (BC-X.3.006 VP-MUTANTS-SCOPE-1-002).
pub(crate) async fn run_until_shutdown<W, S, T>(work: W, shutdown: S) -> RunOutcome<T>
where
    W: std::future::Future<Output = T>,
    S: std::future::Future<Output = ()>,
{
    tokio::pin!(work);
    tokio::pin!(shutdown);
    tokio::select! {
        v = &mut work => RunOutcome::Completed(v),
        _ = &mut shutdown => RunOutcome::Interrupted,
    }
}

/// VP-MUTANTS-SCOPE-1-001 (BC-X.3.006 EC-1) readiness-handshake seam. Debug+Unix only.
///
/// `tokio::signal::ctrl_c()` (the production signal future used by `run()`'s
/// normal ctrl_c fork, below) only registers its OS-level listener on first
/// poll — that makes it unsuitable for guaranteeing "listener installed"
/// ordering to an external test process without a fixed `sleep` (forbidden,
/// BC-X.3.006 EC-1). `tokio::signal::unix::signal(...)` registers its
/// listener synchronously at call time, before this function ever awaits
/// anything, so printing the readiness marker immediately afterward is a
/// genuine ordering guarantee rather than a race. This function is used only
/// by the `JR_TEST_BLOCK_UNTIL_SIGINT` seam in `run()` below — production
/// dispatch is untouched and continues to use `tokio::signal::ctrl_c()`.
#[cfg(all(debug_assertions, unix))]
async fn block_until_sigint_test_seam() {
    use std::io::Write;
    use tokio::signal::unix::{SignalKind, signal};

    let mut sig = signal(SignalKind::interrupt())
        .expect("failed to register SIGINT handler for JR_TEST_BLOCK_UNTIL_SIGINT seam");

    // The listener above is registered synchronously (see doc comment), so
    // it is safe to print the readiness marker now — the caller is
    // guaranteed the signal will be observed once sent.
    println!("JR-TEST-READY");
    // Ensure the marker reaches the reading test process promptly: stdout is
    // piped (not a TTY) under the test harness, where line-buffering is not
    // guaranteed.
    let _ = std::io::stdout().flush();

    sig.recv().await;
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Validate --profile here (not in main) so a bad name flows through
    // the unified error-reporting block — `--output json` callers get
    // a structured `{"error":..,"code":..}` payload instead of a plain
    // stderr line. The validated value is threaded into
    // `Config::load_with` rather than through an env-var seam, since
    // `unsafe { std::env::set_var(...) }` is unsound under
    // #[tokio::main] (tokio worker threads already exist).
    if let Some(p) = cli.profile.as_deref() {
        config::validate_profile_name(p)?;
    }

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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                    cloud_id,
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
                        cloud_id,
                        no_input: cli.no_input,
                        output: cli.output,
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
                    // BC-1.2.047 (issue #663): the global `--profile` flag has no
                    // subcommand-level field to compose against on `auth switch` —
                    // its only observable effect was forcing an extra, confusing
                    // existence-check on `--profile`'s own value (the "jr auth
                    // switch --profile X X" incantation the issue reports). Reject
                    // it outright, before `handle_switch`/`Config::load_with` runs,
                    // rather than silently ignoring it. Keyed ONLY on the CLI flag
                    // (`cli.profile.is_some()`) — never on `JR_PROFILE` or any other
                    // stage of profile resolution (EC-1.2.047-4). Runtime guard, not
                    // clap `conflicts_with`: unreliable for `global = true` args
                    // (clap #5335/#5358) and would yield exit 2, not the required
                    // exit-64 UserError.
                    if cli.profile.is_some() {
                        return Err(error::JrError::UserError(
                            "--profile is not valid for 'auth switch'. The profile to activate is the positional argument. Try: jr auth switch <NAME>".to_string(),
                        )
                        .into());
                    }
                    // Past the guard above, `cli.profile` is provably `None` —
                    // pass `None` explicitly rather than `cli.profile.as_deref()`
                    // so the dead argument doesn't read as though this arm still
                    // composes a caller-supplied profile (it never did; see
                    // handle_switch's `cli_profile` param, used only for
                    // `Config::load_with`'s active-profile resolution).
                    cli::auth::handle_switch(&name, None, &cli.output).await
                }
                cli::AuthCommand::List => {
                    cli::auth::handle_list(&cli.output, cli.profile.as_deref()).await
                }
                cli::AuthCommand::Logout { profile } => {
                    let effective_profile = profile.or_else(|| cli.profile.clone());
                    cli::auth::handle_logout(effective_profile.as_deref(), &cli.output).await
                }
                cli::AuthCommand::Remove { name } => {
                    cli::auth::handle_remove(
                        &name,
                        cli.no_input,
                        cli.profile.as_deref(),
                        &cli.output,
                    )
                    .await
                }
            },
            cli::Command::Me => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::worklog::handle(command, &client, &cli.output).await
            }
            cli::Command::Team { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::team::handle(command, &cli.output, &config, &client).await
            }
            cli::Command::User { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::user::handle(command, &cli.output, &client).await
            }
            cli::Command::Queue { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::queue::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::RequestType { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::requesttype::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Field { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::field::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                )
                .await
            }
            cli::Command::Component { command } => {
                let config = config::Config::load_with(cli.profile.as_deref())?;
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::component::handle(
                    command,
                    &cli.output,
                    &config,
                    &client,
                    cli.project.as_deref(),
                    cli.no_input,
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
                let client =
                    api::client::JiraClient::from_config(&config, cli.verbose, cli.verbose_bodies)?;
                cli::api::handle_api(path, method, data, header, &client).await
            }
        }
    };

    // VP-MUTANTS-SCOPE-1-001 readiness-handshake seam (debug+unix only). See
    // tests/interrupt_signal.rs module doc for the full contract.
    //
    // This gates only WHICH shutdown/work futures are selected below — there
    // is exactly one `RunOutcome::Interrupted` arm in this function (the
    // `match` at the bottom), reached by both the seam path and the
    // production path. That is deliberate: it is what lets VP-001's SIGINT
    // exercise the real `eprintln!("\nInterrupted")` + `std::process::exit(130)`
    // lines instead of a parallel seam-only copy of them.
    //
    // Always `false` outside debug+unix builds, so release/non-unix builds
    // always take the production branches below with zero behavior change.
    #[cfg(all(debug_assertions, unix))]
    let test_seam_active = std::env::var("JR_TEST_BLOCK_UNTIL_SIGINT")
        .map(|v| v == "1")
        .unwrap_or(false);
    #[cfg(not(all(debug_assertions, unix)))]
    let test_seam_active = false;

    // WORK future: the seam blocks forever (`pending()`, typed to match
    // `main_task`'s `anyhow::Result<()>` output so both branches share one
    // type) so the process can only end via the shutdown/interrupt arm below;
    // production dispatches the real command via `main_task`. Boxed because
    // the two branches are different concrete future types.
    let work: std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>>>> =
        if test_seam_active {
            Box::pin(std::future::pending())
        } else {
            Box::pin(main_task)
        };

    // SHUTDOWN future: the seam's unix-signal future prints the
    // `JR-TEST-READY` marker immediately after synchronously registering its
    // listener (see `block_until_sigint_test_seam`'s doc comment for why that
    // ordering is race-free); production uses the real `ctrl_c()` adapter.
    #[cfg(all(debug_assertions, unix))]
    let shutdown: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> = if test_seam_active {
        Box::pin(block_until_sigint_test_seam())
    } else {
        Box::pin(async {
            let _ = tokio::signal::ctrl_c().await;
        })
    };
    #[cfg(not(all(debug_assertions, unix)))]
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    // The ONE interrupt branch, reached by both the seam and production
    // paths — see the comment above `test_seam_active`.
    match run_until_shutdown(work, shutdown).await {
        RunOutcome::Completed(result) => result,
        RunOutcome::Interrupted => {
            eprintln!("\nInterrupted");
            // 130 is the conventional exit code for a SIGINT-terminated process
            // (128 + SIGINT's signal number, 2). This is an explicit literal
            // chosen to match that convention, not a value the OS computes for
            // us — `std::process::exit` always takes exactly the code given.
            std::process::exit(130);
        }
    }
}

// VP-MUTANTS-SCOPE-1-002 (BC-X.3.006): portable, cross-platform coverage of the
// `run_until_shutdown` arm-selection decision. This module is inline (not in
// `tests/`) because `src/main.rs` is the binary crate's entry point — a
// separate compilation unit from the `jr` library crate that `tests/*.rs`
// files link against via `lib.rs`. Items here, even `pub(crate)`, are not
// reachable from `tests/`.
//
// IMPLEMENTED (S-MUTANTS-SCOPE-1, F4 Test Writer -> implementer): `run_until_shutdown`
// and `RunOutcome` above match the interface contract this module was written
// against, and `run()` is now their sole production call site (see the single
// `RunOutcome::Interrupted` arm at the bottom of `run()`, shared by both the
// debug+unix test seam and real production dispatch):
//
//     pub(crate) enum RunOutcome<T> { Completed(T), Interrupted }
//
//     pub(crate) async fn run_until_shutdown<W, S, T>(work: W, shutdown: S) -> RunOutcome<T>
//     where
//         W: std::future::Future<Output = T>,
//         S: std::future::Future<Output = ()>,
//     { ... }
//
// `run_until_shutdown` contains NO `eprintln!` and NO `std::process::exit`
// call — both stay at the `run()` call site (AC-006), which is what lets the
// tests below inject `std::future::{ready, pending}` and assert on
// `RunOutcome` in-process without tearing down the test harness.
#[cfg(test)]
mod tests {
    use super::RunOutcome;

    /// AC-008 (1/2): when `work` resolves first, `run_until_shutdown` returns
    /// `RunOutcome::Completed(value)` — the shutdown arm must NOT be selected
    /// even though it is also injected (as a never-resolving future).
    #[tokio::test]
    async fn test_run_until_shutdown_returns_completed_when_work_finishes_first() {
        let work = std::future::ready(42_u32);
        let shutdown = std::future::pending::<()>();

        let outcome = super::run_until_shutdown(work, shutdown).await;

        match outcome {
            RunOutcome::Completed(value) => assert_eq!(value, 42),
            RunOutcome::Interrupted => panic!(
                "expected RunOutcome::Completed(42) when work resolves first, got Interrupted"
            ),
        }
    }

    /// AC-008 (2/2): when `shutdown` resolves first, `run_until_shutdown`
    /// returns `RunOutcome::Interrupted` — a mutant that always returns
    /// `Completed` regardless of which arm won must be killable by this test.
    #[tokio::test]
    async fn test_run_until_shutdown_returns_interrupted_when_shutdown_fires_first() {
        let work = std::future::pending::<()>();
        let shutdown = std::future::ready(());

        let outcome = super::run_until_shutdown(work, shutdown).await;

        assert!(
            matches!(outcome, RunOutcome::Interrupted),
            "expected RunOutcome::Interrupted when shutdown resolves first"
        );
    }
}
