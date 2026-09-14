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
use super::mentions;

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
    // S-cycle5-mention-resolution-wiring (AC-013/AC-014/AC-015): `--no-mentions`
    // opt-out, threaded through to skip mention resolution entirely and use
    // `adf::markdown_to_adf_no_mentions` instead.
    pub(super) no_mentions: bool,
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
        no_mentions,
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

    // S-cycle12-jsm-adf-autoconvert AC-003/004/007/008/013/015(a) (ADR-0024
    // DQ-6 Option (b)): ADF detection/conversion resolution pass over bare
    // --field extra fields. Gated on >=1 bare (kind.is_none()) pair per
    // AC-015(a) — a no-`--field` create or a hinted-only create issues ZERO
    // GET .../requesttype/{id}/field calls.
    let has_bare_field_pair = extra_fields.values().any(|spec| spec.kind.is_none());
    let JsmAdfFieldResolution {
        extra_fields,
        resolved_adf_values,
        is_adf_request: extra_fields_is_adf_request,
    } = if has_bare_field_pair {
        resolve_jsm_adf_extra_fields(
            client,
            profile,
            &service_desk_id,
            &request_type_id,
            extra_fields,
        )
        .await
    } else {
        JsmAdfFieldResolution {
            extra_fields,
            resolved_adf_values: std::collections::BTreeMap::new(),
            is_adf_request: false,
        }
    };

    // S-cycle5-mention-resolution-wiring (AC-013): resolve mentions BEFORE
    // constructing `JsmRequestBuilder` and calling its synchronous,
    // effect-free `.build()` — the resolution result is threaded in as
    // plain data (`build()` gains no `async`/`Client` capability).
    // `--no-mentions` skips resolution entirely (AC-014/AC-015): zero
    // resolver HTTP calls, `build()` uses `markdown_to_adf_no_mentions`.
    let mention_resolutions = if markdown && desc_text.is_some() && !no_mentions {
        Some(
            mentions::resolve_mentions(client, desc_text.as_deref().unwrap_or(""), no_input)
                .await?,
        )
    } else {
        None
    };

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
        no_mentions,
        mentions: mention_resolutions.as_ref(),
        extra_fields: &extra_fields,
        resolved_adf_values: &resolved_adf_values,
        is_adf_request: extra_fields_is_adf_request,
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

