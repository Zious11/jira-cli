//! `jr field options <field>` — enumerate a custom field's allowed options.
//!
//! Anchors BC-X.14.001..004 (issue #580). Structural mirror of
//! `src/cli/requesttype.rs` per ADR-0019 §1. Three mutually-exclusive
//! MODE-SELECTOR flags (`--type`, `--request-type`, `--issue`) pick the
//! enumeration mechanism (M2 createmeta / M3 JSM requesttype-fields / M1
//! editmeta respectively); `--project` is never itself a mode selector — it
//! is a companion flag whose role depends on the selected mode.

use anyhow::{Result, anyhow};

use crate::api::client::JiraClient;
use crate::api::jsm::servicedesks;
use crate::cache;
use crate::cli::{FieldCommand, OutputFormat};
use crate::config::Config;
use crate::error::JrError;
use crate::output;
use crate::partial_match::{self, MatchResult};
use crate::types::jira::AllowedValue;

/// Table-mode glyph for a missing `id` (BC-X.14.003 degenerate-entry
/// rendering). Reused convention from `changelog.rs`/`user.rs`/
/// `requesttype.rs` — not a new glyph, just a module-local copy of the same
/// literal (those modules keep their own private constants too).
const NULL_GLYPH: &str = "—";

/// Table-mode literal for a missing `label` (BC-X.14.003). Never falls back
/// to the entry's own `id`.
const UNNAMED_LABEL: &str = "(unnamed)";

/// Recursion-depth cap for the cascading-`children` walk (C-LOW, CWE-674).
///
/// `normalize_from_allowed_values`/`normalize_from_valid_values`/
/// `filter_one`/`render_rows_recursive` all recurse into `FieldOption`'s
/// (or the pre-normalized wire shape's) nested `children`. In practice
/// Jira cascading-select nesting is 1-2 levels deep and serde's own
/// deserialize recursion limit (~128) already bounds a *deserialized*
/// input, but `normalize_from_valid_values` walks `serde_json::Value`
/// AFTER deserialization (M3's `validValues` is untyped JSON), so a
/// pathologically deep `children` array from a misbehaving/malicious
/// Jira-shaped response is not otherwise bounded before it reaches this
/// module's own recursion. Mirrors the precedent set by `adf.rs`'s
/// `MAX_ADF_DEPTH` (SEC-001, BC-7.2.012) — same cap value, same "modest,
/// explicit depth guard" rationale, applied here to a read-only
/// enumeration path rather than ADF write construction. Least-surprising
/// choice per the never-drop invariant (EC-X.14.001-7, which governs
/// per-entry `id`/`label` presence, not tree depth): TRUNCATE — stop
/// recursing at the cap and treat any child beyond it as a leaf with no
/// further `children`, rather than erroring out or dropping the
/// already-collected top-level/ancestor entries.
const MAX_FIELD_OPTION_DEPTH: usize = 256;

/// The enumeration mechanism selected by `resolve_field_context`'s pure
/// arity check (ADR-0019 §1 / §Amendment D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// M2 — project + issue-type createmeta (`--type`).
    Createmeta,
    /// M3 — JSM request-type fields (`--request-type`).
    RequestType,
    /// M1 — issue editmeta (`--issue`).
    Editmeta,
}

/// Mode-selector arity failure — zero or two-or-more of
/// `{--type, --request-type, --issue}` were supplied. Carries no message of
/// its own (pure core, per ADR-0019 §Amendment D1); the caller maps this to
/// the canonical `JrError::UserError` exit-64 text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArityError {
    /// None of the three mode-selector flags were present.
    Zero,
    /// Two or more of the three mode-selector flags were present.
    Multiple,
}

/// Normalized option-enumeration model shared by all three source mechanisms
/// (M1/M2's `allowedValues[]`, M3's `validValues[]`).
///
/// `id`/`label` are `Option<String>` (ADR-0019 §Amendment F-B) — a faithful
/// pass-through of the already-optional wire shape one layer below. A
/// source entry missing either or both fields is NEVER dropped
/// (EC-X.14.001-7) — it degrades to `None` on its own field(s) only.
/// `children` is always present, never `Option` (EC-X.14.001-4) — empty for
/// a non-cascading option.
///
/// CLI-local by design (ADR-0019 "Why does the normalization component
/// belong in `cli/field.rs`, not `types::jira::`?") — this is a
/// jr-synthesized display shape reconciling three different wire shapes,
/// not a typed mirror of any single Jira API response.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(crate) struct FieldOption {
    pub id: Option<String>,
    pub label: Option<String>,
    pub children: Vec<FieldOption>,
}

/// Top-level dispatch for `jr field options <field>`.
///
/// Mirrors `requesttype::handle`'s signature shape. Effectful shell: HTTP
/// (M2/M3 paths), cache reads (M3 path only, via `require_service_desk`/
/// `get_or_fetch_project_meta`), stdout/stderr rendering.
///
/// Per BC-X.14.001 Invariant 2, this command is strictly read-only — zero
/// mutating HTTP under any invocation.
pub async fn handle(
    command: FieldCommand,
    output_format: &OutputFormat,
    config: &Config,
    client: &JiraClient,
    project_override: Option<&str>,
) -> Result<()> {
    let FieldCommand::Options {
        field,
        r#type,
        request_type,
        issue,
        project,
        value,
    } = command;

    // Step 1 (BC-X.14.001 Invariant 1): pure mode-selector arity check,
    // BEFORE any HTTP call of any kind.
    let mode = resolve_field_context(r#type.is_some(), request_type.is_some(), issue.is_some())
        .map_err(|_| {
            JrError::UserError(
                "You must specify exactly one of --type, --request-type, --issue.".to_string(),
            )
        })?;

    // Step 2 (AC-011): resolve <field> to a field id. `customfield_NNNNN`
    // literals bypass `list_fields()` entirely; otherwise resolved via the
    // per-profile fields cache / `list_fields()` + `partial_match`.
    let profile = &config.active_profile_name;
    let field_id = resolve_field_id(client, profile, &field).await?;

    // `--project` companion: the FieldCommand::Options-local flag wins over
    // the global `--project` (ADR-0019 — the local flag lets `--project`
    // appear after the subcommand without relying on clap's global-arg
    // positioning rules).
    let cli_project = project.as_deref().or(project_override);

    let options: Vec<FieldOption> = match mode {
        Mode::Createmeta => {
            let project_key = resolve_m2_project(cli_project, config).ok_or_else(|| {
                JrError::UserError(
                    "--type needs a resolvable project — pass --project <P> or configure a default."
                        .to_string(),
                )
            })?;
            let type_name = r#type.expect("Mode::Createmeta implies --type is Some");

            let issue_types = client
                .get_issue_types_for_project(&project_key)
                .await
                .map_err(|e| map_project_not_found(e, &project_key))?;
            let type_lower = type_name.to_lowercase();
            // `.find()` with no duplicate-name detection is the deliberate,
            // established convention for this exact resolution shape — see
            // `cli/issue/edit.rs::handle_edit_bulk_fields`'s `--type`
            // resolver (BC-3.4.018/S-331), which this M2 path mirrors.
            // Jira enforces unique issue-type names within one project, so
            // ambiguity is not a realistic outcome here (unlike the field-
            // name resolver above, which searches ACROSS all fields and can
            // genuinely collide) — not adding a duplicate check that no
            // real Jira project can ever trigger.
            let issue_type_id = issue_types
                .iter()
                .find(|it| it.name.to_lowercase() == type_lower)
                .map(|it| it.id.clone())
                .ok_or_else(|| {
                    let valid: Vec<&str> = issue_types.iter().map(|it| it.name.as_str()).collect();
                    JrError::UserError(format!(
                        "Issue type '{type_name}' not found for project {project_key}. \
                         Valid types: {}.",
                        valid.join(", ")
                    ))
                })?;

            let fields = client
                .get_createmeta_fields(&project_key, &issue_type_id)
                .await?;
            let target = fields
                .into_iter()
                .find(|f| f.field_id == field_id)
                .ok_or_else(|| {
                    JrError::UserError(format!(
                        "Field '{field_id}' is not available for issue type '{type_name}' \
                         in project '{project_key}'."
                    ))
                })?;

            normalize_or_degrade(
                &target.name,
                &target.allowed_values,
                normalize_from_allowed_values,
                DegradeSchemaInfo {
                    field_type: &target.schema.field_type,
                    custom: target.schema.custom.as_deref(),
                    system: target.schema.system.as_deref(),
                    auto_complete_url: target.auto_complete_url.as_deref(),
                },
            )
        }
        Mode::RequestType => {
            let project_key = resolve_m2_project(cli_project, config).ok_or_else(|| {
                JrError::UserError(
                    "--request-type needs a resolvable project — pass --project <P> or \
                     configure a default."
                        .to_string(),
                )
            })?;
            let rt_query = request_type.expect("Mode::RequestType implies --request-type is Some");

            let sd_id = servicedesks::require_service_desk(
                client,
                &project_key,
                "`jr field options --request-type` requires",
            )
            .await
            .map_err(|e| map_project_not_found(e, &project_key))?;

            let rt_id = resolve_request_type_id(&rt_query, &sd_id, &project_key, client).await?;

            let response = client.get_request_type_fields(&sd_id, &rt_id).await?;
            let target = response
                .request_type_fields
                .into_iter()
                .find(|f| f.field_id == field_id)
                .ok_or_else(|| {
                    JrError::UserError(format!(
                        "Field '{field_id}' is not available on request type '{rt_query}'."
                    ))
                })?;

            let schema_type = target
                .jira_schema
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let schema_custom = target
                .jira_schema
                .get("custom")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let schema_system = target
                .jira_schema
                .get("system")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            normalize_or_degrade(
                &target.name,
                &target.valid_values,
                normalize_from_valid_values,
                DegradeSchemaInfo {
                    field_type: &schema_type,
                    custom: schema_custom.as_deref(),
                    system: schema_system.as_deref(),
                    auto_complete_url: target.auto_complete_url.as_deref(),
                },
            )
        }
        Mode::Editmeta => {
            let issue_key = issue.expect("Mode::Editmeta implies --issue is Some");
            let editmeta = client
                .get_editmeta(&issue_key)
                .await
                .map_err(|e| map_issue_not_found(e, &issue_key))?;
            let target = editmeta.fields.get(&field_id).ok_or_else(|| {
                JrError::UserError(format!(
                    "Field '{field_id}' is not on the Edit screen for issue {issue_key} \
                     (or is not available)."
                ))
            })?;

            normalize_or_degrade(
                &target.name,
                &target.allowed_values,
                normalize_from_allowed_values,
                DegradeSchemaInfo {
                    field_type: &target.schema.field_type,
                    custom: target.schema.custom.as_deref(),
                    system: target.schema.system.as_deref(),
                    auto_complete_url: target.auto_complete_url.as_deref(),
                },
            )
        }
    };

    let filtered = filter_options(&options, value.as_deref());
    let rows = render_option_rows(&filtered);
    output::print_output(output_format, &["ID", "Label"], &rows, &filtered)
}

/// Map a 404 from a project-resolution HTTP call (`get_issue_types_for_project`
/// / `require_service_desk` → `get_or_fetch_project_meta`) to the canonical
/// VP-580-012 exit-64 message. Every other error variant (401/403/UserError/
/// network) passes through unchanged.
fn map_project_not_found(e: anyhow::Error, project_key: &str) -> anyhow::Error {
    match e.downcast::<JrError>() {
        Ok(JrError::ApiError { status: 404, .. }) => anyhow!(JrError::UserError(format!(
            "Could not resolve project \"{project_key}\" — project not found or not accessible. \
             Run `jr project list` to see available projects."
        ))),
        Ok(other) => anyhow!(other),
        Err(other) => other,
    }
}

/// Map a 404 from `get_editmeta` (M1 `--issue` resolution) to the canonical
/// exit-64 message. Every other error variant passes through unchanged.
fn map_issue_not_found(e: anyhow::Error, key: &str) -> anyhow::Error {
    match e.downcast::<JrError>() {
        Ok(JrError::ApiError { status: 404, .. }) => anyhow!(JrError::UserError(format!(
            "Issue {key} not found or not accessible."
        ))),
        Ok(other) => anyhow!(other),
        Err(other) => other,
    }
}

/// Schema classification inputs for the BC-X.14.004 graceful-degrade hint,
/// bundled into one struct so `normalize_or_degrade`/`degrade_hint_for_schema`
/// stay under clippy's `too_many_arguments` threshold rather than growing an
/// ever-longer flat parameter list (`field_type`/`custom`/`system`/
/// `auto_complete_url` are always passed together, one per enumeration
/// mode's schema shape — M1/M2's typed `EditMetaFieldSchema` or M3's raw
/// `jiraSchema` JSON).
struct DegradeSchemaInfo<'a> {
    field_type: &'a str,
    custom: Option<&'a str>,
    system: Option<&'a str>,
    auto_complete_url: Option<&'a str>,
}

