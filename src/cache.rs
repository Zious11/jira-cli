use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const CACHE_TTL_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTeam {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamCache {
    pub fetched_at: DateTime<Utc>,
    pub teams: Vec<CachedTeam>,
}

pub fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("jr")
    }
}

pub fn read_team_cache() -> Result<Option<TeamCache>> {
    let path = cache_dir().join("teams.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: TeamCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_team_cache(teams: &[CachedTeam]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = TeamCache {
        fetched_at: Utc::now(),
        teams: teams.to_vec(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("teams.json"), content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub project_type: String,
    pub simplified: bool,
    pub project_id: String,
    pub service_desk_id: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_project_meta(project_key: &str) -> Result<Option<ProjectMeta>> {
    let path = cache_dir().join("project_meta.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let map: HashMap<String, ProjectMeta> = serde_json::from_str(&content)?;

    match map.get(project_key) {
        Some(meta) => {
            let age = Utc::now() - meta.fetched_at;
            if age.num_days() >= CACHE_TTL_DAYS {
                Ok(None)
            } else {
                Ok(Some(meta.clone()))
            }
        }
        None => Ok(None),
    }
}

pub fn write_project_meta(project_key: &str, meta: &ProjectMeta) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("project_meta.json");

    // Read existing map or start fresh
    let mut map: HashMap<String, ProjectMeta> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    map.insert(project_key.to_string(), meta.clone());

    let content = serde_json::to_string_pretty(&map)?;
    std::fs::write(&path, content)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceCache {
    pub workspace_id: String,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_workspace_cache() -> Result<Option<WorkspaceCache>> {
    let path = cache_dir().join("workspace.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: WorkspaceCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_workspace_cache(workspace_id: &str) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = WorkspaceCache {
        workspace_id: workspace_id.to_string(),
        fetched_at: Utc::now(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("workspace.json"), content)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CmdbFieldsCache {
    pub fields: Vec<(String, String)>,
    pub fetched_at: DateTime<Utc>,
}

pub fn read_cmdb_fields_cache() -> Result<Option<CmdbFieldsCache>> {
    let path = cache_dir().join("cmdb_fields.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: CmdbFieldsCache = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_cmdb_fields_cache(fields: &[(String, String)]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = CmdbFieldsCache {
        fields: fields.to_vec(),
        fetched_at: Utc::now(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("cmdb_fields.json"), content)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedObjectTypeAttr {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub label: bool,
    #[serde(default)]
    pub position: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectTypeAttrCache {
    pub fetched_at: DateTime<Utc>,
    pub types: HashMap<String, Vec<CachedObjectTypeAttr>>,
}

pub fn read_object_type_attr_cache(
    object_type_id: &str,
) -> Result<Option<Vec<CachedObjectTypeAttr>>> {
    let path = cache_dir().join("object_type_attrs.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: ObjectTypeAttrCache = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(cache.types.get(object_type_id).cloned())
}

pub fn write_object_type_attr_cache(
    object_type_id: &str,
    attrs: &[CachedObjectTypeAttr],
) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("object_type_attrs.json");

    let mut cache: ObjectTypeAttrCache = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or(ObjectTypeAttrCache {
            fetched_at: Utc::now(),
            types: HashMap::new(),
        })
    } else {
        ObjectTypeAttrCache {
            fetched_at: Utc::now(),
            types: HashMap::new(),
        }
    };

    cache
        .types
        .insert(object_type_id.to_string(), attrs.to_vec());
    cache.fetched_at = Utc::now();

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_temp_cache<F: FnOnce()>(f: F) {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        // SAFETY: test holds ENV_MUTEX, so no concurrent env access.
        unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path()) };
        f();
        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    }

    #[test]
    fn read_missing_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_team_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_returns_data() {
        with_temp_cache(|| {
            let teams = vec![
                CachedTeam {
                    id: "uuid-1".into(),
                    name: "Alpha".into(),
                },
                CachedTeam {
                    id: "uuid-2".into(),
                    name: "Beta".into(),
                },
            ];
            write_team_cache(&teams).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should exist");
            assert_eq!(cache.teams.len(), 2);
            assert_eq!(cache.teams[0].name, "Alpha");
            assert_eq!(cache.teams[1].name, "Beta");
        });
    }

    #[test]
    fn expired_cache_returns_none() {
        with_temp_cache(|| {
            let expired = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(8),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Old".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let result = read_team_cache().unwrap();
            assert!(result.is_none(), "expired cache should return None");
        });
    }

    #[test]
    fn valid_cache_within_ttl() {
        with_temp_cache(|| {
            let recent = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(3),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Recent".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&recent).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should be valid");
            assert_eq!(cache.teams.len(), 1);
            assert_eq!(cache.teams[0].name, "Recent");
        });
    }

    #[test]
    fn read_missing_project_meta_returns_none() {
        with_temp_cache(|| {
            let result = read_project_meta("NOEXIST").unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_project_meta() {
        with_temp_cache(|| {
            let meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now(),
            };
            write_project_meta("HELPDESK", &meta).unwrap();

            let loaded = read_project_meta("HELPDESK")
                .unwrap()
                .expect("should exist");
            assert_eq!(loaded.project_type, "service_desk");
            assert_eq!(loaded.service_desk_id.as_deref(), Some("15"));
            assert_eq!(loaded.project_id, "10042");
            assert!(!loaded.simplified);
        });
    }

    #[test]
    fn expired_project_meta_returns_none() {
        with_temp_cache(|| {
            let meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now() - chrono::Duration::days(8),
            };
            write_project_meta("HELPDESK", &meta).unwrap();

            let result = read_project_meta("HELPDESK").unwrap();
            assert!(result.is_none(), "expired project meta should return None");
        });
    }

    #[test]
    fn project_meta_multiple_projects() {
        with_temp_cache(|| {
            let jsm = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now(),
            };
            let software = ProjectMeta {
                project_type: "software".into(),
                simplified: true,
                project_id: "10001".into(),
                service_desk_id: None,
                fetched_at: Utc::now(),
            };
            write_project_meta("HELPDESK", &jsm).unwrap();
            write_project_meta("DEV", &software).unwrap();

            let jsm_loaded = read_project_meta("HELPDESK")
                .unwrap()
                .expect("should exist");
            assert_eq!(jsm_loaded.project_type, "service_desk");

            let sw_loaded = read_project_meta("DEV").unwrap().expect("should exist");
            assert_eq!(sw_loaded.project_type, "software");
            assert!(sw_loaded.service_desk_id.is_none());
        });
    }

    #[test]
    fn read_missing_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_workspace_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_workspace_cache() {
        with_temp_cache(|| {
            write_workspace_cache("abc-123-def").unwrap();

            let cache = read_workspace_cache().unwrap().expect("should exist");
            assert_eq!(cache.workspace_id, "abc-123-def");
        });
    }

    #[test]
    fn expired_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let expired = WorkspaceCache {
                workspace_id: "old-id".into(),
                fetched_at: Utc::now() - chrono::Duration::days(8),
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("workspace.json"), content).unwrap();

            let result = read_workspace_cache().unwrap();
            assert!(
                result.is_none(),
                "expired workspace cache should return None"
            );
        });
    }

    #[test]
    fn read_missing_cmdb_fields_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_cmdb_fields_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_cmdb_fields_cache() {
        with_temp_cache(|| {
            write_cmdb_fields_cache(&[
                ("customfield_10191".into(), "Client".into()),
                ("customfield_10245".into(), "Hardware".into()),
            ])
            .unwrap();

            let cache = read_cmdb_fields_cache().unwrap().expect("should exist");
            assert_eq!(
                cache.fields,
                vec![
                    ("customfield_10191".to_string(), "Client".to_string()),
                    ("customfield_10245".to_string(), "Hardware".to_string()),
                ]
            );
        });
    }

    #[test]
    fn expired_cmdb_fields_cache_returns_none() {
        with_temp_cache(|| {
            let expired = CmdbFieldsCache {
                fields: vec![("customfield_10191".into(), "Client".into())],
                fetched_at: Utc::now() - chrono::Duration::days(8),
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("cmdb_fields.json"), content).unwrap();

            let result = read_cmdb_fields_cache().unwrap();
            assert!(
                result.is_none(),
                "expired cmdb fields cache should return None"
            );
        });
    }

    #[test]
    fn read_missing_object_type_attr_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_object_type_attr_cache("23").unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_object_type_attr_cache() {
        with_temp_cache(|| {
            let attrs = vec![
                CachedObjectTypeAttr {
                    id: "134".into(),
                    name: "Key".into(),
                    system: true,
                    hidden: false,
                    label: false,
                    position: 0,
                },
                CachedObjectTypeAttr {
                    id: "135".into(),
                    name: "Name".into(),
                    system: false,
                    hidden: false,
                    label: true,
                    position: 1,
                },
            ];
            write_object_type_attr_cache("23", &attrs).unwrap();

            let loaded = read_object_type_attr_cache("23")
                .unwrap()
                .expect("should exist");
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].name, "Key");
            assert!(loaded[0].system);
            assert_eq!(loaded[1].name, "Name");
            assert!(loaded[1].label);
        });
    }

    #[test]
    fn expired_object_type_attr_cache_returns_none() {
        with_temp_cache(|| {
            let expired = ObjectTypeAttrCache {
                fetched_at: Utc::now() - chrono::Duration::days(8),
                types: {
                    let mut m = HashMap::new();
                    m.insert(
                        "23".to_string(),
                        vec![CachedObjectTypeAttr {
                            id: "134".into(),
                            name: "Key".into(),
                            system: true,
                            hidden: false,
                            label: false,
                            position: 0,
                        }],
                    );
                    m
                },
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("object_type_attrs.json"), content).unwrap();

            let result = read_object_type_attr_cache("23").unwrap();
            assert!(result.is_none(), "expired cache should return None");
        });
    }

    #[test]
    fn object_type_attr_cache_multiple_types() {
        with_temp_cache(|| {
            let attrs_a = vec![CachedObjectTypeAttr {
                id: "134".into(),
                name: "Key".into(),
                system: true,
                hidden: false,
                label: false,
                position: 0,
            }];
            let attrs_b = vec![CachedObjectTypeAttr {
                id: "200".into(),
                name: "Hostname".into(),
                system: false,
                hidden: false,
                label: false,
                position: 3,
            }];
            write_object_type_attr_cache("23", &attrs_a).unwrap();
            write_object_type_attr_cache("45", &attrs_b).unwrap();

            let loaded_a = read_object_type_attr_cache("23")
                .unwrap()
                .expect("type 23 should exist");
            assert_eq!(loaded_a[0].name, "Key");

            let loaded_b = read_object_type_attr_cache("45")
                .unwrap()
                .expect("type 45 should exist");
            assert_eq!(loaded_b[0].name, "Hostname");
        });
    }

    #[test]
    fn object_type_attr_cache_corrupt_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("object_type_attrs.json"), "not json").unwrap();

            let result = read_object_type_attr_cache("23").unwrap();
            assert!(result.is_none(), "corrupt cache should return None");
        });
    }
}
