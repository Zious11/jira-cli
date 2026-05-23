use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::path::PathBuf;

const CACHE_TTL_DAYS: i64 = 7;

/// Implemented by cache structs that carry a timestamp for TTL checks.
pub(crate) trait Expiring {
    fn fetched_at(&self) -> DateTime<Utc>;
}

/// Read a whole-file cache. Returns `Ok(None)` on missing, expired, or corrupt
/// (unparseable) files. Propagates I/O errors.
fn read_cache<T: DeserializeOwned + Expiring>(profile: &str, filename: &str) -> Result<Option<T>> {
    let path = cache_dir(profile).join(filename);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let cache: T = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: cache file {filename} unreadable ({e}); will refetch");
            return Ok(None);
        }
    };
    if (Utc::now() - cache.fetched_at()).num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }
    Ok(Some(cache))
}

/// Write a whole-file cache. Creates the cache directory if needed.
// NFR-R-G: Non-atomic cache write — direct std::fs::write means a crash mid-write leaves
// indeterminate file state. Self-healing via deserialization-failure → cache-miss path;
// LOW severity for single-user CLI. Optional improvement: temp-file + atomic rename pattern.
fn write_cache<T: Serialize>(profile: &str, filename: &str, data: &T) -> Result<()> {
    let dir = cache_dir(profile);
    std::fs::create_dir_all(&dir)?;
    let content = serde_json::to_string_pretty(data)?;
    std::fs::write(dir.join(filename), content)?;
    Ok(())
}

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

impl Expiring for TeamCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

/// Root cache directory: `$XDG_CACHE_HOME/jr` or `~/.cache/jr`.
pub fn cache_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("jr")
    }
}

/// Per-profile cache directory: `<cache_root>/v1/<profile>/`.
pub fn cache_dir(profile: &str) -> PathBuf {
    cache_root().join("v1").join(profile)
}