/// Normalize a source `Option<Vec<T>>` into [`FieldOption`]s, or — when the
/// field has no enumerable options at all (`None` or an empty `Vec`) —
/// emit the BC-X.14.004 graceful-degrade hint to stderr and return an empty
/// list (exit 0, never an error).
fn normalize_or_degrade<T>(
    display_name: &str,
    values: &Option<Vec<T>>,
    normalize: impl Fn(&[T]) -> Vec<FieldOption>,
    schema: DegradeSchemaInfo<'_>,
) -> Vec<FieldOption> {
    match values.as_deref() {
        Some(v) if !v.is_empty() => normalize(v),
        _ => {
            eprintln!("{}", degrade_hint_for_schema(display_name, schema));
            Vec::new()
        }
    }
}

/// BC-X.14.004 graceful-degrade hint text, classified from the field's
/// schema `type`/`system`/`custom` (typed `EditMetaFieldSchema` for M1/M2,
/// raw JSON for M3's `jiraSchema`). All three arms share the `"no
/// enumerable options"` prefix so callers can detect a degrade uniformly.
/// `display_name` is the field's human-readable name (`EditMetaField.name`
/// / `CreateMetaField.name` / `RequestTypeField.name`) — folding it in here
/// gives every caller a real use of that field instead of leaving it dead.
///
/// Classification order (per the BC-X.14.004 "Graceful degradation" table):
/// 1. **Assets/CMDB** — `schema.custom` names the CMDB object custom-field
///    type, REGARDLESS of `schema.type` (`"object"` for a single-select
///    Assets field, `"array"` for a multi-select one — EC-X.14.004-1 /
///    the array-typed-CMDB broadening covers both shapes with one check).
/// 2. **Dynamic/suggestion-backed** — user-picker (`type == "user"`),
///    multi-user-picker/Approvers (`custom` names a user-picker-family
///    type), or `labels` (a system field with no `custom` key at all,
///    identified via `schema.system`) — anything Jira resolves live via a
///    suggestion endpoint rather than a fixed `allowedValues`/`validValues`
///    set. `+ autoCompleteUrl` is appended when the field schema carries
///    one (AC-014).
/// 3. **Free-text/number/date/other** — no finite option set and no
///    suggestion endpoint.
fn degrade_hint_for_schema(display_name: &str, schema: DegradeSchemaInfo<'_>) -> String {
    let DegradeSchemaInfo {
        field_type,
        custom,
        system,
        auto_complete_url,
    } = schema;

    let is_cmdb = custom
        .map(|c| c.to_lowercase().contains("cmdb"))
        .unwrap_or(false);
    if is_cmdb {
        return format!(
            "no enumerable options for '{display_name}' — this field uses Assets (CMDB). \
             Search assets separately via `jr assets search`."
        );
    }

    let is_dynamic = field_type == "user"
        || custom
            .map(|c| {
                let c_lower = c.to_lowercase();
                c_lower.contains("userpicker") || c_lower.contains("approv")
            })
            .unwrap_or(false)
        || system
            .map(|s| s.eq_ignore_ascii_case("labels"))
            .unwrap_or(false)
        || auto_complete_url.is_some();

    if is_dynamic {
        let mut hint = format!(
            "no enumerable options for '{display_name}' (dynamic/lookup field) — values are \
             resolved live and cannot be enumerated by this command."
        );
        if let Some(url) = auto_complete_url {
            hint.push_str(&format!(" autoCompleteUrl: {url}"));
        }
        return hint;
    }

    format!(
        "no enumerable options for '{display_name}' — this field type has no fixed \
         value set."
    )
}

/// Resolve `<field>` to a `customfield_NNNNN`-shaped field id (AC-011).
///
/// `customfield_NNNNN` literals bypass `list_fields()` entirely (zero HTTP).
/// Otherwise resolves via the per-profile fields cache (`cache::
/// read_fields_cache`), falling back to `list_fields()` on a cache miss or
/// a field absent from the cached list, writing the fresh list back
/// (best-effort) before re-searching exactly once.
async fn resolve_field_id(
    client: &JiraClient,
    profile: &crate::profile::Profile,
    query: &str,
) -> Result<String> {
    if is_customfield_literal(query) {
        return Ok(query.to_string());
    }
    if query.is_empty() {
        return Err(JrError::UserError(
            "Field '' not found. The field name must not be empty.".to_string(),
        )
        .into());
    }

    let query_lower = query.to_lowercase();

    if let Some(fc) = cache::read_fields_cache(profile)? {
        if let Some(found) = search_field_list(&fc.fields, &query_lower, query)? {
            return Ok(found);
        }
    }

    // Cache miss (or field absent from the cached list) — fetch fresh once.
    let raw = client.list_fields().await?;
    let fresh: Vec<(String, String)> = raw.into_iter().map(|f| (f.id, f.name)).collect();
    cache::write_fields_cache(profile, &fresh)?;

    match search_field_list(&fresh, &query_lower, query)? {
        Some(found) => Ok(found),
        None => Err(JrError::UserError(format!(
            "Field '{query}' not found. Run `jr project fields --output json` to list \
             available fields."
        ))
        .into()),
    }
}

