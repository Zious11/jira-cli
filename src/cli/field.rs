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
//
// EVERY test below MUST currently FAIL — either by hitting a `todo!()` panic
// (all functions in this module are still stubs) or, for the M1/M2 cascading
// case, by a genuine assertion failure documented at that test (see the
// KNOWN GAP comment on `test_bc_x_14_001_cascading_children_round_trip_m1_m2`).
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
                    let popcount =
                        has_type as u8 + has_request_type as u8 + has_issue as u8;
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

    /// AC-001: `has_project` is not a parameter of `resolve_field_context` at
    /// all (ADR-0019 § Amendment D1) — this is enforced structurally by the
    /// function's own 3-argument signature (a 4th argument would be a compile
    /// error), so there is no runtime behavior to assert beyond the arity
    /// table above. This test exists as a documentation anchor only.
    #[test]
    fn test_bc_x_14_001_resolve_field_context_has_no_project_parameter() {
        // Calling with exactly 3 arguments compiles; that IS the assertion.
        let _ = resolve_field_context(true, false, false);
    }

    // ── AC-004 / VP-580-010: resolve_m2_project (flag OR profile default) ──

    fn config_with_profile_project(project: Option<&str>) -> Config {
        let mut config = Config::default();
        config.active_profile_name = "default".to_string();
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
            },
            AllowedValue {
                id: None,
                value: Some("Missing id".to_string()),
                name: None,
            },
            AllowedValue {
                id: Some("10003".to_string()),
                value: None,
                name: None,
            },
            AllowedValue {
                id: None,
                value: None,
                name: None,
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
        let result = filter_options(&options, Some("blk"));
        // Matches via label substring ("Blocked" contains "blk" case-insensitively)
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
        assert_eq!(result.len(), 1, "parent retained as context for the matching child");
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
        let options = vec![
            opt(Some("10001"), Some("Anything")),
            opt(None, None),
        ];
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
        let options = vec![
            opt(Some("10001"), Some("A")),
            opt(Some("10002"), Some("B")),
        ];
        let result = filter_options(&options, None);
        assert_eq!(result, options, "--value absent returns the full list unchanged");
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
        // The child row's label must be indented relative to the parent's.
        assert!(
            rows[1][1].starts_with("  ") || rows[1][1] != "Child",
            "cascading child row must render indented under its parent; got {:?}",
            rows[1]
        );
        assert!(
            rows[1][1].trim() == "Child",
            "child row label content must still be 'Child' once indentation is trimmed; got {:?}",
            rows[1]
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
}