/// Remove all cached data for a single profile. No-op if the directory does
/// not exist; other profiles are untouched.
pub fn clear_profile_cache(profile: &str) -> Result<()> {
    let dir = cache_dir(profile);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn read_team_cache(profile: &str) -> Result<Option<TeamCache>> {
    read_cache(profile, "teams.json")
}

pub fn write_team_cache(profile: &str, teams: &[CachedTeam]) -> Result<()> {
    write_cache(
        profile,
        "teams.json",
        &TeamCache {
            fetched_at: Utc::now(),
            teams: teams.to_vec(),
        },
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub project_type: String,
    pub simplified: bool,
    pub project_id: String,
    pub service_desk_id: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

/// Read cached project metadata for a specific project key.
///
/// Keyed cache — not genericized because TTL is checked per-entry
/// (`ProjectMeta.fetched_at`), unlike whole-file caches.
pub fn read_project_meta(profile: &str, project_key: &str) -> Result<Option<ProjectMeta>> {
    let path = cache_dir(profile).join("project_meta.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let map: HashMap<String, ProjectMeta> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: project_meta.json unreadable ({e}); will refetch");
            return Ok(None);
        }
    };

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

/// Write cached project metadata for a specific project key.
///
/// Merges into the existing map file, preserving entries for other projects.
pub fn write_project_meta(profile: &str, project_key: &str, meta: &ProjectMeta) -> Result<()> {
    let dir = cache_dir(profile);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("project_meta.json");

    // Read existing map or start fresh
    let mut map: HashMap<String, ProjectMeta> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!(
                "warning: project_meta.json unreadable ({e}); starting fresh — other cached projects will be lost"
            );
            HashMap::new()
        })
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

impl Expiring for WorkspaceCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_workspace_cache(profile: &str) -> Result<Option<WorkspaceCache>> {
    read_cache(profile, "workspace.json")
}

pub fn write_workspace_cache(profile: &str, workspace_id: &str) -> Result<()> {
    write_cache(
        profile,
        "workspace.json",
        &WorkspaceCache {
            workspace_id: workspace_id.to_string(),
            fetched_at: Utc::now(),
        },
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResolution {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolutionsCache {
    pub resolutions: Vec<CachedResolution>,
    pub fetched_at: DateTime<Utc>,
}

impl Expiring for ResolutionsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_resolutions_cache(profile: &str) -> Result<Option<ResolutionsCache>> {
    read_cache(profile, "resolutions.json")
}

pub fn write_resolutions_cache(profile: &str, resolutions: &[CachedResolution]) -> Result<()> {
    write_cache(
        profile,
        "resolutions.json",
        &ResolutionsCache {
            resolutions: resolutions.to_vec(),
            fetched_at: Utc::now(),
        },
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CmdbFieldsCache {
    pub fields: Vec<(String, String)>,
    pub fetched_at: DateTime<Utc>,
}

impl Expiring for CmdbFieldsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_cmdb_fields_cache(profile: &str) -> Result<Option<CmdbFieldsCache>> {
    read_cache(profile, "cmdb_fields.json")
}

pub fn write_cmdb_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> {
    write_cache(
        profile,
        "cmdb_fields.json",
        &CmdbFieldsCache {
            fields: fields.to_vec(),
            fetched_at: Utc::now(),
        },
    )
}

/// Per-profile cache of `GET /rest/api/3/field` results (all Jira fields).
///
/// Mirrors `CmdbFieldsCache` exactly in struct layout and TTL behaviour.
/// Path: `~/.cache/jr/v1/<profile>/fields.json`. TTL: 7 days.
///
/// Content: `(id, name)` tuples — same tuple format as `CmdbFieldsCache`.
/// Old format (if ever changed) fails serde and self-heals as a cache miss;
/// no special migration needed. To break compatibility cleanly, bump the
/// cache root from `v1/` to `v2/` — old files orphan harmlessly.
#[derive(Debug, Serialize, Deserialize)]
pub struct FieldsCache {
    pub fields: Vec<(String, String)>,
    pub fetched_at: DateTime<Utc>,
}

impl Expiring for FieldsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_fields_cache(profile: &str) -> Result<Option<FieldsCache>> {
    read_cache(profile, "fields.json")
}

/// Best-effort writer: swallows disk-write errors with `eprintln!` and returns
/// `Ok(())`. A missed write costs at most one extra HTTP call on the next
/// invocation. Cache write failures MUST NOT break a successful API call.
///
/// See "best-effort writer" pattern in CLAUDE.md Gotchas (request-type cache
/// writers). Chosen model: (b) swallow + warn — this cache is a read-
/// acceleration shortcut, not a correctness-critical store.
pub fn write_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> {
    let result = write_cache(
        profile,
        "fields.json",
        &FieldsCache {
            fields: fields.to_vec(),
            fetched_at: Utc::now(),
        },
    );
    if let Err(e) = result {
        eprintln!("warning: failed to write fields cache: {e}");
    }
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

/// Read cached attributes for a specific object type.
///
/// Keyed cache — not genericized because TTL is checked per-file
/// (`ObjectTypeAttrCache.fetched_at`) but lookup is per-key, with a different
/// return type (`Vec<CachedObjectTypeAttr>`) than the stored wrapper struct.
pub fn read_object_type_attr_cache(
    profile: &str,
    object_type_id: &str,
) -> Result<Option<Vec<CachedObjectTypeAttr>>> {
    let path = cache_dir(profile).join("object_type_attrs.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: ObjectTypeAttrCache = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: object_type_attrs.json unreadable ({e}); will refetch");
            return Ok(None);
        }
    };

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(cache.types.get(object_type_id).cloned())
}

/// Write cached attributes for a specific object type.
///
/// Merges into the existing map file, preserving entries for other object types.
pub fn write_object_type_attr_cache(
    profile: &str,
    object_type_id: &str,
    attrs: &[CachedObjectTypeAttr],
) -> Result<()> {
    let dir = cache_dir(profile);
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("object_type_attrs.json");

    let mut cache: ObjectTypeAttrCache = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!(
                "warning: object_type_attrs.json unreadable ({e}); starting fresh — other cached object types will be lost"
            );
            ObjectTypeAttrCache {
                fetched_at: Utc::now(),
                types: HashMap::new(),
            }
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

/// Cached list of request types for a (profile, serviceDeskId) pair.
/// 7-day TTL. Cache file: ~/.cache/jr/v1/<profile>/request_types_<service_desk_id>.json
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RequestTypeCache {
    types: Vec<crate::types::jsm::RequestType>,
    fetched_at: DateTime<Utc>,
}

impl Expiring for RequestTypeCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_request_type_cache(
    profile: &str,
    service_desk_id: &str,
) -> Result<Option<Vec<crate::types::jsm::RequestType>>> {
    debug_assert!(
        service_desk_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "service_desk_id contains unsafe characters for filename: {service_desk_id:?}"
    );
    let filename = format!("request_types_{service_desk_id}.json");
    Ok(read_cache::<RequestTypeCache>(profile, &filename)?.map(|c| c.types))
}

/// Write the request-type list to cache.
///
/// **Best-effort writer**: a `write_cache` failure (disk full, permission error)
/// is logged to stderr but does NOT propagate as an error. The contract is that
/// cache hygiene must never break a successful API call — at worst the next
/// invocation pays a cache miss.
///
/// (Diverges from `write_team_cache` / `write_workspace_cache` etc. which
/// propagate via `?`. Justified because the request-type cache is the first
/// cache where a write failure could leak a confusing exit code into a
/// scripted pipeline like `jr requesttype list --output json | jq ...`.)
pub fn write_request_type_cache(
    profile: &str,
    service_desk_id: &str,
    types: &[crate::types::jsm::RequestType],
) -> Result<()> {
    debug_assert!(
        service_desk_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "service_desk_id contains unsafe characters for filename: {service_desk_id:?}"
    );
    let filename = format!("request_types_{service_desk_id}.json");
    let result = write_cache(
        profile,
        &filename,
        &RequestTypeCache {
            types: types.to_vec(),
            fetched_at: Utc::now(),
        },
    );
    if let Err(e) = result {
        eprintln!("warning: failed to write request type cache: {e}");
    }
    Ok(())
}

/// Cached fields for a specific request type within a service desk.
/// 7-day TTL. Cache file: ~/.cache/jr/v1/<profile>/request_type_fields_<service_desk_id>_<request_type_id>.json
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RequestTypeFieldsCache {
    response: crate::types::jsm::RequestTypeFieldsResponse,
    fetched_at: DateTime<Utc>,
}

impl Expiring for RequestTypeFieldsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_request_type_fields_cache(
    profile: &str,
    service_desk_id: &str,
    request_type_id: &str,
) -> Result<Option<crate::types::jsm::RequestTypeFieldsResponse>> {
    debug_assert!(
        service_desk_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "service_desk_id contains unsafe characters for filename: {service_desk_id:?}"
    );
    debug_assert!(
        request_type_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "request_type_id contains unsafe characters for filename: {request_type_id:?}"
    );
    let filename = format!("request_type_fields_{service_desk_id}_{request_type_id}.json");
    Ok(read_cache::<RequestTypeFieldsCache>(profile, &filename)?.map(|c| c.response))
}

/// Write the request-type fields response to cache.
///
/// **Best-effort writer**: a `write_cache` failure (disk full, permission error)
/// is logged to stderr but does NOT propagate as an error. The contract is that
/// cache hygiene must never break a successful API call — at worst the next
/// invocation pays a cache miss.
///
/// (Diverges from `write_team_cache` / `write_workspace_cache` etc. which
/// propagate via `?`. Justified because the request-type cache is the first
/// cache where a write failure could leak a confusing exit code into a
/// scripted pipeline like `jr requesttype fields <NAME> --output json | jq ...`.)
pub fn write_request_type_fields_cache(
    profile: &str,
    service_desk_id: &str,
    request_type_id: &str,
    response: &crate::types::jsm::RequestTypeFieldsResponse,
) -> Result<()> {
    // SAFETY: JSM service desk IDs and request type IDs are documented as
    // numeric strings (verified via Atlassian REST API v3 schema). The filename
    // uses `_` as the delimiter; ambiguity would only arise if either ID
    // contained `_`, which the Atlassian schema does not permit. If Atlassian
    // ever changes IDs to non-numeric strings, switch to a structural delimiter
    // (e.g., urlencoding both components) and bump the cache root to `v2/`.
    // Charset constraint enforced by debug_assert! above (in debug builds).
    debug_assert!(
        service_desk_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "service_desk_id contains unsafe characters for filename: {service_desk_id:?}"
    );
    debug_assert!(
        request_type_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "request_type_id contains unsafe characters for filename: {request_type_id:?}"
    );
    let filename = format!("request_type_fields_{service_desk_id}_{request_type_id}.json");
    let result = write_cache(
        profile,
        &filename,
        &RequestTypeFieldsCache {
            response: response.clone(),
            fetched_at: Utc::now(),
        },
    );
    if let Err(e) = result {
        eprintln!("warning: failed to write request type fields cache: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    pub(super) fn with_temp_cache<F: FnOnce()>(f: F) {
        // Recover from poison: catch_unwind below ensures env cleanup completed
        // even if a prior test panicked, so the guarded state is consistent.
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME;
        // the variable is only read inside cache functions called within this
        // lock, so no concurrent env access occurs.
        unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path()) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
        drop(guard);
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn cache_dir_includes_v1_and_profile_subdir() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            assert!(dir.ends_with("v1/default"), "got: {}", dir.display());
        });
    }

    #[test]
    fn cross_profile_isolation_team_cache() {
        with_temp_cache(|| {
            write_team_cache(
                "prod",
                &[CachedTeam {
                    id: "t1".into(),
                    name: "Prod Team".into(),
                }],
            )
            .unwrap();

            let prod = read_team_cache("prod").unwrap().unwrap();
            assert_eq!(prod.teams[0].name, "Prod Team");

            assert!(read_team_cache("sandbox").unwrap().is_none());
        });
    }

    #[test]
    fn clear_profile_cache_removes_only_that_profile() {
        with_temp_cache(|| {
            write_team_cache(
                "prod",
                &[CachedTeam {
                    id: "p".into(),
                    name: "P".into(),
                }],
            )
            .unwrap();
            write_team_cache(
                "sandbox",
                &[CachedTeam {
                    id: "s".into(),
                    name: "S".into(),
                }],
            )
            .unwrap();

            clear_profile_cache("prod").unwrap();

            assert!(
                read_team_cache("prod").unwrap().is_none(),
                "prod cache cleared"
            );
            assert!(
                read_team_cache("sandbox").unwrap().is_some(),
                "sandbox cache preserved"
            );
        });
    }

    #[test]
    fn read_missing_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_team_cache("default").unwrap();
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
            write_team_cache("default", &teams).unwrap();

            let cache = read_team_cache("default")
                .unwrap()
                .expect("cache should exist");
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
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let result = read_team_cache("default").unwrap();
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
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&recent).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let cache = read_team_cache("default")
                .unwrap()
                .expect("cache should be valid");
            assert_eq!(cache.teams.len(), 1);
            assert_eq!(cache.teams[0].name, "Recent");
        });
    }

    #[test]
    fn read_missing_project_meta_returns_none() {
        with_temp_cache(|| {
            let result = read_project_meta("default", "NOEXIST").unwrap();
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
            write_project_meta("default", "HELPDESK", &meta).unwrap();

            let loaded = read_project_meta("default", "HELPDESK")
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
            write_project_meta("default", "HELPDESK", &meta).unwrap();

            let result = read_project_meta("default", "HELPDESK").unwrap();
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
            write_project_meta("default", "HELPDESK", &jsm).unwrap();
            write_project_meta("default", "DEV", &software).unwrap();

            let jsm_loaded = read_project_meta("default", "HELPDESK")
                .unwrap()
                .expect("should exist");
            assert_eq!(jsm_loaded.project_type, "service_desk");

            let sw_loaded = read_project_meta("default", "DEV")
                .unwrap()
                .expect("should exist");
            assert_eq!(sw_loaded.project_type, "software");
            assert!(sw_loaded.service_desk_id.is_none());
        });
    }

    #[test]
    fn read_missing_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_workspace_cache("default").unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_workspace_cache() {
        with_temp_cache(|| {
            write_workspace_cache("default", "abc-123-def").unwrap();

            let cache = read_workspace_cache("default")
                .unwrap()
                .expect("should exist");
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
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("workspace.json"), content).unwrap();

            let result = read_workspace_cache("default").unwrap();
            assert!(
                result.is_none(),
                "expired workspace cache should return None"
            );
        });
    }

    #[test]
    fn read_missing_cmdb_fields_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_cmdb_fields_cache("default").unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_cmdb_fields_cache() {
        with_temp_cache(|| {
            write_cmdb_fields_cache(
                "default",
                &[
                    ("customfield_10191".into(), "Client".into()),
                    ("customfield_10245".into(), "Hardware".into()),
                ],
            )
            .unwrap();

            let cache = read_cmdb_fields_cache("default")
                .unwrap()
                .expect("should exist");
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
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("cmdb_fields.json"), content).unwrap();

            let result = read_cmdb_fields_cache("default").unwrap();
            assert!(
                result.is_none(),
                "expired cmdb fields cache should return None"
            );
        });
    }

    // M-2: `write_fields_cache` swallow behavior — mirrors the best-effort-writer
    // pattern documented in CLAUDE.md and tested end-to-end by tests 18 and 19 in
    // `tests/issue_edit_field.rs`.  This unit test pins the library-function
    // invariant directly: on I/O error, `Ok(())` is returned and the warning is
    // emitted.  We use the `with_temp_cache` + `ENV_MUTEX` pattern to avoid race
    // conditions with other cache tests.
    #[test]
    fn test_write_fields_cache_swallow_io_error_returns_ok() {
        // Override XDG_CACHE_HOME to a *file* (not a directory) so create_dir_all
        // inside write_cache fails immediately with ENOTDIR.
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        // Acquire ENV_MUTEX to serialise env access with all other cache tests.
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME.
        unsafe { std::env::set_var("XDG_CACHE_HOME", &fake_cache_home) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_fields_cache(
                "test-m2-swallow",
                &[("customfield_10001".to_string(), "Severity".to_string())],
            )
        }));
        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
        drop(guard);

        let result = result.expect("write_fields_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_fields_cache must return Ok(()) on I/O error (best-effort writer); got: {result:?}"
        );
    }

    #[test]
    fn read_missing_object_type_attr_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_object_type_attr_cache("default", "23").unwrap();
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
            write_object_type_attr_cache("default", "23", &attrs).unwrap();

            let loaded = read_object_type_attr_cache("default", "23")
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
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("object_type_attrs.json"), content).unwrap();

            let result = read_object_type_attr_cache("default", "23").unwrap();
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
            write_object_type_attr_cache("default", "23", &attrs_a).unwrap();
            write_object_type_attr_cache("default", "45", &attrs_b).unwrap();

            let loaded_a = read_object_type_attr_cache("default", "23")
                .unwrap()
                .expect("type 23 should exist");
            assert_eq!(loaded_a[0].name, "Key");

            let loaded_b = read_object_type_attr_cache("default", "45")
                .unwrap()
                .expect("type 45 should exist");
            assert_eq!(loaded_b[0].name, "Hostname");
        });
    }

    #[test]
    fn object_type_attr_cache_corrupt_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("object_type_attrs.json"), "not json").unwrap();

            let result = read_object_type_attr_cache("default", "23").unwrap();
            assert!(result.is_none(), "corrupt cache should return None");
        });
    }

    #[test]
    fn corrupt_team_cache_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("teams.json"), "not json").unwrap();
            let result = read_team_cache("default").unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("teams.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_team_cache("default").unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }

    #[test]
    fn corrupt_workspace_cache_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("workspace.json"), "not json").unwrap();
            let result = read_workspace_cache("default").unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("workspace.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_workspace_cache("default").unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }

    #[test]
    fn corrupt_project_meta_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();

            // Garbage data
            std::fs::write(dir.join("project_meta.json"), "not json").unwrap();
            let result = read_project_meta("default", "ANY").unwrap();
            assert!(result.is_none(), "garbage data should return None");

            // Valid JSON, wrong shape
            std::fs::write(dir.join("project_meta.json"), r#"{"unexpected": true}"#).unwrap();
            let result = read_project_meta("default", "ANY").unwrap();
            assert!(result.is_none(), "wrong-shape JSON should return None");
        });
    }
}