/// `customfield_NNNNN` literal-bypass predicate (mirrors
/// `resolve_edit_fields`'s Step 1 in `cli/issue/field_resolve.rs`).
fn is_customfield_literal(query: &str) -> bool {
    const PREFIX: &str = "customfield_";
    query.starts_with(PREFIX)
        && query.len() > PREFIX.len()
        && query[PREFIX.len()..].chars().all(|c| c.is_ascii_digit())
}

/// Search a resolved `(id, name)` field list for `query` — exact match
/// first, then case-insensitive substring. `Ok(None)` means "not found in
/// THIS list" (caller may still fall back to a fresh fetch); `Err` means a
/// definitive ambiguity the caller must surface immediately.
fn search_field_list(
    list: &[(String, String)],
    query_lower: &str,
    query: &str,
) -> Result<Option<String>> {
    let exact: Vec<&(String, String)> = list
        .iter()
        .filter(|(_, n)| n.to_lowercase() == query_lower)
        .collect();
    if exact.len() == 1 {
        return Ok(Some(exact[0].0.clone()));
    }
    if exact.len() > 1 {
        let candidates: Vec<String> = exact.iter().map(|(id, n)| format!("{n} ({id})")).collect();
        return Err(JrError::UserError(format!(
            "Field name '{query}' matches multiple fields: {}. Use the field ID directly \
             (e.g. customfield_NNNNN) to disambiguate.",
            candidates.join(", ")
        ))
        .into());
    }

    let sub: Vec<&(String, String)> = list
        .iter()
        .filter(|(_, n)| n.to_lowercase().contains(query_lower))
        .collect();
    if sub.len() == 1 {
        return Ok(Some(sub[0].0.clone()));
    }
    if sub.len() > 1 {
        let candidates: Vec<String> = sub.iter().map(|(id, n)| format!("{n} ({id})")).collect();
        return Err(JrError::UserError(format!(
            "Field name '{query}' is ambiguous — matches: {}. Use a more specific name or \
             the field ID directly (e.g. customfield_NNNNN).",
            candidates.join(", ")
        ))
        .into());
    }
    Ok(None)
}

