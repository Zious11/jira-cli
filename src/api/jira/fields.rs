use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::client::JiraClient;

#[derive(Debug, Deserialize, Serialize)]
pub struct Field {
    pub id: String,
    pub name: String,
    pub custom: Option<bool>,
    pub schema: Option<FieldSchema>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    pub custom: Option<String>,
}

impl JiraClient {
    pub async fn list_fields(&self) -> Result<Vec<Field>> {
        self.get("/rest/api/3/field").await
    }

    pub async fn find_team_field_id(&self) -> Result<Option<String>> {
        let fields = self.list_fields().await?;
        Ok(fields
            .iter()
            .find(|f| f.name.to_lowercase() == "team" && f.custom == Some(true))
            .map(|f| f.id.clone()))
    }

    pub async fn find_story_points_field_id(&self) -> Result<Vec<(String, String)>> {
        let fields = self.list_fields().await?;
        Ok(filter_story_points_fields(&fields))
    }

    pub async fn find_cmdb_fields(&self) -> Result<Vec<(String, String)>> {
        let fields = self.list_fields().await?;
        Ok(filter_cmdb_fields(&fields))
    }
}

const KNOWN_SP_SCHEMA_TYPES: &[&str] = &[
    "com.atlassian.jira.plugin.system.customfieldtypes:float",
    "com.pyxis.greenhopper.jira:jsw-story-points",
];

pub fn filter_story_points_fields(fields: &[Field]) -> Vec<(String, String)> {
    let known_names: &[&str] = &["story points", "story point estimate"];

    let mut matches: Vec<(String, String, bool)> = fields
        .iter()
        .filter(|f| {
            let is_custom = f.custom == Some(true);
            let is_number = f
                .schema
                .as_ref()
                .map(|s| s.field_type == "number")
                .unwrap_or(false);
            let name_matches = known_names.iter().any(|n| f.name.to_lowercase() == *n);
            is_custom && is_number && name_matches
        })
        .map(|f| {
            let has_known_schema = f
                .schema
                .as_ref()
                .and_then(|s| s.custom.as_deref())
                .map(|c| KNOWN_SP_SCHEMA_TYPES.contains(&c))
                .unwrap_or(false);
            (f.id.clone(), f.name.clone(), has_known_schema)
        })
        .collect();

    matches.sort_by_key(|m| std::cmp::Reverse(m.2));
    matches
        .into_iter()
        .map(|(id, name, _)| (id, name))
        .collect()
}

const CMDB_SCHEMA_TYPE: &str = "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype";

pub fn filter_cmdb_fields(fields: &[Field]) -> Vec<(String, String)> {
    fields
        .iter()
        .filter(|f| {
            f.custom == Some(true)
                && f.schema
                    .as_ref()
                    .and_then(|s| s.custom.as_deref())
                    .map(|c| c == CMDB_SCHEMA_TYPE)
                    .unwrap_or(false)
        })
        .map(|f| (f.id.clone(), f.name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_field(
        id: &str,
        name: &str,
        custom: bool,
        schema_type: &str,
        schema_custom: &str,
    ) -> Field {
        Field {
            id: id.to_string(),
            name: name.to_string(),
            custom: Some(custom),
            schema: Some(FieldSchema {
                field_type: schema_type.to_string(),
                custom: Some(schema_custom.to_string()),
            }),
        }
    }

    #[test]
    fn filter_finds_classic_story_points() {
        let fields = vec![
            make_field(
                "customfield_10031",
                "Story Points",
                true,
                "number",
                "com.atlassian.jira.plugin.system.customfieldtypes:float",
            ),
            make_field(
                "customfield_10042",
                "Task progress",
                true,
                "number",
                "com.atlassian.jira.plugin.system.customfieldtypes:float",
            ),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "customfield_10031");
    }

    #[test]
    fn filter_finds_jsw_story_point_estimate() {
        let fields = vec![make_field(
            "customfield_10016",
            "Story point estimate",
            true,
            "number",
            "com.pyxis.greenhopper.jira:jsw-story-points",
        )];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "customfield_10016");
    }

    #[test]
    fn filter_finds_both_variants() {
        let fields = vec![
            make_field(
                "customfield_10031",
                "Story Points",
                true,
                "number",
                "com.atlassian.jira.plugin.system.customfieldtypes:float",
            ),
            make_field(
                "customfield_10016",
                "Story point estimate",
                true,
                "number",
                "com.pyxis.greenhopper.jira:jsw-story-points",
            ),
        ];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_ignores_non_custom_fields() {
        let fields = vec![Field {
            id: "timeestimate".to_string(),
            name: "Remaining Estimate".to_string(),
            custom: Some(false),
            schema: Some(FieldSchema {
                field_type: "number".to_string(),
                custom: None,
            }),
        }];
        let result = filter_story_points_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_ignores_non_number_fields() {
        let fields = vec![make_field(
            "customfield_10099",
            "Story Points",
            true,
            "string",
            "com.atlassian.jira.plugin.system.customfieldtypes:textfield",
        )];
        let result = filter_story_points_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_case_insensitive_name_match() {
        let fields = vec![make_field(
            "customfield_10031",
            "STORY POINTS",
            true,
            "number",
            "com.atlassian.jira.plugin.system.customfieldtypes:float",
        )];
        let result = filter_story_points_fields(&fields);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_cmdb_fields_finds_assets_type() {
        let fields = vec![make_field(
            "customfield_10191",
            "Client",
            true,
            "any",
            "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
        )];
        let result = filter_cmdb_fields(&fields);
        assert_eq!(
            result,
            vec![("customfield_10191".to_string(), "Client".to_string())]
        );
    }

    #[test]
    fn filter_cmdb_fields_ignores_non_cmdb() {
        let fields = vec![
            make_field(
                "customfield_10031",
                "Story Points",
                true,
                "number",
                "com.atlassian.jira.plugin.system.customfieldtypes:float",
            ),
            make_field(
                "customfield_10191",
                "Client",
                true,
                "any",
                "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
            ),
        ];
        let result = filter_cmdb_fields(&fields);
        assert_eq!(
            result,
            vec![("customfield_10191".to_string(), "Client".to_string())]
        );
    }

    #[test]
    fn filter_cmdb_fields_empty_when_no_cmdb() {
        let fields = vec![make_field(
            "customfield_10031",
            "Story Points",
            true,
            "number",
            "com.atlassian.jira.plugin.system.customfieldtypes:float",
        )];
        let result: Vec<(String, String)> = filter_cmdb_fields(&fields);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_cmdb_fields_multiple() {
        let fields = vec![
            make_field(
                "customfield_10191",
                "Client",
                true,
                "any",
                "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
            ),
            make_field(
                "customfield_10245",
                "Server",
                true,
                "any",
                "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype",
            ),
        ];
        let result = filter_cmdb_fields(&fields);
        assert_eq!(
            result,
            vec![
                ("customfield_10191".to_string(), "Client".to_string()),
                ("customfield_10245".to_string(), "Server".to_string()),
            ]
        );
    }
}
