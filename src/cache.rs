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

/// Pure fallback for the Windows `%LOCALAPPDATA%` path when `dirs::cache_dir()` returns
/// `None`. Accepts the raw `env::var("LOCALAPPDATA").ok()` value so the logic can be
/// tested on any platform without a `#[cfg(windows)]` gate.
///
/// Rules (BC-6.2.016 EC-1, EC-4):
/// - `Some(s)` where `s` is non-empty → `PathBuf::from(s)`
/// - `Some(s)` where `s` is empty → `PathBuf::from(".")` (treated as unset)
/// - `None` → `PathBuf::from(".")`
pub fn cache_localappdata_fallback(env_val: Option<String>) -> PathBuf {
    env_val
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Root cache directory: `%LOCALAPPDATA%\jr` on Windows, `$XDG_CACHE_HOME/jr` or
/// `~/.cache/jr` on Unix.
pub fn cache_root() -> PathBuf {
    // JR_CACHE_DIR override is debug builds only — release binaries ignore this env
    // var to prevent path-injection attacks (BC-6.2.017). Seam must be first in body,
    // before any OS-branch logic, so it fires on all platforms (S-WIN-2 prerequisite).
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var("JR_CACHE_DIR").ok().filter(|s| !s.is_empty()) {
        return PathBuf::from(dir);
    }

    #[cfg(windows)]
    {
        // Windows: %LOCALAPPDATA%\jr  (e.g., C:\Users\Alice\AppData\Local\jr)
        // BC-6.2.016: dirs::cache_dir() maps to %LOCALAPPDATA% (Local) on Windows.
        // LOCALAPPDATA fallback filters empty string: unset and empty both route to ".".
        dirs::cache_dir()
            .unwrap_or_else(|| cache_localappdata_fallback(std::env::var("LOCALAPPDATA").ok()))
            .join("jr")
    }

    #[cfg(not(windows))]
    {
        // Unix: $XDG_CACHE_HOME/jr or ~/.cache/jr (unchanged)
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(xdg).join("jr")
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".cache")
                .join("jr")
        }
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

/// Invalidate the cached project metadata for a specific project key.
///
/// Removes the entry for `project_key` from `project_meta.json` for the given
/// profile. Used by SEC-576-006 stale-ID self-heal: when
/// `attach_temporary_file` returns 404/403 with a cached `sdId`, the caller
/// invalidates this entry so `get_or_fetch_project_meta` does a fresh HTTP
/// fetch on the next call.
///
/// Model-b cache writer: disk errors are swallowed with a warning so a failed
/// invalidation never breaks the upload command. Returns `()` unconditionally.
pub fn invalidate_project_meta_cache(profile: &str, project_key: &str) {
    let path = cache_dir(profile).join("project_meta.json");
    if !path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to invalidate project_meta cache for {project_key}: {e}");
            return;
        }
    };
    let mut map: HashMap<String, ProjectMeta> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: failed to invalidate project_meta cache for {project_key}: {e}");
            return;
        }
    };
    if map.remove(project_key).is_none() {
        return;
    }
    let new_content = match serde_json::to_string_pretty(&map) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to invalidate project_meta cache for {project_key}: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, new_content) {
        eprintln!("warning: failed to invalidate project_meta cache for {project_key}: {e}");
    }
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

/// Best-effort writer: swallows disk-write errors with `eprintln!` and returns
/// `Ok(())`. A missed write costs at most one extra HTTP call on the next
/// invocation. Cache write failures MUST NOT break a successful API call.
///
/// Chosen model: (b) swallow + warn — this cache is a read-acceleration
/// shortcut, not a correctness-critical store. The call site in
/// `src/api/assets/linked.rs` does NOT use `let _ =`; errors are absorbed
/// inside this writer. Do not re-introduce `let _ =` or `?` at the call site.
pub fn write_cmdb_fields_cache(profile: &str, fields: &[(String, String)]) -> Result<()> {
    let result = write_cache(
        profile,
        "cmdb_fields.json",
        &CmdbFieldsCache {
            fields: fields.to_vec(),
            fetched_at: Utc::now(),
        },
    );
    if let Err(e) = result {
        eprintln!("warning: failed to write cmdb_fields cache: {e}");
    }
    Ok(())
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
///
/// Best-effort writer: swallows disk-write errors with `eprintln!` and returns
/// `Ok(())`. A missed write costs at most one extra HTTP call on the next
/// invocation. Cache write failures MUST NOT break a successful API call.
///
/// Chosen model: (b) swallow + warn — this cache is a read-acceleration
/// shortcut, not a correctness-critical store. The call site in
/// `src/api/assets/objects.rs` does NOT use `let _ =`; errors are absorbed
/// inside this writer. Do not re-introduce `let _ =` or `?` at the call site.
pub fn write_object_type_attr_cache(
    profile: &str,
    object_type_id: &str,
    attrs: &[CachedObjectTypeAttr],
) -> Result<()> {
    let result = (|| -> Result<()> {
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
    })();
    if let Err(e) = result {
        eprintln!("warning: failed to write object_type_attrs cache: {e}");
    }
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

/// Slim representation of a project component stored in the cache.
///
/// Holds only `id` and `name` — just enough for name-based resolution
/// (`resolve_component`) without round-tripping the full resource on every
/// resolver invocation. ADR-0018 Decision §2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedComponent {
    pub id: String,
    pub name: String,
}

/// One project's component list plus the timestamp for TTL checks.
///
/// Stored as a map entry: `components_<profile>.json` →
/// `HashMap<project_key, ComponentsCacheEntry>`. Mirrors the `ProjectMeta`
/// keyed-cache pattern — TTL is checked per-entry. ADR-0018 Decision §2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentsCacheEntry {
    pub components: Vec<CachedComponent>,
    pub fetched_at: DateTime<Utc>,
}

