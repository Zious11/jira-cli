# S-382 Adversary Pass 01

## Verdict
**CLEAN — counter advances to 1/3**

## Findings
None across all severities (CRITICAL/HIGH/MEDIUM/LOW/NIT).

## Confirmed Invariants (17 cross-axis checks PASS)

1. AC-1 variant signature `{ message: String, required_scope: Option<String> }` confirmed at `src/error.rs:17-20`
2. AC-2 thiserror expression-arg with Empty-Some filter confirmed at `src/error.rs:15`
3. AC-3 two-part assertion test at `src/error.rs:195-209`
4. AC-4 Empty-Some fallback test at `src/error.rs:211-222`
5. AC-5 T-2 preserved at `tests/api_client.rs:131-143` (None-fallback)
6. AC-6 3 production call-sites: `client.rs:700`/`970` None + `create.rs:1983` Some("write:servicedesk-request")
7. AC-7 M-2 destructure `{ message, .. }` at `src/cli/issue/create.rs:1982`
8. Display template byte-for-byte preserved incl. `(while PUT/GET succeed)` at `src/error.rs:11`
9. T-1b + T-1 construction calls updated with `required_scope: None`
10. thiserror v2 expression-arg semantics (Cargo.toml:28 pins "2")
11. L-288-pr2-02 grep clean (no `||` / `.or_else()` accept-either)
12. No `#[allow]` suppressions added
13. No `unsafe` blocks added
14. No dead code / unused imports
15. End-to-end dispatch path verified (send + parse_error → C-3 re-wrap)
16. InsufficientScope grep parity matches lookup table
17. C-3 dual-rendering wart confirmed as accepted (per F1d pass-03)

## Reviewed Surfaces
- src/error.rs (full), src/api/client.rs (680-723, 950-988), src/cli/issue/create.rs (1900-1996), src/api/jsm/requests.rs, tests/api_client.rs, tests/issue_create_jsm.rs, Cargo.toml
- .factory/phase-f1-delta-analysis/issue-382/delta-analysis.md
- .factory/specs/prd/bc-1-auth-identity.md (BC-1.6.042)
- .factory/cycles/cycle-001/S-382/implementation/red-gate-log.md

## Novelty Assessment
**LOW** — textbook execution of converged F1d plan. No findings to report.

## Process-gap findings
None.
