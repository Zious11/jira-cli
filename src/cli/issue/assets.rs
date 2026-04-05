use anyhow::Result;

use crate::api::assets::linked::{
    cmdb_field_ids as extract_cmdb_ids, enrich_assets, extract_linked_assets,
    get_or_fetch_cmdb_fields,
};
use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::error::JrError;
use crate::output;

pub(super) async fn handle_issue_assets(
    key: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let cmdb_fields = get_or_fetch_cmdb_fields(client).await?;
    let cmdb_field_id_list = extract_cmdb_ids(&cmdb_fields);

    if cmdb_field_id_list.is_empty() {
        return Err(JrError::UserError(
            "No Assets custom fields found on this Jira instance. \
             Assets requires Jira Service Management Premium or Enterprise."
                .into(),
        )
        .into());
    }

    let extra_fields: Vec<&str> = cmdb_field_id_list.iter().map(|s| s.as_str()).collect();
    let issue = client.get_issue(key, &extra_fields).await?;
    let mut assets = extract_linked_assets(&issue.fields.extra, &cmdb_field_id_list);

    if assets.is_empty() {
        eprintln!("No assets linked to {}.", key);
        return Ok(());
    }

    enrich_assets(client, &mut assets).await;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&assets)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = assets
                .iter()
                .map(|a| {
                    vec![
                        a.key.clone().unwrap_or_else(|| {
                            a.id.as_ref()
                                .map(|id| format!("#{}", id))
                                .unwrap_or_else(|| "-".into())
                        }),
                        a.asset_type.clone().unwrap_or_else(|| "-".into()),
                        a.name.clone().unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();

            output::print_output(output_format, &["Key", "Type", "Name"], &rows, &assets)?;
        }
    }

    Ok(())
}