/// Resolve M3's `--request-type` value to a request-type id. All-ASCII-digit
/// input bypasses resolution (numeric-ID convention shared with `jr
/// requesttype fields`); otherwise resolves via `list_request_types` +
/// `partial_match`.
async fn resolve_request_type_id(
    query: &str,
    sd_id: &str,
    project_key: &str,
    client: &JiraClient,
) -> Result<String> {
    if !query.is_empty() && query.chars().all(|c| c.is_ascii_digit()) {
        return Ok(query.to_string());
    }

    let types = client.list_request_types(sd_id, None).await?;
    let names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();

    match partial_match::partial_match(query, &names) {
        MatchResult::Exact(name) => Ok(types
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id.clone())
            .expect("matched name from partial_match::Exact must exist in types")),
        MatchResult::ExactMultiple(name) => {
            let name_lower = name.to_lowercase();
            let ids: Vec<String> = types
                .iter()
                .filter(|t| t.name.to_lowercase() == name_lower)
                .map(|t| t.id.clone())
                .collect();
            Err(JrError::UserError(format!(
                "Multiple request types named \"{name}\" found (IDs: {}). Pass the numeric \
                 ID directly.",
                ids.join(", ")
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous request type \"{query}\" matches: {}. Run `jr requesttype list \
             --project {project_key}` to see all request types.",
            matches
                .iter()
                .map(|m| format!("\"{m}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        MatchResult::None(_) => Err(JrError::UserError(format!(
            "Request type \"{query}\" not found. Run `jr requesttype list --project \
             {project_key}` to see all request types."
        ))
        .into()),
    }
}

/// Pure arity check over the three MODE-SELECTOR booleans ONLY.
///
/// `has_project` is deliberately NOT a parameter (ADR-0019 §Amendment D1) —
/// project resolvability is a separate, post-arity, M2-only step handled by
/// [`resolve_m2_project`]. Exactly one of the three booleans present →
/// `Ok(Mode)`; zero or two-or-more present → `Err(ArityError)`.
///
/// Proptested exhaustively over the 3-boolean flag-presence space
/// (VP-580-006). No I/O — pure core, same purity class as
/// `config::validate_profile_name`.
pub(crate) fn resolve_field_context(
    has_type: bool,
    has_request_type: bool,
    has_issue: bool,
) -> std::result::Result<Mode, ArityError> {
    let count = u8::from(has_type) + u8::from(has_request_type) + u8::from(has_issue);
    match count {
        1 if has_type => Ok(Mode::Createmeta),
        1 if has_request_type => Ok(Mode::RequestType),
        1 => Ok(Mode::Editmeta),
        0 => Err(ArityError::Zero),
        _ => Err(ArityError::Multiple),
    }
}

/// Post-arity, M2-only project resolution step (ADR-0019 §Amendment D1).
///
/// Resolves the project to use for `get_createmeta_fields` as: the explicit
/// `--project` flag value, OR the active profile/config default — the same
/// source BC-3.3.010's create-path project resolution and M3's optional
/// `--project` companion fallback already read. `None` means neither is
/// available (caller maps this to the incomplete-M2 exit-64 error).
///
/// Sibling pure function to [`resolve_field_context`], not a widened Step 1
/// — reads only already-loaded in-process `Config` state, no HTTP
/// (VP-580-010).
pub(crate) fn resolve_m2_project(cli_project: Option<&str>, config: &Config) -> Option<String> {
    config.project_key(cli_project)
}

/// Normalize M1 (editmeta) / M2 (createmeta) `allowedValues[]` entries into
/// [`FieldOption`]s.
///
/// Both sources share the identical `{id, value, name, children}` shape
/// (ADR-0019 §1 "Type reuse" note), so one function serves both. MUST emit
/// exactly one `FieldOption` per source item — a missing `id`/`label`
/// degrades that entry's own field(s) to `None`, never drops the entry
/// (EC-X.14.001-7). Cascading children nest recursively under `children`
/// (EC-X.14.001-4). Pure core — no I/O.
pub(crate) fn normalize_from_allowed_values(values: &[AllowedValue]) -> Vec<FieldOption> {
    normalize_from_allowed_values_at_depth(values, 0)
}

/// Depth-tracked worker for [`normalize_from_allowed_values`]. See
/// [`MAX_FIELD_OPTION_DEPTH`] — beyond the cap, a child's own `children`
/// are truncated (treated as a leaf) rather than recursed into further;
/// the child entry itself is never dropped.
fn normalize_from_allowed_values_at_depth(
    values: &[AllowedValue],
    depth: usize,
) -> Vec<FieldOption> {
    values
        .iter()
        .map(|v| FieldOption {
            id: v.id.clone(),
            label: v.value.clone(),
            children: if depth >= MAX_FIELD_OPTION_DEPTH {
                Vec::new()
            } else {
                normalize_from_allowed_values_at_depth(&v.children, depth + 1)
            },
        })
        .collect()
}

/// Normalize M3 (JSM requesttype-fields) `validValues[]` entries into
/// [`FieldOption`]s.
///
/// M3's wire shape is untyped `serde_json::Value` (per
/// `RequestTypeField.valid_values: Option<Vec<serde_json::Value>>`), keyed
/// by `.value` for the option id (NOT `.id` — the naming collision with
/// M1/M2's label-bearing `value` key is deliberate Atlassian API
/// inconsistency, not a `jr` bug) and `.label` for display text. Same
/// never-drop / cascading-children contract as
/// [`normalize_from_allowed_values`]. Pure core — no I/O.
pub(crate) fn normalize_from_valid_values(values: &[serde_json::Value]) -> Vec<FieldOption> {
    normalize_from_valid_values_at_depth(values, 0)
}

/// Depth-tracked worker for [`normalize_from_valid_values`]. See
/// [`MAX_FIELD_OPTION_DEPTH`] — beyond the cap, a child's own `children`
/// are truncated (treated as a leaf) rather than recursed into further;
/// the child entry itself is never dropped. Particularly load-bearing
/// here versus the M1/M2 sibling: this walks untyped `serde_json::Value`
/// AFTER deserialization, so serde's own recursion limit does not bound
/// it.
fn normalize_from_valid_values_at_depth(
    values: &[serde_json::Value],
    depth: usize,
) -> Vec<FieldOption> {
    values
        .iter()
        .map(|v| {
            let id = v.get("value").and_then(|x| x.as_str()).map(str::to_string);
            let label = v.get("label").and_then(|x| x.as_str()).map(str::to_string);
            let children = if depth >= MAX_FIELD_OPTION_DEPTH {
                Vec::new()
            } else {
                v.get("children")
                    .and_then(|c| c.as_array())
                    .map(|arr| normalize_from_valid_values_at_depth(arr, depth + 1))
                    .unwrap_or_default()
            };
            FieldOption {
                id,
                label,
                children,
            }
        })
        .collect()
}

/// Client-side `--value <substring>` filter (BC-X.14.002).
///
/// Applied AFTER the full fetch — no server-side filtering exists for any
/// of the three enumeration mechanisms. Case-insensitive; matches when
/// EITHER `label` OR `id` contains the substring. A child matching `--value`
/// is retained under its parent (parent retained as context) even when the
/// parent's own label/id doesn't match; a parent matching `--value` retains
/// ALL its children unfiltered. `value == Some("")` is the IDENTITY filter
/// (matches every entry unconditionally, including a fully degenerate
/// entry); `value == None` returns `options` unchanged. Pure core — no I/O.
pub(crate) fn filter_options(options: &[FieldOption], value: Option<&str>) -> Vec<FieldOption> {
    match value {
        // Absent filter, and the identity `""` filter, both return every
        // entry unchanged (including a fully degenerate one) — handled as a
        // single early return so `filter_one`'s "does this entry match"
        // logic never needs to special-case an always-true needle.
        None => options.to_vec(),
        Some("") => options.to_vec(),
        Some(v) => {
            let needle = v.to_lowercase();
            options
                .iter()
                .filter_map(|opt| filter_one(opt, &needle, 0))
                .collect()
        }
    }
}

/// Recursive per-entry filter helper for [`filter_options`]. A self-match
/// retains the entry with ALL its children unfiltered; otherwise the entry
/// is retained (as context, with only its matching children) if ANY
/// descendant matches, and dropped entirely if none does. See
/// [`MAX_FIELD_OPTION_DEPTH`] — beyond the cap, descendants are no longer
/// searched (treated as non-matching), never causing the ancestor entry
/// itself to be dropped.
fn filter_one(opt: &FieldOption, needle: &str, depth: usize) -> Option<FieldOption> {
    let self_match = opt
        .id
        .as_deref()
        .map(|s| s.to_lowercase().contains(needle))
        .unwrap_or(false)
        || opt
            .label
            .as_deref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false);

    if self_match {
        return Some(opt.clone());
    }

    if depth >= MAX_FIELD_OPTION_DEPTH {
        return None;
    }

    let filtered_children: Vec<FieldOption> = opt
        .children
        .iter()
        .filter_map(|c| filter_one(c, needle, depth + 1))
        .collect();

    if filtered_children.is_empty() {
        None
    } else {
        Some(FieldOption {
            id: opt.id.clone(),
            label: opt.label.clone(),
            children: filtered_children,
        })
    }
}

/// Render `options` into table rows (BC-X.14.003) — ID / Label columns,
/// cascading children indented under their parent. Table mode only; JSON
/// mode preserves nested `children[]` verbatim via `output::render_json`
/// (never flattened). Degenerate-entry glyphs: missing `id` -> [`NULL_GLYPH`]
/// (`"—"`), missing `label` -> [`UNNAMED_LABEL`] (`"(unnamed)"`, never a
/// fallback to the entry's own `id`). Pure core — no I/O.
pub(crate) fn render_option_rows(options: &[FieldOption]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    render_rows_recursive(options, 0, &mut rows);
    rows
}

/// Depth-first row emitter for [`render_option_rows`]. `depth` drives the
/// two-space-per-level indent on the Label column; the ID column is never
/// indented. See [`MAX_FIELD_OPTION_DEPTH`] — every entry up to and
/// including the cap is still rendered as its own row; only descendants
/// beyond the cap stop recursing (defense-in-depth alongside the
/// normalize-time truncation, which already keeps `children` empty past
/// the cap in practice).
fn render_rows_recursive(options: &[FieldOption], depth: usize, rows: &mut Vec<Vec<String>>) {
    let indent = "  ".repeat(depth);
    for opt in options {
        let id_str = opt.id.clone().unwrap_or_else(|| NULL_GLYPH.to_string());
        let label_str = opt
            .label
            .clone()
            .unwrap_or_else(|| UNNAMED_LABEL.to_string());
        rows.push(vec![id_str, format!("{indent}{label_str}")]);
        if depth < MAX_FIELD_OPTION_DEPTH {
            render_rows_recursive(&opt.children, depth + 1, rows);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Red Gate test suite (S-580-1, Step 3 — test-writer pass).
//
// These tests cover the PURE-CORE functions in this module:
// `resolve_field_context`, `resolve_m2_project`, `normalize_from_allowed_values`,
// `normalize_from_valid_values`, `filter_options`, `render_option_rows`.
//
// Integration coverage for `handle()` (M1/M2/M3 dispatch, error taxonomy,
// graceful degradation) lives in `tests/field_options.rs` (wiremock +
// subprocess, mirroring `tests/requesttype_commands.rs`'s pattern).
// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProfileConfig};
    use proptest::prelude::*;

    // ── AC-001 / BC-X.14.001 Invariant 1 / VP-580-006: resolve_field_context arity ──

    /// AC-001: exactly one of the three mode-selector booleans present -> Ok;
    /// zero or two-or-more present -> Err. Exhaustive over the 3-boolean space
    /// (8 combinations) as a fast, deterministic companion to the proptest below.
    #[test]
    fn test_bc_x_14_001_resolve_field_context_exhaustive_8_combinations() {
        for has_type in [false, true] {
            for has_request_type in [false, true] {
                for has_issue in [false, true] {
                    let popcount = has_type as u8 + has_request_type as u8 + has_issue as u8;
                    let result = resolve_field_context(has_type, has_request_type, has_issue);
                    match popcount {
                        1 => {
                            assert!(
                                result.is_ok(),
                                "exactly-one-true ({has_type},{has_request_type},{has_issue}) must be Ok, got {result:?}"
                            );
                            let mode = result.unwrap();
                            if has_type {
                                assert_eq!(mode, Mode::Createmeta);
                            } else if has_request_type {
                                assert_eq!(mode, Mode::RequestType);
                            } else {
                                assert_eq!(mode, Mode::Editmeta);
                            }
                        }
                        0 => assert_eq!(
                            result,
                            Err(ArityError::Zero),
                            "zero-true ({has_type},{has_request_type},{has_issue}) must be Err(Zero)"
                        ),
                        _ => assert_eq!(
                            result,
                            Err(ArityError::Multiple),
                            "multi-true ({has_type},{has_request_type},{has_issue}) must be Err(Multiple)"
                        ),
                    }
                }
            }
        }
    }

    proptest! {
        /// AC-001 / VP-580-006: proptested exhaustively (the input space is
        /// tiny — 3 bools — so this doubles as a randomized cross-check of
        /// the exhaustive test above; required by the story regardless of
        /// space size).
        #[test]
        fn test_bc_x_14_001_resolve_field_context_arity_proptest(
            has_type: bool,
            has_request_type: bool,
            has_issue: bool,
        ) {
            let popcount = has_type as u8 + has_request_type as u8 + has_issue as u8;
            let result = resolve_field_context(has_type, has_request_type, has_issue);
            match popcount {
                1 => prop_assert!(result.is_ok()),
                0 => prop_assert_eq!(result, Err(ArityError::Zero)),
                _ => prop_assert_eq!(result, Err(ArityError::Multiple)),
            }
        }
    }

    // ── AC-011 / VP-580-004: field-name resolution (search_field_list /
    // is_customfield_literal / resolve_field_id) — S-580-1 convergence pass ──

    /// `is_customfield_literal`: the happy-path bypass, plus the malformed
    /// shapes that must NOT bypass (no digits, non-digit suffix, bare
    /// prefix, unrelated string).
    #[test]
    fn test_bc_x_14_001_is_customfield_literal_accepts_and_rejects() {
        assert!(is_customfield_literal("customfield_10084"));
        assert!(is_customfield_literal("customfield_0"));
        assert!(
            !is_customfield_literal("customfield_"),
            "bare prefix with no digits must not bypass"
        );
        assert!(
            !is_customfield_literal("customfield_abc"),
            "non-digit suffix must not bypass"
        );
        assert!(
            !is_customfield_literal("Story Points"),
            "an ordinary human name must not bypass"
        );
        assert!(
            !is_customfield_literal("customfield_10084x"),
            "a digit run followed by a non-digit must not bypass"
        );
    }

    /// `search_field_list`: a single case-insensitive EXACT match resolves
    /// directly to that field's id.
    #[test]
    fn test_bc_x_14_001_search_field_list_exact_single_match() {
        let list = vec![
            ("customfield_10001".to_string(), "Story Points".to_string()),
            ("customfield_10002".to_string(), "Sprint".to_string()),
        ];
        let result = search_field_list(&list, "story points", "Story Points").unwrap();
        assert_eq!(result, Some("customfield_10001".to_string()));
    }

    /// `search_field_list`: exact match is case-insensitive — a differently
    /// cased query still resolves to the single exact match.
    #[test]
    fn test_bc_x_14_001_search_field_list_case_insensitive() {
        let list = vec![("customfield_10001".to_string(), "Story Points".to_string())];
        // `query_lower` is the CALLER-lowercased form (resolve_field_id
        // lowercases before calling); `query` retains the original casing
        // for use in error messages only.
        let result = search_field_list(&list, "story points", "STORY points").unwrap();
        assert_eq!(result, Some("customfield_10001".to_string()));
    }

    /// `search_field_list`: no exact match, but a single case-insensitive
    /// SUBSTRING match resolves to that field's id.
    #[test]
    fn test_bc_x_14_001_search_field_list_substring_single_match() {
        let list = vec![
            ("customfield_10084".to_string(), "SOC Client".to_string()),
            ("customfield_10002".to_string(), "Sprint".to_string()),
        ];
        let result = search_field_list(&list, "soc", "soc").unwrap();
        assert_eq!(result, Some("customfield_10084".to_string()));
    }

    /// `search_field_list`: no exact or substring match -> `Ok(None)` (the
    /// caller decides whether to fall back to a fresh fetch or exit 64).
    #[test]
    fn test_bc_x_14_001_search_field_list_zero_match_returns_none() {
        let list = vec![("customfield_10002".to_string(), "Sprint".to_string())];
        let result = search_field_list(&list, "nonexistent", "nonexistent").unwrap();
        assert_eq!(result, None);
    }

    /// `search_field_list`: multiple case-insensitive EXACT matches ->
    /// `Err`, naming every candidate and its id.
    #[test]
    fn test_bc_x_14_001_search_field_list_exact_multiple_is_err() {
        let list = vec![
            ("customfield_10084".to_string(), "SOC Client".to_string()),
            ("customfield_10085".to_string(), "soc client".to_string()),
        ];
        let err = search_field_list(&list, "soc client", "soc client").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("customfield_10084") && msg.contains("customfield_10085"));
    }

    /// `search_field_list`: multiple SUBSTRING matches (no exact match) ->
    /// `Err`, naming every candidate and its id.
    #[test]
    fn test_bc_x_14_001_search_field_list_substring_multiple_is_err() {
        let list = vec![
            ("customfield_10084".to_string(), "SOC Client A".to_string()),
            ("customfield_10085".to_string(), "SOC Client B".to_string()),
        ];
        let err = search_field_list(&list, "soc client", "soc client").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("customfield_10084") && msg.contains("customfield_10085"));
    }

    /// `resolve_request_type_id`'s `MatchResult::ExactMultiple` arm
    /// (round-4 mutation-coverage sweep, B-M1) — sibling of
    /// `search_field_list`'s `exact_multiple_is_err` test above, which was
    /// already covered; `resolve_request_type_id`'s own `ExactMultiple`
    /// branch had NO discriminating test until this one. Two request types
    /// sharing an IDENTICAL name (`partial_match::ExactMultiple`, distinct
    /// from `Ambiguous` — a SUBSTRING match across several DIFFERENT names,
    /// already covered by `test_bc_x_14_001_m3_request_type_name_ambiguous_exits_64`
    /// in `tests/field_options.rs`) must exit via `JrError::UserError`
    /// naming BOTH gathered candidate IDs, never silently pick the first.
    #[tokio::test]
    async fn test_bc_x_14_001_resolve_request_type_id_exact_multiple_is_err() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/servicedesk/10/requesttype"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "size": 2,
                "start": 0,
                "limit": 50,
                "isLastPage": true,
                "_links": {},
                "values": [
                    {
                        "id": "11001",
                        "name": "Get Help",
                        "description": null,
                        "helpText": null,
                        "issueTypeId": null,
                        "groupIds": []
                    },
                    {
                        "id": "11002",
                        "name": "Get Help",
                        "description": null,
                        "helpText": null,
                        "issueTypeId": null,
                        "groupIds": []
                    }
                ]
            })))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
        let err = resolve_request_type_id("Get Help", "10", "HELP", &client)
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Multiple request types named"),
            "expected the ExactMultiple canonical message; got: {msg}"
        );
        assert!(
            msg.contains("11001") && msg.contains("11002"),
            "expected BOTH gathered candidate IDs (not just the first) in the \
             error message; got: {msg}"
        );
    }

    /// `resolve_field_id`: the `customfield_NNNNN` literal bypass never
    /// touches the cache or performs HTTP — proven by pointing the client at
    /// an address nothing listens on (`localhost:1`) and asserting the call
    /// still succeeds, which is only possible if the bypass branch returns
    /// before any `.await` on the client.
    #[tokio::test]
    async fn test_bc_x_14_001_resolve_field_id_customfield_literal_bypasses_http() {
        let client = JiraClient::new_for_test("http://127.0.0.1:1".to_string(), "x".to_string());
        let result = resolve_field_id(
            &client,
            &crate::profile::Profile::from("default"),
            "customfield_10084",
        )
        .await;
        assert_eq!(result.unwrap(), "customfield_10084");
    }

    /// AC-001: `has_project` is not a parameter of `resolve_field_context` at
    /// all (ADR-0019 § Amendment D1) — this is enforced structurally by the
    /// function's own 3-argument signature (a 4th argument would be a compile
    /// error). `resolve_field_context(true, false, false)` must resolve to
    /// `Mode::Createmeta`, exercising the same call the documentation-anchor
    /// version of this test only compiled without checking.
    #[test]
    fn test_bc_x_14_001_resolve_field_context_has_no_project_parameter() {
        let result = resolve_field_context(true, false, false);
        assert_eq!(result, Ok(Mode::Createmeta));
    }

    // ── AC-004 / VP-580-010: resolve_m2_project (flag OR profile default) ──

    fn config_with_profile_project(project: Option<&str>) -> Config {
        let mut config = Config {
            active_profile_name: "default".into(),
            ..Default::default()
        };
        config.global.profiles.insert(
            "default".to_string(),
            ProfileConfig {
                project: project.map(String::from),
                ..Default::default()
            },
        );
        config
    }

    #[test]
    fn test_bc_x_14_001_resolve_m2_project_flag_wins() {
        let config = config_with_profile_project(Some("DEFAULTPROJ"));
        let resolved = resolve_m2_project(Some("FLAGPROJ"), &config);
        assert_eq!(
            resolved,
            Some("FLAGPROJ".to_string()),
            "explicit --project flag must win over the profile default"
        );
    }

    #[test]
    fn test_bc_x_14_001_resolve_m2_project_falls_back_to_config_default() {
        let config = config_with_profile_project(Some("DEFAULTPROJ"));
        let resolved = resolve_m2_project(None, &config);
        assert_eq!(
            resolved,
            Some("DEFAULTPROJ".to_string()),
            "absent --project flag must fall back to the profile/config default"
        );
    }

    #[test]
    fn test_bc_x_14_001_resolve_m2_project_neither_present_returns_none() {
        let config = config_with_profile_project(None);
        let resolved = resolve_m2_project(None, &config);
        assert_eq!(
            resolved, None,
            "neither an explicit flag nor a profile default -> None (caller maps to incomplete-M2 error)"
        );
    }

    // ── AC-009 / EC-X.14.001-7 / VP-580-005: normalizer never-drop invariant ──

    /// M1/M2 normalizer: a fixture mixing well-formed and degenerate
    /// (missing id/label/both) `AllowedValue` entries must preserve
    /// entry-count — one `FieldOption` per source item, never fewer.
    #[test]
    fn test_bc_x_14_001_normalizer_never_drops_degenerate_entries() {
        let values = vec![
            AllowedValue {
                id: Some("10001".to_string()),
                value: Some("Well-formed".to_string()),
                name: None,
                // NOTE (S-580-1 implementer pass): `children` added here — a
                // mechanical consequence of the AUTHORIZED SCOPE CORRECTION
                // extending `AllowedValue` with `#[serde(default)] pub
                // children: Vec<AllowedValue>` (ADR-0019 §Amendment D4).
                // This struct-literal fixture predates that extension and
                // would not compile without it; the value (`Vec::new()`) is
                // fully consistent with the test's own trailing assertion
                // (`assert!(opt.children.is_empty())` for every entry) —
                // no assertion or behavior in this test changed.
                children: Vec::new(),
            },
            AllowedValue {
                id: None,
                value: Some("Missing id".to_string()),
                name: None,
                children: Vec::new(),
            },
            AllowedValue {
                id: Some("10003".to_string()),
                value: None,
                name: None,
                children: Vec::new(),
            },
            AllowedValue {
                id: None,
                value: None,
                name: None,
                children: Vec::new(),
            },
        ];
        let result = normalize_from_allowed_values(&values);
        assert_eq!(
            result.len(),
            values.len(),
            "normalizer must emit exactly one FieldOption per source item, never dropping degenerate entries"
        );
        assert_eq!(result[0].id, Some("10001".to_string()));
        assert_eq!(result[0].label, Some("Well-formed".to_string()));
        assert_eq!(result[1].id, None);
        assert_eq!(result[1].label, Some("Missing id".to_string()));
        assert_eq!(result[2].id, Some("10003".to_string()));
        assert_eq!(result[2].label, None);
        assert_eq!(result[3].id, None);
        assert_eq!(result[3].label, None);
        // EC-X.14.001-4: children always present, never absent, empty for non-cascading.
        for opt in &result {
            assert!(opt.children.is_empty());
        }
    }

    /// M3 normalizer: same never-drop contract, over arbitrary
    /// `serde_json::Value` items (VP-580-005 "tolerates arbitrary JSON,
    /// never unwraps a missing field").
    #[test]
    fn test_bc_x_14_001_normalizer_from_valid_values_never_drops_degenerate_entries() {
        let values = vec![
            serde_json::json!({"value": "10001", "label": "Well-formed"}),
            serde_json::json!({"label": "Missing id"}),
            serde_json::json!({"value": "10003"}),
            serde_json::json!({}),
        ];
        let result = normalize_from_valid_values(&values);
        assert_eq!(
            result.len(),
            values.len(),
            "M3 normalizer must never drop a degenerate/empty JSON entry"
        );
        assert_eq!(result[0].id, Some("10001".to_string()));
        assert_eq!(result[0].label, Some("Well-formed".to_string()));
        assert_eq!(result[1].id, None);
        assert_eq!(result[1].label, Some("Missing id".to_string()));
        assert_eq!(result[2].id, Some("10003".to_string()));
        assert_eq!(result[2].label, None);
        assert_eq!(result[3].id, None);
        assert_eq!(result[3].label, None);
    }

    proptest! {
        /// VP-580-005: the M3 normalizer must never panic on arbitrary JSON
        /// shapes (wrong types, missing keys, extra keys, nulls).
        #[test]
        fn test_bc_x_14_001_normalize_from_valid_values_never_panics(
            entries in proptest::collection::vec(
                proptest::collection::hash_map(
                    "[a-z]{1,8}",
                    prop_oneof![
                        Just(serde_json::Value::Null),
                        any::<String>().prop_map(serde_json::Value::from),
                        any::<i64>().prop_map(serde_json::Value::from),
                        any::<bool>().prop_map(serde_json::Value::from),
                    ],
                    0..5,
                ).prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
                0..8,
            )
        ) {
            let result = normalize_from_valid_values(&entries);
            prop_assert_eq!(result.len(), entries.len());
        }
    }

    /// A recursive `serde_json::Value` strategy — bounded depth — used to
    /// fuzz BOTH the top-level entry shape (Array/String/Number/Bool/Null,
    /// not just Object) AND the recursive `children` branch of
    /// `normalize_from_valid_values` (VP-580-005 §2 strengthening, S-580-1
    /// convergence pass, LOW: "extend to non-object top-level entries and
    /// nested `children` arrays"). Object keys are biased toward
    /// `value`/`label`/`children` so generated fixtures actually exercise
    /// the normalizer's real key lookups, not just its `_ => degrade`
    /// fallback path.
    fn arb_json_value() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            any::<String>().prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(4, 32, 4, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
                proptest::collection::hash_map(
                    prop_oneof![
                        Just("value".to_string()),
                        Just("label".to_string()),
                        Just("children".to_string()),
                        "[a-z]{1,6}".prop_map(String::from),
                    ],
                    inner,
                    0..4,
                )
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        /// VP-580-005 §2 strengthening (LOW, S-580-1 convergence pass): the
        /// never-panic + never-drop guarantee must hold over ARBITRARY
        /// top-level JSON shapes (not just well-formed objects) AND over
        /// fixtures whose `children` key holds arbitrary nested JSON,
        /// fuzzing the recursive branch of `normalize_from_valid_values`.
        /// A non-object top-level entry (Array/String/Number/Bool/Null) has
        /// no `.get("value")`/`.get("children")` to read at all — the
        /// normalizer must degrade that entry to `{id: None, label: None,
        /// children: []}` rather than panicking or dropping it.
        #[test]
        fn test_bc_x_14_001_normalize_from_valid_values_never_panics_arbitrary_shapes(
            entries in proptest::collection::vec(arb_json_value(), 0..8)
        ) {
            let result = normalize_from_valid_values(&entries);
            prop_assert_eq!(result.len(), entries.len());
        }
    }

    // ── AC-010 / EC-X.14.001-4 / VP-580-003: cascading children round-trip ──

    /// M3: `validValues[].children` (untyped JSON) nests recursively into
    /// `FieldOption.children`.
    #[test]
    fn test_bc_x_14_001_cascading_children_round_trip_m3() {
        let values = vec![serde_json::json!({
            "value": "parent-1",
            "label": "Parent",
            "children": [
                {"value": "child-1", "label": "Child One"},
                {"value": "child-2", "label": "Child Two"},
            ],
        })];
        let result = normalize_from_valid_values(&values);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, Some("parent-1".to_string()));
        assert_eq!(result[0].children.len(), 2);
        assert_eq!(result[0].children[0].id, Some("child-1".to_string()));
        assert_eq!(result[0].children[0].label, Some("Child One".to_string()));
        assert_eq!(result[0].children[1].id, Some("child-2".to_string()));
    }

    /// M3: non-cascading entries always have `children: []`, never omitted.
    #[test]
    fn test_bc_x_14_001_non_cascading_m3_entry_has_empty_children() {
        let values = vec![serde_json::json!({"value": "10001", "label": "Solo"})];
        let result = normalize_from_valid_values(&values);
        assert_eq!(result[0].children, Vec::<FieldOption>::new());
    }

    /// AC-010 / EC-X.14.001-4: M1/M2's `allowedValues[].children[]` must
    /// round-trip into `FieldOption.children` identically to M3's shape.
    ///
    /// KNOWN GAP (flagged for implementer/orchestrator, DONE_WITH_CONCERNS):
    /// as of this Red Gate pass, `types::jira::editmeta::AllowedValue`
    /// (`src/types/jira/editmeta.rs`) has NO `children` field — confirmed by
    /// reading the as-built struct (`{id, value, name}` only) and by
    /// ADR-0019 §D4's own text ("verified as-built to currently carry no
    /// `children` field"). This story's own File Structure Requirements list
    /// `src/types/jira/editmeta.rs` as "MUST NOT change... the `children`
    /// field extension belongs to S-578-2 (D4)" — but D4 pins that extension
    /// for the WRITE-side cascading `:option` composer, not for THIS story's
    /// READ-side enumeration normalizer. Because `AllowedValue` cannot carry
    /// `children` data at all today, this test constructs its fixture via
    /// `serde_json::from_value::<AllowedValue>` over JSON that INCLUDES a
    /// `children` key — that key is silently dropped by serde (no
    /// `deny_unknown_fields` on `AllowedValue`), so this test can only pass
    /// once `AllowedValue` gains `#[serde(default)] pub children:
    /// Vec<AllowedValue>` (or equivalent). This is therefore RED for a
    /// second, structural reason beyond `normalize_from_allowed_values`
    /// being a stub — see the test-writer's final report for this concern.
    #[test]
    fn test_bc_x_14_001_cascading_children_round_trip_m1_m2() {
        let parent: AllowedValue = serde_json::from_value(serde_json::json!({
            "id": "parent-1",
            "value": "Parent",
            "children": [
                {"id": "child-1", "value": "Child One"},
                {"id": "child-2", "value": "Child Two"},
            ],
        }))
        .expect("AllowedValue must deserialize (extra `children` key is tolerated, not rejected)");
        let result = normalize_from_allowed_values(std::slice::from_ref(&parent));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, Some("parent-1".to_string()));
        assert_eq!(
            result[0].children.len(),
            2,
            "M1/M2 cascading children must round-trip into FieldOption.children — \
             requires AllowedValue to gain a `children` field (see KNOWN GAP doc comment above)"
        );
    }

    // ── B-M2 (round-2 convergence, MEDIUM): MAX_FIELD_OPTION_DEPTH=256
    // regression pin. Existing proptests bound recursion depth at 3-4
    // (`prop_recursive(4, 32, 4, ...)`), far below the 256 cap, so the
    // `depth >= MAX_FIELD_OPTION_DEPTH` guard branch itself had ZERO
    // coverage before these two tests. A ~300-level-deep fixture is built
    // for each normalizer (M3's untyped-JSON path and M1/M2's typed
    // `AllowedValue` path) so the guard actually fires: the TOP-LEVEL entry
    // must always be preserved (never dropped, regardless of depth), and
    // the entry sitting exactly AT nesting depth 256 must have its own
    // `children` truncated to empty — even though the source data nests
    // further beneath it. A mutant that deletes the guard (unconditional
    // recursion) leaves that entry's `children` non-empty; a mutant that
    // flips `>=` to `>` pushes truncation one level later (depth 257) —
    // both are caught by the final `is_empty()` assertion below.

    /// M3 (`normalize_from_valid_values`): depth-cap truncation over a
    /// ~300-level-deep untyped-JSON `children` chain.
    #[test]
    fn test_bc_x_14_001_normalize_from_valid_values_depth_cap_truncates_children_beyond_256() {
        // Build innermost-out: "n0" is the leaf; each wrap adds one level of
        // nesting, so after the loop `node` is the OUTERMOST/top-level entry
        // "n299" (depth 0), with "n0" nested 299 levels beneath it.
        let mut node = serde_json::json!({"value": "n0", "label": "leaf"});
        for i in 1..300 {
            node = serde_json::json!({
                "value": format!("n{i}"),
                "label": format!("depth label {i}"),
                "children": [node],
            });
        }
        let values = vec![node];
        let result = normalize_from_valid_values(&values);

        // (a) the top-level entry is never dropped.
        assert_eq!(
            result.len(),
            1,
            "top-level entry must be preserved, never dropped"
        );
        assert_eq!(result[0].id, Some("n299".to_string()));

        // Walk down 256 levels (depth 0 -> depth 256) via `.children[0]`;
        // (b) this terminates cleanly (no stack overflow) by construction —
        // reaching this point at all is part of the proof.
        let mut cursor = &result[0];
        for depth in 0..256 {
            assert_eq!(
                cursor.children.len(),
                1,
                "entry at depth {depth} must still carry its one child — \
                 truncation starts strictly AT depth 256, not before"
            );
            cursor = &cursor.children[0];
        }
        // `cursor` is now the entry at depth 256, corresponding to source id
        // "n43" (299 - 256 = 43) — built, never dropped.
        assert_eq!(cursor.id, Some("n43".to_string()));
        // (c) but its children ARE truncated, even though the source JSON
        // nests further beneath it (down to "n0").
        assert!(
            cursor.children.is_empty(),
            "MAX_FIELD_OPTION_DEPTH must truncate children beyond depth 256 — \
             a mutant deleting the guard or flipping `>=` to `>` must fail \
             this test"
        );
    }

    /// M1/M2 (`normalize_from_allowed_values`): depth-cap truncation over a
    /// ~300-level-deep typed `AllowedValue` `children` chain.
    #[test]
    fn test_bc_x_14_001_normalize_from_allowed_values_depth_cap_truncates_children_beyond_256() {
        let mut node = AllowedValue {
            id: Some("n0".to_string()),
            value: Some("leaf".to_string()),
            name: None,
            children: Vec::new(),
        };
        for i in 1..300 {
            node = AllowedValue {
                id: Some(format!("n{i}")),
                value: Some(format!("depth label {i}")),
                name: None,
                children: vec![node],
            };
        }
        let values = vec![node];
        let result = normalize_from_allowed_values(&values);

        // (a) the top-level entry is never dropped.
        assert_eq!(
            result.len(),
            1,
            "top-level entry must be preserved, never dropped"
        );
        assert_eq!(result[0].id, Some("n299".to_string()));

        // (b) walk down 256 levels cleanly (no stack overflow).
        let mut cursor = &result[0];
        for depth in 0..256 {
            assert_eq!(
                cursor.children.len(),
                1,
                "entry at depth {depth} must still carry its one child — \
                 truncation starts strictly AT depth 256, not before"
            );
            cursor = &cursor.children[0];
        }
        assert_eq!(cursor.id, Some("n43".to_string()));
        // (c) children beyond depth 256 are truncated.
        assert!(
            cursor.children.is_empty(),
            "MAX_FIELD_OPTION_DEPTH must truncate children beyond depth 256 — \
             a mutant deleting the guard or flipping `>=` to `>` must fail \
             this test"
        );
    }

    // ── AC-012 / BC-X.14.002 / VP-580-007: --value client-side filter ──

    fn opt(id: Option<&str>, label: Option<&str>) -> FieldOption {
        FieldOption {
            id: id.map(String::from),
            label: label.map(String::from),
            children: Vec::new(),
        }
    }

    fn opt_with_children(
        id: Option<&str>,
        label: Option<&str>,
        children: Vec<FieldOption>,
    ) -> FieldOption {
        FieldOption {
            id: id.map(String::from),
            label: label.map(String::from),
            children,
        }
    }

    #[test]
    fn test_bc_x_14_002_value_filter_case_insensitive_id_or_label() {
        let options = vec![
            opt(Some("10001"), Some("Blocked")),
            opt(Some("10002"), Some("In Progress")),
            opt(Some("BLK-99"), Some("Something else")),
        ];
        let result = filter_options(&options, Some("bl"));
        // Matches via label substring ("Blocked" contains "bl" case-insensitively)
        // AND via id substring ("BLK-99").
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|o| o.id.as_deref() == Some("10001")));
        assert!(result.iter().any(|o| o.id.as_deref() == Some("BLK-99")));
    }

    #[test]
    fn test_bc_x_14_002_value_filter_cascading_child_retains_parent_context() {
        let options = vec![opt_with_children(
            Some("p1"),
            Some("Parent Unrelated"),
            vec![
                opt(Some("c1"), Some("Matching Child")),
                opt(Some("c2"), Some("Other Child")),
            ],
        )];
        let result = filter_options(&options, Some("matching"));
        assert_eq!(
            result.len(),
            1,
            "parent retained as context for the matching child"
        );
        assert_eq!(
            result[0].children.len(),
            1,
            "only the matching child is retained under the non-matching parent"
        );
        assert_eq!(result[0].children[0].id, Some("c1".to_string()));
    }

    #[test]
    fn test_bc_x_14_002_value_filter_parent_match_retains_all_children_unfiltered() {
        let options = vec![opt_with_children(
            Some("p1"),
            Some("MatchMe"),
            vec![
                opt(Some("c1"), Some("Child A")),
                opt(Some("c2"), Some("Child B")),
            ],
        )];
        let result = filter_options(&options, Some("matchme"));
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].children.len(),
            2,
            "a matching parent retains ALL its children unfiltered"
        );
    }

    #[test]
    fn test_bc_x_14_002_value_empty_string_is_identity_filter_including_degenerate() {
        let options = vec![opt(Some("10001"), Some("Anything")), opt(None, None)];
        let result = filter_options(&options, Some(""));
        assert_eq!(
            result.len(),
            options.len(),
            "--value \"\" is the identity filter, including a fully degenerate entry"
        );
    }

    #[test]
    fn test_bc_x_14_002_value_nonempty_excludes_fully_degenerate_entry() {
        let options = vec![opt(None, None)];
        let result = filter_options(&options, Some("anything"));
        assert!(
            result.is_empty(),
            "a non-empty --value substring has no match source on a fully degenerate entry"
        );
    }

    #[test]
    fn test_bc_x_14_002_value_none_returns_unchanged() {
        let options = vec![opt(Some("10001"), Some("A")), opt(Some("10002"), Some("B"))];
        let result = filter_options(&options, None);
        assert_eq!(
            result, options,
            "--value absent returns the full list unchanged"
        );
    }

    #[test]
    fn test_bc_x_14_002_value_partial_field_none_still_matches_remaining_field() {
        // id=None, label=Some — must still match via label.
        let options = vec![opt(None, Some("MatchLabel"))];
        let result = filter_options(&options, Some("matchlabel"));
        assert_eq!(result.len(), 1);

        // label=None, id=Some — must still match via id.
        let options2 = vec![opt(Some("MATCHID"), None)];
        let result2 = filter_options(&options2, Some("matchid"));
        assert_eq!(result2.len(), 1);
    }

    /// A recursive `FieldOption` strategy — bounded depth — for VP-580-007's
    /// property test below. `id`/`label` are independently `Option<String>`
    /// (including the fully-degenerate `{None, None}` combination) so the
    /// property test exercises the `Option<String>` reconciliation
    /// sub-points (g)/(h)/(i) alongside totality/monotonicity.
    fn arb_field_option() -> impl Strategy<Value = FieldOption> {
        let field = proptest::option::of("[a-zA-Z0-9]{0,6}");
        let leaf = (field.clone(), field.clone())
            .prop_map(|(id, label)| opt(id.as_deref(), label.as_deref()));
        leaf.prop_recursive(3, 16, 4, move |inner| {
            (
                field.clone(),
                field.clone(),
                proptest::collection::vec(inner, 0..3),
            )
                .prop_map(|(id, label, children)| FieldOption {
                    id,
                    label,
                    children,
                })
        })
    }

    proptest! {
        /// VP-580-007 (LOW, S-580-1 convergence pass): `filter_options` is a
        /// TOTAL function (never panics on a `None` field or a degenerate
        /// entry) and NARROWING at the top level (never grows the result
        /// beyond the input length) for an arbitrary substring needle.
        #[test]
        fn test_bc_x_14_002_filter_options_never_panics_and_is_narrowing(
            options in proptest::collection::vec(arb_field_option(), 0..6),
            needle in "[a-zA-Z0-9]{0,4}"
        ) {
            let result = filter_options(&options, Some(&needle));
            prop_assert!(
                result.len() <= options.len(),
                "filter_options must never grow the top-level result beyond the input"
            );
        }

        /// VP-580-007(i): `--value ""` and `--value` absent are IDENTICAL to
        /// each other, and both are the identity over an arbitrary fixture —
        /// including one containing a fully degenerate `{id: None, label:
        /// None}` entry (never-drop preserved through the filter).
        #[test]
        fn test_bc_x_14_002_filter_options_empty_string_and_none_are_identity(
            options in proptest::collection::vec(arb_field_option(), 0..6)
        ) {
            let with_none = filter_options(&options, None);
            let with_empty = filter_options(&options, Some(""));
            prop_assert_eq!(&with_none, &options);
            prop_assert_eq!(&with_empty, &options);
            prop_assert_eq!(with_none, with_empty);
        }
    }

    // ── AC-013 / BC-X.14.003 / VP-580-008: table rendering ──

    #[test]
    fn test_bc_x_14_003_render_option_rows_two_columns_and_cascading_indent() {
        let options = vec![opt_with_children(
            Some("p1"),
            Some("Parent"),
            vec![opt(Some("c1"), Some("Child"))],
        )];
        let rows = render_option_rows(&options);
        // One row for the parent + one row for the child = 2 rows total.
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(row.len(), 2, "exactly two columns: ID, Label");
        }
        assert_eq!(rows[0], vec!["p1".to_string(), "Parent".to_string()]);
        // EXACT indent pin (round-4 mutation-coverage sweep, LOW #4): the
        // child (depth 1) row's label column must be byte-for-byte
        // `"  Child"` — exactly `"  ".repeat(1)` prepended, no more, no
        // less. The prior assertion (`starts_with("  ") ||
        // rows[1][1] != "Child"`) was vacuously true for ANY label content
        // that wasn't the bare, unindented string "Child" — a mutant
        // reducing `"  ".repeat(depth)` to one space, three spaces, or a
        // tab all satisfied it. Byte-equality closes that gap.
        assert_eq!(
            rows[1],
            vec!["c1".to_string(), "  Child".to_string()],
            "child row must be exactly \"  \" (two spaces per level) + label, \
             not flattened, not under/over-indented; got {:?}",
            rows[1]
        );
    }

    /// EXACT indent pin, THREE levels deep (round-4 mutation-coverage sweep,
    /// LOW #4 companion): closes the residual gap the two-level test above
    /// cannot — depth-1 and depth-2 indentation are NOT numerically related
    /// by a single-space-per-level mutant (e.g. `"  ".repeat(depth)` mutated
    /// to a CONSTANT `"  "` for every depth would still pass the depth-1
    /// case but must fail here at depth 2), and a mutant that FLATTENS the
    /// tree (drops nesting, emits all rows at depth 0) is caught by the
    /// grandchild row's presence/indent at all.
    #[test]
    fn test_bc_x_14_003_render_option_rows_exact_indent_three_levels_deep() {
        let options = vec![opt_with_children(
            Some("p1"),
            Some("Parent"),
            vec![opt_with_children(
                Some("c1"),
                Some("Child"),
                vec![opt(Some("g1"), Some("Grandchild"))],
            )],
        )];
        let rows = render_option_rows(&options);
        assert_eq!(rows.len(), 3, "parent + child + grandchild = 3 rows");
        assert_eq!(rows[0], vec!["p1".to_string(), "Parent".to_string()]);
        assert_eq!(rows[1], vec!["c1".to_string(), "  Child".to_string()]);
        assert_eq!(
            rows[2],
            vec!["g1".to_string(), format!("{}Grandchild", "  ".repeat(2))],
            "depth-2 grandchild must be indented exactly twice the per-level \
             amount (four spaces), not the same as depth-1; got {:?}",
            rows[2]
        );
    }

    #[test]
    fn test_bc_x_14_003_render_option_rows_degenerate_glyphs() {
        let options = vec![opt(None, Some("Has Label")), opt(Some("10001"), None)];
        let rows = render_option_rows(&options);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0][0], NULL_GLYPH,
            "missing id renders the NULL_GLYPH (\"—\") in table mode"
        );
        assert_eq!(rows[0][1], "Has Label");
        assert_eq!(rows[1][0], "10001");
        assert_eq!(
            rows[1][1], UNNAMED_LABEL,
            "missing label renders the literal \"(unnamed)\" in table mode, never falls back to id"
        );
    }

    // ── BC-X.14.003 degenerate-entry JSON shape (no substitution, null stays null) ──

    #[test]
    fn test_bc_x_14_003_field_option_json_serializes_none_as_null_not_omitted() {
        let degenerate = opt(None, None);
        let value = serde_json::to_value(&degenerate).unwrap();
        let obj = value.as_object().unwrap();
        assert!(
            obj.contains_key("id"),
            "the `id` key must be present even when None (no #[serde(skip_serializing_if)])"
        );
        assert_eq!(obj.get("id"), Some(&serde_json::Value::Null));
        assert!(obj.contains_key("label"));
        assert_eq!(obj.get("label"), Some(&serde_json::Value::Null));
        assert_eq!(obj.get("children"), Some(&serde_json::Value::Array(vec![])));
    }

    // ── BC-X.14.004 / degrade_hint_for_schema classifier — SOLE-TRIGGER
    // pins for each `is_dynamic` disjunct (round-4 mutation-coverage sweep,
    // LOW #3). The integration tests in `tests/field_options.rs`
    // (userpicker, labels, group-picker, Approvers) each pile TWO+
    // classifying signals onto one fixture (e.g. `field_type == "user"`
    // together with a real `autoCompleteUrl`), so a mutant that deletes any
    // ONE disjunct from the `||` chain still passes every existing test —
    // the remaining disjunct(s) independently trip `is_dynamic`. These
    // unit tests call `degrade_hint_for_schema` directly (pure fn, no HTTP)
    // with exactly ONE signal present at a time, isolating each disjunct.

    /// `field_type == "user"` ALONE (no `autoCompleteUrl`, no
    /// userpicker-family `custom`, no `labels` `system`) must still trip
    /// `is_dynamic`.
    #[test]
    fn test_bc_x_14_004_is_dynamic_sole_trigger_field_type_user() {
        let hint = degrade_hint_for_schema(
            "Nominee",
            DegradeSchemaInfo {
                field_type: "user",
                custom: None,
                system: None,
                auto_complete_url: None,
            },
        );
        assert!(
            hint.contains("dynamic"),
            "field_type == \"user\" alone must trip is_dynamic; got: {hint}"
        );
        assert!(
            !hint.contains("autoCompleteUrl"),
            "no autoCompleteUrl was supplied — the hint must not fabricate one; got: {hint}"
        );
    }

    /// `system.eq_ignore_ascii_case("labels")` ALONE (no `autoCompleteUrl`,
    /// no user-type `field_type`, no userpicker/approver `custom`) must
    /// still trip `is_dynamic`.
    #[test]
    fn test_bc_x_14_004_is_dynamic_sole_trigger_system_labels() {
        let hint = degrade_hint_for_schema(
            "Labels",
            DegradeSchemaInfo {
                field_type: "array",
                custom: None,
                system: Some("labels"),
                auto_complete_url: None,
            },
        );
        assert!(
            hint.contains("dynamic"),
            "system == \"labels\" alone must trip is_dynamic; got: {hint}"
        );
    }

    /// `custom` containing `"userpicker"` ALONE must still trip
    /// `is_dynamic` — closes the disjunct a fixture piling on
    /// `field_type == "user"` too can never isolate.
    #[test]
    fn test_bc_x_14_004_is_dynamic_sole_trigger_custom_userpicker() {
        let hint = degrade_hint_for_schema(
            "Reviewers",
            DegradeSchemaInfo {
                field_type: "array",
                custom: Some("com.atlassian.jira.plugin.system.customfieldtypes:multiuserpicker"),
                system: None,
                auto_complete_url: None,
            },
        );
        assert!(
            hint.contains("dynamic"),
            "custom containing \"userpicker\" alone must trip is_dynamic; got: {hint}"
        );
    }

    /// `custom` containing `"approv"` ALONE must still trip `is_dynamic`.
    #[test]
    fn test_bc_x_14_004_is_dynamic_sole_trigger_custom_approv() {
        let hint = degrade_hint_for_schema(
            "Approvers",
            DegradeSchemaInfo {
                field_type: "array",
                custom: Some("com.atlassian.servicedesk.approvals-plugin:sd-approvals"),
                system: None,
                auto_complete_url: None,
            },
        );
        assert!(
            hint.contains("dynamic"),
            "custom containing \"approv\" alone must trip is_dynamic; got: {hint}"
        );
    }

    /// `auto_complete_url.is_some()` ALONE (none of the keyword/system
    /// signals present) must still trip `is_dynamic` — the "OTHER
    /// suggestion-backed fields" branch of BC-X.14.004's degrade table.
    #[test]
    fn test_bc_x_14_004_is_dynamic_sole_trigger_autocompleteurl_only() {
        let hint = degrade_hint_for_schema(
            "Reviewer Group",
            DegradeSchemaInfo {
                field_type: "group",
                custom: Some("com.atlassian.jira.plugin.system.customfieldtypes:grouppicker"),
                system: None,
                auto_complete_url: Some("https://example.atlassian.net/rest/api/1.0/groups/picker"),
            },
        );
        assert!(
            hint.contains("dynamic"),
            "autoCompleteUrl alone (no keyword/system match) must trip is_dynamic; got: {hint}"
        );
        assert!(hint.contains("https://example.atlassian.net/rest/api/1.0/groups/picker"));
    }

    /// Negative control: NONE of the `is_dynamic` disjuncts present ->
    /// the generic "no fixed value set" hint, not the dynamic one. Without
    /// this, a mutant that always returns `true` for `is_dynamic` (or
    /// `||`s in an unconditional `true`) would pass every sole-trigger test
    /// above.
    #[test]
    fn test_bc_x_14_004_is_dynamic_negative_control_generic_hint() {
        let hint = degrade_hint_for_schema(
            "Description",
            DegradeSchemaInfo {
                field_type: "string",
                custom: None,
                system: None,
                auto_complete_url: None,
            },
        );
        assert!(
            hint.contains("no fixed value set"),
            "with zero dynamic signals, the generic hint must fire; got: {hint}"
        );
        assert!(!hint.contains("dynamic"));
    }
}
