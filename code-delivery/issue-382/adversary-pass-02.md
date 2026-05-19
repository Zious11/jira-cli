# S-382 Adversary Pass 02

## Verdict
**CLEAN — counter advances to 2/3** (fresh-context re-derivation; no findings)

## Findings
None.

## Confirmed Invariants
- Variant signature `{ message: String, required_scope: Option<String> }` at `src/error.rs:17-20`
- thiserror expression-arg `scope_hint = required_scope.as_deref().filter(|s| !s.is_empty()).unwrap_or("write:jira-work")` at `src/error.rs:15`
- All 5 BC-1.6.042 required Display substrings present in template
- `(while PUT/GET succeed)` parenthetical preserved (`src/error.rs:11`)
- Exit-code mapping unchanged at `src/error.rs:79` (`{ .. } => 2`)
- 3 production sites with correct values per lookup table
- 4 test construction sites in `src/error.rs` (T-1b @ 135 None, T-1 @ 176 None, AC-3 @ 196 Some, AC-4 @ 213 Some(empty))
- M-2 destructure `{ message, .. }` at `src/cli/issue/create.rs:1982`
- T-2 at `tests/api_client.rs:99-144` not modified; passes via None-fallback

## Reviewed Surfaces
Same as pass-01 with independent re-read; full coverage of src/error.rs, src/api/client.rs (680-715, 945-985), src/cli/issue/create.rs (1960-2010), tests/api_client.rs (95-250), tests/issue_create_jsm.rs (1500-1583), tests/oauth_flow_holdouts.rs, F1d plan, BC-1.6.042 body. Plus grep sweep for InsufficientScope/required_scope.

## Novelty Assessment
**LOW** — implementation matches F1d plan exactly; minimal surgical change; no silent behavior changes; no lint suppressions; no unsafe.

## Process-gap findings
None.
