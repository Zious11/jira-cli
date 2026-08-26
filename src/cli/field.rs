//! `jr field options <field>` — enumerate a custom field's allowed options.
//!
//! Anchors BC-X.14.001..004 (issue #580). Structural mirror of
//! `src/cli/requesttype.rs` per ADR-0019 §1. Three mutually-exclusive
//! MODE-SELECTOR flags (`--type`, `--request-type`, `--issue`) pick the
//! enumeration mechanism (M2 createmeta / M3 JSM requesttype-fields / M1
//! editmeta respectively); `--project` is never itself a mode selector — it
//! is a companion flag whose role depends on the selected mode.
//!
//! STUB NOTICE (S-580-1, stub-architect pass): every non-trivial body below
//! is `todo!()` per BC-5.38.001 — the implementer fills these in against the
//! Red Gate test suite in `tests/field_options.rs`. Types and signatures are
//! real so the crate compiles and the CLI surface (`jr field options --help`)
//! parses.

use anyhow::Result;

use crate::api::client::JiraClient;
use crate::cli::{FieldCommand, OutputFormat};
use crate::config::Config;
use crate::types::jira::AllowedValue;

/// Table-mode glyph for a missing `id` (BC-X.14.003 degenerate-entry
/// rendering). Reused convention from `changelog.rs`/`user.rs`/
/// `requesttype.rs` — not a new glyph, just a module-local copy of the same
/// literal (those modules keep their own private constants too).
const NULL_GLYPH: &str = "—";

/// Table-mode literal for a missing `label` (BC-X.14.003). Never falls back
/// to the entry's own `id`.
const UNNAMED_LABEL: &str = "(unnamed)";

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
    // Non-trivial effectful shell — mode-selector arity check, `<field>`
    // resolution (customfield_NNNNN bypass / list_fields + partial_match),
    // per-mode enumeration (M1 editmeta / M2 createmeta / M3 requesttype
    // fields, via `api::jsm::servicedesks::{require_service_desk,
    // get_or_fetch_project_meta}` and `cache::{read,write}_fields_cache`),
    // `--value` filtering, and table/JSON rendering all compose here.
    // Implementer fills this in against tests/field_options.rs.
    let _ = (command, output_format, config, client, project_override);
    todo!("jr field options handler — implemented in TDD Red→Green cycle")
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
    todo!("pure mode-selector arity check — see BC-X.14.001 Invariant 1 / VP-580-006")
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
    todo!("M2 project resolution — flag OR profile/config default, see ADR-0019 §Amendment D1")
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
    todo!("M1/M2 allowedValues[] -> Vec<FieldOption> normalizer, never-drop (EC-X.14.001-7)")
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
    todo!("M3 validValues[] -> Vec<FieldOption> normalizer, never-drop (EC-X.14.001-7)")
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
    todo!("BC-X.14.002 client-side --value substring filter, id-or-label, cascading-aware")
}

/// Render `options` into table rows (BC-X.14.003) — ID / Label columns,
/// cascading children indented under their parent. Table mode only; JSON
/// mode preserves nested `children[]` verbatim via `output::render_json`
/// (never flattened). Degenerate-entry glyphs: missing `id` -> [`NULL_GLYPH`]
/// (`"—"`), missing `label` -> [`UNNAMED_LABEL`] (`"(unnamed)"`, never a
/// fallback to the entry's own `id`). Pure core — no I/O.
pub(crate) fn render_option_rows(options: &[FieldOption]) -> Vec<Vec<String>> {
    let _ = (NULL_GLYPH, UNNAMED_LABEL);
    todo!("BC-X.14.003 table rendering — ID/Label columns, cascading indentation")
}
