use anyhow::Result;

use crate::adf;
use crate::api::assets::linked::{
    enrich_assets, enrich_json_assets, extract_linked_assets, get_or_fetch_cmdb_fields,
};
use crate::api::client::JiraClient;
use crate::cli::{IssueCommand, OutputFormat};
use crate::config::Config;
use crate::output;
use crate::types::assets::LinkedAsset;
use crate::types::assets::linked::format_linked_assets;

use super::format;
use super::format::format_comment_date;
use super::helpers;

pub(super) async fn handle_view(
    command: IssueCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
) -> Result<()> {
    let IssueCommand::View { key } = command else {
        unreachable!()
    };

    let sp_field_id = config.global.fields.story_points_field_id.as_deref();
    let team_field_id: Option<&str> = config.global.fields.team_field_id.as_deref();
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await.unwrap_or_default();
    let extra_owned = helpers::compose_extra_fields(config, &cmdb_fields);
    let extra: Vec<&str> = extra_owned.iter().map(String::as_str).collect();
    let mut issue = client.get_issue(&key, &extra).await?;

    // Extract and enrich assets per-field (shared by both JSON and table paths).
    // Iterate cmdb_fields directly so we always have (field_id, field_name) together —
    // avoids any name-based reverse lookups that could break with duplicate field names.
    let per_field_enriched: Vec<(String, String, Vec<LinkedAsset>)> = if !cmdb_fields.is_empty() {
        // Extract per-field, keeping both ID and name
        let mut per_field: Vec<(String, String, Vec<LinkedAsset>)> = Vec::new();
        for (field_id, field_name) in &cmdb_fields {
            let assets = extract_linked_assets(&issue.fields.extra, std::slice::from_ref(field_id));
            if !assets.is_empty() {
                per_field.push((field_id.clone(), field_name.clone(), assets));
            }
        }

        // Collect all assets for batch enrichment
        let mut all_assets: Vec<LinkedAsset> = per_field
            .iter()
            .flat_map(|(_, _, assets)| assets.clone())
            .collect();
        enrich_assets(client, &mut all_assets).await;

        // Redistribute enriched assets back
        let mut enriched = Vec::new();
        let mut offset = 0;
        for (field_id, field_name, original_assets) in &per_field {
            let count = original_assets.len();
            let assets = all_assets[offset..offset + count].to_vec();
            offset += count;
            enriched.push((field_id.clone(), field_name.clone(), assets));
        }
        enriched
    } else {
        Vec::new()
    };

    match output_format {
        OutputFormat::Json => {
            // Inject enriched data back into JSON before printing
            if !per_field_enriched.is_empty() {
                let per_field_by_id: Vec<(String, Vec<LinkedAsset>)> = per_field_enriched
                    .iter()
                    .map(|(id, _, assets)| (id.clone(), assets.clone()))
                    .collect();
                enrich_json_assets(&mut issue.fields.extra, &per_field_by_id);
            }
            println!("{}", output::render_json(&issue)?);
        }
        OutputFormat::Table => {
            let desc_text = issue
                .fields
                .description
                .as_ref()
                .map(adf::adf_to_text)
                .unwrap_or_else(|| "(no description)".into());

            let mut rows = vec![
                vec!["Key".into(), issue.key.clone()],
                vec!["Summary".into(), issue.fields.summary.clone()],
                vec![
                    "Type".into(),
                    issue
                        .fields
                        .issue_type
                        .as_ref()
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Status".into(),
                    issue
                        .fields
                        .status
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Priority".into(),
                    issue
                        .fields
                        .priority
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default(),
                ],
                vec![
                    "Assignee".into(),
                    issue
                        .fields
                        .assignee
                        .as_ref()
                        .map(|a| a.display_name.clone())
                        .unwrap_or_else(|| "Unassigned".into()),
                ],
                vec![
                    "Reporter".into(),
                    issue
                        .fields
                        .reporter
                        .as_ref()
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| "(none)".into()),
                ],
                vec![
                    "Created".into(),
                    issue
                        .fields
                        .created
                        .as_deref()
                        .map(|c| format_comment_date(c, client.verbose()))
                        .unwrap_or_else(|| "-".into()),
                ],
                vec![
                    "Updated".into(),
                    issue
                        .fields
                        .updated
                        .as_deref()
                        .map(|c| format_comment_date(c, client.verbose()))
                        .unwrap_or_else(|| "-".into()),
                ],
                vec![
                    "Project".into(),
                    issue
                        .fields
                        .project
                        .as_ref()
                        .map(|p| format!("{} ({})", p.name.as_deref().unwrap_or(""), p.key))
                        .unwrap_or_default(),
                ],
                vec![
                    "Labels".into(),
                    issue
                        .fields
                        .labels
                        .as_ref()
                        .filter(|l| !l.is_empty())
                        .map(|l| l.join(", "))
                        .unwrap_or_else(|| "(none)".into()),
                ],
            ];

            rows.push(vec![
                "Parent".into(),
                issue
                    .fields
                    .parent
                    .as_ref()
                    .map(|p| {
                        let summary = p
                            .fields
                            .as_ref()
                            .and_then(|f| f.summary.as_deref())
                            .unwrap_or("");
                        format!("{} ({})", p.key, summary)
                    })
                    .unwrap_or_else(|| "(none)".into()),
            ]);

            let links_display = issue
                .fields
                .issuelinks
                .as_ref()
                .filter(|links| !links.is_empty())
                .map(|links| {
                    links
                        .iter()
                        .map(|link| {
                            if let Some(ref outward) = link.outward_issue {
                                let desc = link
                                    .link_type
                                    .outward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = outward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, outward.key, summary)
                            } else if let Some(ref inward) = link.inward_issue {
                                let desc = link
                                    .link_type
                                    .inward
                                    .as_deref()
                                    .unwrap_or(&link.link_type.name);
                                let summary = inward
                                    .fields
                                    .as_ref()
                                    .and_then(|f| f.summary.as_deref())
                                    .unwrap_or("");
                                format!("{} {} ({})", desc, inward.key, summary)
                            } else {
                                link.link_type.name.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "(none)".into());
            rows.push(vec!["Links".into(), links_display]);

            // Per-field asset rows (replaces the old single "Assets" row)
            for (_, field_name, assets) in &per_field_enriched {
                let display = format_linked_assets(assets);
                rows.push(vec![field_name.clone(), display]);
            }

            if let Some(field_id) = sp_field_id {
                let points_display = issue
                    .fields
                    .story_points(field_id)
                    .map(format::format_points)
                    .unwrap_or_else(|| "(none)".into());
                rows.push(vec!["Points".into(), points_display]);
            }

            if let Some(field_id) = team_field_id {
                if let Some(team_uuid) = issue.fields.team_id(field_id, client.verbose()) {
                    let team_display = match crate::cache::read_team_cache() {
                        Ok(Some(c)) => c
                            .teams
                            .into_iter()
                            .find(|t| t.id == team_uuid)
                            .map(|t| t.name)
                            .unwrap_or_else(|| {
                                format!(
                                    "{} (name not cached — run 'jr team list --refresh')",
                                    team_uuid
                                )
                            }),
                        Ok(None) => format!(
                            "{} (name not cached — run 'jr team list --refresh')",
                            team_uuid
                        ),
                        Err(e) => {
                            eprintln!("warning: failed to read team cache: {e}");
                            format!("{} (team cache unreadable)", team_uuid)
                        }
                    };
                    rows.push(vec!["Team".into(), team_display]);
                }
            }

            rows.push(vec!["Description".into(), desc_text]);

            println!("{}", output::render_table(&["Field", "Value"], &rows));
        }
    }

    Ok(())
}
