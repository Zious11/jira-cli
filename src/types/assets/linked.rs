use serde::Serialize;

/// An asset reference extracted from a CMDB custom field on a Jira issue.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkedAsset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub asset_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

impl LinkedAsset {
    /// Human-readable display: "OBJ-1 (Acme Corp)", "OBJ-1", or "#12345 (run "jr init" to resolve asset names)".
    pub fn display(&self) -> String {
        match (&self.key, &self.name) {
            (Some(key), Some(name)) => format!("{} ({})", key, name),
            (Some(key), None) => key.clone(),
            (None, Some(name)) => name.clone(),
            (None, None) => match &self.id {
                Some(id) => format!("#{} (run \"jr init\" to resolve asset names)", id),
                None => "(unknown)".into(),
            },
        }
    }
}

/// Format a list of linked assets for display in a table cell.
pub fn format_linked_assets(assets: &[LinkedAsset]) -> String {
    if assets.is_empty() {
        return "(none)".into();
    }
    assets
        .iter()
        .map(|a| a.display())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format for list table: first asset + count if multiple.
pub fn format_linked_assets_short(assets: &[LinkedAsset]) -> String {
    match assets.len() {
        0 => "-".into(),
        1 => assets[0].display(),
        n => format!("{} (+{} more)", assets[0].display(), n - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_key_and_name() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme Corp".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "OBJ-1 (Acme Corp)");
    }

    #[test]
    fn display_key_only() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "OBJ-1");
    }

    #[test]
    fn display_name_only() {
        let a = LinkedAsset {
            name: Some("Acme Corp".into()),
            ..Default::default()
        };
        assert_eq!(a.display(), "Acme Corp");
    }

    #[test]
    fn display_id_fallback_with_hint() {
        let a = LinkedAsset {
            id: Some("12345".into()),
            ..Default::default()
        };
        assert_eq!(
            a.display(),
            "#12345 (run \"jr init\" to resolve asset names)"
        );
    }

    #[test]
    fn display_nothing() {
        let a = LinkedAsset::default();
        assert_eq!(a.display(), "(unknown)");
    }

    #[test]
    fn format_empty_list() {
        assert_eq!(format_linked_assets(&[]), "(none)");
    }

    #[test]
    fn format_single_asset() {
        let assets = vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        }];
        assert_eq!(format_linked_assets(&assets), "OBJ-1 (Acme)");
    }

    #[test]
    fn format_multiple_assets() {
        let assets = vec![
            LinkedAsset {
                key: Some("OBJ-1".into()),
                name: Some("Acme".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-2".into()),
                name: Some("Globex".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            format_linked_assets(&assets),
            "OBJ-1 (Acme), OBJ-2 (Globex)"
        );
    }

    #[test]
    fn format_short_empty() {
        assert_eq!(format_linked_assets_short(&[]), "-");
    }

    #[test]
    fn format_short_single() {
        let assets = vec![LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        }];
        assert_eq!(format_linked_assets_short(&assets), "OBJ-1 (Acme)");
    }

    #[test]
    fn format_short_multiple() {
        let assets = vec![
            LinkedAsset {
                key: Some("OBJ-1".into()),
                name: Some("Acme".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-2".into()),
                ..Default::default()
            },
            LinkedAsset {
                key: Some("OBJ-3".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            format_linked_assets_short(&assets),
            "OBJ-1 (Acme) (+2 more)"
        );
    }

    #[test]
    fn serialize_json_skips_none() {
        let a = LinkedAsset {
            key: Some("OBJ-1".into()),
            name: Some("Acme".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json.get("key").unwrap(), "OBJ-1");
        assert_eq!(json.get("name").unwrap(), "Acme");
        assert!(json.get("id").is_none());
        assert!(json.get("workspace_id").is_none());
    }
}
