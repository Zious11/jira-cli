use anyhow::Result;

use crate::api::client::JiraClient;
use crate::api::jsm::requests::JsmRequestBuilder;
use crate::api::jsm::servicedesks;
use crate::cache;
use crate::cli::OutputFormat;
use crate::config::Config;
use crate::error::{API_TOKEN_EXPIRY_HINT, JrError};
use crate::output;
use crate::partial_match::{self, MatchResult};

use super::create::{FieldValueKind, FieldValueSpec, parse_field_kv};
use super::helpers;

/// Argument bundle for `handle_jsm_create`.
///
/// Reduces argument count on `handle_jsm_create` to satisfy `clippy::too_many_arguments`
/// (CLAUDE.md policy: refactor rather than `#[allow]`).
///
/// # Field policy
///
/// `IssueCommand::Create` carries 16+ flags. The JSM dispatch path uses a subset.
/// Each `Create` flag falls into one of three categories:
///
/// **Pass-through to JSM (used in request body):**
/// - `project`, `request_type`, `summary`, `description`, `description_stdin`,
///   `priority`, `labels`, `markdown`, `on_behalf_of`, `field_pairs`
///
/// **Ignored with stderr warning (carried for step-5 warning-emission at
/// canonical step 5 inside `handle_jsm_create` — AFTER `require_service_desk`
/// returns `Ok`, before request-type resolution — per BC-3.8.010 + BC-3.8.011):**
/// - `issue_type` (`--type`): JSM request types replace it
/// - `team` (`--team`): not in JSM request schema
/// - `points` (`--points`): not in JSM request schema
/// - `parent` (`--parent`): JSM requests cannot be sub-tasks
/// - `to` (`--to`): superseded by `--on-behalf-of` (raiseOnBehalfOf)
/// - `account_id` (`--account-id`): superseded by `--on-behalf-of`
///
/// **No-op on JSM (silently dropped):**
/// - (none currently — every Create flag is either passed or warned)
///
/// When adding a new `Create` flag, decide which category it belongs to and add it
/// to this list to keep future maintainers from re-discovering the matrix.
pub(super) struct JsmCreateArgs {
    // Pass-through to JSM request body
    pub(super) project: Option<String>,
    pub(super) request_type: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) description: Option<String>,
    pub(super) description_stdin: bool,
    pub(super) priority: Option<String>,
    pub(super) labels: Vec<String>,
    pub(super) markdown: bool,
    pub(super) on_behalf_of: Option<String>,
    pub(super) field_pairs: Vec<String>,
    // Platform-only flags carried for step-5 warning emission (BC-3.8.010, BC-3.8.011).
    // Warnings fire AFTER `require_service_desk` returns Ok — suppressed on non-JSM projects.
    pub(super) issue_type: Option<String>,
    pub(super) team: Option<String>,
    pub(super) points: Option<f64>,
    pub(super) parent: Option<String>,
    pub(super) to: Option<String>,
    pub(super) account_id: Option<String>,
}

