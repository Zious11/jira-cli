use std::collections::HashMap;

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::OutputFormat;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};

/// Resolve a --schema flag to a single schema, matching by ID (exact) or name (partial).
pub(super) fn resolve_schema<'a>(
    input: &str,
    schemas: &'a [crate::types::assets::ObjectSchema],
) -> Result<&'a crate::types::assets::ObjectSchema> {
    // Try exact ID match first
    if let Some(s) = schemas.iter().find(|s| s.id == input) {
        return Ok(s);
    }
    // Partial match on name
    let names: Vec<String> = schemas.iter().map(|s| s.name.clone()).collect();
    match partial_match::partial_match(input, &names) {
        MatchResult::Exact(name) => Ok(schemas.iter().find(|s| s.name == name).unwrap()),
        MatchResult::ExactMultiple(_) => {
            let input_lower = input.to_lowercase();
            let duplicates: Vec<String> = schemas
                .iter()
                .filter(|s| s.name.to_lowercase() == input_lower)
                .map(|s| format!("{} (id: {})", s.name, s.id))
                .collect();
            Err(JrError::UserError(format!(
                "Multiple schemas named \"{}\": {}. Use the schema ID instead.",
                input,
                duplicates.join(", ")
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous schema \"{}\". Matches: {}",
            input,
            matches.join(", ")
        ))
        .into()),
        MatchResult::None(all) => {
            let available = if all.is_empty() {
                "none".to_string()
            } else {
                all.join(", ")
            };
            Err(JrError::UserError(format!(
                "No schema matching \"{}\". Available: {}",
                input, available
            ))
            .into())
        }
    }
}

pub async fn handle_schemas(
    workspace_id: &str,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let rows: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| {
            vec![
                s.id.clone(),
                s.object_schema_key.clone(),
                s.name.clone(),
                s.description.clone().unwrap_or_else(|| "\u{2014}".into()),
                s.object_type_count.to_string(),
                s.object_count.to_string(),
            ]
        })
        .collect();

    output::print_output(
        output_format,
        &["ID", "Key", "Name", "Description", "Types", "Objects"],
        &rows,
        &schemas,
    )
}