#[cfg(test)]
mod resolution_cache_tests {
    use super::tests::with_temp_cache;
    use super::*;

    #[test]
    fn resolution_cache_round_trip() {
        with_temp_cache(|| {
            let input = vec![
                CachedResolution {
                    id: "10000".into(),
                    name: "Done".into(),
                    description: Some("Work complete".into()),
                },
                CachedResolution {
                    id: "10001".into(),
                    name: "Won't Do".into(),
                    description: None,
                },
            ];
            write_resolutions_cache("default", &input).unwrap();
            let loaded = read_resolutions_cache("default").unwrap().unwrap();

            assert_eq!(loaded.resolutions.len(), 2);
            assert_eq!(loaded.resolutions[0].name, "Done");
            assert_eq!(loaded.resolutions[1].description, None);
        });
    }

    #[test]
    fn resolution_cache_missing_returns_none() {
        with_temp_cache(|| {
            let loaded = read_resolutions_cache("default").unwrap();
            assert!(loaded.is_none());
        });
    }
}

/// M-5 (adv-01): Cross-profile isolation unit tests for the new request-type
/// and request-type-fields caches.
///
/// POLICY multi-profile-cache (CRITICAL) requires direct unit test coverage for
/// every cache family. These mirror `cross_profile_isolation_team_cache` exactly,
/// using the new `(profile, serviceDeskId)` and `(profile, sid, rtId)` keys.
#[cfg(test)]
mod request_type_cache_tests {
    use super::tests::with_temp_cache;
    use super::*;