/// Orchestrate a JSM customer-request creation.
///
/// Called by [`super::create::handle_create`] when `--request-type` is present. Never called
/// when `--request-type` is absent (platform path is the fall-through).
///
/// Steps (BC-3.8.001..017) — Canonical Guard Ordering:
/// 0. Resolve project key (BC-3.8.002) — may exit 64, no HTTP.
/// 1. Empty/whitespace-only `--request-type` guard (BC-3.8.016) — exit 64, no HTTP.
/// 2. `--markdown` + `--field description=` conflict guard (BC-3.8.017) — exit 64, no HTTP.
/// 3. `--markdown`-requires-`--description` guard — exit 64, no HTTP.
/// 4. Resolve service desk ID via [`servicedesks::require_service_desk`]
///    (label `` "`jr issue create --request-type` requires" ``) — FIRST HTTP call.
/// 5. Emit stderr warnings for platform-only flags (`--type`, `--team`, `--points`,
///    `--parent`, `--to`, `--account-id`) — AFTER `require_service_desk` returns `Ok`,
///    before request-type resolution (BC-3.8.010, BC-3.8.011, single-site F-02).
///    On a non-JSM project, `require_service_desk` fails at step 4 → step 5 is never
///    reached → warnings are suppressed (not emitted for non-JSM projects).
/// 6. Resolve `request_type_arg`: if all-digits → use as-is (numeric bypass,
///    BC-3.8.004); else → read cache / fetch via `list_request_types` /
///    `partial_match`. Ambiguous or missing → exit 64.
/// 7. Build `requestFieldValues` from `--summary`, `--description` (ADF),
///    `--priority`, `--label`, `--field` via [`parse_field_kv`].
/// 8. Build body via [`JsmRequestBuilder`].
/// 9. POST via [`JiraClient::create_jsm_request`].
///    Emit `{"key": "<issue_key>"}` on stdout (`--output json` shape per AC-015).
pub(super) async fn handle_jsm_create(
    client: &JiraClient,
    config: &Config,
    output_format: &OutputFormat,
    project_override: Option<&str>,
    no_input: bool,
    args: JsmCreateArgs,
) -> Result<()> {
    let JsmCreateArgs {
        project,
        request_type,
        summary,
        description,
        description_stdin,
        priority,
        labels,
        markdown,
        on_behalf_of,
        field_pairs,
        issue_type,
        team,
        points,
        parent,
        to,
        account_id,
    } = args;

    // Resolve the request_type arg — we know it's Some because this function is only
    // called when request_type.is_some().
    let request_type_arg = request_type.expect("handle_jsm_create called without --request-type");

    // Step 0: Resolve project key (BC-3.8.002).
    let project_key = project
        .or_else(|| config.project_key(project_override))
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Project key").ok()
            }
        })
        .ok_or_else(|| {
            JrError::UserError(
                "Project key is required for JSM request creation. \
                 Use --project or configure .jr.toml. \
                 Run \"jr project list\" to see available JSM projects."
                    .into(),
            )
        })?;

    // Step 1: Empty/whitespace-only --request-type guard (BC-3.8.016).
    // Fires before require_service_desk (step 4) — zero HTTP on this path.
    // Guard evaluates trim().is_empty() to cover both "" and "   " inputs (EC-3.8.016-1).
    if request_type_arg.trim().is_empty() {
        return Err(JrError::UserError("request type cannot be empty".into()).into());
    }

    // Step 2: --markdown + --field description= conflict guard (BC-3.8.017).
    // Fires before require_service_desk (step 4) — zero HTTP on this path.
    // Key match: raw substring before the first '=' must be EXACTLY "description"
    // (case-SENSITIVE, no trim — mirrors parse_field_kv extraction).
    if markdown {
        let has_description_field = field_pairs.iter().any(|pair| {
            pair.find('=')
                .is_some_and(|pos| &pair[..pos] == "description")
        });
        if has_description_field {
            return Err(JrError::UserError(
                "`--field description=...` cannot be combined with `--markdown`: \
                 it would overwrite the ADF description with plain text, \
                 desyncing `isAdfRequest: true` with a plain-string description value \
                 (may result in a JSM 400 error or silently dropped ADF formatting). \
                 Pass `--description` with `--markdown`, or omit `--markdown`."
                    .into(),
            )
            .into());
        }
    }

    // Step 3: M-01 (adversary pass-02-retry): --markdown requires a description
    // source on the JSM path. No platform-path equivalent exists (S-639-1,
    // EC-3.8.012-5) — on the platform path, --markdown with no description is
    // simply a no-op (the markdown flag is only consulted when desc_text is
    // Some). This function only runs when --request-type is present (the
    // caller's dispatch fork routes here); when --field/--on-behalf-of are
    // supplied WITHOUT --request-type, BC-3.8.012/013's pre-flight guard in
    // create.rs::handle_create fires on the platform path instead, so this
    // JSM-specific --markdown guard is structurally unreachable without
    // --request-type routing.
    if markdown && description.is_none() && !description_stdin {
        return Err(JrError::UserError(
            "--markdown requires --description or --description-stdin to take effect. \
             Pass a description alongside --markdown, or omit --markdown."
                .into(),
        )
        .into());
    }

    // Step 4: Resolve service desk ID — errors with BC-X.8.004 message for non-JSM
    // projects (BC-3.8.002). Call-site label "`jr issue create --request-type` requires".
    let service_desk_id = servicedesks::require_service_desk(
        client,
        &project_key,
        "`jr issue create --request-type` requires",
    )
    .await?;

    // Step 5: Emit stderr warnings for platform-only flags (BC-3.8.010, BC-3.8.011).
    // Fires AFTER require_service_desk returns Ok (single-site F-02).
    // On a non-JSM project, require_service_desk fails at step 4 — this step is never
    // reached, so warnings are suppressed for non-JSM projects.
    if issue_type.is_some() {
        eprintln!(
            "warning: --type is ignored when --request-type is set; request type encodes the issue type"
        );
    }
    if team.is_some() {
        eprintln!(
            "warning: --team is ignored when --request-type is set; teams are managed by the request type's workflow"
        );
    }
    if points.is_some() {
        eprintln!(
            "warning: --points is ignored when --request-type is set; story points are not part of JSM request schema"
        );
    }
    if parent.is_some() {
        eprintln!(
            "warning: --parent is ignored when --request-type is set; JSM requests cannot be sub-tasks"
        );
    }
    if to.is_some() {
        eprintln!(
            "warning: --to is ignored when --request-type is set; use --on-behalf-of to set the requester"
        );
    }
    if account_id.is_some() {
        eprintln!(
            "warning: --account-id is ignored when --request-type is set; use --on-behalf-of to set the requester"
        );
    }

    let profile = &config.active_profile_name;

    // Resolve request type ID (BC-3.8.003, BC-3.8.004).
    let request_type_id = if request_type_arg.chars().all(|c| c.is_ascii_digit()) {
        // Numeric bypass — use directly without list endpoint call (BC-3.8.004).
        request_type_arg.clone()
    } else {
        // Name resolution: cache → API → partial_match (BC-3.8.003).
        resolve_jsm_request_type_id(
            &request_type_arg,
            &service_desk_id,
            &project_key,
            profile,
            client,
        )
        .await?
    };

    // Resolve summary (BC-3.8.005).
    let summary_text = summary
        .or_else(|| {
            if no_input {
                None
            } else {
                helpers::prompt_input("Summary").ok()
            }
        })
        .ok_or_else(|| {
            JrError::UserError(
                "summary is required for JSM request submission. Use --summary.".into(),
            )
        })?;

    // Resolve description. spawn_blocking isolates the blocking stdin read from the
    // tokio runtime so later async work isn't starved while waiting on piped input.
    let desc_text = if description_stdin {
        let buf = tokio::task::spawn_blocking(|| {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            Ok::<_, std::io::Error>(buf)
        })
        .await??;
        Some(buf)
    } else {
        description
    };

    // Parse --field NAME=VALUE pairs (BC-3.8.008).
    let parsed_field_pairs = parse_field_kv(&field_pairs)?;

    // S-578-3 (BC-3.8.008 amendment, AC-006): resolve `:asset` hints' L2-side
    // workspace-id segment BEFORE `JsmRequestBuilder::build()` ever sees the
    // value — mirrors `edit.rs`/`field_resolve.rs`'s L2-resolves/L4-wraps
    // split for the platform path (S-578-2, ADR-0019 §2 Architecture
    // Compliance Rules 1-3). `build()`'s `Some(Asset)` match arm performs
    // PURE array-wrapping only; it is never given an unresolved bare
    // `:asset` value.
    let mut extra_fields: std::collections::HashMap<String, FieldValueSpec> =
        std::collections::HashMap::with_capacity(parsed_field_pairs.len());
    for (name, spec) in parsed_field_pairs {
        if spec.kind == Some(FieldValueKind::Asset) {
            let resolved = resolve_asset_field_l2(client, &spec.value).await?;
            extra_fields.insert(name, resolved);
        } else {
            extra_fields.insert(name, spec);
        }
    }

    // Build the POST body (BC-3.8.005..009).
    let body = JsmRequestBuilder {
        service_desk_id: &service_desk_id,
        request_type_id: &request_type_id,
        summary: &summary_text,
        description: desc_text.as_deref(),
        markdown,
        priority: priority.as_deref(),
        labels: &labels,
        on_behalf_of: on_behalf_of.as_deref(),
        extra_fields: &extra_fields,
    }
    .build()?;

    // POST to /rest/servicedeskapi/request (BC-3.8.001).
    //
    // On 401, gate error-hint dispatch on auth scheme (BC-3.8.014 / BC-3.8.015):
    //
    //   Basic-auth (is_oauth_auth() == false): REWRITE any incoming variant
    //     (NotAuthenticated or InsufficientScope) to NotAuthenticated with the
    //     API-token-expiry hint. The InsufficientScope rewrite is required because
    //     the `"scope does not match"` body check in `send_inner` fires BEFORE the
    //     Bearer-scheme guard, so a Basic-auth 401 with a scope-mismatch body lands
    //     as InsufficientScope without the rewrite — exposing misleading OAuth language
    //     to Basic users.
    //
    //   OAuth (is_oauth_auth() == true): preserve existing pre-#384 behavior
    //     unchanged for both arms — BOTH produce the write:servicedesk-request hint
    //     (BC-3.8.015 / H-NEW-JSM-RT-003). The NotAuthenticated arm already rewrites
    //     to inject the hint; the InsufficientScope arm augments the message with
    //     scope-specific guidance.
    let is_oauth = client.is_oauth_auth();
    let created =
        client
            .create_jsm_request(body)
            .await
            .map_err(|e| match e.downcast::<JrError>() {
                Ok(JrError::NotAuthenticated { .. }) => {
                    if is_oauth {
                        // OAuth: preserve existing behavior (write:servicedesk-request hint).
                        anyhow::anyhow!(JrError::NotAuthenticated {
                            hint: "The `write:servicedesk-request` OAuth scope may be missing. \
                           Run `jr auth refresh` or `jr auth login` to re-consent with \
                           the updated scope."
                                .to_string(),
                        })
                    } else {
                        // Basic: API-token-expiry hint (BC-3.8.014 postcondition 1).
                        anyhow::anyhow!(JrError::NotAuthenticated {
                            hint: API_TOKEN_EXPIRY_HINT.to_string(),
                        })
                    }
                }
                Ok(JrError::InsufficientScope { message, .. }) => {
                    if is_oauth {
                        // OAuth: augment with scope-specific guidance (BC-3.8.015 / C-01).
                        anyhow::anyhow!(JrError::InsufficientScope {
                            message: format!(
                                "{message} (`jr issue create --request-type` requires the \
                             `write:servicedesk-request` OAuth scope. \
                             Run `jr auth refresh` to refresh, or `jr auth login` to re-authorize \
                             with updated scopes.)"
                            ),
                            required_scope: Some("write:servicedesk-request".to_string()),
                        })
                    } else {
                        // Basic: rewrite InsufficientScope → NotAuthenticated with
                        // API-token-expiry hint (BC-3.8.014 postcondition 2).
                        // The `"scope does not match"` body check in `send_inner` fires before
                        // the Bearer-scheme guard, so a Basic-auth scope-mismatch body arrives
                        // as InsufficientScope; rewriting here prevents misleading OAuth language
                        // for Basic users.
                        anyhow::anyhow!(JrError::NotAuthenticated {
                            hint: API_TOKEN_EXPIRY_HINT.to_string(),
                        })
                    }
                }
                Ok(other) => anyhow::anyhow!(other),
                Err(other) => other,
            })?;

    // Emit output (AC-015, BC-3.8.001).
    let issue_key = &created.issue_key;
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&serde_json::json!({"key": issue_key}))?
            );
        }
        OutputFormat::Table => {
            output::print_success(&format!("Created request {issue_key}"));
        }
    }

    Ok(())
}