/// Read the cached component list for a single project.
///
/// Returns `Ok(None)` on missing file, missing key, expired entry, or corrupt
/// JSON (self-heal via cache-miss → re-fetch). ADR-0018 Decision §2.
/// Profile is FIRST arg per ADR-0007 multi-profile invariant.
///
/// **FOUNDATION, not yet wired (F5-A-L1/C-003):** as of this cycle, no
/// production code path calls this function — `helpers::resolve_component`
/// and every `jr component`/`jr issue list --component` resolver always
/// performs a fresh `list_components` GET, never consulting this cache. It
/// exists ahead of its consumer per ADR-0018 §2, laid down for `jr component
/// rename` (S-608-1), which is expected to read through it. Do not treat its
/// presence as evidence that component resolution is currently cached.
pub fn read_components_cache(
    profile: &str,
    project_key: &str,
) -> Result<Option<ComponentsCacheEntry>> {
    let path = cache_dir(profile).join(format!("components_{profile}.json"));
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let map: HashMap<String, ComponentsCacheEntry> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: components_{profile}.json unreadable ({e}); will refetch");
            return Ok(None);
        }
    };

    match map.get(project_key) {
        Some(entry) => {
            let age = Utc::now() - entry.fetched_at;
            if age.num_days() >= CACHE_TTL_DAYS {
                Ok(None)
            } else {
                Ok(Some(entry.clone()))
            }
        }
        None => Ok(None),
    }
}

/// Write the component list for a project into the components cache.
///
/// Merges into the existing `components_<profile>.json` map, preserving
/// entries for other projects (same merge strategy as `write_project_meta`).
///
/// **Model-b writer (ADR-0018 Decision §2):** a failed disk write is swallowed
/// with `eprintln!("warning: …")` and `Ok(())` is returned unconditionally — a
/// failed cache write must never break a successful `component list`. Callers
/// MUST use `.ok()` to discard the infallible return value.
/// Profile is FIRST arg per ADR-0007 multi-profile invariant.
///
/// **FOUNDATION, not yet wired (F5-A-L1/C-003):** no production code path
/// calls this function today — `jr component list` and every `--component`
/// resolver fetch fresh from the API on every invocation rather than
/// populating this cache. It is laid down ahead of its consumer per ADR-0018
/// §2, intended for `jr component rename` (S-608-1). Only
/// `invalidate_components_cache` has real production call sites right now
/// (`cli/component.rs` create/edit/delete), and since nothing writes this
/// cache in production those calls currently have no cached entry to remove
/// — see that function's doc comment.
pub fn write_components_cache(
    profile: &str,
    project_key: &str,
    components: &[CachedComponent],
) -> Result<()> {
    let dir = cache_dir(profile);

    let result = (|| -> Result<()> {
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("components_{profile}.json"));

        let mut map: HashMap<String, ComponentsCacheEntry> = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "warning: components_{profile}.json unreadable ({e}); starting fresh — other cached projects will be lost"
                );
                HashMap::new()
            })
        } else {
            HashMap::new()
        };

        map.insert(
            project_key.to_string(),
            ComponentsCacheEntry {
                components: components.to_vec(),
                fetched_at: Utc::now(),
            },
        );

        let content = serde_json::to_string_pretty(&map)?;
        std::fs::write(&path, content)?;
        Ok(())
    })();

    // Model-b writer: swallow disk errors, never propagate (ADR-0018 Decision §2).
    if let Err(e) = result {
        eprintln!("warning: failed to write components cache: {e}");
    }
    Ok(())
}

