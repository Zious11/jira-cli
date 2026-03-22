use anyhow::Result;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::{OutputFormat, WorklogCommand};
use crate::duration;
use crate::output;

/// Handle all worklog subcommands.
pub async fn handle(
    command: WorklogCommand,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    match command {
        WorklogCommand::Add {
            key,
            duration: dur,
            message,
        } => handle_add(&key, &dur, message.as_deref(), client, output_format).await,
        WorklogCommand::List { key } => handle_list(&key, client, output_format).await,
    }
}

async fn handle_add(
    key: &str,
    dur: &str,
    message: Option<&str>,
    client: &JiraClient,
    output_format: &OutputFormat,
) -> Result<()> {
    let seconds = duration::parse_duration(dur, 8, 5)?;
    let comment = message.map(adf::text_to_adf);

    let worklog = client.add_worklog(key, seconds, comment).await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&worklog)?);
        }
        OutputFormat::Table => {
            let formatted = duration::format_duration(seconds);
            output::print_success(&format!("Logged {formatted} on {key}"));
        }
    }

    Ok(())
}

async fn handle_list(key: &str, client: &JiraClient, output_format: &OutputFormat) -> Result<()> {
    let worklogs = client.list_worklogs(key).await?;

    let rows: Vec<Vec<String>> = worklogs
        .iter()
        .map(|w| {
            let author = w
                .author
                .as_ref()
                .map(|a| a.display_name.clone())
                .unwrap_or_else(|| "-".into());
            let time = w.time_spent.clone().unwrap_or_else(|| {
                w.time_spent_seconds
                    .map(duration::format_duration)
                    .unwrap_or_else(|| "-".into())
            });
            let started = w.started.clone().unwrap_or_else(|| "-".into());
            vec![author, time, started]
        })
        .collect();

    output::print_output(
        output_format,
        &["Author", "Time", "Started"],
        &rows,
        &worklogs,
    )?;

    Ok(())
}
