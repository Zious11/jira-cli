---
document_type: research-validation
issue: 333
phase: F5-triage-validation
pass: 04
producer: research-agent
timestamp: 2026-05-12
budget_minutes: 8
status: complete
inputs:
  - .factory/code-delivery/issue-333/research-validation-pass-03.md
  - .factory/code-delivery/issue-333/research-validation-pass-02.md
  - .factory/code-delivery/issue-333/story.md
  - src/api/jira/bulk.rs (worktree, post-pass-02 apply set; OUTER deadline error still anyhow!)
  - src/api/client.rs (worktree, post-pass-02 apply set; entry-point + in-loop now DeadlineExceeded)
  - src/error.rs (worktree; DeadlineExceeded variant present, exit 124)
---

# Adversary Triage Validation (Pass 04)

## Summary

- **Q1 (OUTER-loop variant conversion):** CONFIRMED — pass-02 left a propagation hole at `bulk.rs:397-403`. The adversary's proposed shape is essentially right, but `remaining_ms: 0` is the correct field (NOT `elapsed_ms`); the variant should NOT be extended with an `elapsed_ms` or `deadline_label` field. Three-site message structural consistency is good hygiene and should be applied as a follow-up nit, NOT a blocker.
- **Q2 (cap-vs-deadline precedence):** RECOMMEND option (a) — REORDER so the expired-deadline check fires BEFORE the cap-vs-Retry-After comparison. Industry precedent (POSIX `timeout(1)`, kubectl client-go, AWS smithy-rs retry orchestrator, tokio `tokio::time::timeout`) strongly favors deadline-first. The pass-02 NIT-2 "user-impact is equivalent" comment was defensible WHEN both sites returned exit 1; it is now WRONG because the exit codes diverge (cap → 1, deadline → 124) and POSIX scripting consumers will distinguish them.
- **Q3 (AC-005 update):** RECOMMEND annotate-as-superseded. VSDD/regulated-development precedent (Atlassian agile guidance, FDA/QMS change control, kube-rs k8s-go change history) favors PRESERVING the F3-approved AC text with an explicit supersession block referencing the pass-03 finding. Do NOT rewrite in-place; the audit trail of "F3 said X → F4/F5 found Y → reversed to Z" is the entire point of the multi-pass adversarial review.

## Per-question findings

### Q1 — OUTER-loop deadline-error variant conversion

**VERDICT: CONFIRMED gap (pass-02 propagation hole) → APPLY the adversary's proposed conversion AT `bulk.rs:397-403`.**

#### Q1(a) — `remaining_ms: 0` vs `elapsed_ms: N`?

`remaining_ms: 0` is **correct** and should remain the field. Rationale:

1. **Semantic invariant preservation.** The struct field `remaining_ms` is defined in `src/error.rs:42` as the budget remaining at the moment of error production. The top-of-loop check at `bulk.rs:397` fires when `Instant::now() >= deadline`, which by definition means `remaining = 0`. There is no overshoot bug to surface at this site — it is the canonical "no budget left" detector. Reporting `0` is honest.