/// Invalidate the cached component list for a specific project.
///
/// Removes the `project_key` entry from `components_<profile>.json`.
/// Called by S-604-2 / S-604-3 / S-608-1 mutating commands before or after
/// they change components on the project, so the next `list` fetches fresh data.
///
/// **Model-b invalidator:** disk errors are swallowed with `eprintln!` so a
/// failed invalidation never breaks the mutating command. Returns `()`.
/// Profile is FIRST arg per ADR-0007 multi-profile invariant.
///
/// **Currently a no-op in practice (F5-A-L1/C-003):** this function DOES have
/// real production call sites (`cli/component.rs`'s create/edit/delete
/// handlers, S-604-2/S-604-3) and does execute on every mutating command —
/// but because `write_components_cache` has no production caller, there is
/// never an on-disk entry for it to find and remove; `map.remove(project_key)`
/// returns `None` and the function short-circuits before rewriting the file.
/// It is FOUNDATION for `jr component rename` (S-608-1) per ADR-0018 §2: once
/// a future read path starts populating this cache via `write_components_cache`,
/// these already-wired invalidation call sites make it correct immediately,
/// with no further caller-side changes needed. Do not read its call sites as
/// evidence that component list results are cached today.
pub fn invalidate_components_cache(profile: &str, project_key: &str) {
    let path = cache_dir(profile).join(format!("components_{profile}.json"));
    if !path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to invalidate components cache for {project_key}: {e}");
            return;
        }
    };
    let mut map: HashMap<String, ComponentsCacheEntry> = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: failed to invalidate components cache for {project_key}: {e}");
            return;
        }
    };
    if map.remove(project_key).is_none() {
        return;
    }
    let new_content = match serde_json::to_string_pretty(&map) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: failed to invalidate components cache for {project_key}: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, new_content) {
        eprintln!("warning: failed to invalidate components cache for {project_key}: {e}");
    }
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
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME /
        // JR_CACHE_DIR; the variables are only read inside cache functions called
        // within this lock, so no concurrent env access occurs.
        //
        // JR_CACHE_DIR is the cross-platform debug seam (BC-6.2.017): on Windows,
        // cache_root() uses %LOCALAPPDATA% and ignores XDG_CACHE_HOME, so we must
        // also set JR_CACHE_DIR to dir/jr (matching what the XDG branch returns on
        // Unix) to keep all platforms writing to the same tempdir.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", dir.path());
            std::env::set_var("JR_CACHE_DIR", dir.path().join("jr"));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    /// Set `var` to `value`, run `f`, then unconditionally remove `var` — even if
    /// `f` panics. Mirrors `with_temp_cache` so BC-6.2.017 seam tests cannot leak
    /// `JR_CACHE_DIR` / `JR_CONFIG_DIR` into subsequent tests on panic.
    #[cfg(debug_assertions)]
    pub(super) fn with_env_var<F: FnOnce() -> R, R>(var: &str, value: &str, f: F) -> R {
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX held; no concurrent env reads occur while we hold the lock.
        unsafe { std::env::set_var(var, value) };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe { std::env::remove_var(var) };
        drop(guard);
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
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
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME /
        // JR_CACHE_DIR.
        //
        // JR_CACHE_DIR is the cross-platform seam (BC-6.2.017): on Windows,
        // cache_root() uses %LOCALAPPDATA% and ignores XDG_CACHE_HOME.  Set
        // JR_CACHE_DIR = fake_cache_home (the file path, without ".join(jr)")
        // so cache_root() returns the file path on both platforms, causing the
        // same ENOTDIR / "not a directory" I/O failure that the test validates.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_fields_cache(
                "test-m2-swallow",
                &[("customfield_10001".to_string(), "Severity".to_string())],
            )
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_fields_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_fields_cache must return Ok(()) on I/O error (best-effort writer); got: {result:?}"
        );
    }

    // AC-003 / CR-007: `write_cmdb_fields_cache` swallow behavior — mirrors
    // `test_write_fields_cache_swallow_io_error_returns_ok` above.
    // Forces an I/O error by pointing JR_CACHE_DIR at a file (not a dir), so
    // create_dir_all inside write_cache fails with ENOTDIR.  The model-b writer
    // must return Ok(()) and NOT panic.
    #[test]
    fn test_write_cmdb_fields_cache_swallow_io_error_returns_ok() {
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_cmdb_fields_cache(
                "test-cmdb-swallow",
                &[("customfield_10191".to_string(), "Client".to_string())],
            )
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_cmdb_fields_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_cmdb_fields_cache must return Ok(()) on I/O error (best-effort writer); got: {result:?}"
        );
    }

    // AC-003 / CR-007: `write_object_type_attr_cache` swallow behavior — same
    // ENOTDIR pattern as above.
    #[test]
    fn test_write_object_type_attr_cache_swallow_io_error_returns_ok() {
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_object_type_attr_cache("test-objattr-swallow", "99", &[])
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_object_type_attr_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_object_type_attr_cache must return Ok(()) on I/O error (best-effort writer); got: {result:?}"
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

    // -----------------------------------------------------------------------
    // BC-6.2.017 — JR_CACHE_DIR debug-only path-isolation seam
    //
    // These tests pin AC-002, AC-004, and AC-008 from S-WIN-2.
    //
    // Pre-implementation Red Gate: the seam does not exist in cache_root() yet.
    // AC-002 will FAIL because cache_root() does not read JR_CACHE_DIR at all.
    // AC-004 is a negative (independence) test that asserts JR_CONFIG_DIR does not
    // bleed into cache_root().
    // AC-008 (empty-string treated as unset) mirrors AC-003 in config tests.
    // -----------------------------------------------------------------------

    /// BC-6.2.017 postcondition (debug path) — AC-002.
    ///
    /// In a debug build, `cache_root()` must return `PathBuf::from(value)` when
    /// `JR_CACHE_DIR` is set to a non-empty string. The XDG/home-dir logic must
    /// be bypassed entirely.
    ///
    /// Pre-implementation Red Gate: ASSERTION FAILURE — `cache_root()` does not
    /// read `JR_CACHE_DIR` so it returns the XDG or home-dir path instead of the
    /// seam value.
    #[cfg(debug_assertions)]
    #[test]
    fn test_bc_6_2_017_cache_dir_seam_overrides_path() {
        let seam_path = "/tmp/jr-seam-test-cache-dir-overrides-path";
        // Clear XDG_CACHE_HOME inside the closure so the non-seam branch cannot
        // accidentally produce the seam path via a coincidental XDG value.
        // ENV_MUTEX is acquired inside with_env_var for the full duration of the closure.
        let result = with_env_var("JR_CACHE_DIR", seam_path, || {
            // SAFETY: ENV_MUTEX held by with_env_var.
            unsafe { std::env::remove_var("XDG_CACHE_HOME") };
            cache_root()
        });
        assert_eq!(
            result,
            std::path::PathBuf::from(seam_path),
            "AC-002 (BC-6.2.017): cache_root() must return the JR_CACHE_DIR value \
             as-is (no suffix appended) when the seam is set in a debug build. \
             Got: {}",
            result.display()
        );
    }

    /// BC-6.2.017 EC-3 — AC-004.
    ///
    /// When only `JR_CONFIG_DIR` is set (and `JR_CACHE_DIR` is NOT set),
    /// `cache_root()` must return the OS-determined path, not any value derived
    /// from `JR_CONFIG_DIR`. The two seams are independent.
    ///
    /// Pre-implementation Red Gate: This test PASSES even without the seam
    /// (cache_root() never reads JR_CONFIG_DIR regardless). Included as a
    /// regression guard once the seam exists.
    #[cfg(debug_assertions)]
    #[test]
    fn test_bc_6_2_017_config_seam_does_not_affect_cache() {
        let config_seam_path = "/tmp/jr-seam-test-config-only-no-cache-effect";
        // with_env_var sets JR_CONFIG_DIR and wraps the closure in catch_unwind so
        // the var is always removed even on panic. JR_CACHE_DIR and XDG_CACHE_HOME are
        // cleared inside the closure while ENV_MUTEX is still held.
        let result = with_env_var("JR_CONFIG_DIR", config_seam_path, || {
            // SAFETY: ENV_MUTEX held by with_env_var.
            unsafe {
                std::env::remove_var("JR_CACHE_DIR");
                // Clear XDG_CACHE_HOME for a deterministic OS-path result.
                std::env::remove_var("XDG_CACHE_HOME");
            }
            cache_root()
        });
        assert_ne!(
            result,
            std::path::PathBuf::from(config_seam_path),
            "AC-004 (BC-6.2.017 EC-3): cache_root() must NOT be influenced by \
             JR_CONFIG_DIR. When only JR_CONFIG_DIR is set, cache_root() must \
             return the OS-determined path, not the config seam value. \
             Got: {}",
            result.display()
        );
        // The returned path must be non-empty (OS logic fired).
        assert!(
            !result.as_os_str().is_empty(),
            "AC-004: cache_root() with only JR_CONFIG_DIR set must return a \
             non-empty OS path. Got: {}",
            result.display()
        );
    }

    /// BC-6.2.017 EC-5 — AC-008.
    ///
    /// When `JR_CACHE_DIR` is set to an empty string in a debug build, the seam
    /// is treated as unset (`.filter(|s| !s.is_empty())` fires). `cache_root()`
    /// must NOT return `PathBuf::from("")`. It must proceed to OS-branch logic,
    /// returning a non-empty path. Symmetric to AC-003 for the config side.
    ///
    /// This test is load-bearing: it pins the `.filter(|s| !s.is_empty())` guard in
    /// `cache_root()` (AC-008, BC-6.2.017 EC-5). Without that filter, setting
    /// `JR_CACHE_DIR=""` would cause the seam to return `PathBuf::from("")` whose
    /// `as_os_str().is_empty()` is true — the `assert_ne!` below would then fail.
    /// Dropping the filter is exactly the mutation this test kills.
    #[cfg(debug_assertions)]
    #[test]
    fn test_bc_6_2_017_empty_cache_dir_uses_os_path() {
        let result = with_env_var("JR_CACHE_DIR", "", cache_root);
        assert_ne!(
            result,
            std::path::PathBuf::from(""),
            "AC-008 (BC-6.2.017 EC-5): cache_root() must NOT return PathBuf::from(\"\") \
             when JR_CACHE_DIR is set to the empty string. The empty-string filter must \
             treat it as unset and proceed to OS logic. Got: {}",
            result.display()
        );
        assert!(
            !result.as_os_str().is_empty(),
            "AC-008 (BC-6.2.017 EC-5): OS-branch result must be a non-empty path. \
             Got: {}",
            result.display()
        );
    }

    // -------------------------------------------------------------------------
    // BC-6.2.016 / BC-6.2.004 — Windows LocalAppData cache path tests (S-WIN-1)
    // All tests below are `#[cfg(windows)]`-gated. They compile out on macOS/Linux
    // (zero impact on Unix CI) and run only on a Windows runner (S-WIN-5).
    //
    // RED GATE RATIONALE: `cache_root()` currently has NO `#[cfg(windows)]` branch —
    // on a Windows build it falls through to the XDG/home_dir Unix path, so
    // `dirs::cache_dir()` (= %LOCALAPPDATA%) is never consulted. Every assertion
    // below would therefore FAIL on a Windows runner against the current code.
    // The implementation in S-WIN-1 adds the `#[cfg(windows)]` branch that makes
    // these tests pass.
    // -------------------------------------------------------------------------

    /// AC-005 / BC-6.2.016 postcondition — on Windows, `cache_root()` returns
    /// `dirs::cache_dir().join("jr")` which resolves to `%LOCALAPPDATA%\jr` (Local).
    ///
    /// Verifies the structural postcondition using PathBuf component comparison (not
    /// string literals with `/`) per F-WIN-F3-005: on Windows `PathBuf::join` produces
    /// `\`-separated paths.
    ///
    /// Also verifies that the path ends with component "jr" (the platform-appended
    /// subdirectory), and that its parent equals `dirs::cache_dir()`.
    ///
    /// Traces: BC-6.2.016 postcondition, AC-005.
    #[cfg(windows)]
    #[test]
    fn test_bc_6_2_016_windows_cache_root_uses_localappdata() {
        // On Windows, dirs::cache_dir() returns Some(%LOCALAPPDATA% Local path).
        // The function under test must return dirs::cache_dir().unwrap().join("jr").
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Scrub the debug seam vars so they cannot short-circuit cache_root()
        // and perturb the assertion (RB-102).
        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        unsafe {
            std::env::remove_var("JR_CACHE_DIR");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let expected_parent = dirs::cache_dir()
            .expect("dirs::cache_dir() must return Some on a Windows system with a user profile");
        let expected = expected_parent.join("jr");
        let result = cache_root();
        assert_eq!(
            result,
            expected,
            "AC-005 (BC-6.2.016): on Windows, cache_root() must return \
             dirs::cache_dir().join(\"jr\") = %LOCALAPPDATA%\\jr. \
             Expected: {}, got: {}",
            expected.display(),
            result.display()
        );
        // Structural assertion: must end with component "jr"
        assert!(
            result.ends_with("jr"),
            "AC-005 (BC-6.2.016): path must end with 'jr' component, got: {}",
            result.display()
        );
        // Invariant: must NOT be rooted under %APPDATA% (Roaming) — must be Local
        let roaming_marker = "Roaming";
        assert!(
            !result.to_string_lossy().contains(roaming_marker),
            "AC-005 (BC-6.2.016 invariant): cache path must use %LOCALAPPDATA% (Local), \
             NOT %APPDATA% (Roaming). Got: {}",
            result.display()
        );
    }

    /// AC-006 / BC-6.2.016 EC-1 — `cache_localappdata_fallback` pure helper.
    ///
    /// Exercises the extracted `cache_localappdata_fallback` helper directly so that
    /// mutations to the production fallback (e.g. dropping `.filter(|s| !s.is_empty())`
    /// or changing `PathBuf::from(".")`) are caught on every platform, not only on a
    /// Windows CI runner.
    ///
    /// The helper is un-gated (no `#[cfg(windows)]`) so this test compiles and runs
    /// on macOS/Linux in CI, genuinely killing the empty-filter/default mutants.
    ///
    /// Traces: BC-6.2.016 EC-1, EC-4, AC-006.
    #[test]
    fn test_bc_6_2_016_localappdata_env_fallback() {
        // EC-4: empty string → treated as unset → PathBuf::from(".")
        assert_eq!(
            cache_localappdata_fallback(Some(String::new())),
            PathBuf::from("."),
            "EC-4: empty LOCALAPPDATA must yield PathBuf::from(\".\")"
        );

        // EC-1: None (unset LOCALAPPDATA) → PathBuf::from(".")
        assert_eq!(
            cache_localappdata_fallback(None),
            PathBuf::from("."),
            "EC-1: None (unset LOCALAPPDATA) must yield PathBuf::from(\".\")"
        );

        // Happy-path: non-empty value is passed through unchanged
        assert_eq!(
            cache_localappdata_fallback(Some("C:\\Users\\Alice\\AppData\\Local".into())),
            PathBuf::from("C:\\Users\\Alice\\AppData\\Local"),
            "non-empty LOCALAPPDATA must be returned as-is"
        );
    }

    /// AC-007 / BC-6.2.004, BC-6.2.016 postcondition — on Windows, the per-profile
    /// cache path includes the `v1/` versioning root.
    ///
    /// `cache_dir(profile)` = `cache_root().join("v1").join(profile)`.
    /// On Windows this must equal `%LOCALAPPDATA%\jr\v1\<profile>`.
    ///
    /// The `v1/` versioning root must be present on Windows the same as on Unix.
    /// Uses PathBuf component comparison per F-WIN-F3-005.
    ///
    /// Traces: BC-6.2.004 Windows clause, BC-6.2.016 postcondition, AC-007.
    #[cfg(windows)]
    #[test]
    fn test_bc_6_2_004_windows_per_profile_path_includes_v1() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Scrub the debug seam vars so they cannot short-circuit cache_root()
        // and perturb the assertion (RB-102).
        // SAFETY: ENV_MUTEX is held for the duration; no concurrent env access occurs.
        unsafe {
            std::env::remove_var("JR_CACHE_DIR");
            std::env::remove_var("JR_CONFIG_DIR");
        }
        let root = cache_root();
        let profile_dir = cache_dir("default");

        // Must be: cache_root().join("v1").join("default")
        let expected = root.join("v1").join("default");
        assert_eq!(
            profile_dir,
            expected,
            "AC-007 (BC-6.2.004): cache_dir(\"default\") must equal \
             cache_root().join(\"v1\").join(\"default\") on Windows. \
             Expected: {}, got: {}",
            expected.display(),
            profile_dir.display()
        );

        // The path must contain the "v1" component (versioning root preserved on Windows)
        let has_v1 = profile_dir.components().any(|c| c.as_os_str() == "v1");
        assert!(
            has_v1,
            "AC-007 (BC-6.2.004): Windows per-profile cache path must contain \
             the 'v1' versioning component. Got: {}",
            profile_dir.display()
        );

        // The path must end with the profile name component
        assert!(
            profile_dir.ends_with("default"),
            "AC-007 (BC-6.2.004): path must end with profile component 'default'. \
             Got: {}",
            profile_dir.display()
        );

        // Verify parent of profile dir is the v1 dir
        let parent = profile_dir.parent().unwrap();
        assert!(
            parent.ends_with("v1"),
            "AC-007 (BC-6.2.004): parent of profile dir must be the 'v1' component. \
             Parent: {}, full path: {}",
            parent.display(),
            profile_dir.display()
        );
    }

    // ── P3: request-type writer swallow-on-IO-error (BC-X.12.008 D5) ─────────
    //
    // `write_request_type_cache` and `write_request_type_fields_cache` are model-b
    // best-effort writers: disk-write failures are logged to stderr and the function
    // returns `Ok(())`. This ensures a cache-write error never breaks a successful
    // API call or pollutes a pipeline (`jr requesttype list --output json | jq`).
    //
    // Exact warning text emitted on IO error:
    //   write_request_type_cache:        "warning: failed to write request type cache: {e}"
    //   write_request_type_fields_cache: "warning: failed to write request type fields cache: {e}"
    //
    // Pattern mirrors `test_write_cmdb_fields_cache_swallow_io_error_returns_ok`
    // and `test_write_object_type_attr_cache_swallow_io_error_returns_ok` above.
    // BC anchor: BC-X.12.008 (request-type caching BCs — 7-day TTL + best-effort write model).

    /// BC-X.12.008 (D5) — `write_request_type_cache` swallows disk-write errors.
    ///
    /// Forces an I/O error by pointing JR_CACHE_DIR at a *file* (not a directory),
    /// so `create_dir_all` inside `write_cache` fails with ENOTDIR. The model-b
    /// writer must return `Ok(())` and NOT panic or propagate the error.
    ///
    /// Non-tautology: would fail if the writer were changed to propagate via ?
    #[test]
    fn test_write_request_type_cache_swallows_disk_error() {
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        // Acquire ENV_MUTEX to serialise env access with all other cache tests.
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME / JR_CACHE_DIR.
        // JR_CACHE_DIR is the cross-platform seam (BC-6.2.017): set to the file path
        // so cache_root() returns a file path on both platforms, causing ENOTDIR in
        // create_dir_all inside write_cache.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let types = vec![crate::types::jsm::RequestType {
            id: "11001".to_string(),
            name: "Get IT Help".to_string(),
            description: None,
            help_text: None,
            issue_type_id: None,
            group_ids: vec![],
        }];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_request_type_cache("p3-swallow-test", "10", &types)
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_request_type_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_request_type_cache must return Ok(()) on I/O error \
             (model-b best-effort writer, BC-X.12.008); got: {result:?}"
        );
    }

    /// BC-X.12.008 (D5) — `write_request_type_fields_cache` swallows disk-write errors.
    ///
    /// Forces an I/O error by pointing JR_CACHE_DIR at a *file* (not a directory),
    /// so `create_dir_all` inside `write_cache` fails with ENOTDIR. The model-b
    /// writer must return `Ok(())` and NOT panic or propagate the error.
    ///
    /// Non-tautology: would fail if the writer were changed to propagate via ?
    #[test]
    fn test_write_request_type_fields_cache_swallows_disk_error() {
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        // Acquire ENV_MUTEX to serialise env access with all other cache tests.
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME / JR_CACHE_DIR.
        // Same ENOTDIR-forcing pattern as test_write_request_type_cache_swallows_disk_error.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let response = crate::types::jsm::RequestTypeFieldsResponse {
            can_raise_on_behalf_of: true,
            can_add_request_participants: false,
            request_type_fields: vec![crate::types::jsm::RequestTypeField {
                field_id: "summary".to_string(),
                name: "What do you need?".to_string(),
                description: None,
                required: true,
                visible: true,
                default_values: None,
                valid_values: None,
                jira_schema: serde_json::json!({"type": "string", "system": "summary"}),
                auto_complete_url: None,
            }],
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_request_type_fields_cache("p3-swallow-test", "10", "200", &response)
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_request_type_fields_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "write_request_type_fields_cache must return Ok(()) on I/O error \
             (model-b best-effort writer, BC-X.12.008); got: {result:?}"
        );
    }

    // ── S-604-1: AC-019 / ADR-0018 Decision §2 — components cache round-trip ─

    /// AC-019 / ADR-0018 Decision §2: write then read returns the same component set
    /// within the 7-day TTL; invalidate removes it; a failed write (model-b) returns
    /// Ok(()) and does NOT propagate an Err.
    ///
    /// `read_components_cache` / `write_components_cache` / `invalidate_components_cache`
    /// are implemented (ADR-0018 §2, see src/cache.rs). Part 1 asserts a successful
    /// write→read→invalidate round-trip within TTL; Part 2 asserts the model-b writer
    /// swallows an I/O error and returns `Ok(())`.
    #[test]
    fn test_adr_0018_components_cache_round_trip_and_model_b_writer() {
        // Part 1: round-trip write → read → invalidate
        with_temp_cache(|| {
            let components = vec![
                CachedComponent {
                    id: "10001".to_string(),
                    name: "Backend".to_string(),
                },
                CachedComponent {
                    id: "10002".to_string(),
                    name: "Frontend".to_string(),
                },
            ];

            write_components_cache("default", "FOO", &components)
                .expect("write_components_cache must succeed in a writable temp dir");

            let entry = read_components_cache("default", "FOO")
                .expect("read_components_cache must not error");
            assert!(
                entry.is_some(),
                "read_components_cache must return Some after a write (within TTL)"
            );
            let entry = entry.unwrap();
            assert_eq!(
                entry.components.len(),
                2,
                "Round-trip must preserve both components"
            );
            assert_eq!(entry.components[0].id, "10001");
            assert_eq!(entry.components[0].name, "Backend");
            assert_eq!(entry.components[1].id, "10002");
            assert_eq!(entry.components[1].name, "Frontend");

            // Invalidate must clear the cache
            invalidate_components_cache("default", "FOO");
            let after = read_components_cache("default", "FOO")
                .expect("read_components_cache must not error after invalidation");
            assert!(
                after.is_none(),
                "read_components_cache must return None after invalidation"
            );
        });

        // Part 2: model-b writer — I/O error must NOT propagate (swallow + warn)
        // Force an I/O error by pointing JR_CACHE_DIR at a file (not a directory),
        // causing create_dir_all to fail with ENOTDIR — same pattern used by
        // test_write_cmdb_fields_cache_swallow_io_error_returns_ok above.
        let outer_dir = tempfile::TempDir::new().unwrap();
        let fake_cache_home = outer_dir.path().join("i_am_a_file");
        std::fs::write(&fake_cache_home, "file, not a dir").unwrap();

        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_MUTEX serialises all tests that touch XDG_CACHE_HOME / JR_CACHE_DIR.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &fake_cache_home);
            std::env::set_var("JR_CACHE_DIR", &fake_cache_home);
        }
        let components = vec![CachedComponent {
            id: "10001".to_string(),
            name: "Backend".to_string(),
        }];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            write_components_cache("default", "FOO", &components)
        }));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("JR_CACHE_DIR");
        }
        drop(guard);

        let result = result.expect("write_components_cache must not panic on I/O error");
        assert!(
            result.is_ok(),
            "Model-b writer must return Ok(()) on I/O error (ADR-0018 Decision §2 swallow-+warn); \
             got: {result:?}"
        );
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

