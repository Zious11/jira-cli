use anyhow::Result;

use crate::adf;
use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::output;

use super::format::{comment_visibility, format_comment_row};

pub(super) async fn handle_comments(
    key: &str,
    limit: Option<u32>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let comments = client.list_comments(key, limit).await?;

    // Show Visibility column only if any comment has sd.public.comment property
    let has_visibility = comments.iter().any(|c| comment_visibility(c).is_some());

    match output_format {
        OutputFormat::Json => {
            output::print_output(output_format, &[], &[], &comments)?;
        }
        OutputFormat::Table => {
            let verbose = client.verbose();
            let (headers, rows) = if has_visibility {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        let visibility = comment_visibility(c).unwrap_or("External");
                        let mut row =
                            format_comment_row(author, created, body_text.as_deref(), verbose);
                        // Insert Visibility before Body (index 2)
                        row.insert(2, visibility.to_string());
                        row
                    })
                    .collect();
                (vec!["Author", "Date", "Visibility", "Body"], rows)
            } else {
                let rows: Vec<Vec<String>> = comments
                    .iter()
                    .map(|c| {
                        let author = c.author.as_ref().map(|a| a.display_name.as_str());
                        let created = c.created.as_deref();
                        let body_text = c.body.as_ref().map(adf::adf_to_text);
                        format_comment_row(author, created, body_text.as_deref(), verbose)
                    })
                    .collect();
                (vec!["Author", "Date", "Body"], rows)
            };

            output::print_output(output_format, &headers, &rows, &comments)?;
        }
    }

    Ok(())
}
