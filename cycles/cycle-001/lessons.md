---
document_type: lessons-learned
level: ops
version: "1.0"
status: in-progress
producer: state-manager
timestamp: 2026-05-07T00:00:00
cycle: "cycle-001"
inputs: [STATE.md]
input-hash: "[live-state]"
traces_to: STATE.md
---

# Lessons Learned — cycle-001

<!-- Durable lessons from this cycle for future VSDD factory runs.
     Organized by category: agent-level, process-level, infrastructure-level.
     Each lesson is numbered continuously and includes the pass/burst
     where it was discovered. -->

## Agent-Level

_(none yet)_

## Process-Level

1. **PR-body 'Deviations' section + reviewer prompt catches scope-creep** — S-0.04 review cycle 1 surfaced a CLAUDE.md violation (cross-profile cache fallback in cache.rs) that would have shipped undetected. The implementer listed the deviation explicitly in the PR body under a "Deviations" section, and the reviewer prompt was drafted to evaluate scope-creep. This pattern validated: surface implementer deviations explicitly in PR body so reviewer can triage (revert vs. accept) rather than accepting silently.
   _Discovered: S-0.04 review cycle 1, 2026-05-07_

2. **Factory-dispatcher mid-session policy enforcement** — The factory-dispatcher hook permitted admin merges for PRs #289-#292, then began blocking them at PR #293 (orchestrator direct path AND pr-manager sub-agent path both rejected with `block_intent=true exit_code=2`). Workaround: surface to user for manual `gh pr merge --admin` invocation. Codification candidate: orchestrator should detect dispatcher-block in pr-manager output and immediately surface to user rather than chasing through retries. Add to S-7.02 codification register: "When pr-manager returns dispatcher-blocked status on merge, orchestrator MUST present clear option list (manual merge vs UI approval) and ScheduleWakeup polling rather than retrying."
   _Discovered: S-0.05 merge attempt, 2026-05-07_
   **Recurrence #2 confirmed at PR #294 (S-0.06), 2026-05-07.** Pattern stable; codification candidate now urgent. Track: every Wave 0 PR after #292 has been blocked by dispatcher — manual merge required. ETA codification needed before Wave 1 entry.

## Infrastructure-Level

3. **Local clippy < CI clippy version skew** — S-0.05 passed local `cargo clippy -- -D warnings` but failed CI on Rust 1.95.0 (`doc_lazy_continuation`, `assertions_on_constants`). Local toolchain was an older version. Codification: implementers should run `cargo clippy --all --all-features --tests -- -D warnings` matching CI's exact flag set, and consider pinning rustup default toolchain to match CI for parity. Add to story-writer template: "Quality gate command should be the EXACT CI command, not just `cargo clippy -- -D warnings`."
   _Discovered: S-0.05 CI failure requiring clippy-fix commit c82832c, 2026-05-07_

## Policy Candidates

<!-- Lessons that should be formalized as governance policies.
     Reference the lesson number and proposed policy scope. -->

| Lesson | Proposed Policy | Scope | Status |
|--------|----------------|-------|--------|
| 1 | Require "Deviations" section in all Phase 3 PR bodies; reviewer must explicitly accept or reject each deviation | Phase 3 fix-PR delivery (vsdd-factory:fix-pr-delivery) | proposed |
| 2 | When pr-manager returns dispatcher-blocked status on merge, orchestrator MUST present clear option list (manual merge vs UI approval) and ScheduleWakeup polling rather than retrying | S-7.02 codification register | proposed |
| 3 | Quality gate command in story templates must be the EXACT CI command; consider pinning rustup toolchain to match CI | Story-writer template (vsdd-factory:create-story) | proposed |
| 4 | Orchestrator should attempt non-admin merge first; only fall back to admin merge or surface to user when non-admin fails | Orchestrator merge strategy (vsdd-factory:pr-create + deliver-story) | proposed |

---