/// P1 — Cross-profile isolation unit tests for the six families that were
/// previously untested for this dimension (BC-6.2.009).
///
/// POLICY multi-profile-cache (CRITICAL): cross-profile cache leakage is a
/// stated correctness bug — sandbox vs prod custom-field IDs differ.  Families
/// 1 (team), 8 (request_types) and 9 (request_type_fields) already have
/// explicit cross-profile tests.  This module adds equivalent coverage for the
/// remaining six: workspace, resolutions, cmdb_fields, fields, object_type_attrs,
/// and project_meta.
///
/// Pattern for each test (mirrors `cross_profile_isolation_team_cache`):
///   1. Write a value under profile "prod".
///   2. Write a *different* value under profile "sandbox".
///   3. Assert `read_*("prod")` returns the prod value.
///   4. Assert `read_*("sandbox")` returns the sandbox value.
///   5. Assert the two on-disk paths are distinct.
///
/// BC anchor: BC-6.2.009 (cross-profile isolation — the team-cache example BC).
/// Note: the audit doc cited BC-6.3.001 as "the closest existing BC", but
/// BC-6.3.001 covers per-profile *config* field IDs surviving Config::save_global(),
/// not on-disk cache-file isolation.  BC-6.2.009 is the correct anchor.
#[cfg(test)]
mod cache_profile_isolation_tests {
    use super::tests::with_temp_cache;
    use super::*;