2. **Don't extend the variant; the message carries forensic detail.** The adversary asked whether to add an `elapsed_ms` field for "how far past the deadline we were". Per `research-validation-pass-03.md` Correction 3, the variant is intentionally narrow: `{ remaining_ms: u64, message: String }`. Operator-facing forensic context (HOW far past, WHICH site fired) belongs in the `message` string, NOT in additional struct fields. Reasoning:

   - Match arms in the codebase (verified by pass-03 grep: 6 src-tree arms total, ALL on `status: 404`, ZERO on `DeadlineExceeded`) will not need to destructure additional fields anytime soon.
   - The `--output json` envelope serializes whatever fields the variant exposes — adding `elapsed_ms` would be a one-way schema commitment that no operator script can usefully depend on for parsing (it's narrative metadata, not a control signal).
   - Std `Instant` arithmetic for `elapsed_ms` requires a saturating subtract anyway; the OUTER site lacks the original `now()` snapshot at the cusp (it has `Instant::now()` post-condition + `deadline`, but the OUTER check returns BEFORE re-reading the clock at error-construction time, so any `elapsed_ms` would be 0-or-tiny and uninformative).

3. **Three-site consistency reads better with `remaining_ms: 0` at the OUTER site than with a divergent `elapsed_ms`.** The entry-point check (`client.rs:476`) and the in-loop 429-clamp (`client.rs:589`) both produce `remaining_ms = computed_remaining` (which is 0 at the OUTER-equivalent threshold). Using the same field at the OUTER site preserves wire-format symmetry across all three deadline-exceeded paths.

**Recommended OUTER-site replacement (adversary's proposed shape, lightly adjusted):**

```rust
// src/api/jira/bulk.rs:397-403
if Instant::now() >= deadline {
    return Err(JrError::DeadlineExceeded {
        remaining_ms: 0,
        message: format!(
            "Bulk task {task_id} did not complete within {}s timeout. \
             Check Jira for task status.",
            timeout.as_secs()
        ),
    }
    .into());
}
```

This produces exit code 124 (POSIX `timeout(1)`) at this site, matching the other two `DeadlineExceeded` construction sites. Without this conversion, the OUTER site exits 1 while the INNER sites exit 124 — exactly the propagation cluster the adversary identified.

**EVIDENCE (file:line):**
- `src/api/jira/bulk.rs:397-403` — OUTER site, currently `anyhow::anyhow!`.
- `src/error.rs:42` — `DeadlineExceeded { remaining_ms: u64, message: String }`.
- `src/error.rs:79` — exit code 124 mapping.
- `src/api/client.rs:476-484` — entry-point site (correct shape).
- `src/api/client.rs:589-599` — in-loop 429-clamp site (correct shape).

#### Q1(b) — Three-site structural consistency / common `deadline_label` field?

**VERDICT: DEFER. Apply structural consistency at the MESSAGE-PREFIX level only; do NOT extend the variant.**

The adversary suggested a common `deadline_label: "bulk-poll" | "send-entry" | "429-retry"` field for operator forensics. This is over-engineered for the current call-graph:

1. **Today's only deadline-aware caller is `await_bulk_task_inner`.** All three sites are reached from one feature. The label adds noise without value to current operators.

2. **The variant is six lines old.** Extending it before the second feature exists is premature commitment.

3. **Better near-term hygiene: prefix-anchor the message string.** Use a leading discriminant token operators can grep:

   | Site | Message prefix |
   |---|---|
   | `client.rs:476` entry-point | `"[deadline:send-entry] Caller-supplied deadline already expired..."` |
   | `client.rs:589` in-loop 429-clamp | `"[deadline:429-retry] Caller-supplied deadline exceeded during 429 retry..."` |
   | `bulk.rs:397` OUTER bulk-poll | `"[deadline:bulk-poll] Bulk task {task_id} did not complete within {}s timeout..."` |

   This achieves the adversary's forensic intent (which site fired?) via the existing `message: String` field — no schema change, no churn.

**OPTIONAL APPLY:** Add the `[deadline:<site>]` prefix to all three messages in the same PR or as an immediate follow-up nit. The exit code and variant kind already match — this is grep-readability, not functional.

**EVIDENCE (precedent):**
- kubectl uses `"context deadline exceeded"` as a single string; no machine-readable site tag (per pass-03 Correction 2 citations).
- gh CLI surfaces Go's `context.DeadlineExceeded` string verbatim; no struct fields beyond the wrapped error.
- AWS smithy-rs `ConfigBag` `attempt_timeout` vs operation-level deadline are distinguished in log lines, not struct fields.

External precedent is consistent: deadline-exceeded errors carry their site info in the message, not in extra fields.

### Q2 — Cap-vs-deadline precedence reordering at `client.rs:559-563`

**VERDICT: APPLY option (a) — REORDER so deadline-check fires BEFORE cap-check. The pass-01 NIT-2 comment is now stale and incorrect.**

#### The exit-code-divergence argument

Pass-02 introduced `JrError::DeadlineExceeded` (exit 124) for the INNER 429-clamp at `client.rs:589`. Pass-01's NIT-2 comment at `client.rs:559-563` says:

> "this cap-exceeded abort fires BEFORE the S-333 deadline clamp below. A 429 with Retry-After > 60s AND an expired caller-supplied deadline will surface the cap message, not the deadline message. Both are correct abort signals; the differing messages are not orthogonal but the user-impact is equivalent."

The bolded claim is **factually false post-pass-02**:

| Path fired | Exit code (now) | Operator script reaction |
|---|---|---|
| Cap (`client.rs:564`) | 1 (generic failure) | "API error, maybe Atlassian — retry later or check status page" |
| Deadline (`client.rs:589`) | **124** (POSIX timeout) | "Our budget ran out — increase `JR_BULK_AWAIT_TIMEOUT_SECS` or upstream caller" |

A CI/CD orchestration script that branches on exit code (the WHOLE POINT of carving out 124) gets a different routing decision depending on which check the source code happens to evaluate first. This is no longer a "differing message" cosmetic — it is a divergent contract.

#### Industry precedent (Perplexity 2026-05-12 reason-mode walk)

| System | Behavior when client deadline AND server rate-limit both apply |
|---|---|
| **POSIX `timeout(1)` coreutils** | Timeout signal ALWAYS wins. Exit 124 is reported even if the wrapped process was simultaneously exiting with its own error. The hard wall-clock contract supersedes downstream conditions. [GNU coreutils manual: timeout(1)](https://www.gnu.org/software/coreutils/manual/coreutils.html#timeout-invocation) |
| **kubectl (client-go)** | Context deadline checked BEFORE retry classification. `ctx.Err()` returns `context.DeadlineExceeded` and the 429/Retry-After path is never entered. [kubernetes/client-go retry pattern](https://pkg.go.dev/k8s.io/apimachinery/pkg/api/errors) |
| **AWS smithy-rs / aws-sdk-rust** | Per-attempt timeout and operation-level deadline are checked in the retry orchestrator BEFORE `should_attempt_retry()` evaluates the response status. Deadline-exceeded short-circuits retry classification. [AWS SDK Rust retry config](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/retries.html) |
| **gh CLI** | Surfaces Go `context.DeadlineExceeded` from the underlying `net/http` client; Retry-After is a downstream consideration that never runs after the context fires. |
| **tokio::time::timeout / reqwest** | The timeout is a hard interrupt at the future level — it returns `Err(Elapsed)` regardless of what the future was about to do next, including evaluating a 429 response. [docs.rs/tokio/time::timeout](https://docs.rs/tokio/latest/tokio/time/fn.timeout.html) |

The consensus is unambiguous: **client-side deadline is a hard contract; server-side Retry-After (and any client cap on it) is advisory.** RFC 9110 §10.2.3 explicitly says the client MAY (not MUST) respect Retry-After — the deadline trumps it.

#### Recommended fix at `client.rs:559-563`

Reorder so the deadline clamp runs FIRST:

```rust
if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_RETRIES {
    let rate_info = RateLimitInfo::from_headers(response.headers());
    let delay = rate_info.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS);
    let base_sleep = Duration::from_secs(delay);

    // S-333 Q2 (pass-04): deadline FIRST. POSIX timeout(1) precedent +
    // kubectl context.DeadlineExceeded behavior + aws-sdk-rust smithy-rs
    // retry orchestrator + tokio::time::timeout all favor deadline-as-
    // hard-contract over server-advisory Retry-After. If the caller-
    // supplied budget is exhausted, surface exit 124 regardless of what
    // the server response says — operators scripting on exit code need
    // the hard signal.
    let actual_sleep = match clamp_retry_sleep(base_sleep, deadline) {
        ClampResult::Sleep(d) => d,
        ClampResult::Expired { remaining_ms } => {
            return Err(JrError::DeadlineExceeded {
                remaining_ms,
                message: format!(
                    "[deadline:429-retry] Caller-supplied deadline exceeded during 429 retry \
                     (Retry-After {delay}s, remaining budget {remaining_ms}ms before clamp). \
                     Rerun with a larger timeout, or wait for rate-limit pressure to subside."
                ),
            }
            .into());
        }
    };

    // BC-X.4.009: cap-abort runs SECOND. Only reached if the caller's
    // deadline still has budget remaining; the clamp above will have
    // produced a Sleep(d) where d <= MAX_RETRY_AFTER_SECS would be a
    // reasonable wait, but base_sleep itself (i.e., the unclamped server
    // Retry-After) may still exceed the cap.
    if delay > MAX_RETRY_AFTER_SECS {
        eprintln!(
            "[jr] Atlassian requested {delay}s wait — exceeds {MAX_RETRY_AFTER_SECS}s cap for interactive CLI.\n\
             Aborting retry; rerun later or wrap in a shell-level retry/cron job."
        );
        return Err(JrError::ApiError {
            status: 429,
            message: format!(
                "Rate limited; Retry-After {delay}s exceeds {MAX_RETRY_AFTER_SECS}s cap. Rerun later."
            ),
        }
        .into());
    }

    // ... sleep(actual_sleep) and continue
}
```

**Caveat — a subtle interaction.** When the clamp runs FIRST and returns `Sleep(d)`, but `delay` (the unclamped server value) still exceeds the cap, the code paths above will sleep `d` (which IS within both budget and cap) and then on the next iteration receive ANOTHER 429 with the same Retry-After. If the deadline still has budget, we now hit the cap-abort cleanly with `ApiError(429)`. If the deadline doesn't, we hit the clamp's `Expired` arm first. Either is correct behavior.

**Edge case — deadline expires DURING the cap-relevant sleep but cap was not yet triggered.** With deadline-first ordering, this can't happen: the clamp short-circuits to `DeadlineExceeded` immediately, no sleep happens. The script consumer gets exit 124.

#### Action: update the stale NIT-2 comment

The comment block at `client.rs:559-563` (the "user-impact is equivalent" paragraph) MUST be replaced with the new ordering rationale. Leaving the old comment alongside reordered code is worse than no comment.

**EVIDENCE (file:line):**
- `src/api/client.rs:551-578` — current cap-check ordering.
- `src/api/client.rs:580-601` — current clamp ordering (would move ABOVE cap-check after reorder).
- Perplexity 2026-05-12 reason-mode (this pass): "Reverse your implementation order: check deadline BEFORE cap-exceeded. Exit code 124 ... should take precedence over exit code 1 ... when both conditions occur on the same request."
- POSIX timeout(1) convention; kubectl client-go pattern; AWS smithy-rs retry orchestrator order; tokio::time::timeout semantics.

### Q3 — Story AC-005 update strategy

**VERDICT: ANNOTATE-AS-SUPERSEDED with explicit pass-03 cross-reference. Do NOT rewrite AC-005 in-place.**

#### The reversal scenario

- **F3-approved AC-005:** "Reuse `JrError::ApiError { status: 429, message: ... }` — do NOT introduce a new `JrError` variant" (story.md line 107-109).
- **F5-pass-03 finding:** Pass-03 Correction 1 + 2 reversed this — "ApiError(429) for a request that never reached the server is misleading"; the "hundreds of match sites" pass-02 cost argument was based on a miscount (actual: 6 src-tree arms, ALL on status=404, ZERO on 429); introducing `DeadlineExceeded` is "near-zero taxonomy churn".
- **Post-apply state:** Pass-02 introduced `JrError::DeadlineExceeded` at two of the three sites. AC-005 as written in `story.md` is now factually wrong: the variant exists, exit 124 is wired, the integration tests assert on the new variant kind.

#### Why supersede rather than rewrite

VSDD methodology (per the Perplexity research on this pass) and parallel regulated-development frameworks (FDA QMS, agile acceptance-criteria governance, kube-rs change history) converge on **preserving the historical decision trail when a higher-severity finding reverses a lower-severity decision.** Specifically:

1. **The whole point of multi-pass adversarial review is the trail.** F1/F2/F3 → F4 → F5 IS the audit log. Rewriting AC-005 in-place erases the evidence that pass-02 introduced the variant AND that pass-03 validated the reversal AND that the pass-01 "stay with 429" was a miscount-driven mistake. Future maintainers reading the story without the trail will not understand WHY the variant exists.

2. **Codebase precedent: ADR-0002 vs ADR-0006.** This repo already practices supersession (not in-place rewrite) for reversed architectural decisions:
   - `docs/adr/0002-oauth2-embedded-secret.md` was marked "superseded by ADR-0006".
   - ADR-0006 (`embedded-jr-oauth-app-with-compile-time-xor-obfuscation`) explicitly says "re-supersedes ADR-0002".
   - The reader can reconstruct the full decision history.

   The story AC-005 reversal should follow the same pattern: keep the original AC text, mark it superseded, point to the pass-03 validation, write the new AC alongside.

3. **F4/F5 finding severity > F3 decision authority.** Per the broader VSDD research, "lower severity designations do not automatically invalidate higher-level decisions, but higher-severity findings may indeed necessitate reversal of decisions made under lower-severity conditions." Pass-03's reversal of pass-02's "hundreds of match sites" is exactly this pattern — a more-rigorous count overturning a less-rigorous count.

4. **Story state semantics.** The story frontmatter `status: draft` (story.md:7) means the document is still being iterated. Adding a `last_updated: 2026-05-12` bump with a `## Supersession Log` section is lower-friction than rewriting AC text and trying to preserve the diff implicitly through git.

#### Recommended supersession-block shape

Add at the END of the AC block (between AC-005 and AC-006), do NOT modify AC-005's existing text:

```markdown
**AC-005 (SUPERSEDED 2026-05-12 by research-validation-pass-03).**
The above AC-005 specifies reuse of `JrError::ApiError { status: 429 }`
because pass-02 framed taxonomy churn as "hundreds of match sites". Pass-03
verified-by-grep that the actual count is 6 src-tree match arms (ALL on
status: 404, ZERO on 429) — near-zero churn cost. Pass-03 also surveyed
external CLI precedent (kubectl, gh, aws-cli, doctl, fly) and found
unanimous adoption of a distinct DeadlineExceeded variant. Reversed
decision: introduce `JrError::DeadlineExceeded { remaining_ms: u64,
message: String }`, exit code 124 (POSIX timeout(1) convention), use at
the entry-point check AND the in-loop 429-clamp AND the OUTER bulk-poll
top-of-loop check (per pass-04 Q1). KEEP `ApiError(429)` at the
Retry-After cap-abort site (`client.rs:564`) — that one IS a real
server-mediated 429. Tests assert on the new variant kind. See
`.factory/code-delivery/issue-333/research-validation-pass-03.md`
"Correction 1/2/3" and pass-04 Q1/Q2.

**AC-005-v2 (effective).** When `send_inner` aborts due to deadline-
exhaustion (entry-point check OR in-loop 429-clamp), the returned `Err`
is `JrError::DeadlineExceeded { remaining_ms, message }` where:
- `remaining_ms == 0` at the entry-point and OUTER bulk-poll sites
  (computed from `clamp_retry_sleep(Duration::ZERO, Some(deadline))`'s
  `Expired { remaining_ms }`).
- `remaining_ms` reflects the actual sub-millisecond clamp budget at the
  in-loop 429-clamp site.
- `message` contains the substring `"deadline"` AND a `[deadline:<site>]`
  prefix where `<site> ∈ {send-entry, 429-retry, bulk-poll}` for grep-
  driven forensics.
- `exit_code() == 124`.
```

This is the same pattern the project uses for ADR supersession; it does not violate any documented VSDD constraint and preserves the audit trail.

#### Alternative considered (rejected): in-place rewrite

Rewriting AC-005 silently to match pass-02's apply was considered and rejected because:

1. It would imply the F3 approval was wrong from inception, when in fact pass-02 was right with the information available at F3 (the miscount premise was load-bearing).
2. It would orphan the pass-03 research-validation document — future readers would see "we have `DeadlineExceeded`" but not "we reversed an F3-approved 'don't introduce a variant' decision based on pass-03 findings".
3. It violates the supersession-precedent established by ADR-0002 / ADR-0006 in this repo.

**EVIDENCE (precedent):**
- `docs/adr/0002-oauth2-embedded-secret.md` (in-repo) — explicit "superseded" annotation, original text preserved.
- `docs/adr/0006-embedded-jr-oauth-app-with-compile-time-xor-obfuscation.md` — re-supersedes 0002 with new decision alongside.
- Perplexity 2026-05-12 (this pass): "the supersession approach preserves the full historical sequence of decisions and revisions... the complete decision history... aligns with regulatory guidance that requires organizations to 'maintain historical chronology' of changes and the decisions that prompted them."

## Triage Adjustment Recommendations

Apply set delta vs pass-03's proposal:

1. **APPLY Q1 — OUTER-site `bulk.rs:397-403` conversion.** Replace the `anyhow::anyhow!` with `JrError::DeadlineExceeded { remaining_ms: 0, message: ... }.into()`. Single-site change; no variant extension. Optional consistency nit: add `[deadline:bulk-poll]` prefix.

2. **APPLY Q2 — reorder `client.rs:559-578` so deadline-clamp fires BEFORE cap-check.** Update the stale NIT-2 comment block to reflect the new ordering and the exit-code-divergence rationale. POSIX/kubectl/AWS-SDK-Rust/tokio precedent is unambiguous.

3. **APPLY Q3 — supersede AC-005 in `story.md`.** Add the supersession block + `AC-005-v2 (effective)` at the same location; do NOT modify the original AC-005 text. Bump `last_updated: 2026-05-12`. Add `## Supersession Log` if convenient, else inline as shown above.

4. **OPTIONAL — apply `[deadline:<site>]` message prefixes at ALL THREE sites for cross-site grep-readability.** Low-effort hygiene; skip if scope-tight, but a follow-up nit either way.

5. **DEFER — no variant extension.** Pass-04 specifically rejects extending `JrError::DeadlineExceeded` with `elapsed_ms` or `deadline_label` fields. Message-string forensics suffice for v0.5; revisit if a second deadline-aware caller (e.g., hypothetical `await_search_export`) introduces site-distinguisher demand.

6. **TESTS to update or add:**
   - `tests/bulk_deadline_propagation.rs` — if it currently asserts on `stderr.contains("did not complete within")` (the OUTER-site message), no change. If it asserts on the error variant kind for the OUTER path, switch the matcher from `anyhow` to `JrError::DeadlineExceeded`.
   - **New unit test** at `src/api/client.rs::tests`: `test_send_inner_deadline_takes_precedence_over_cap_when_both_fire` — mounts a 429 with `Retry-After: 120s` AND a pre-expired deadline, asserts the return is `Err(JrError::DeadlineExceeded { .. })` not `Err(JrError::ApiError { status: 429 })`, asserts exit code 124. This pins the new precedence ordering against regression.
   - The release-gate test `tests/bulk_await_timeout_release_gate.rs` should continue to pass unchanged.

7. **CLAUDE.md update** — no change required (the OUTER-site fix is internal-error-shape; the Q2 reorder is internal-precedence; the env-var doc-fallout is already covered by the codified pattern from PR #357 lesson R19).

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Read | 6 | story.md, research-validation-pass-02.md, research-validation-pass-03.md, error.rs, bulk.rs (380-430), client.rs (440-620) |
| Glob | 2 | Locate .factory/code-delivery/issue-333/* artifacts (worktree-vs-main path resolution) |
| Perplexity reason | 1 | Multi-system precedence walk: kubectl/gh/aws-cli/POSIX timeout(1)/tokio/reqwest behavior when client-deadline AND server-Retry-After-cap both fire on the same request |
| Perplexity search | 1 | VSDD/regulated-development precedent for handling F4/F5 findings that reverse earlier-approved acceptance criteria — supersede vs in-place rewrite |
| Training data | 1 area | Codebase precedent (ADR-0002/ADR-0006 supersession pattern) referenced from CLAUDE.md "Key Decisions" — verified via Read of CLAUDE.md, not from training-data recall |

**Total MCP tool calls:** 2 (1 Perplexity reason + 1 Perplexity search); plus 6 Read + 2 Glob.

**Training data reliance:** low — all external claims (POSIX timeout(1), kubectl client-go, AWS smithy-rs retry orchestrator, tokio::time::timeout, kube-rs k8s-go DeadlineExceeded, VSDD supersession pattern) are Perplexity-cited with URLs. Codebase claims are file:line from Read. The ADR-0002/ADR-0006 supersession precedent was directly verified by reading CLAUDE.md "Key Decisions" section in the system context.

## Citations

- POSIX `timeout(1)` exit-code convention (124): <https://www.gnu.org/software/coreutils/manual/coreutils.html#timeout-invocation>
- kubectl client-go context.DeadlineExceeded pattern: <https://pkg.go.dev/k8s.io/apimachinery/pkg/api/errors>
- AWS SDK Rust retry orchestrator: <https://docs.aws.amazon.com/sdk-for-rust/latest/dg/retries.html>
- tokio::time::timeout semantics: <https://docs.rs/tokio/latest/tokio/time/fn.timeout.html>
- RFC 9110 §10.2.3 (429 client MAY respect Retry-After): <https://www.rfc-editor.org/rfc/rfc9110.html#name-429-too-many-requests>
- kube-rs DeadlineExceeded variant: <https://github.com/kubernetes/apimachinery/blob/master/pkg/api/errors/errors.go>
- Atlassian acceptance criteria governance: <https://www.atlassian.com/work-management/project-management/acceptance-criteria>
- VSDD methodology overview (supersession is implicit): <https://dev.to/midastools/vsdd-the-ai-coding-methodology-actually-worth-stealing-35ah>