4. **Dispatcher policy variability — try non-admin merge first** — Wave 0 PRs #293/#294 were dispatcher-blocked at admin merge step (required user manual `gh pr merge --admin` invocation). S-1.01 PR #295 merged cleanly via `gh pr merge --squash --delete-branch` (no `--admin` flag needed). Hypothesis: factory-dispatcher policy is configurable per-PR or per-branch. Codification candidate: orchestrator should attempt non-admin merge first; only fall back to admin merge or surface to user when non-admin fails. This avoids spurious dispatcher blocks on PRs that don't need admin escalation.
   _Discovered: S-1.01 merge (PR #295), 2026-05-07_
   **Confirmed at S-1.02 (PR #296 merged 2026-05-07).** Wave 1 PRs use clean non-admin squash-merge. Pattern: Wave 0 #289-#294 needed `--admin` (or manual user gh CLI); Wave 1 #295/#296 don't. Codification stable; ready for promotion to orchestrator skill update.

---

## Wave 0 Retrospective (2026-05-07)

Wave 0 COMPLETE. 7/7 stories delivered. Final metrics:

- **Stories**: 7 total — 4 MUST-FIX bugs (S-0.01..S-0.04) + 2 security decisions (S-0.05 SD-002, S-0.06 SD-003) + 1 spec-only holdout (S-0.07)
- **PRs**: 6 merged to develop (#289-#294); 1 spec-only on factory-artifacts direct (S-0.07)
- **Tests added**: ~40 new tests (issue_open OAuth URL, worklog pagination, multi-workspace HashMap, multi-profile fields, auth_header_release_gate, verbose_bodies, 2 cli_handler rewrites)
- **Holdouts activated**: H-045, H-046, H-036, H-NEW-MP-001, H-NEW-VERBOSE-001, H-NEW-VERBOSE-002 (all MUST-PASS); H-NEW-AUTH-002 formalized (gated behind JR_RUN_RELEASE_AUTH_GATE_TEST=1)
- **Deferred findings**: 5 open (R1-001, R1-002, S-0.03-S1, S-0.05-F1/F2/F3); S-0.05-DEV resolved in-session
- **Production regressions**: 0; ~151 subprocess integration tests preserved via cfg(debug_assertions) canonization
- **Pattern identified**: Admin merge dispatcher blocks required manual gh CLI invocation for PRs #293 and #294 — recurrence confirmed (Lesson 2). Codification candidate escalated to Wave 1 planning.

_Recorded: S-0.07 delivery, 2026-05-07_

---

5. **Regression-pin discipline provides cheap forward-looking insurance** — Wave 1 holdout suites (S-1.06, S-1.07, S-1.08) each pinned existing behavior before implementing net-new coverage. All tests passed on current develop at time of authoring, confirming no regressions were already present. Writing regression tests while the code is fresh in mind is cheap; catching a future regression they prevent is very cheap. Codification candidate: story template for holdout suites should include explicit AC "all tests pass on HEAD at time of authoring" as merge prerequisite.
   _Discovered: S-1.06/S-1.07/S-1.08, 2026-05-08_

---

## Wave 1 Retrospective (2026-05-08)

Wave 1 COMPLETE. 8 stories delivered (3 facade CI/config + 1 strict observability + 4 strict regression-pin). PRs #295-#302.

- **Mean time-per-story**: ~30-60 min from start to merge (smaller stories faster; S-1.06 OAuth suite took longer due to test breadth)
- **Implementer deviation catch**: S-0.04 cache.rs scope-creep was surfaced and reverted before merge — PR-body Deviations section pattern validated across the full Wave 0→1 arc
- **Mid-PR clippy fix**: S-1.03 required a SHOULD-FIX docstring commit (06c2252) due to Rust 1.95 vs local toolchain skew — Lesson 3 applies here; exact CI flag set should be matched locally
- **Dispatcher pattern**: Wave 1 PRs #295-#302 all merged cleanly via non-admin squash-merge (no admin bypass needed) — Lesson 4 confirmed and stable for Wave 2
- **0 production regressions** across 614 lib + integration tests
- **5 deferred items + 1 PENDING_MANUAL**: manageable; none blocking Wave 2

Lesson 5 candidate: regression-pin discipline — writing tests that pass on current code provides forward-looking insurance against future regressions; cheap to author when code is fresh in mind.

_Recorded: Wave 1 COMPLETE, 2026-05-08_

---

6. **Streamlined PR flow under API instability** — When agent dispatch hits API errors mid-burst, orchestrator can fallback to direct gh CLI for PR creation/merge using the same body content the agent would have generated. Loses some review formality (no separate code-reviewer dispatch) but preserves forward velocity. Pattern validated S-2.02 PR #304 (merged via direct gh after agent API error). Trade-off: regression-pin stories with no source code changes are lower-risk for skipping formal review. Codification: orchestrator skill should document this as approved fallback path.
   _Discovered: S-2.02 PR #304, 2026-05-08_

---

## Wave 2 Progress (partial — 2/7 as of 2026-05-08)

S-2.01 and S-2.02 merged. S-2.03 active. Running metrics:

- **Mean time-per-story**: consistent with Wave 1 (~30-60 min)
- **Regression-pin discipline**: 11 total tests across S-2.01 (7) + S-2.02 (4); all pass on develop at time of authoring
- **S-1.05-AC-001 RESOLVED**: user enabled secret_scanning + push_protection on Zious11/jira-cli (2026-05-08)
- **1 deferred item**: S-2.02-DEFER (transitioned vs changed JSON field name — BC-3.2.001 spec vs actual code; test pinned to actual implementation)
- **Lesson 6 candidate**: API-hiccup fallback to direct gh CLI validated for regression-pin stories with no source code changes

---

## 2026-05-11 — Lessons from PR #348 (issue #110 part 2)

### [codified] Copilot finds data-loss class bugs that all 3 VSDD fresh-context reviewers miss

Round 5 of Copilot review surfaced: `jr issue edit FOO-1 FOO-2 --label add:foo --summary "X"` silently
drops `--summary` because the dispatch routes to `handle_edit_bulk_labels` if `!labels.is_empty()`
without checking for concurrent non-label fields. None of the prior reviewers caught this:
- pr-review-toolkit:code-reviewer (1 pass with full context)
- zclaude:security-reviewer (1 pass focused on attack surface)
- vsdd-factory:adversary x 5 fresh-context passes (3 consecutive CLEAN to declare F5 convergence)

Adversary prompts should explicitly include "silently-dropped flag combinations" and "dispatch
branches that ignore subsequent flags" as review axes. Filed as a self-improvement to the
adversarial-review SKILL.md checklist.

_Discovered: PR #348 Copilot round 5, 2026-05-10_

### [codified] clap `requires` interacts unreliably with `conflicts_with`

When `--max requires = "jql"` is paired with `jql conflicts_with = "keys"`, clap elides the
`requires` constraint when positional `keys` are present. The user passing
`jr issue edit FOO-1 --max 100 --label add:foo` slipped past clap's parse-time check.

Robust pattern: handler-level validation with explicit `JrError::UserError`. The existing
round-5 `--label` + non-label-field guard already uses this pattern. Codify as a CLAUDE.md
gotcha so future clap work doesn't repeat the assumption.

_Discovered: PR #348 Copilot round 8, 2026-05-10_

### [codified] Schema-best-guess + loose `body_string_contains` matchers + deferred empirical verification

PR1 (#325) and PR2 (#348) both ship best-guess Atlassian Bulk API shapes for `priority`,
`issueType`, and `labels`. Tests use `body_string_contains(...)` (loose substring) instead of
`body_partial_json(...)` (structural) so the wrong shape passes tests but fails on a real Jira
tenant. Empirical verification is deferred to a sandbox-required follow-up issue (#331).

Pattern is acceptable when documented as "deferred-pending-sandbox" with the follow-up issue
linked from the SCHEMA NOTES comment, the BulkEditRequest type doc, and the PR description.
Codify the pattern: SCHEMA NOTES → loose matchers → follow-up issue → PR-description disclaimer.

_Discovered: PR #348 F5 adversarial pass 1 (ADV-P5-PR2-010), 2026-05-10_

### [codified] validated-feature-lifecycle skill bypasses VSDD `.factory/` documentation

Both PR1 (#325) and the early phase of PR2 (#348) went through the `validated-feature-lifecycle`
skill which writes only `.factory/code-delivery/issue-NNN/{pr-description.md, review-findings.md}`
and skips the per-cycle adversarial/security/consistency review evidence and lessons codification.

The PR2 mid-flight pivot to VSDD Feature Mode (F1-F7) corrected the agent dispatch (specialist
agents with TDD discipline) but the on-disk audit trail had to be remediated retroactively (this
commit). Codify: orchestrator must dispatch state-manager LAST in every burst per existing
orchestrator constraint, including bursts driven by validated-feature-lifecycle.

_Discovered: PR #348 documentation remediation, 2026-05-11_

---

## 2026-05-11 — Standing rule: Perplexity-validate every Copilot review

### [codified] Always validate Copilot findings with Perplexity BEFORE acting

User-issued rule (2026-05-11, during PR #351 round 1). For each Copilot inline
comment: identify the external-fact claim (stdlib semantics, crate behavior,
API shape, language feature), run `mcp__perplexity__search` with a targeted
query, then act based on validation. Examples from this cycle:
- PR #348 round 2 C1 (claimed compile error): Copilot WRONG — CI was green.
- PR #351 round 1 C1 (`is_err()` semantics): Copilot CORRECT — `Ok("1")` is canonical.
- PR #351 round 1 C2 (COMPLETED not in OpenAPI): Copilot CORRECT.
- PR #351 round 2 C1 (panic too macOS-specific): Copilot CORRECT — keyring crate is cross-platform.

Codified in MEMORY.md as `feedback_perplexity_copilot_reviews.md` for cross-session durability.

### [codified] Long-lived PRs incur develop-drift; CI merge-result builds catch it

PR #351 was branched off `origin/develop` at `f6487ab` BEFORE PR #348 merged.
PR #348 added `failure_reason: Option<String>` to `BulkOperationProgress` post-branch.
Local builds on the stale worktree passed (struct hadn't gained the field there);
CI builds the merge-result and caught the missing field initializer in the
`progress_with_status` test helper. Resolution: rebase onto current develop + add
the missing field. Force-push with `--force-with-lease`. Perplexity-validation then
confirmed the fix was correct and the post-rebase Copilot re-request returned 0 new
comments, demonstrating that fix quality held through a force-push.

Future practice: rebase long-lived PRs onto develop proactively when a sibling PR
with overlapping files merges, OR trust CI's merge-result builds to catch the
divergence (worked here).

_Discovered: PR #351 post-rebase CI failure + fix, 2026-05-11_

---

## 2026-05-11 — PR #352 Round 1 Copilot reply tooling

### [codified] Shell expands backticks inside `gh api -f body` arguments — use `jq -Rs` with `--input -` instead

When replying to Copilot threads via `gh api`, using `-f body="... \`jr issue move\` ..."` is
unsafe even with escaped backticks. The shell evaluates command-substitution sequences before gh
sees the argument. In this case the shell tried to execute `jr issue move`, which exited with
a missing-arguments error; the reply was posted with the substitution slot replaced by the empty
string, producing a subtly wrong reply ("Verified — only accepts positional keys + --to ...").

**Failure example:**
```bash
gh api -X POST repos/.../pulls/352/comments/3220034266/replies \
  -f body="... \`jr issue move\` ..."
# Result: backtick expansion fires; reply posted without "jr issue move" token
```

**Fix applied:** PATCH the comment to correct the text using `printf` + `jq -Rs`:
```bash
printf '%s' '... `jr issue move` ...' \
  | jq -Rs '{body: .}' \
  | gh api -X PATCH repos/.../pulls/352/comments/3220057819 --input -
```

**Rule for all future Copilot reply rounds:** Always use
`printf '%s' '<body text>' | jq -Rs '{body: .}' | gh api -X POST <endpoint> --input -`
when the reply body may contain shell metacharacters (backticks, dollar signs, single quotes).
Never use `-f body="..."` with backticks in the value, even escaped — bash's
command-substitution evaluation happens before gh sees the argument.

_Discovered: PR #352 Round 1 Copilot reply, 2026-05-11_

---

## 2026-05-11 — PR #353 Post-hoc Perplexity Validation

### [candidate] Trivial-refactor PRs that consolidate same-typed-but-distinct-named constants MUST run Perplexity to confirm the underlying constraint is shared

The trivial-changes path (no adversarial review, Perplexity in the skip column) is correct
for mechanical refactors with no design decisions. However, when two constants of the same
type have **distinct names** suggesting they might differ (e.g., `BULK_MAX_KEYS` vs
`BULK_MOVE_MAX_KEYS`), the distinct naming is itself an implicit external-knowledge claim:
the author who originally wrote two names may have believed the underlying constraints
differ, or may have used distinct names defensively for future-proofing.

If both constants happen to have identical values at the time of consolidation, that
coincidence does NOT prove the constraint is shared. The consolidation is a semantic claim
("these two constants represent the same limit") that requires external validation.

**Rule for future trivial-refactor PRs that consolidate constants:**
Even on the trivial-changes path, run Perplexity when consolidating two same-typed
constants with distinct names that imply potentially different external constraints.
The query cost is low; the regression risk of shipping a wrong consolidation is high.

**Validated instance (PR #353, 2026-05-11):**
`BULK_MAX_KEYS` (bulk edit) and `BULK_MOVE_MAX_KEYS` (bulk transition) were both 1000.
Perplexity confirmed both Atlassian endpoints share the 1000 per-call cap:
- https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
  "A single request can accommodate a maximum of 1000 issues (including subtasks)" (bulk edit)
  "You can transition up to 1,000 issues in a single operation" (bulk transition)
- https://developer.atlassian.com/cloud/jira/platform/bulk-operation-additional-examples-and-faqs/
  "The maximum number of issues, including subtasks, that you can update at once is capped at 1000."

Consolidation confirmed correct. No regression. Lesson: the validation step that was
skipped (it was on the trivial path) should be added as a conditional: "trivial path
EXCEPT when consolidating distinct-named constants of the same type — run Perplexity."

_Discovered: PR #353 post-hoc Perplexity validation, 2026-05-11_
_Tagged: [process-gap] — refines the trivial-changes path in validated-feature-lifecycle_
_Status: [candidate] — flagged for human review before promotion to codified rule_

---

## 2026-05-11 — PR #354 R1→R2 Isomorphic-Pattern Gap

### [candidate] When documenting one instance of an isomorphic pattern, check all other instances before pushing

PR #354 documented the `labels` dry-run-vs-POST shape divergence (issue #342). The R1 fix
rewrote the wording of the docstring to remove a self-contradiction. However, the NOTE covered
only `labels`, even though the same dry-run-vs-POST divergence pattern also applies to
`priority` and `issueType` in the same code block. Copilot Round 2 caught this.

The cost of R2 (one extra review round) could have been avoided by applying a pre-push
breadth check: "I am documenting a pattern in one field — are there sibling fields in the
same builder with the same pattern?" In this case, the builder (the dry-run JSON block in
`handle_edit`) handles `labels`, `priority`, and `issueType` in adjacent lines, all using
bare-string representations that diverge from the POST body's wrapped shapes. A single
visual scan of the surrounding builder code (~20 lines) would have surfaced the other two.

**Rule for future docs-only PRs that document a divergence or pattern in one instance:**
Before pushing, scan the surrounding code block (or function) for sibling fields that
follow the same pattern. If found, extend the documentation to cover all instances
uniformly. The cost of broader coverage is a few extra doc lines; the cost of false
completeness is a Copilot round (or worse, misleading documentation that persists undetected).

This rule applies especially when:
- The documented pattern is about a shape divergence between two code paths (e.g., dry-run
  vs POST body, serialization vs deserialization, display vs storage)
- The builder handles multiple fields of the same conceptual type in adjacent code
- The divergence is caused by a systemic design choice (e.g., "best-guess pending sandbox
  verification") rather than a field-specific quirk

**Validated instance (PR #354, 2026-05-11):**
R1 fix covered labels. R2 caught that priority + issueType have the identical dry-run-vs-POST
pattern. The R2→R3 fix (+30 -17 lines) resolved the scope gap with no behavioral change.
Reinforces the iterate-until-clean discipline: a doc fix can itself introduce false
completeness that the next review round surfaces.

_Discovered: PR #354 Copilot Round 2, 2026-05-11_
_Tagged: [process-gap] — refines pre-push review for docs-only PRs that document patterns_
_Status: [candidate] — flagged for human review before promotion to codified rule_

---

## 2026-05-11 — PR #355 R2 Perplexity Calibration

### [codified] Perplexity hallucinated about Rust `{:?}` Debug formatter escape behavior — local empirical verification is authoritative for observable Rust stdlib behavior

**Context:** During PR #355 Round 2 triage, Copilot raised a CWE-117 finding asserting that
`await_bulk_task` interpolated an unvalidated `task_id` into a timeout error message before
`poll_bulk_task`'s call-site validation ran. Per DEC-018, ran Perplexity validation before
acting: queried whether Rust's `{:?}` Debug formatter for `&str` escapes ASCII control
characters (`\r`, `\n`, `\0`, `\t`, ANSI escape sequences), and whether `{:?}` constitutes a
defense against CWE-117.

**Perplexity result (INCORRECT):** Perplexity responded with high confidence that `{:?}` does
NOT escape control characters for `&str`, claiming "control chars render literally" and that
`{:?}` "fails CWE-117." Citations pointed to `https://doc.rust-lang.org/std/fmt/trait.Debug.html`
and similar correct documentation, but the behavioral claim was factually wrong.

**Local empirical verification (CONTRADICTED Perplexity):** Ran a 5-line Rust program:
```rust
fn main() {
    let s = "abc\r\ndef\0\t\x1b[31mred\x1b[0m";
    println!("Display: {}", s);  // renders literal control chars
    println!("Debug:   {:?}", s); // outputs: "abc\r\ndef\0\t\u{1b}[31mred\u{1b}[0m"
}
```
Output via `| cat -v` confirmed Rust's Debug formatter for `&str` DOES escape:
`\r` → `\r`, `\n` → `\n`, `\0` → `\0`, `\t` → `\t`, `\x1b` → `\u{1b}`
via `str::escape_debug`. Perplexity's claim was a hallucination.

**Fix decision:** Rather than debate Display vs Debug, the correct defense was to call
`validate_task_id(task_id)?` at the VERY START of `await_bulk_task`, before the deadline
computation. This guarantees ALL interpolation sites inside the function see only
ASCII-allowlisted input — making the Display vs Debug formatter choice moot. Fix commit:
62766f4 (+10 lines). This is a cleaner defense-in-depth posture than relying on formatter
escape behavior.

**Calibration rule:** For any Rust language/stdlib behavior question answerable by a 5-line
program, run the program. Perplexity is reliable for external API semantics, CWE class
definitions, and RFC specifications, but has demonstrated a pattern of hallucinating about
observable Rust language/stdlib behavior while citing correct documentation URLs. This is the
third documented instance of this pattern in this codebase (prior: Rust module visibility,
insta snapshot naming, environment variable syntax).

**Standing rule unchanged:** DEC-018 (Perplexity-validate Copilot reviews) is still correct;
it produced the right answer in R1 (confirmed RFC 3986 §5.2.4 path-confusion) and the right
final outcome in R2 (empirical local verification caught the hallucination before the wrong
diagnosis was acted on). The tiered-validation strategy — Perplexity first, empirical
verification when Perplexity's claim is about observable behavior — is the correct procedure.

_Discovered: PR #355 Round 2, 2026-05-11_
_Tagged: [codified] — third documented instance of the Perplexity-vs-empirical pattern_
_Tiered-validation rule reinforced: Perplexity for external API/CWE/RFC; local empirical verification for Rust stdlib behavior_

---

## 2026-05-11 — PR #356 R1–R4 Process Gaps

### [codified] Inconsistent Perplexity-validation undermines DEC-018

Across 4 Copilot rounds on PR #356, Perplexity-validation was applied to R1 (cited CWE-117
and OWASP length-capping guidance) and R4 (cited `Cow<str>` idiom per Rust API Guidelines
C-COST) but SKIPPED on R2 and R3. The rationalization was that R2 and R3 findings were
"empirically verifiable from the code" (arithmetic: 1025 > 1024 + 30; 1 byte → 4 bytes).

This is exactly the failure mode DEC-018 was designed to prevent. The standing rule is
"always validate Copilot reviews with Perplexity" — it applies regardless of how obvious
the claim looks at first glance. The R1 and R4 validations both added context (OWASP
defense-in-depth citation, Cow<str> idiom naming) that improved the fix justification.
The R2 and R3 skips simply incurred process-gap debt without any time saving — the code
analysis was still done; only the external citation was omitted.

**Calibration rule added:** Per Copilot review round, run Perplexity validation on at
least one external-claim aspect of EACH finding, even if the finding looks code-internal.
Common external-claim aspects that are easy to miss: CWE/OWASP confirmation, stdlib/crate
behavior, RFC or spec reference, idiomatic Rust pattern name.

_Discovered: PR #356 post-round-4 audit remediation, 2026-05-11_
_Tagged: [codified] — refines DEC-018 calibration; confirms standing rule applies to all rounds_
_Process gap: Perplexity skipped on R2 + R3 for PR #356_

### [codified] Skipping state-manager between Copilot rounds creates audit-trail debt

After PR #356 opened, state-manager was dispatched once (Burst N+2, initial open). For all
4 subsequent Copilot rounds (R1 fix commit 51e2807, R2 fix commit d061b14, R3 fix commit
274961c, R4 fix commit fe25e22), state-manager dispatch was skipped entirely. Documentation
was deferred to "the next merge event." This produced an out-of-band remediation pass
(this commit) costing more than 4 incremental dispatch calls would have cost.

This is the same pattern that earlier in the cycle prompted "are all the changes running
through the vsdd factory or at least getting documented in the process correctly" — and the
same answer (no) applies here.

**Calibration rule added:** Dispatch state-manager AFTER EACH Copilot-round fix commit,
not just after PR open and convergence. The marginal cost is one Agent tool call per round;
the marginal benefit is a real-time audit trail that lets the user introspect the cycle at
any point without a retroactive remediation pass.

_Discovered: PR #356 post-round-4 remediation dispatch, 2026-05-11_
_Tagged: [codified] — refines orchestrator dispatch discipline for Copilot-round bursts_
_Process gap: state-manager skipped after R1, R2, R3, R4 fix commits for PR #356_

---

## 2026-05-12 — PR #356 R14 Doc-Fallout Cluster

### [codified] When a major behavioral change expands escape/encoding coverage, audit ALL doc comments in the same commit

PR #356 R14 switched `sanitize_for_stderr` from `is_ascii_control()` to `char::is_control()`,
adding Unicode C1 control escaping (U+0080..U+009F → `\u{NNNN}` format). This was a legitimate
defense-in-depth improvement (Perplexity CONFIRMED). However, the R14 fix commit updated only
the implementation and the immediately adjacent inline comments — it did not audit all other
documentation sites that described the escape behavior.

**Result:** 4 subsequent rounds (R15, R16, R17, R18) were consumed exclusively by documentation
cleanup caused by this single omission:
- R15 (2 findings): fast-path comment still described byte-level scan; stale R-number annotation
- R16 (3 findings): strategy bullets described only ASCII path; C1 description technically wrong
  ("rejected as invalid UTF-8" — actually valid Unicode, terminals ignore semantics); integration
  test comment said "only ASCII control bytes are escaped"
- R17 (1 finding): integration-test header comment said chars render "as \xNN literals"
- R18 (1 finding): extract_error_message public-API doc comment described only ASCII branch

All 7 of these findings were purely from the R14 behavioral change. All were valid. None required
Perplexity validation (internal comment accuracy only). Each was individually small, but together
they consumed 4 rounds that could have been avoided.

**Rule:** When a commit changes the behavior of an escape, encoding, validation, or classification
function — particularly when it EXPANDS the set of values that are handled differently — the
implementer MUST perform a project-wide grep for all documentation sites that describe the old
behavior and update them in the SAME commit.

**Minimum sweep for escape-set expansions:**
```bash
grep -rn "escape\|sanitize\|control\|ASCII\|unicode\|C1\|C0\|DEL\|\\\\x" \
  src/ tests/ --include="*.rs" | grep -i "comment\|//\|doc"
```

Or more specifically: search for any comment, doc-string, or test header that references the
function name or the old escape format (e.g., "\\xNN", "ASCII control", "is_ascii_control").

**Anti-pattern name:** doc-fallout cluster — when a behavioral change produces a cascade of
documentation-only findings in subsequent review rounds.

**Prediction value:** If you see R15+ findings that are ALL documentation accuracy (no
behavioral gaps), you are in a doc-fallout cluster from an earlier behavioral change. Identify
the root-cause commit and do a complete doc sweep rather than patching sites one at a time.

_Discovered: PR #356 R14-R18 pattern analysis at R19 convergence, 2026-05-12_
_Tagged: [codified] — new lesson; R14 behavioral change produced 4-round doc-fallout cluster (R15:2+R16:3+R17:1+R18:1=7 findings)_
_Scope: any commit that changes the behavior of escape, encoding, validation, or classification functions_

---

## 2026-05-12 — PR #357 Retroactive Dispatch (Lessons 1+2 Recurrence)

### [codified] Lesson 1 addendum: "pattern already in same file" is a rationalization, not an exemption

PR #357 implemented issue #335 (release-gate `JR_BASE_URL` behind `#[cfg(debug_assertions)]`).
The fix is a direct mirror of the existing `JR_AUTH_HEADER` gate in the same file (~line 72),
established under SD-002. The rationalization for skipping Perplexity pre-validation was:
"pattern already established in same file — behavior is known."

This is the same class of reasoning DEC-018 was designed to prevent: the standing rule is
"always validate Copilot reviews with Perplexity" — DEC-018's spirit extends to any external
claim made in the design of a fix, not only to Copilot review triage. In this case the
external claim is: "`#[cfg(debug_assertions)]` is the correct compile-time gate and cannot be
accidentally enabled in a release build."

Perplexity validation (run retroactively after user course-correction) surfaced a non-obvious
caveat: `debug-assertions = true` CAN be set in `[profile.release]` in Cargo.toml. The fix
is sound ONLY because the project's Cargo.toml has no such override. Skipping the validation
step meant this caveat was verified after the fact rather than before. The fix happened to be
correct, but the audit trail was incomplete.

**Rule clarified:** Even for "mirror this existing pattern" fixes, run Perplexity on at least
one external-claim aspect before opening the PR. The cost is one search query. The benefit is
an explicit caveat check (e.g., Cargo.toml override) that makes the fix verifiably sound
rather than coincidentally sound.

_Discovered: PR #357 retroactive validation, 2026-05-12_
_Tagged: [codified] — addendum to Lesson 1 / DEC-018; same rationalization pattern ("obvious from file context") as R2/R3 skips on PR #356_

---

### [codified] Lesson 2 addendum: state-manager dispatch is required at PR creation, not only per-Copilot-round

The prior codification of Lesson 2 (PR #356 post-round-4 remediation) established: dispatch
state-manager AFTER EACH Copilot-round fix commit, not just at PR open and convergence.

PR #357 adds a second failure mode: state-manager dispatch was skipped at PR creation
entirely, deferring the first audit-trail entry until a user course-correction prompted
this retroactive dispatch. The rationalization was "this is a small 8-line fix; state
updates are for bigger PRs."

There is no size threshold for state-manager dispatch. The rule is:
1. Dispatch state-manager when the PR is opened (record: branch, head SHA, issue, scope, test results).
2. Dispatch state-manager after each Copilot-round fix commit (record: round N, findings, fix SHA).
3. Dispatch state-manager when the PR is merged (record: merge SHA, issue closed).

"Small fix" is not an exemption. The audit trail purpose is to capture ALL in-cycle work so
that any session resume can reconstruct full context from STATE.md + burst-log without
visiting GitHub. A missing PR-creation entry leaves a gap regardless of diff size.

_Discovered: PR #357 retroactive dispatch, 2026-05-12_
_Tagged: [codified] — addendum to Lesson 2; extends the per-Copilot-round rule to include the PR creation event itself_

---

## 2026-05-12 — PR #357 R1 Process Gap

### [codified] Lesson 1 sub-lesson: "Perplexity validates the APPROACH; grep validates the SURFACE AREA"

**Context:** PR #357 (issue #335) implemented `#[cfg(debug_assertions)]` gating on
`JR_BASE_URL` in `src/api/client.rs`. The approach was Perplexity-validated (retroactively,
per the Lesson 1 addendum above) and confirmed correct: `#[cfg(debug_assertions)]` is the
idiomatic compile-time gate; `cargo build --release` reliably disables it; Cargo.toml has no
`debug-assertions = true` override.

**The gap:** The approach was correct. The surface area was incomplete. `JR_BASE_URL` is read
in TWO places in the codebase:

1. `src/api/client.rs` — `JiraClient::new` base-URL override (the read site that was edited)
2. `src/config.rs:357` — `Config::base_url()` (the primary read site, missed entirely)

A `grep -rn JR_BASE_URL src/` before pushing cb3e8a3 would have revealed both sites.
That grep was not run. Copilot caught the missed site in R1 (comment 3223330261, CRITICAL).

**Concrete failure sequence:**
1. Identified the env-var read in `src/api/client.rs` — the one that was touched in
   the original SD-002 gating work for `JR_AUTH_HEADER`.
2. Applied the gate to that one site.
3. Perplexity confirmed `#[cfg(debug_assertions)]` is correct — approach validated.
4. Pushed cb3e8a3 without grepping for other read sites.
5. Copilot R1 caught `src/config.rs:357` — token-leak vector remained open.

**Rule:** For security-sensitive env-var gating, the workflow is:
1. **Perplexity**: validate the compile-time gate APPROACH (idiomatic? correct gate for this
   use case? Cargo.toml clean? Prior art?).
2. **grep**: validate the SURFACE AREA — `grep -rn <VAR_NAME> src/` to find ALL read sites
   before claiming the gate is complete.
3. Apply the gate to every read site found in step 2.
4. Re-run grep to confirm no sites remain ungated.

**Generalization:** This sub-lesson applies beyond env-var gating. Any security fix that
addresses "how X is done" (the approach) must also audit "everywhere X is done" (the surface
area). Perplexity can validate the approach; only a codebase-wide search validates the
surface area.

**Concrete example:** PR #357 R1 — gated one of two `JR_BASE_URL` read sites; Copilot
caught the second in one round. Fix cost: 1 extra Copilot round + additional test file.
Prevention cost: 1 `grep -rn JR_BASE_URL src/` command before pushing.

_Discovered: PR #357 R1 Copilot finding 3223330261 (CRITICAL), 2026-05-12_
_Tagged: [codified] — sub-lesson under Lesson 1 / DEC-018; "Perplexity validates APPROACH; grep validates SURFACE AREA"_
_Scope: all security-sensitive env-var gating; generalizes to any "fix how X is done → audit everywhere X is done" class_

---

## 2026-05-12 — PR #357 MERGE: Successful Application of Doc-Fallout Lesson

### [confirmed-applied] Doc-fallout lesson (PR #356 R14-R18) was successfully applied in PR #357

**Context:** The doc-fallout cluster lesson was codified during PR #356 R19 convergence
(2026-05-12). It states: "When a commit changes the behavior of an escape, encoding,
validation, or classification function — particularly when it EXPANDS the set of values
handled differently — the implementer MUST audit ALL documentation sites describing the
old behavior and update them in the SAME commit."

**PR #357 application:** Commit 144aaff (the R1 fix) updated three artifacts atomically:
1. `src/config.rs` — added `#[cfg(debug_assertions)]` gate to `Config::base_url()`
2. `tests/base_url_release_gate.rs` — created 4 regression tests (test_335_*)
3. `CLAUDE.md` — updated "AI Agent Notes" to document two-site gating

The CLAUDE.md update in the SAME commit as the code fix is the direct application of
the doc-fallout lesson. In PR #356, the R14 behavioral change updated only the implementation
and immediately adjacent inline comments — producing 4 subsequent rounds (R15-R18: 7 findings)
of documentation-only cleanup. PR #357 avoided this entirely.

**Outcome:** PR #357 converged in 2 rounds (vs PR #356's 19). Round counts:
- Rounds attributable to missing doc sync (doc-fallout class): 0
- Rounds attributable to the substantive security gap (Config::base_url() ungated): 1
- Rounds to confirm convergence: 1
Total: 2.

**Quantified benefit:** The doc-fallout lesson, applied for the first time here, avoided
at minimum 4 documentation-only review rounds. The investment was 4 CLAUDE.md lines added
to a commit that was already being written.

**Conclusion:** The lesson-to-practice loop closed for the first time in this cycle with PR #357.
Lesson codified in R19 (PR #356), applied in R1 (PR #357), verified effective at merge.

_Confirmed applied: PR #357 merge @ d208a6d, 2026-05-12T03:03:12Z_
_Tagged: [confirmed-applied] — first successful application of the doc-fallout cluster lesson; closes the lesson-to-practice loop_
_Reference: doc-fallout cluster lesson (2026-05-12 PR #356 R14-R18 section above)_

---

## 2026-05-12 — PR #358 R3 Doc-Fallout Sub-Lesson (Second Cluster in 2 Days)

### [codified] Sub-lesson: grep narration-style comments (Strategy:, Logic:, etc.) before pushing a behavior-expanding commit

**Context:** PR #358 R3 returned 2 findings, both doc-fallout from R2's tolerant-matcher commit
(c708211). This is the SECOND doc-fallout cluster in 2 consecutive PRs in 2 days — PR #356
R15–R18 was the first (4 rounds, 7 findings from the R14 behavioral change).

**Root cause:** The doc-fallout lesson was codified during PR #356 R19 convergence and
successfully applied in PR #357 (same-commit CLAUDE.md update). But it was NOT applied in
PR #358 R2, even though R2 was a behavior-expanding commit that introduced the
`is_matching_closing_brace` closure.

**Why it was missed:** The strategy doc and `Logic:` block describing the old behavior were
located ~15 lines above the changed closure in the same function. When the closure was edited,
the implementer did not scroll up to re-read the strategy narration. The changed code and its
narration were in different visual paragraphs — close enough to be in scope, far enough to be
skipped without a deliberate audit.

**The gap in the existing doc-fallout lesson:** The existing lesson focuses on "audit ALL doc
comments in the same commit after a behavior expansion." This is necessary but not sufficient.
The harder sub-problem is identifying which comments to audit when the changed code has
narration-style commentary (Strategy:, Logic:, Note:, Algorithm:, etc.) that describes the
implementation in natural language. These prose blocks are more expensive to keep in sync than
inline `//` comments because they are written as durable explanations, not just annotations.

**Sub-lesson rule:** Before pushing any commit that changes the behavior of a function that
has narration-style comments (blocks labeled `Strategy:`, `Logic:`, `Note:`, `Algorithm:`,
`Approach:`, or equivalent prose description), run a targeted grep to find all such blocks
in the file and verify each one still accurately describes the post-change behavior:

```bash
grep -n "Strategy:\|Logic:\|Algorithm:\|Approach:\|Note:\|Overview:" src/<file>.rs
```

Review every match in the same function and its immediately surrounding context. If any
narration describes a behavior the commit changes, update it in the same commit.

**Why this is distinct from the existing doc-fallout lesson:** The existing lesson triggers
on "escape, encoding, validation, or classification function" changes. The sub-lesson triggers
on ANY behavior-expanding commit to a function with prose-style narration comments — including
test helpers, parser functions, and string matchers. Prose narration is more durable (intended
to survive multiple edits) and therefore more likely to go stale after a behavioral change.

**Concrete example (PR #358 R2 → R3):**
- R2 changed: the `is_matching_closing_brace` closure (behavior: exact `"    },"` → tolerant
  trim_start + flexible closer)
- Not changed: the `Strategy:` block above the function describing "8-space indent + `},` exact
  close" behavior; the `Logic:` annotation referencing "8-space indent (clap variant fields use
  8-space indent)"
- Cost: 1 extra Copilot round (R3: 2 findings, both documentation-only)
- Prevention: 1 `grep -n "Strategy:\|Logic:" src/cli/issue/create.rs` + 2 doc lines updated

**Quantified pattern:** Second doc-fallout cluster in 2 PRs; combined cluster cost: R15–R18
(PR #356, 4 rounds, 7 findings) + R3 (PR #358, 1 round, 2 findings) = 5 extra rounds,
9 documentation-only findings. Both clusters were preventable by a pre-push grep step.

_Discovered: PR #358 R3 post-analysis, 2026-05-12_
_Tagged: [codified] — sub-lesson under the doc-fallout cluster lesson; second occurrence in 2 days_
_Scope: any commit that changes behavior in a function with narration-style (Strategy:/Logic:/etc.) prose comments_
_Root-cause: changed code and its narration were in different visual paragraphs; no pre-push narration grep was run_