pub async fn handle_types(
    workspace_id: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Build a map of schema_id → schema_name for injection
    let schema_names: HashMap<&str, &str> = schemas
        .iter()
        .map(|s| (s.id.as_str(), s.name.as_str()))
        .collect();

    let mut all_types = Vec::new();
    for schema in &target_schemas {
        let types = client.list_object_types(workspace_id, &schema.id).await?;
        all_types.extend(types);
    }

    match output_format {
        OutputFormat::Json => {
            // Inject schemaName into each entry
            let mut json_types: Vec<serde_json::Value> = Vec::new();
            for t in &all_types {
                let mut val = serde_json::to_value(t)?;
                if let Some(map) = val.as_object_mut() {
                    let schema_name = schema_names.get(t.object_schema_id.as_str()).unwrap_or(&"");
                    map.insert(
                        "schemaName".to_string(),
                        serde_json::Value::String(schema_name.to_string()),
                    );
                }
                json_types.push(val);
            }
            println!("{}", output::render_json(&json_types)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = all_types
                .iter()
                .map(|t| {
                    let schema_name = schema_names
                        .get(t.object_schema_id.as_str())
                        .unwrap_or(&"\u{2014}");
                    vec![
                        t.id.clone(),
                        t.name.clone(),
                        schema_name.to_string(),
                        t.description.clone().unwrap_or_else(|| "\u{2014}".into()),
                        t.object_count.to_string(),
                    ]
                })
                .collect();

            output::print_output(
                output_format,
                &["ID", "Name", "Schema", "Description", "Objects"],
                &rows,
                &all_types,
            )?;
        }
    }
    Ok(())
}

/// Build an ambiguous type error with schema-labeled matches.
fn ambiguous_type_error(
    input: &str,
    matches: &[String],
    candidates: &[(crate::types::assets::ObjectTypeEntry, String)],
) -> JrError {
    let labeled: Vec<String> = candidates
        .iter()
        .filter(|(t, _)| matches.contains(&t.name))
        .map(|(t, s)| format!("{} ({})", t.name, s))
        .collect();
    JrError::UserError(format!(
        "Ambiguous type \"{}\". Matches: {}. Use --schema to narrow results.",
        input,
        labeled.join(", ")
    ))
}

/// Format the Type column for an attribute definition.
fn format_attribute_type(attr: &crate::types::assets::ObjectTypeAttributeDef) -> String {
    if let Some(ref dt) = attr.default_type {
        return dt.name.clone();
    }
    if let Some(ref rot) = attr.reference_object_type {
        return format!("Reference \u{2192} {}", rot.name);
    }
    "Unknown".to_string()
}

pub async fn handle_schema(
    workspace_id: &str,
    type_name: &str,
    schema_filter: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let schemas = client.list_object_schemas(workspace_id).await?;
    if schemas.is_empty() {
        return Err(JrError::UserError("No asset schemas found in this workspace.".into()).into());
    }

    let target_schemas: Vec<&crate::types::assets::ObjectSchema> = match &schema_filter {
        Some(input) => vec![resolve_schema(input, &schemas)?],
        None => schemas.iter().collect(),
    };

    // Collect all object types with their schema name
    let mut candidates: Vec<(crate::types::assets::ObjectTypeEntry, String)> = Vec::new();
    for schema in &target_schemas {
        let types = client.list_object_types(workspace_id, &schema.id).await?;
        for t in types {
            candidates.push((t, schema.name.clone()));
        }
    }

    if candidates.is_empty() {
        return Err(JrError::UserError(
            "No object types found. Run \"jr assets schemas\" to verify your workspace has schemas."
                .into(),
        )
        .into());
    }

    // Partial match on type name — deduplicated for partial_match, then
    // check for cross-schema duplicates on the resolved name.
    let mut deduped_names: Vec<String> = candidates.iter().map(|(t, _)| t.name.clone()).collect();
    deduped_names.sort();
    deduped_names.dedup();
    let matched_name = match partial_match::partial_match(type_name, &deduped_names) {
        MatchResult::Exact(name) => name,
        // Case-sensitive dedup upstream; treat like Exact if case-variant duplicates slip through
        MatchResult::ExactMultiple(name) => name,
        MatchResult::Ambiguous(matches) => {
            return Err(ambiguous_type_error(type_name, &matches, &candidates).into());
        }
        MatchResult::None(_) => {
            return Err(JrError::UserError(format!(
                "No object type matching \"{}\". Run \"jr assets types\" to see available types.",
                type_name
            ))
            .into());
        }
    };

    // Check for cross-schema duplicates: same name in multiple schemas
    let same_name: Vec<&(crate::types::assets::ObjectTypeEntry, String)> = candidates
        .iter()
        .filter(|(t, _)| t.name == matched_name)
        .collect();
    if same_name.len() > 1 {
        let labeled: Vec<String> = same_name
            .iter()
            .map(|(t, s)| format!("{} ({})", t.name, s))
            .collect();
        return Err(JrError::UserError(format!(
            "Ambiguous type \"{}\". Matches: {}. Use --schema to narrow results.",
            type_name,
            labeled.join(", ")
        ))
        .into());
    }

    let (matched_type, schema_name) = same_name.first().unwrap();

    // Fetch attributes
    let attrs = client
        .get_object_type_attributes(workspace_id, &matched_type.id)
        .await?;

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&attrs)?);
        }
        OutputFormat::Table => {
            println!(
                "Object Type: {} (Schema: {})\n",
                matched_type.name, schema_name
            );

            let mut visible: Vec<&crate::types::assets::ObjectTypeAttributeDef> =
                attrs.iter().filter(|a| !a.system && !a.hidden).collect();
            visible.sort_by_key(|a| a.position);

            let rows: Vec<Vec<String>> = visible
                .iter()
                .map(|a| {
                    vec![
                        a.position.to_string(),
                        a.name.clone(),
                        format_attribute_type(a),
                        if a.minimum_cardinality >= 1 {
                            "Yes".into()
                        } else {
                            "No".into()
                        },
                        if a.editable {
                            "Yes".into()
                        } else {
                            "No".into()
                        },
                    ]
                })
                .collect();

            if rows.is_empty() {
                println!("No user-defined attributes.");
            } else {
                println!(
                    "{}",
                    output::render_table(&["Pos", "Name", "Type", "Required", "Editable"], &rows)
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::assets::{DefaultType, ObjectTypeAttributeDef, ReferenceObjectType};

    fn make_attr_def(
        default_type: Option<DefaultType>,
        reference_object_type: Option<ReferenceObjectType>,
    ) -> ObjectTypeAttributeDef {
        ObjectTypeAttributeDef {
            id: "1".into(),
            name: "test".into(),
            system: false,
            hidden: false,
            label: false,
            position: 0,
            default_type,
            reference_type: None,
            reference_object_type,
            minimum_cardinality: 0,
            maximum_cardinality: 1,
            editable: true,
            description: None,
            options: None,
        }
    }

    #[test]
    fn format_attr_type_default_type() {
        let attr = make_attr_def(
            Some(DefaultType {
                id: 0,
                name: "Text".into(),
            }),
            None,
        );
        assert_eq!(format_attribute_type(&attr), "Text");
    }

    #[test]
    fn format_attr_type_reference() {
        let attr = make_attr_def(
            None,
            Some(ReferenceObjectType {
                id: "122".into(),
                name: "Service".into(),
            }),
        );
        assert_eq!(format_attribute_type(&attr), "Reference \u{2192} Service");
    }

    #[test]
    fn format_attr_type_unknown() {
        let attr = make_attr_def(None, None);
        assert_eq!(format_attribute_type(&attr), "Unknown");
    }

    #[test]
    fn format_attr_type_default_takes_precedence() {
        let attr = make_attr_def(
            Some(DefaultType {
                id: 0,
                name: "Text".into(),
            }),
            Some(ReferenceObjectType {
                id: "1".into(),
                name: "Svc".into(),
            }),
        );
        assert_eq!(format_attribute_type(&attr), "Text");
    }

    // ── resolve_schema tests ─────────────────────────────────────

    fn make_schema(id: &str, name: &str) -> crate::types::assets::ObjectSchema {
        crate::types::assets::ObjectSchema {
            id: id.into(),
            name: name.into(),
            object_schema_key: format!("KEY{}", id),
            description: None,
            object_count: 0,
            object_type_count: 0,
        }
    }

    #[test]
    fn resolve_schema_exact_id_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = resolve_schema("10", &schemas).unwrap();
        assert_eq!(result.id, "10");
        assert_eq!(result.name, "ITSM");
    }

    #[test]
    fn resolve_schema_exact_name_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = resolve_schema("ITSM", &schemas).unwrap();
        assert_eq!(result.id, "10");
    }

    #[test]
    fn resolve_schema_case_insensitive_name_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let result = resolve_schema("itsm", &schemas).unwrap();
        assert_eq!(result.id, "10");
    }

    #[test]
    fn resolve_schema_single_substring_is_ambiguous() {
        // Single substring hits are now Ambiguous — callers must use the exact name.
        let schemas = vec![make_schema("10", "ITSM Assets"), make_schema("20", "HR")];
        let err = resolve_schema("itsm", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Ambiguous"), "got: {msg}");
        assert!(msg.contains("ITSM Assets"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_no_match() {
        let schemas = vec![make_schema("10", "ITSM"), make_schema("20", "HR")];
        let err = resolve_schema("Finance", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No schema matching"), "got: {msg}");
        assert!(msg.contains("Finance"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_ambiguous_match() {
        let schemas = vec![
            make_schema("10", "IT Assets"),
            make_schema("20", "IT Services"),
        ];
        let err = resolve_schema("IT", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Ambiguous"), "got: {msg}");
    }

    #[test]
    fn resolve_schema_duplicate_names_returns_error_with_ids() {
        let schemas = vec![make_schema("10", "Assets"), make_schema("20", "Assets")];
        let err = resolve_schema("Assets", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple schemas"), "got: {msg}");
        assert!(msg.contains("id: 10"), "should list first ID, got: {msg}");
        assert!(msg.contains("id: 20"), "should list second ID, got: {msg}");
        assert!(
            msg.contains("Use the schema ID instead"),
            "should suggest using ID, got: {msg}"
        );
    }

    #[test]
    fn resolve_schema_duplicate_names_case_insensitive() {
        let schemas = vec![make_schema("10", "Assets"), make_schema("20", "assets")];
        let err = resolve_schema("assets", &schemas).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Multiple schemas"), "got: {msg}");
        assert!(msg.contains("id: 10"), "should list first ID, got: {msg}");
        assert!(msg.contains("id: 20"), "should list second ID, got: {msg}");
    }

    #[test]
    fn resolve_schema_id_takes_priority_over_name() {
        // Schema ID "HR" matches exactly, even though name "ITSM" doesn't
        let schemas = vec![make_schema("HR", "ITSM"), make_schema("20", "HR")];
        let result = resolve_schema("HR", &schemas).unwrap();
        assert_eq!(result.id, "HR");
        assert_eq!(result.name, "ITSM");
    }
}