/// Result of the ADF detection/conversion resolution pass over bare
/// `--field` extra fields on the JSM create path
/// (S-cycle12-jsm-adf-autoconvert AC-001/004/007/008/013;
/// BC-3.8.019/020/021/022).
///
/// # DQ-6 type/signature decision (ADR-0024 §Decision "DQ-6", AC-001) — Option (b) chosen
///
/// ADR-0024 leaves the DQ-6 plumbing shape open between (a) widening
/// [`FieldValueSpec::value`] from `String` to `serde_json::Value`, or (b) a
/// parallel `resolved_adf_values` map alongside `extra_fields`. This story
/// picks **Option (b)**, for two reasons:
///
/// 1. `FieldValueSpec` is a SHARED type with three call sites: the platform
///    create path (`create.rs`), the platform edit path (`edit.rs`), and
///    this JSM create path (`jsm_create.rs`). Widening `value` to
///    `serde_json::Value` would ripple into `field_resolve.rs`'s
///    `dispatch_field_value` and every hint-kind composer on the platform
///    paths — `field_resolve.rs` is VERIFY ONLY for this story (no new
///    logic), so a widening change there is out of scope.
/// 2. Option (b) confines the new plumbing entirely to `jsm_create.rs` +
///    `JsmRequestBuilder` (`api/jsm/requests.rs`) — zero blast radius on the
///    platform paths, zero risk of an accidental behavior change to
///    `dispatch_field_value`'s type dispatch or any hinted-kind composer.
///
/// `extra_fields` below stays a `HashMap<String, FieldValueSpec>` —
/// `FieldValueSpec.value` stays `String`, unchanged from pre-cycle-012.
/// ADF-converted values live exclusively in `resolved_adf_values`.
pub(super) struct JsmAdfFieldResolution {
    /// The extra-fields map with empty, bare (`kind.is_none()`), ADF-backed
    /// entries REMOVED (AC-007 empty-omit guard). Every other entry
    /// (non-ADF-backed, hinted, or non-empty-and-already-ADF-converted-into-
    /// `resolved_adf_values`) is passed through unchanged from the caller's
    /// input map. `JsmRequestBuilder::build()`'s pre-existing string-wrap
    /// loop over this map is still correct for every entry NOT also present
    /// as a key in `resolved_adf_values`.
    pub(super) extra_fields: std::collections::HashMap<String, FieldValueSpec>,
    /// ADF document objects (`{"type":"doc","version":1,"content":[...]}`)
    /// for non-empty bare ADF-backed extra fields, keyed by field name.
    /// `JsmRequestBuilder::build()` (AC-001/013) must insert these into
    /// `requestFieldValues`, superseding any string-wrap for the same key —
    /// `build()` NEVER derives ADF-ness by inspecting value shapes.
    pub(super) resolved_adf_values: std::collections::BTreeMap<String, serde_json::Value>,
    /// Accumulated `isAdfRequest` contribution from THIS resolution pass —
    /// `true` iff at least one extra field was ADF-converted (BC-3.8.022).
    /// `JsmRequestBuilder::build()` must OR this with its own
    /// `self.description`-derived flag; it is never recomputed by inspecting
    /// `requestFieldValues` entries (AC-013 mutant-kill invariant — see
    /// VP-FIELD-ADF-004 Axis (c) I-2, the hinted-`:id`/`:name`-object
    /// discriminator-precision regression).
    pub(super) is_adf_request: bool,
}

/// Pure gate: does the ADF empty-omit guard (AC-007) apply to this `--field`
/// token's kind? (VP-FIELD-ADF-003 Axis D — JSM sub-case, AC-009.)
///
/// Fires ONLY for the bare form (`kind.is_none()`) — a hinted `:option`/
/// `:id`/`:name`/`:asset` value bypasses ADF auto-detection entirely and
/// routes to its hint composer regardless of emptiness (ADR-0024 "Hint kinds
/// opt out of ADF on both platform and JSM paths" invariant). Callers must
/// additionally check `is_adf_field_value(...)` and `value.trim().is_empty()`
/// — this function is ONLY the `kind.is_none()` half of the three-part gate,
/// extracted so it can be unit-tested independent of RT-field metadata
/// (F4 extraction obligation, AC-009).
///
/// # GREEN-BY-DESIGN
///
/// Correct-by-construction: a single method call with no branching syntax
/// (`if`/`match`/`?`/`unwrap`), no I/O, no calls to non-trivial helpers, one
/// line. See the stub commit report's GREEN-BY-DESIGN table.
pub(super) fn jsm_adf_empty_omit_guard_applies(kind: Option<FieldValueKind>) -> bool {
    kind.is_none()
}