    /// BC-6.2.009 — workspace cache is isolated per profile.
    ///
    /// Regression guard: if `write_workspace_cache` or `cache_dir` ever dropped
    /// the profile path segment, both profiles would share a single file and
    /// this test would catch the leak (prod would read sandbox's workspace ID).
    #[test]
    fn test_workspace_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            write_workspace_cache("prod", "workspace-prod-abc").unwrap();
            write_workspace_cache("sandbox", "workspace-sandbox-xyz").unwrap();

            let prod = read_workspace_cache("prod")
                .unwrap()
                .expect("prod workspace cache must exist");
            assert_eq!(
                prod.workspace_id, "workspace-prod-abc",
                "prod profile must return 'workspace-prod-abc', not sandbox data"
            );

            let sandbox = read_workspace_cache("sandbox")
                .unwrap()
                .expect("sandbox workspace cache must exist");
            assert_eq!(
                sandbox.workspace_id, "workspace-sandbox-xyz",
                "sandbox profile must return 'workspace-sandbox-xyz', not prod data"
            );

            // Verify on-disk paths are distinct — path leak would make these identical.
            let prod_path = cache_dir("prod").join("workspace.json");
            let sandbox_path = cache_dir("sandbox").join("workspace.json");
            assert!(
                prod_path.exists(),
                "prod workspace.json must exist at {prod_path:?}"
            );
            assert!(
                sandbox_path.exists(),
                "sandbox workspace.json must exist at {sandbox_path:?}"
            );
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox workspace cache paths must be distinct"
            );
        });
    }

    /// BC-6.2.009 — resolutions cache is isolated per profile.
    ///
    /// Regression guard: if the profile scoping were removed, a prod resolution
    /// list (e.g. containing "Fixed") would be returned for the sandbox profile
    /// (which might have "Resolved" instead), silently sending the wrong
    /// resolution name to the Jira API on `jr issue move`.
    #[test]
    fn test_resolutions_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_res = vec![CachedResolution {
                id: "10000".into(),
                name: "Fixed".into(),
                description: None,
            }];
            let sandbox_res = vec![CachedResolution {
                id: "20000".into(),
                name: "Resolved".into(),
                description: None,
            }];

            write_resolutions_cache("prod", &prod_res).unwrap();
            write_resolutions_cache("sandbox", &sandbox_res).unwrap();

            let prod = read_resolutions_cache("prod")
                .unwrap()
                .expect("prod resolutions cache must exist");
            assert_eq!(
                prod.resolutions[0].name, "Fixed",
                "prod profile must return 'Fixed', not sandbox data"
            );

            let sandbox = read_resolutions_cache("sandbox")
                .unwrap()
                .expect("sandbox resolutions cache must exist");
            assert_eq!(
                sandbox.resolutions[0].name, "Resolved",
                "sandbox profile must return 'Resolved', not prod data"
            );

            let prod_path = cache_dir("prod").join("resolutions.json");
            let sandbox_path = cache_dir("sandbox").join("resolutions.json");
            assert!(prod_path.exists());
            assert!(sandbox_path.exists());
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox resolutions cache paths must be distinct"
            );
        });
    }

    /// BC-6.2.009 — cmdb_fields cache is isolated per profile.
    ///
    /// Regression guard: highest sub-risk family — cmdb_fields stores
    /// custom-field IDs (e.g. customfield_10191) that differ between sandbox
    /// and prod Jira instances.  A leak would silently write the wrong field ID
    /// into an asset-enriched issue payload.
    #[test]
    fn test_cmdb_fields_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_fields = vec![("customfield_10191".to_string(), "Client".to_string())];
            let sandbox_fields = vec![("customfield_20001".to_string(), "Client".to_string())];

            // write_cmdb_fields_cache is a model-b (always-Ok) writer: it swallows
            // disk errors internally and always returns Ok(()).  The .unwrap() here
            // is therefore trivially infallible.  To catch a silent write no-op we
            // assert the cache files exist immediately after each call.
            write_cmdb_fields_cache("prod", &prod_fields).unwrap();
            assert!(
                cache_dir("prod").join("cmdb_fields.json").exists(),
                "write_cmdb_fields_cache did not create the cache file for 'prod'"
            );
            write_cmdb_fields_cache("sandbox", &sandbox_fields).unwrap();
            assert!(
                cache_dir("sandbox").join("cmdb_fields.json").exists(),
                "write_cmdb_fields_cache did not create the cache file for 'sandbox'"
            );

            let prod = read_cmdb_fields_cache("prod")
                .unwrap()
                .expect("prod cmdb_fields cache must exist");
            assert_eq!(
                prod.fields[0].0, "customfield_10191",
                "prod profile must return customfield_10191, not sandbox's customfield_20001"
            );

            let sandbox = read_cmdb_fields_cache("sandbox")
                .unwrap()
                .expect("sandbox cmdb_fields cache must exist");
            assert_eq!(
                sandbox.fields[0].0, "customfield_20001",
                "sandbox profile must return customfield_20001, not prod's customfield_10191"
            );

            let prod_path = cache_dir("prod").join("cmdb_fields.json");
            let sandbox_path = cache_dir("sandbox").join("cmdb_fields.json");
            assert!(prod_path.exists());
            assert!(sandbox_path.exists());
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox cmdb_fields cache paths must be distinct"
            );
        });
    }

    /// BC-6.2.009 — fields cache is isolated per profile.
    ///
    /// Regression guard: `fields.json` caches Jira field IDs for `issue edit
    /// --field`.  The Story Points field ID typically differs between a sandbox
    /// and prod instance; a leak would silently target the wrong field on write.
    #[test]
    fn test_fields_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_fields = vec![("customfield_10016".to_string(), "Story Points".to_string())];
            let sandbox_fields =
                vec![("customfield_10028".to_string(), "Story Points".to_string())];

            // write_fields_cache is a model-b (always-Ok) writer: it swallows
            // disk errors internally and always returns Ok(()).  The .unwrap() here
            // is therefore trivially infallible.  To catch a silent write no-op we
            // assert the cache files exist immediately after each call.
            write_fields_cache("prod", &prod_fields).unwrap();
            assert!(
                cache_dir("prod").join("fields.json").exists(),
                "write_fields_cache did not create the cache file for 'prod'"
            );
            write_fields_cache("sandbox", &sandbox_fields).unwrap();
            assert!(
                cache_dir("sandbox").join("fields.json").exists(),
                "write_fields_cache did not create the cache file for 'sandbox'"
            );

            let prod = read_fields_cache("prod")
                .unwrap()
                .expect("prod fields cache must exist");
            assert_eq!(
                prod.fields[0].0, "customfield_10016",
                "prod profile must return customfield_10016, not sandbox's customfield_10028"
            );

            let sandbox = read_fields_cache("sandbox")
                .unwrap()
                .expect("sandbox fields cache must exist");
            assert_eq!(
                sandbox.fields[0].0, "customfield_10028",
                "sandbox profile must return customfield_10028, not prod's customfield_10016"
            );

            let prod_path = cache_dir("prod").join("fields.json");
            let sandbox_path = cache_dir("sandbox").join("fields.json");
            assert!(prod_path.exists());
            assert!(sandbox_path.exists());
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox fields cache paths must be distinct"
            );
        });
    }

    /// BC-6.2.009 — object_type_attrs cache is isolated per profile.
    ///
    /// Regression guard: object-type attribute IDs (e.g. "134") can differ
    /// across CMDB workspaces tied to different Jira instances.  A profile
    /// leak would cause AQL queries built with wrong attribute IDs.
    #[test]
    fn test_object_type_attr_cache_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_attrs = vec![CachedObjectTypeAttr {
                id: "134".into(),
                name: "Key".into(),
                system: true,
                hidden: false,
                label: false,
                position: 0,
            }];
            let sandbox_attrs = vec![CachedObjectTypeAttr {
                id: "999".into(),
                name: "Key".into(),
                system: true,
                hidden: false,
                label: false,
                position: 0,
            }];

            write_object_type_attr_cache("prod", "23", &prod_attrs).unwrap();
            write_object_type_attr_cache("sandbox", "23", &sandbox_attrs).unwrap();

            let prod = read_object_type_attr_cache("prod", "23")
                .unwrap()
                .expect("prod object_type_attrs cache must exist");
            assert_eq!(
                prod[0].id, "134",
                "prod profile must return attr id '134', not sandbox's '999'"
            );

            let sandbox = read_object_type_attr_cache("sandbox", "23")
                .unwrap()
                .expect("sandbox object_type_attrs cache must exist");
            assert_eq!(
                sandbox[0].id, "999",
                "sandbox profile must return attr id '999', not prod's '134'"
            );

            let prod_path = cache_dir("prod").join("object_type_attrs.json");
            let sandbox_path = cache_dir("sandbox").join("object_type_attrs.json");
            assert!(prod_path.exists());
            assert!(sandbox_path.exists());
            // The path-distinctness check below is trivially true (paths always
            // differ by the profile directory segment).  The primary isolation
            // guard is the per-profile value round-trips above; this assert_ne!
            // only confirms that the two profiles use separate on-disk directories.
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox object_type_attrs cache paths must be distinct"
            );
        });
    }

    /// BC-6.2.009 — project_meta cache is isolated per profile.
    ///
    /// Regression guard: the existing `project_meta_multiple_projects` test
    /// only exercises multiple project keys within ONE profile.  This test
    /// pins that two profiles with the same project key ("HELPDESK") each
    /// see their own service_desk_id and never the other profile's value.
    #[test]
    fn test_project_meta_cross_profile_isolation() {
        with_temp_cache(|| {
            let prod_meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "10042".into(),
                service_desk_id: Some("15".into()),
                fetched_at: Utc::now(),
            };
            let sandbox_meta = ProjectMeta {
                project_type: "service_desk".into(),
                simplified: false,
                project_id: "99999".into(),
                service_desk_id: Some("77".into()),
                fetched_at: Utc::now(),
            };

            write_project_meta("prod", "HELPDESK", &prod_meta).unwrap();
            write_project_meta("sandbox", "HELPDESK", &sandbox_meta).unwrap();

            let prod = read_project_meta("prod", "HELPDESK")
                .unwrap()
                .expect("prod project_meta must exist");
            assert_eq!(
                prod.service_desk_id.as_deref(),
                Some("15"),
                "prod profile must return service_desk_id '15', not sandbox's '77'"
            );
            assert_eq!(prod.project_id, "10042");

            let sandbox = read_project_meta("sandbox", "HELPDESK")
                .unwrap()
                .expect("sandbox project_meta must exist");
            assert_eq!(
                sandbox.service_desk_id.as_deref(),
                Some("77"),
                "sandbox profile must return service_desk_id '77', not prod's '15'"
            );
            assert_eq!(sandbox.project_id, "99999");

            let prod_path = cache_dir("prod").join("project_meta.json");
            let sandbox_path = cache_dir("sandbox").join("project_meta.json");
            assert!(prod_path.exists());
            assert!(sandbox_path.exists());
            assert_ne!(
                prod_path, sandbox_path,
                "prod and sandbox project_meta cache paths must be distinct"
            );
        });
    }
}

