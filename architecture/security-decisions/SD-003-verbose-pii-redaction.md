# SD-003: --verbose PII Redaction

**Status:** PENDING
**Owner:** Phase 3 SECURITY-DECIDE
**Deadline:** Phase 1 → 2 gate (decision required before Phase 2 story decomposition begins)
**References:** NFR-S-C (nfr-catalog.md), R-M0 (risk-register.md; previously tracked as R-H3 — see Pass 6 reclassification), `src/api/client.rs:200-203,274-278`

---

## Context

When `--verbose` is set, `client.rs` prints the full HTTP request body to stderr via `String::from_utf8_lossy`. This body includes:

- ADF comment text and issue descriptions (user-authored content)
- Issue summaries
- Account IDs and email addresses
- Any field value passed as JSON to the Jira API

The `Authorization` header is NOT logged (only the request method and URL), but the body is fully exposed.

**Risk scenarios:**
1. Developer pipes `jr ... 2>debug.log` for debugging — log file now contains PII and potentially sensitive content.
2. AI-agent harnesses (e.g., this Claude session) may capture `--verbose` stderr in transcripts — leaking payload bytes into AI training or logging pipelines.
3. Incident response engineers running `jr` with verbose logging in a shared terminal session expose colleague data.

---

## Options

### Option A: Add `redact_body()` helper (default on)

- Add `fn redact_body(body: &str) -> String` in `src/api/client.rs` or `src/observability.rs`.
- Replace field values matching patterns: `accountId`, `emailAddress`, and ADF `text` node content with `[REDACTED]`.
- Complex to implement correctly for arbitrary JSON; risk of over-redaction hiding useful debug signal.

### Option B: Header-only verbose by default; opt-in body logging

- Default `--verbose` shows: `[verbose] {METHOD} {URL}` + response status only.
- New flag `--verbose-bodies` enables body logging (explicit opt-in with PII awareness warning).
- Breaking change: developers who relied on `--verbose` for body inspection must migrate to `--verbose-bodies`.
- Clear UX contract; no regex-based redaction complexity.

### Option C: Document and defer (accepted risk)

- Add a warning to CLAUDE.md and `--help` text: "`--verbose` logs full request bodies. Do not use in shared terminals or AI-agent contexts where PII must not be captured."
- No code change.
- Acceptable only if the security review concludes the risk is LOW given current user base.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| TBD  | PENDING  | Awaiting Phase 3 security review |
| **Decide-by** | **Phase 1 → 2 gate** | Required before Phase 2 story decomposition begins (ADV-P2-009) |

---

## Resolution Requirement

Before closing this SD, the Phase 3 implementer must:
1. Choose Option A, B, or C.
2. If A or B: implement and add a test that verifies account IDs are not present in `--verbose` stderr output for a mock create-issue call.
3. Record the outcome in this document.
4. Update `cross-cutting.md §2` (`--verbose` mode documentation) to reflect the resolved behavior.