/// Fetch RT-field metadata (cache-first, fail-open) and resolve ADF
/// detection/conversion for bare `--field` extra fields
/// (S-cycle12-jsm-adf-autoconvert AC-003, AC-004, AC-007, AC-008;
/// BC-3.8.019/020/021/022).
///
/// Effectful (L2/L3): calls [`JiraClient::get_request_type_fields`]
/// (`api/jsm/request_types.rs`) via [`crate::cache::read_request_type_fields_cache`]
/// / [`crate::cache::write_request_type_fields_cache`] (cache-first, 7-day
/// TTL, keyed on `(profile, service_desk_id, request_type_id)`); emits a
/// single global stderr `warning:` line on fail-open (AC-008(a)).
///
/// Only invoked by [`handle_jsm_create`] when `>=1` extra-field pair is bare
/// (`kind.is_none()`) — the "GET fires IFF >=1 bare pair" contract
/// (AC-015(a)) is enforced at the call site, not inside this function.
///
/// # Fail-open contract (BC-3.8.019 EC-3.8.019-2, AC-008)
///
/// On ANY fetch failure (network error, non-200 status including
/// 401/403/404/500, deserialization error, or a cache-miss-with-no-network):
/// emit exactly ONE global `"warning: …"` stderr line (never per-field,
/// never mentioning `write:servicedesk-request` — AC-015(c), that hint is
/// reserved for the create POST itself), degrade ALL bare `--field` values
/// (including empty ones) to `Value::String(value)` verbatim, and NEVER exit
/// 64 — this function is infallible by design (no `Result` return type).
///
/// # ADF detection contract (AC-002, ADR-0024 canonical `jiraSchema` contract)
///
/// For each bare extra field whose NAME matches a fetched
/// `RequestTypeField.field_id`, call
/// `field_resolve::is_adf_field_value(&rt_field.jira_schema)` — passing the
/// INNER schema block DIRECTLY. `rt_field.jira_schema` IS the inner schema;
/// do NOT wrap it in another `json!({"jiraSchema": ...})` (the
/// double-nesting anti-pattern AC-002 exists to prevent).
async fn resolve_jsm_adf_extra_fields(
    client: &JiraClient,
    profile: &crate::profile::Profile,
    service_desk_id: &str,
    request_type_id: &str,
    extra_fields: std::collections::HashMap<String, FieldValueSpec>,
) -> JsmAdfFieldResolution {
    let Some(fields_response) =
        fetch_request_type_fields_cached(client, profile, service_desk_id, request_type_id).await
    else {
        // AC-008: fail-open — exactly one global warning; ALL bare --field values
        // degrade to Value::String (this is already `extra_fields`'s untouched
        // shape — build()'s pre-existing None-kind string-wrap arm handles it),
        // isAdfRequest stays ABSENT, and the create is NEVER aborted here.
        eprintln!(
            "warning: could not fetch request type fields; --field values will be sent as \
             plain strings"
        );
        return JsmAdfFieldResolution {
            extra_fields,
            resolved_adf_values: std::collections::BTreeMap::new(),
            is_adf_request: false,
        };
    };

    let mut output_extra_fields = std::collections::HashMap::with_capacity(extra_fields.len());
    let mut resolved_adf_values = std::collections::BTreeMap::new();
    let mut is_adf_request = false;

    for (name, spec) in extra_fields {
        // Hinted kinds bypass ADF detection entirely (AC-009, ADR-0024 "Hint
        // kinds opt out of ADF" invariant) — pass through unchanged.
        if !jsm_adf_empty_omit_guard_applies(spec.kind) {
            output_extra_fields.insert(name, spec);
            continue;
        }

        let rt_field = fields_response
            .request_type_fields
            .iter()
            .find(|f| f.field_id == name);

        // I-1 rule: NAME absent from a successfully-fetched RT field list falls
        // through verbatim — no warning, unchanged string-wrap (AC-005 OBS-1 item 3).
        let Some(rt_field) = rt_field else {
            output_extra_fields.insert(name, spec);
            continue;
        };

        // AC-002: pass the inner schema block DIRECTLY — no double-nesting.
        if !crate::cli::issue::field_resolve::is_adf_field_value(&rt_field.jira_schema) {
            output_extra_fields.insert(name, spec);
            continue;
        }

        if spec.value.trim().is_empty() {
            // AC-007: omit entirely — do NOT set is_adf_request, do NOT
            // re-insert into output_extra_fields (JSM create-omit semantics,
            // distinct from platform edit's clear-doc).
            continue;
        }

        resolved_adf_values.insert(name, crate::adf::text_to_adf(&spec.value));
        is_adf_request = true;
    }

    JsmAdfFieldResolution {
        extra_fields: output_extra_fields,
        resolved_adf_values,
        is_adf_request,
    }
}