/// Resolves the `:asset` hint's L2-side workspace-id segment before
/// `JsmRequestBuilder::build()` sees it (S-578-3, AC-006) — mirrors S-578-2's
/// L2-resolves/L4-wraps split (`field_resolve.rs::compose_asset_hint`) for
/// this (JSM create) call site (Architecture Compliance Rule 2/3): an
/// explicit `WORKSPACE:OBJECTID` value (a `:` present) composes directly
/// with NO cache lookup; a bare `<objectId>` value (no `:`) calls
/// [`crate::api::assets::workspace::get_or_fetch_workspace_id`] first.
/// `get_or_fetch_workspace_id` is called AT MOST ONCE per invocation
/// (mirrors the platform-path invariant).
///
/// Returns a [`FieldValueSpec`] with `kind: Some(FieldValueKind::Asset)` and
/// `value` set to the fully-qualified `WORKSPACE:OBJECTID` pair — this is
/// the ONLY shape `JsmRequestBuilder::build()`'s `Some(Asset)` arm
/// (`compose_asset_wire`) ever receives; it never sees an unresolved bare
/// value.
///
/// # Malformed-shape errors (BC-3.4.031 EC-2/EC-3, BC-3.8.008 shared malformed-hint
/// catalog, adversary Pass-1 HIGH ADV-S578-3-P1-001)
///
/// Mirrors `field_resolve.rs::compose_asset_hint`'s validation EXACTLY — same
/// checks, same precedence, same canonical message substrings — so a malformed
/// `:asset` value fires ZERO workspace GET and ZERO POST on the JSM path,
/// matching the platform path's behavior. Checked in this order (EC-2c's
/// empty-workspace-segment check MUST run BEFORE the objectId-segment checks —
/// `:asset=:` triggers EC-2c, never EC-2b):
/// 1. Empty `VALUE` → "asset reference cannot be empty" (EC-2a).
/// 2. `:` present, workspace segment empty → "workspace segment cannot be
///    empty…" (EC-2c).
/// 3. `:` present, remainder contains a SECOND `:` → "unexpected extra
///    ':'…" (EC-2d).
/// 4. objectId segment (ASCII `[0-9]+` only, NOT Unicode `\d`) empty or
///    non-numeric → "objectId must be numeric" (EC-2b/EC-3).
///
/// # Errors
///
/// Propagates `get_or_fetch_workspace_id`'s cold-cache failure taxonomy
/// (BC-3.4.030, VP-578-022, AC-007): 403/404 → "Assets is not available…";
/// 200 + zero entries → "No Assets workspace found…"; 401/5xx/network →
/// standard `JrError` mappings.
async fn resolve_asset_field_l2(client: &JiraClient, value: &str) -> Result<FieldValueSpec> {
    if value.is_empty() {
        return Err(JrError::UserError(
            "asset reference cannot be empty. Use --field NAME:asset=OBJECTID (workspace \
             id resolved from cache) or --field NAME:asset=WORKSPACE:OBJECTID."
                .into(),
        )
        .into());
    }

    let resolved_value = match value.split_once(':') {
        Some((workspace_id, object_id)) => {
            if workspace_id.is_empty() {
                return Err(JrError::UserError(
                    "workspace segment cannot be empty when ':' is present; omit the \
                     workspace prefix entirely to use the cached workspace id."
                        .into(),
                )
                .into());
            }
            if object_id.contains(':') {
                return Err(JrError::UserError(format!(
                    "unexpected extra ':' in :asset value '{value}' — expected \
                     WORKSPACE:OBJECTID."
                ))
                .into());
            }
            if object_id.is_empty() || !object_id.chars().all(|c| c.is_ascii_digit()) {
                return Err(JrError::UserError(format!(
                    "objectId must be numeric (ASCII digits only); got '{object_id}'."
                ))
                .into());
            }
            // Explicit WORKSPACE:OBJECTID — compose directly, no cache lookup.
            format!("{workspace_id}:{object_id}")
        }
        None => {
            if !value.chars().all(|c| c.is_ascii_digit()) {
                return Err(JrError::UserError(format!(
                    "objectId must be numeric (ASCII digits only); got '{value}'."
                ))
                .into());
            }
            // Bare <objectId> — resolve workspace id via cache/API first.
            let workspace_id =
                crate::api::assets::workspace::get_or_fetch_workspace_id(client).await?;
            format!("{workspace_id}:{value}")
        }
    };
    Ok(FieldValueSpec {
        kind: Some(FieldValueKind::Asset),
        value: resolved_value,
    })
}