    fn make_request_type(id: &str, name: &str) -> crate::types::jsm::RequestType {
        crate::types::jsm::RequestType {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            help_text: None,
            issue_type_id: None,
            group_ids: vec![],
        }
    }

    fn make_fields_response(field_name: &str) -> crate::types::jsm::RequestTypeFieldsResponse {
        crate::types::jsm::RequestTypeFieldsResponse {
            can_raise_on_behalf_of: true,
            can_add_request_participants: false,
            request_type_fields: vec![crate::types::jsm::RequestTypeField {
                field_id: "summary".to_string(),
                name: field_name.to_string(),
                description: None,
                required: true,
                visible: true,
                default_values: None,
                valid_values: None,
                jira_schema: serde_json::json!({"type": "string", "system": "summary"}),
            }],
        }
    }

    /// M-5 test 1: `request_type_cache` is isolated per profile.
    ///
    /// Both profiles write to the same service desk ID "10". Reads must return
    /// the data written for that profile only — not the other profile's data.
    /// Also verifies that the on-disk paths are distinct.
    #[test]
    fn test_request_type_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_types = vec![make_request_type("1", "Prod RT")];
            let sandbox_types = vec![make_request_type("2", "Sandbox RT")];

            write_request_type_cache("prod", "10", &prod_types).unwrap();
            write_request_type_cache("sandbox", "10", &sandbox_types).unwrap();