/// Cache-first, network-fallback fetch of a request type's field metadata
/// (AC-003, AC-015(b)). Returns `None` on ANY failure — network error,
/// non-200 status, or deserialization error — so the caller can fail open
/// (AC-008). Never propagates an error and never panics.
async fn fetch_request_type_fields_cached(
    client: &JiraClient,
    profile: &crate::profile::Profile,
    service_desk_id: &str,
    request_type_id: &str,
) -> Option<crate::types::jsm::RequestTypeFieldsResponse> {
    if let Ok(Some(cached)) =
        cache::read_request_type_fields_cache(profile, service_desk_id, request_type_id)
    {
        return Some(cached);
    }

    match client
        .get_request_type_fields(service_desk_id, request_type_id)
        .await
    {
        Ok(fetched) => {
            // Best-effort writer per CLAUDE.md gotcha — swallows IO errors.
            let _ = cache::write_request_type_fields_cache(
                profile,
                service_desk_id,
                request_type_id,
                &fetched,
            );
            Some(fetched)
        }
        Err(_) => None,
    }
}

/// Resolve a request type name to its ID for the JSM create path.
///
/// Mirrors `cli/requesttype.rs::resolve_request_type_id` — cache → fetch → `partial_match`.
async fn resolve_jsm_request_type_id(
    name: &str,
    service_desk_id: &str,
    project_key: &str,
    profile: &crate::profile::Profile,
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

/// S-cycle12-jsm-adf-autoconvert Red Gate: AC-002 (pure `is_adf_field_value`
/// call-site contract), AC-009 (pure `jsm_adf_empty_omit_guard_applies`
/// gate), and AC-004/007/008/013 (resolution-layer `resolve_jsm_adf_extra_fields`
/// behavior, driven against a wiremock `MockServer`).
///
/// The resolution-layer tests isolate `JR_CACHE_DIR`/`XDG_CACHE_HOME` per
/// test (via `with_isolated_cache`) so `resolve_jsm_adf_extra_fields`'s
/// on-disk request-type-fields cache never touches a developer's real
/// `~/.cache/jr`, and so parallel `cargo test` execution cannot race on the
/// process-global env vars `crate::cache`'s read path consults.
#[cfg(test)]
mod adf_resolution_tests {
    use super::*;
    use crate::api::client::JiraClient;
    use crate::api::jsm::requests::JsmRequestBuilder;
    use crate::profile::Profile;
    use serde_json::{Value, json};
    use std::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Serializes `JR_CACHE_DIR`/`XDG_CACHE_HOME` mutation across this
    /// module's tests and points them at a fresh temp dir for the duration
    /// of `f`, restoring afterward even on panic. Mirrors `cache.rs`'s own
    /// `mod tests::with_env_var` helper, which is `pub(super)`-scoped to
    /// `crate::cache` and therefore not reusable here.
    ///
    /// `f` returns a `Future` (an async block), which is driven to
    /// completion via a fresh current-thread Tokio runtime — this function
    /// itself stays synchronous so the `ENV_MUTEX` guard is never held
    /// across a syntactic `.await` point (clippy::await_holding_lock).
    fn with_isolated_cache<F, Fut, R>(f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: ENV_MUTEX held; no concurrent env reads occur while we
        // hold the lock (mirrors cache.rs's own test-isolation pattern).
        unsafe {
            std::env::set_var("JR_CACHE_DIR", dir.path().join("jr"));
            std::env::set_var("XDG_CACHE_HOME", dir.path());
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt.block_on(f())));
        unsafe {
            std::env::remove_var("JR_CACHE_DIR");
            std::env::remove_var("XDG_CACHE_HOME");
        }
        drop(guard);
        match result {
            Ok(v) => v,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    fn rt_fields_response(fields: Vec<Value>) -> Value {
        json!({
            "canRaiseOnBehalfOf": false,
            "canAddRequestParticipants": false,
            "requestTypeFields": fields
        })
    }

    fn description_field() -> Value {
        json!({
            "fieldId": "description",
            "name": "Description",
            "description": Value::Null,
            "required": false,
            "visible": true,
            "defaultValues": Value::Null,
            "validValues": Value::Null,
            "jiraSchema": {"type": "string", "system": "description"}
        })
    }

    fn textarea_field(field_id: &str) -> Value {
        json!({
            "fieldId": field_id,
            "name": "Steps to Reproduce",
            "description": Value::Null,
            "required": false,
            "visible": true,
            "defaultValues": Value::Null,
            "validValues": Value::Null,
            "jiraSchema": {
                "type": "string",
                "custom": "com.atlassian.jira.plugin.system.customfieldtypes:textarea"
            }
        })
    }

    fn plain_field(field_id: &str) -> Value {
        json!({
            "fieldId": field_id,
            "name": "Labels",
            "description": Value::Null,
            "required": false,
            "visible": true,
            "defaultValues": Value::Null,
            "validValues": Value::Null,
            "jiraSchema": {"type": "string"}
        })
    }

    /// Recursively asserts no ADF `text` node's `text` attribute contains a
    /// raw `\n` or `\r` (INV-1, VP-FIELD-ADF-004 AC-004(b)).
    fn assert_no_raw_newline_in_text_nodes(value: &Value) {
        match value {
            Value::Object(map) => {
                if map.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(t) = map.get("text").and_then(Value::as_str) {
                        assert!(
                            !t.contains('\n') && !t.contains('\r'),
                            "INV-1: ADF text node must not contain a raw newline; got: {t:?}"
                        );
                    }
                }
                for v in map.values() {
                    assert_no_raw_newline_in_text_nodes(v);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    assert_no_raw_newline_in_text_nodes(v);
                }
            }
            _ => {}
        }
    }

    fn bare(value: &str) -> FieldValueSpec {
        FieldValueSpec {
            kind: None,
            value: value.to_string(),
        }
    }

    /// Assemble the final POST body from a [`JsmAdfFieldResolution`],
    /// mirroring `handle_jsm_create`'s own `JsmRequestBuilder` construction
    /// (AC-004(d), AC-013).
    fn build_body(resolution: &JsmAdfFieldResolution) -> Value {
        JsmRequestBuilder {
            service_desk_id: "10",
            request_type_id: "11002",
            summary: "test",
            description: None,
            markdown: false,
            priority: None,
            labels: &[],
            on_behalf_of: None,
            no_mentions: false,
            mentions: None,
            extra_fields: &resolution.extra_fields,
            resolved_adf_values: &resolution.resolved_adf_values,
            is_adf_request: resolution.is_adf_request,
        }
        .build()
        .unwrap()
    }

    /// AC-002 (BC-3.8.019 precondition, VP-FIELD-ADF-004 Axis a): the JSM
    /// call site passes `rt_field.jira_schema` DIRECTLY to
    /// `is_adf_field_value` — never double-nested inside another
    /// `{"jiraSchema": ...}` wrapper (the anti-pattern that produced
    /// `is_adf_field_value` returning `false` for every input before this
    /// fix, ADR-0024 canonical `jiraSchema` contract).
    ///
    /// Test method: DEFAULT CI (pure function, no network).
    #[test]
    fn test_bc_3_8_019_is_adf_field_value_receives_inner_schema_not_double_nested() {
        let rt_field: crate::types::jsm::RequestTypeField = serde_json::from_value(json!({
            "fieldId": "description",
            "name": "Description",
            "description": Value::Null,
            "required": false,
            "visible": true,
            "defaultValues": Value::Null,
            "validValues": Value::Null,
            "jiraSchema": {"type": "string", "system": "description"}
        }))
        .unwrap();

        assert!(
            crate::cli::issue::field_resolve::is_adf_field_value(&rt_field.jira_schema),
            "AC-002: is_adf_field_value(&rt_field.jira_schema) — the inner schema \
             passed directly — must return true for an allowlist match"
        );

        let double_nested = json!({"jiraSchema": rt_field.jira_schema});
        assert!(
            !crate::cli::issue::field_resolve::is_adf_field_value(&double_nested),
            "AC-002: double-nesting — is_adf_field_value(&json!({{\"jiraSchema\": ...}})) \
             — must return false; this demonstrates the anti-pattern this AC exists to \
             prevent"
        );
    }

    /// AC-009 / VP-FIELD-ADF-003 Axis D (JSM sub-case): the empty-omit
    /// guard's `kind.is_none()` half applies ONLY to the bare form — every
    /// hinted kind (`:option`/`:id`/`:name`/`:asset`) bypasses it, routing
    /// to the hint composer regardless of emptiness.
    ///
    /// Test method: DEFAULT CI (pure, no network — GREEN-BY-DESIGN per the
    /// stub's rustdoc; authored per the story's explicit instruction to
    /// author this test anyway).
    #[test]
    fn test_adf_empty_guard_fires_only_on_bare_form_not_hinted_jsm() {
        assert!(
            jsm_adf_empty_omit_guard_applies(None),
            "AC-009: the bare form (kind: None) must satisfy the empty-omit guard's gate"
        );
        assert!(
            !jsm_adf_empty_omit_guard_applies(Some(FieldValueKind::Option)),
            "AC-009: a ':option'-hinted value must bypass the empty-omit guard"
        );
        assert!(
            !jsm_adf_empty_omit_guard_applies(Some(FieldValueKind::Id)),
            "AC-009: a ':id'-hinted value must bypass the empty-omit guard"
        );
        assert!(
            !jsm_adf_empty_omit_guard_applies(Some(FieldValueKind::Name)),
            "AC-009: a ':name'-hinted value must bypass the empty-omit guard"
        );
        assert!(
            !jsm_adf_empty_omit_guard_applies(Some(FieldValueKind::Asset)),
            "AC-009: an ':asset'-hinted value must bypass the empty-omit guard"
        );
    }

    /// AC-004 Axis (b) (BC-3.8.019 postcondition): a non-empty bare
    /// `--field description=VALUE` extra field — `system == "description"` —
    /// is ADF-converted via `text_to_adf` and accumulates `is_adf_request`.
    ///
    /// Test method: DEFAULT CI (resolution-layer unit test, DQ-6-gated
    /// shape; wiremock-backed for the RT-fields fetch only).
    #[test]
    fn test_bc_3_8_019_jsm_description_extra_field_adf_converted() {
        with_isolated_cache(|| async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(
                    "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
                ))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(rt_fields_response(vec![description_field()])),
                )
                .mount(&server)
                .await;

            let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
            let profile = Profile::from("s-cycle12-desc");

            let mut extra_fields = std::collections::HashMap::new();
            extra_fields.insert("description".to_string(), bare("Hello world"));

            let resolution =
                resolve_jsm_adf_extra_fields(&client, &profile, "10", "11002", extra_fields).await;

            let adf = resolution
                .resolved_adf_values
                .get("description")
                .expect("AC-004(a): description must be ADF-converted into resolved_adf_values");
            assert_eq!(
                adf.get("type").and_then(Value::as_str),
                Some("doc"),
                "AC-004(a): must be an ADF doc; got {adf}"
            );
            assert_eq!(adf.get("version").and_then(Value::as_i64), Some(1));
            assert!(
                adf.get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|c| !c.is_empty()),
                "AC-004(a): content must be a non-empty array for non-empty input; got {adf}"
            );
            assert_no_raw_newline_in_text_nodes(adf);
            assert!(
                resolution.is_adf_request,
                "AC-004(c): is_adf_request must be true"
            );

            let body = build_body(&resolution);
            assert_eq!(
                body.get("requestFieldValues")
                    .and_then(|rfv| rfv.get("description")),
                Some(adf),
                "AC-004(a): requestFieldValues['description'] in the assembled POST body \
                 must be the ADF doc, not a plain string"
            );
            assert_eq!(
                body.get("isAdfRequest").and_then(Value::as_bool),
                Some(true),
                "AC-004(d): isAdfRequest must be true in the assembled POST body"
            );
        });
    }

    /// AC-004 Axis (b) (BC-3.8.020 postcondition): a non-empty bare
    /// `--field NAME=VALUE` extra field whose `custom` ends `:textarea` is
    /// ADF-converted and accumulates `is_adf_request`. Uses a multi-line
    /// value to also exercise INV-1 (hardBreak, not raw `\n`, AC-004(b)).
    ///
    /// Test method: DEFAULT CI (resolution-layer unit test, DQ-6-gated
    /// shape; wiremock-backed for the RT-fields fetch only).
    #[test]
    fn test_bc_3_8_020_jsm_textarea_extra_field_adf_converted() {
        with_isolated_cache(|| async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(
                    "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
                ))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(rt_fields_response(vec![
                        textarea_field("customfield_10050"),
                    ])),
                )
                .mount(&server)
                .await;

            let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
            let profile = Profile::from("s-cycle12-textarea");

            let mut extra_fields = std::collections::HashMap::new();
            extra_fields.insert("customfield_10050".to_string(), bare("Line one\nLine two"));

            let resolution =
                resolve_jsm_adf_extra_fields(&client, &profile, "10", "11002", extra_fields).await;

            let adf = resolution
                .resolved_adf_values
                .get("customfield_10050")
                .expect("BC-3.8.020: :textarea extra field must be ADF-converted");
            assert_eq!(adf.get("type").and_then(Value::as_str), Some("doc"));
            assert_no_raw_newline_in_text_nodes(adf);
            assert!(resolution.is_adf_request);

            let body = build_body(&resolution);
            assert_eq!(
                body.get("isAdfRequest").and_then(Value::as_bool),
                Some(true),
                "BC-3.8.020: isAdfRequest must be true in the assembled POST body"
            );
        });
    }

    /// AC-004 Axis (c) (BC-3.8.022 Behavior item 2, EC-3.8.022-2): the
    /// `is_adf_request` flag accumulates OR-only across multiple extra
    /// fields — a plain-string field alongside an ADF-converted field must
    /// still yield `true`, and the plain field must NOT itself contribute an
    /// entry to `resolved_adf_values`.
    ///
    /// Test method: DEFAULT CI (resolution-layer unit test, DQ-6-gated
    /// shape; wiremock-backed for the RT-fields fetch only).
    #[test]
    fn test_bc_3_8_022_is_adf_request_accumulated_for_adf_field() {
        with_isolated_cache(|| async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(
                    "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
                ))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(rt_fields_response(vec![
                        textarea_field("customfield_10060"),
                        plain_field("labels_field"),
                    ])),
                )
                .mount(&server)
                .await;

            let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
            let profile = Profile::from("s-cycle12-accum");

            let mut extra_fields = std::collections::HashMap::new();
            extra_fields.insert("labels_field".to_string(), bare("not-adf"));
            extra_fields.insert("customfield_10060".to_string(), bare("adf text"));

            let resolution =
                resolve_jsm_adf_extra_fields(&client, &profile, "10", "11002", extra_fields).await;

            assert!(
                resolution
                    .resolved_adf_values
                    .contains_key("customfield_10060")
            );
            assert!(
                !resolution.resolved_adf_values.contains_key("labels_field"),
                "EC-3.8.022-2: only the ADF-backed field contributes to resolved_adf_values; \
                 the plain field must not be ADF-converted"
            );
            assert!(
                resolution.is_adf_request,
                "BC-3.8.022 item 2: one ADF-converted extra field is enough to accumulate true"
            );

            let body = build_body(&resolution);
            assert_eq!(
                body.get("isAdfRequest").and_then(Value::as_bool),
                Some(true)
            );
        });
    }

    /// AC-004 Axis (c) ABSENT-check (VP-FIELD-ADF-004 pass-14 M-1 finding):
    /// when NO extra field is ADF-converted, `is_adf_request` stays `false`
    /// and the assembled POST body's `isAdfRequest` key is ABSENT — asserted
    /// via `.is_none()`, never `.unwrap_or(false)` (AC-010 strictness).
    ///
    /// Test method: DEFAULT CI (resolution-layer unit test, DQ-6-gated
    /// shape; wiremock-backed for the RT-fields fetch only).
    #[test]
    fn test_bc_3_8_020_is_adf_request_absent_when_no_adf_field_present() {
        with_isolated_cache(|| async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(
                    "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
                ))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(rt_fields_response(vec![plain_field("labels_field")])),
                )
                .mount(&server)
                .await;

            let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
            let profile = Profile::from("s-cycle12-absent");

            let mut extra_fields = std::collections::HashMap::new();
            extra_fields.insert("labels_field".to_string(), bare("plain-value"));

            let resolution =
                resolve_jsm_adf_extra_fields(&client, &profile, "10", "11002", extra_fields).await;

            assert!(resolution.resolved_adf_values.is_empty());
            assert!(!resolution.is_adf_request);

            let body = build_body(&resolution);
            assert!(
                body.get("isAdfRequest").is_none(),
                "VP-FIELD-ADF-004 Axis c: isAdfRequest key must be ABSENT (NOT explicit \
                 false) when no extra field is ADF-converted; got body: {body}"
            );
        });
    }

    /// AC-007 / VP-FIELD-ADF-003 Axis C (BC-3.8.021 postcondition): an empty
    /// or whitespace-only ADF-backed bare extra field is OMITTED from
    /// `requestFieldValues` entirely — not sent as an empty string, not a
    /// clear-doc (JSM create-omit semantics, distinct from platform edit's
    /// clear-doc) — and does NOT contribute to `is_adf_request`.
    ///
    /// Test method: DEFAULT CI (resolution-layer unit test, DQ-6-gated
    /// shape; wiremock-backed for the RT-fields fetch only).
    #[test]
    fn test_bc_3_8_021_jsm_empty_adf_field_omitted_isadfrequest_not_accumulated() {
        with_isolated_cache(|| async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path(
                    "/rest/servicedeskapi/servicedesk/10/requesttype/11002/field",
                ))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(rt_fields_response(vec![description_field()])),
                )
                .mount(&server)
                .await;

            let client = JiraClient::new_for_test(server.uri(), "Basic dGVzdDp0ZXN0".to_string());
            let profile = Profile::from("s-cycle12-empty");

            let mut extra_fields = std::collections::HashMap::new();
            extra_fields.insert("description".to_string(), bare("   "));

            let resolution =
                resolve_jsm_adf_extra_fields(&client, &profile, "10", "11002", extra_fields).await;

            assert!(
                !resolution.extra_fields.contains_key("description"),
                "AC-007: an empty ADF-backed bare field must be OMITTED from extra_fields \
                 entirely"
            );
            assert!(!resolution.resolved_adf_values.contains_key("description"));
            assert!(
                !resolution.is_adf_request,
                "AC-007: is_adf_request must NOT be set for the omitted field"
            );

            let body = build_body(&resolution);
            let rfv = body
                .get("requestFieldValues")
                .expect("requestFieldValues must exist");
            assert!(
                rfv.get("description").is_none(),
                "BC-3.8.021: requestFieldValues must have NO entry for the omitted field; \
                 got rfv: {rfv}"
            );
            assert!(body.get("isAdfRequest").is_none());
        });
    }
}