/// Resolve a request type name to its ID for the JSM create path.
///
/// Mirrors `cli/requesttype.rs::resolve_request_type_id` — cache → fetch → `partial_match`.
async fn resolve_jsm_request_type_id(
    name: &str,
    service_desk_id: &str,
    project_key: &str,
    profile: &str,
    client: &JiraClient,
) -> Result<String> {
    let types = match cache::read_request_type_cache(profile, service_desk_id)? {
        Some(cached) => cached,
        None => {
            let fetched = client.list_request_types(service_desk_id, None).await?;
            // `write_request_type_cache` is a best-effort writer per CLAUDE.md gotcha —
            // it swallows IO errors via eprintln and returns Ok(()). Use `let _` to make
            // the no-propagation intent explicit (the `?` would be dead code).
            let _ = cache::write_request_type_cache(profile, service_desk_id, &fetched);
            fetched
        }
    };

    let names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();

    match partial_match::partial_match(name, &names) {
        MatchResult::Exact(matched_name) => {
            let id = types
                .iter()
                .find(|t| t.name == matched_name)
                .map(|t| t.id.clone())
                .expect("partial_match::Exact match must exist in types");
            Ok(id)
        }
        MatchResult::ExactMultiple(matched_name) => {
            let matched_lower = matched_name.to_lowercase();
            let ids: Vec<String> = types
                .iter()
                .filter(|t| t.name.to_lowercase() == matched_lower)
                .map(|t| t.id.clone())
                .collect();
            Err(JrError::UserError(format!(
                "Multiple request types named \"{matched_name}\" found (IDs: {}). \
                 Pass the numeric ID directly.",
                ids.join(", ")
            ))
            .into())
        }
        MatchResult::Ambiguous(matches) => Err(JrError::UserError(format!(
            "Ambiguous request type \"{name}\" matches: {}. \
             Run `jr requesttype list --project {project_key}` to see all request types.",
            matches
                .iter()
                .map(|m| format!("\"{m}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        MatchResult::None(_) => {
            let cache_path =
                cache::cache_dir(profile).join(format!("request_types_{service_desk_id}.json"));
            Err(JrError::UserError(format!(
                "Request type \"{name}\" not found. \
                 Run `jr requesttype list --project {project_key}` to see all request types, \
                 or delete the cache file at {} \
                 if a recent admin change is suspected.",
                cache_path.display()
            ))
            .into())
        }
    }
}