            // Prod profile reads prod data.
            let prod_read = read_request_type_cache("prod", "10")
                .unwrap()
                .expect("prod cache must exist");
            assert_eq!(
                prod_read[0].name, "Prod RT",
                "prod profile must return 'Prod RT', not sandbox data"
            );

            // Sandbox profile reads sandbox data.
            let sandbox_read = read_request_type_cache("sandbox", "10")
                .unwrap()
                .expect("sandbox cache must exist");
            assert_eq!(
                sandbox_read[0].name, "Sandbox RT",
                "sandbox profile must return 'Sandbox RT', not prod data"
            );

            // Verify on-disk paths are distinct.
            let prod_path = cache_dir("prod").join("request_types_10.json");
            let sandbox_path = cache_dir("sandbox").join("request_types_10.json");
            assert!(
                prod_path.exists(),
                "prod cache file must exist at {prod_path:?}"
            );
            assert!(
                sandbox_path.exists(),
                "sandbox cache file must exist at {sandbox_path:?}"
            );
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox cache paths must be distinct"
            );
        });
    }

    /// M-5 test 2: `request_type_fields_cache` is isolated per profile.
    ///
    /// Both profiles write fields for the same (sid="10", rtId="200"). Reads
    /// must return the field data written for that profile only.
    #[test]
    fn test_request_type_fields_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_fields = make_fields_response("Prod Field Name");
            let sandbox_fields = make_fields_response("Sandbox Field Name");

            write_request_type_fields_cache("prod", "10", "200", &prod_fields).unwrap();
            write_request_type_fields_cache("sandbox", "10", "200", &sandbox_fields).unwrap();

            // Prod profile reads prod fields.
            let prod_read = read_request_type_fields_cache("prod", "10", "200")
                .unwrap()
                .expect("prod fields cache must exist");
            assert_eq!(
                prod_read.request_type_fields[0].name, "Prod Field Name",
                "prod profile must return 'Prod Field Name', not sandbox data"
            );

            // Sandbox profile reads sandbox fields.
            let sandbox_read = read_request_type_fields_cache("sandbox", "10", "200")
                .unwrap()
                .expect("sandbox fields cache must exist");
            assert_eq!(
                sandbox_read.request_type_fields[0].name, "Sandbox Field Name",
                "sandbox profile must return 'Sandbox Field Name', not prod data"
            );

            // Verify on-disk paths are distinct.
            let prod_path = cache_dir("prod").join("request_type_fields_10_200.json");
            let sandbox_path = cache_dir("sandbox").join("request_type_fields_10_200.json");
            assert!(
                prod_path.exists(),
                "prod fields cache file must exist at {prod_path:?}"
            );
            assert!(
                sandbox_path.exists(),
                "sandbox fields cache file must exist at {sandbox_path:?}"
            );
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox fields cache paths must be distinct"
            );
        });
    }

    /// M-2 corrupt-cache regression test 1: corrupt `request_types_<sid>.json`
    /// must self-heal as a cache miss (Ok(None)), NOT propagate an error.
    ///
    /// Mirrors `corrupt_team_cache_returns_none` and `corrupt_workspace_cache_returns_none`
    /// in the parent `tests` module. Establishes sibling-coverage parity for the two new
    /// S-288-pr2 cache families (M-2 axis from adversary pass-03).
    ///
    /// Traces: adversary pass-03 M-2, S-7.01 sibling-coverage
    #[test]
    fn test_corrupt_request_type_cache_returns_none_self_heals() {
        with_temp_cache(|| {
            let dir = cache_dir("test");
            std::fs::create_dir_all(&dir).unwrap();

            // Write malformed JSON bytes to the cache file for service desk "10".
            std::fs::write(dir.join("request_types_10.json"), b"not valid json{").unwrap();

            // Must return Ok(None) — corrupt cache must self-heal as a miss, not Err.
            let result = read_request_type_cache("test", "10").unwrap();
            assert!(
                result.is_none(),
                "corrupt request_types cache must self-heal as Ok(None), not propagate an error"
            );
        });
    }

    /// M-2 corrupt-cache regression test 2: corrupt `request_type_fields_<sid>_<rtid>.json`
    /// must self-heal as a cache miss (Ok(None)), NOT propagate an error.
    ///
    /// Mirrors the request_type_cache corruption test above and the sibling team/workspace
    /// corrupt-cache tests. Establishes sibling-coverage parity for the fields cache family.
    ///
    /// Traces: adversary pass-03 M-2, S-7.01 sibling-coverage
    #[test]
    fn test_corrupt_request_type_fields_cache_returns_none_self_heals() {
        with_temp_cache(|| {
            let dir = cache_dir("test");
            std::fs::create_dir_all(&dir).unwrap();

            // Write malformed JSON bytes to the fields cache file for (sid="10", rtId="200").
            std::fs::write(
                dir.join("request_type_fields_10_200.json"),
                b"not valid json{",
            )
            .unwrap();

            // Must return Ok(None) — corrupt cache must self-heal as a miss, not Err.
            let result = read_request_type_fields_cache("test", "10", "200").unwrap();
            assert!(
                result.is_none(),
                "corrupt request_type_fields cache must self-heal as Ok(None), not propagate an error"
            );
        });
    }
}