/// P2 — Format-drift self-heal for `fields.json` (BC-6.2.011).
///
/// `FieldsCache` has the exact same `Vec<(String,String)>` tuple layout as
/// `CmdbFieldsCache`.  CLAUDE.md documents the cmdb_fields `(id,name)` tuple
/// migration as a real historical format change.  `cmdb_fields` has a
/// self-heal test (in the holdout suite via `worklog_duration_holdouts.rs`);
/// the structurally-identical `fields.json` had none.
///
/// BC anchor: BC-6.2.011 (corrupt/old-shape cache files → `Ok(None)`, no
/// panic, no crash).  The sibling BC-6.2.013 is about the write/merge pattern
/// for `object_type_attrs` and does NOT enumerate fields.json; BC-6.2.011 is
/// the operative self-heal contract here.
///
/// Note: the audit doc cited BC-6.2.013 as the anchor for P2, but BC-6.2.013
/// governs write-merge semantics, not self-heal-on-read.  BC-6.2.011 is the
/// correct anchor (explicit "corrupt/old-shape → Ok(None)" postcondition).
#[cfg(test)]
mod fields_cache_format_drift_tests {
    use super::tests::with_temp_cache;
    use super::*;

    /// BC-6.2.011 — legacy ID-only array in fields.json self-heals as Ok(None).
    ///
    /// Before the `(id, name)` tuple format was introduced, an ID-only JSON
    /// array `["customfield_10001"]` would have been a plausible old shape.
    /// Reading such a file must NOT panic or return Err; it must return Ok(None)
    /// so the caller refetches from the API.
    ///
    /// Regression guard: if `read_cache` stopped swallowing serde errors and
    /// propagated them instead, callers of `read_fields_cache` in
    /// `resolve_edit_fields` would crash on `issue edit --field` for any user
    /// whose `fields.json` predates the tuple format.
    #[test]
    fn test_fields_cache_legacy_id_only_format_self_heals() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();

            // Old/legacy shape: bare string array instead of FieldsCache struct.
            // This is the "ID-only" format that would exist if an older version
            // of jr wrote the file before the (id,name) tuple layout.
            std::fs::write(
                dir.join("fields.json"),
                r#"["customfield_10001", "customfield_10016"]"#,
            )
            .unwrap();

            let result = read_fields_cache("default").unwrap();
            assert!(
                result.is_none(),
                "legacy ID-only fields.json must self-heal as Ok(None), not return Err or Some; \
                 got: {result:?}"
            );
        });
    }

    /// BC-6.2.011 — garbage / completely invalid JSON in fields.json self-heals as Ok(None).
    ///
    /// Mirrors the `corrupt_team_cache_returns_none` test pattern.  Any
    /// unreadable file (truncated write, filesystem corruption, manual edit)
    /// must not propagate an error that breaks `issue edit --field`.
    ///
    /// Regression guard: if `read_cache` changed to propagate serde errors
    /// (e.g. a refactor removes the match-arm that returns Ok(None)), this
    /// test would fail immediately, surfacing the regression before it reaches CI.
    #[test]
    fn test_corrupt_fields_cache_returns_none() {
        with_temp_cache(|| {
            let dir = cache_dir("default");
            std::fs::create_dir_all(&dir).unwrap();

            // --- Case 1: garbage bytes — not JSON at all ---
            // Verifies that a completely unparseable file returns Ok(None)
            // rather than propagating a serde error.
            std::fs::write(dir.join("fields.json"), b"not json {{{{ garbage").unwrap();
            let result = read_fields_cache("default").unwrap();
            assert!(
                result.is_none(),
                "garbage fields.json must return Ok(None), not Err or Some"
            );

            // --- Case 2: valid JSON, wrong shape ---
            // Verifies that a file that parses as JSON but does not match the
            // FieldsCache schema (missing `fields` array) also returns Ok(None).
            std::fs::write(
                dir.join("fields.json"),
                b"{\"unexpected_key\": true, \"no_fields_array\": null}",
            )
            .unwrap();
            let result = read_fields_cache("default").unwrap();
            assert!(
                result.is_none(),
                "wrong-shape fields.json must return Ok(None), not Err or Some"
            );
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
                auto_complete_url: None,
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
