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

---

## 2026-05-12 — PR #358 R4: First Copilot False-Positive in Session

### [codified] Empirical-first when Copilot's claim seems counterintuitive — pushing back with evidence is part of the discipline, not a deviation from it

**Context:** PR #358 Round 4 was the **first Copilot false-positive in 30+ rounds in this session.**
Copilot review 4269011038, comment 3223599553 claimed that `include_str!("../mod.rs")` in
`src/cli/issue/create.rs` reads `src/cli/issue/mod.rs` (the "wrong" file), asserting the meta-test
would fail to find the `Edit` enum variant and panic.

**Why the claim was counterintuitive:** The test has been passing with 0 failures since its introduction
in this PR. If the path were wrong and the file were `src/cli/issue/mod.rs`, the test would fail
immediately because that file contains no `IssueCommand::Edit` variant definition. The claim
contradicted observable CI behavior.

**Empirical verification:** A temporary probe test was added that printed the byte length and first 5
lines of `include_str!("../mod.rs")`. Result: **27619 bytes**, first lines `pub mod api;`, `pub mod
assets;`, etc. — that is `src/cli/mod.rs` (27619 bytes), NOT `src/cli/issue/mod.rs` (3056 bytes).

**Perplexity cross-check:** Confirmed the Rust Reference defines `include_str!` paths as relative to
the filesystem directory of the source file. From `src/cli/issue/create.rs`, `..` resolves to
`src/cli/`, so `../mod.rs` = `src/cli/mod.rs`. Unambiguous.

**Counterfactual (without verification):** Acting on Copilot's claim would have changed
`../mod.rs` to `../../mod.rs`, which from `src/cli/issue/create.rs` resolves to
`src/cli/../../mod.rs` = `src/mod.rs` — a file that does not exist. The "fix" would have **broken**
the working test.

**Prevention protocol:**

1. **Probe first for path/file-content claims.** If Copilot asserts that a path resolves to file X,
   write a minimal probe (print byte count + first N lines of `include_str!` or similar) and run it
   before any code change. The probe is cheap (add one `#[test]`, `cargo test probe`, remove it);
   the cost of acting on a wrong claim is at minimum a broken test.

2. **Perplexity for language-reference semantics.** For claims about Rust's path resolution,
   module system, or stdlib behavior, run a Perplexity query targeting the official Rust Reference
   or The Rust Book. These are stable, well-documented semantics.

3. **Apply both when the claim is counterintuitive.** A claim is "counterintuitive" when it
   contradicts observable behavior (e.g., passing CI) or a firmly held language understanding.
   In that case, apply both the empirical probe AND the Perplexity cross-check before deciding
   whether to act, push back, or escalate.

4. **Pushing back with evidence is part of the discipline.** DEC-018 ("always validate Copilot
   reviews with Perplexity or empirical verification") exists to prevent wrong fixes as much as
   to confirm correct ones. Resolving a thread as "not-applicable" with a documented evidence
   trail (byte count, file name, reference citation) is the correct outcome for a false-positive.
   It is not a deviation from DEC-018 — it is its successful application.

**Meta-rule:** The empirical-first / Perplexity-validate discipline is equally important for
catching false-positives as it is for catching real bugs. The 30 prior rounds in this session
where Copilot was correct are evidence of signal quality; they are not a reason to lower the
verification bar. Maintain the bar uniformly.

**Concrete example (PR #358 R4, 2026-05-12):**
- Copilot claimed: `include_str!("../mod.rs")` reads `src/cli/issue/mod.rs`
- Empirical probe: 27619 bytes, starts `pub mod api;` → this is `src/cli/mod.rs`
- Perplexity: `include_str!` paths relative to source file's directory; `..` from
  `src/cli/issue/create.rs` = `src/cli/` → `../mod.rs` = `src/cli/mod.rs`
- Action: reply with proof, resolve thread as not-applicable, no code change
- Avoided: changing a correct path to a broken one

_Discovered: PR #358 R4, 2026-05-12 — first false-positive in 30+ Copilot rounds this session_
_Tagged: [codified] — new lesson; captures the empirical-first pattern for false-positives_
_Scope: all Copilot review rounds where the claim seems counterintuitive or contradicts observable behavior_
_Prevention: probe test + Perplexity reference check + reply with evidence + resolve as not-applicable_

---

## 2026-05-15 — PR #367 / Issue #365 (search_issue_keys + search_issues dedupe) F7 Pre-Merge Lessons

### L-365-1 [codified] F5 multi-axis review panel missed O(N²) algorithmic complexity issue; Copilot caught it

**Context:** PR #367 (issue #365, in-function dedupe for `search_issue_keys` + `search_issues`)
passed F5 with 3 adversarial CLEAN passes + code-reviewer CONVERGENCE_REACHED + security
LOW-RISK APPROVE — a full multi-axis review. None of the F5 reviewers flagged an O(N²)
algorithmic pattern: `Vec::retain` called with a per-iteration `HashSet::contains` check that
rebuilt the seen-keys set incrementally, but the retain call itself iterated over the entire
retained vec on each invocation. Net effect: O(N²) time in the worst case for large result sets.

Copilot Round 2 (F6) caught this. The fix was to maintain an external `seen_keys: HashSet`
that is built incrementally and tested with `.insert()` (returns false if already present),
replacing the retain-based pattern entirely. This is an O(N) algorithm.

**Why F5 missed it:** Adversarial review (including code-reviewer axis) focuses on
correctness, security, and BC conformance. Algorithmic complexity is a distinct review axis
that is not explicitly covered by the current F5 multi-axis reviewer lineup. The code was
functionally correct; only the time complexity was suboptimal.

**Lesson:** F5 should consider adding a performance/complexity-axis reviewer for any feature
that introduces collection-processing loops (filter, dedupe, sort, accumulate patterns) over
potentially large datasets (API response pages of N=1000+). Alternatively, trust Copilot to
catch this class of issue downstream rather than block at F5. The downstream catch worked
here with no production consequence (PR not yet merged when caught).

**Observation vs. rule:** This is codified as an observation, not an engine-level rule change.
No engine change required at this time. File as DRIFT-006 for orchestrator to consider at
next F5 dispatch design discussion.

_Discovered: PR #367 Copilot Round 2, 2026-05-15_
_Tagged: [codified] — observation; no engine-level rule change at this time_
_Related: DRIFT-006 (F5 multi-axis review missed O(N²) issue in PR #367)_

---

### L-365-2 [codified] F1 product-owner phase boundary violation — BC files are F3's job, not F1's

**Context:** During F1d Round 2 (Pass 14), the product-owner dispatched under the F1 phase
directly edited `.factory/specs/prd/` BC files — adding BC-2.6.051 and updating the BC count.
This is outside the F1-phase scope boundary: F1 produces specs (`.factory/specs/`), research
(`.factory/research/`), and holdout scenarios — NOT BC catalog files. BC files belong to the
F3 implementer phase, which formalizes BCs as part of TDD delivery.

The violation was caught during orchestrator review of `git status`, which showed modified
`.factory/specs/prd/` files in the product-owner worktree. The fix was `git restore` on the
modified BC files, and the product-owner was asked to forward the BC-anchor recommendation
as a free-text observation in the spec (for F3 to act on), rather than editing the BC catalog
directly.

**Why it happened:** The F1 product-owner spec for #365 explicitly mentioned BC anchoring
requirements (new BC-2.6.051 needed for `search_issues` dedupe guarantee). The product-owner
agent interpreted this as permission to create the BC directly.

**Lesson:** Future product-owner dispatches should include an explicit constraint when the
spec touches BC catalog requirements: "DO NOT touch `.factory/specs/prd/` — BC catalog
updates are F3's job. Record BC anchor requirements in the feature spec as a free-text
observation (e.g., 'F3 should create BC-2.6.051 for...')." This prevents the boundary
violation before it occurs rather than catching it in git status.

**Systemic reinforcement:** "F1 phase produces specs/research artifacts only; BC files belong
to F3 implementer." This is an existing engine constraint that was not communicated clearly
enough in the product-owner dispatch prompt for this cycle.

_Discovered: #365 F1d Round 2 Pass 14 — product-owner BC file scope violation caught via git status, 2026-05-15_
_Tagged: [codified] — engine-level prompt constraint for future product-owner dispatches when spec mentions BC anchoring_

---

### L-365-3 [codified] Cascade doc-fallout from algorithmic refactor — grep sweep all OLD mechanism terminology immediately

**Context:** Copilot Round 2 (F6) replaced the O(N²) `Vec::retain` + per-iteration HashSet
rebuild pattern with an incremental external `seen_keys: HashSet` in both `search_issue_keys`
and `search_issues`. This was the correct algorithmic fix. However, references to the old
mechanism ("retain", "HashSet retain", "Vec::retain") existed in multiple locations:
- Test doc comments describing the retain-based strategy
- CLAUDE.md inline notes referencing the old approach
- The feature spec (`docs/specs/`) still describing the old algorithm
- Inline `//` comments in the implementation describing the strategy

Copilot Rounds 3 and 4 were consumed almost entirely by documentation cascade from this
single algorithmic change. Each round surfaced 1-3 stale references that the previous
round's fix had missed.

**Lesson (inverse of "documentation must lead implementation"):** When an algorithmic refactor
lands — particularly one that replaces a named mechanism (`retain`, `HashSet contains`, `Vec::dedup`)
with a different named mechanism (`seen.insert()`, incremental accumulation) — immediately
perform a project-wide grep sweep for ALL occurrences of the OLD mechanism's terminology:

```bash
grep -rn "retain\|per.iteration\|HashSet::contains\|dedup" \
  src/ tests/ docs/ CLAUDE.md --include="*.rs" --include="*.md"
```

Update every reference in the SAME commit as the algorithmic change, or in a single follow-up
commit before requesting the next review round. Do NOT rely on the reviewer to find them
one at a time — this is the cascade anti-pattern that consumed 2 Copilot rounds.

**Naming:** "Algorithmic refactor doc cascade" — when renaming/replacing an algorithm produces
a wave of stale references to the old algorithm's name, idiom, or characteristic code pattern.
Preventable by a pre-request grep sweep using the OLD terminology as the search target.

**Quantified cost:** 2 extra Copilot rounds (R3: doc cascade; R4: remaining cascade) for a
refactor where all behavioral findings were resolved in R2. Prevention cost: 1 grep command
+ update-in-lockstep before R3 request.

_Discovered: PR #367 Copilot Rounds 3-4 pattern analysis, 2026-05-15_
_Tagged: [codified] — algorithmic refactor doc cascade; inverse of doc-fallout cluster lesson (PR #356 R14-R18)_
_Rule: when an algorithmic refactor lands, immediately grep the OLD mechanism's terminology project-wide and update all references in the same commit or a single follow-up before the next review round_

---

### L-365-4 [codified] Long F1d convergence (17 passes) driven by genuine spec-quality findings — pattern works, consider early cross-file invariant checks

**Context:** Issue #365 F1d convergence required 17 total passes across 2 rounds — the longest
F1d convergence in cycle-001 (previous longest: #350 at 11 passes). The 17 passes were
attributable to:
- Round 1 (P1-P11, 11 passes): itertools::unique() consecutive-only behavior misrepresented in
  spec; repeated caller-list errors; BC anchor cross-references missing.
- Round 2 (P12-P17, 6 passes): scope expansion to `search_issues`; caller-list factual errors
  (P13); BC-2.6.051 semantic anchoring required a new BC (P13); BC count propagation BLOCKING
  (P14 — count not propagated to ARCH-INDEX + BC-INDEX); product-owner scope-violation required
  revert (P14). Pass P15 was first CLEAN after revert; P16 had 2 NITs; P17 was fully CLEAN.

**Assessment:** No adversary noise was observed — every pass surfaced substantive spec-quality
findings. The 17-pass trajectory reflects genuine spec complexity: a 2-function feature with
cross-cutting BC implications (3 BC files affected), a scope expansion mid-round, and a BC
count propagation requirement that requires updating 4+ index files.

**Lesson:** The F1d pattern works correctly for this class of feature. However, consider
two front-loading strategies to reduce pass count:

1. **Greppable invariant pre-check before Pass 1:** For features that add or modify BCs,
   run the check-spec-counts script (`scripts/check-spec-counts.sh`) BEFORE dispatching
   the adversary. A count propagation error (like the P14 BLOCKING finding) would be caught
   before consuming a pass.

2. **Caller-list verification by grep before Pass 1:** For features that add new API functions,
   verify the caller-list section of the spec against actual callers via
   `grep -rn "<function_name>" src/` before F1d dispatch. Caller-list errors recurred across
   P1, P5, and P13 — all were greppable without adversary intervention.

These are "cheap pre-pass checks" that the orchestrator or product-owner can run to eliminate
a class of greppable invariant violations before F1d begins, potentially front-loading 3-5
passes worth of mechanical fixes into a pre-pass sweep.

**No engine-level rule change at this time.** Observation codified for the orchestrator to
consider at next F1d dispatch for a BC-touching feature.

_Discovered: #365 F1d round-2 convergence retrospective, 2026-05-15_
_Tagged: [codified] — observation; suggests greppable pre-pass invariant checks (check-spec-counts + caller-list grep) to front-load mechanical fixes before F1d dispatch_
_Scope: F1d convergence for features that add BCs or new public API functions with caller lists_

---

## 2026-05-15 — Cycle 3-feature-search-issue-keys-dedupe-365 Close Summary

### L-365-summary [codified] Cycle 3-feature-search-issue-keys-dedupe-365 closed at PR #367 / e193c16 — full VSDD lifecycle in single cycle

**Cycle closed:** PR #367 MERGED @ e193c16 (squash, 2026-05-15T17:51:09Z; closes #365).

**Overall trajectory:** F1d 17 passes (most in cycle-001 history) → F5 4 passes CONVERGED → F6 5 Copilot rounds CONVERGED → MERGED.

**Notable shape:**

- **F1d longest convergence in cycle-001 (17 passes, 2 rounds):** Driven by genuine spec-quality findings — no adversary noise. Round 1 (P1-P11): itertools::unique() consecutive-only behavior misrepresented; repeated caller-list errors; BC anchor cross-references missing. Round 2 (P12-P17): mid-cycle scope expansion (user approved extending dedupe symmetrically to `search_issues` — DP-4 reversal); caller-list factual errors; BC-2.6.051 creation required; BC count propagation BLOCKING at P14 requiring 4-file index sweep; product-owner BC scope violation caught via git status and reverted. L-365-4 codifies pre-pass greppable invariant checks as a front-loading strategy.

- **F5 (4 passes): clean convergence but missed performance issue:** Adversary 3-clean + code-reviewer CONVERGENCE_REACHED + security LOW-RISK APPROVE. The 3-reviewer panel did not flag the O(N²) `Vec::retain` + per-iteration HashSet rebuild pattern. L-365-1 codifies this as an observation: F5 coverage axes do not currently include algorithmic complexity for collection-processing loops.

- **F6 (5 Copilot rounds): R2 caught real algorithmic improvement:** Round 2 identified the O(N²) issue and proposed the incremental external `seen_keys: HashSet` pattern (O(N)). This is the first instance in cycle-001 where F6 surfaced a substantive correctness-class improvement that the full F5 panel missed. Rounds 3-4 consumed by doc cascade from the R2 algorithmic refactor — L-365-3 codifies the algorithmic refactor doc cascade anti-pattern and the grep-sweep prevention rule. Round 5 clean.

**Lessons codified this cycle:** L-365-1 (F5 missed O(N²)), L-365-2 (F1 product-owner BC boundary violation), L-365-3 (algorithmic refactor doc cascade), L-365-4 (long F1d convergence — greppable pre-pass checks).

**Drift items produced:** PG-365-1 (BC Trace stale-count pattern), PG-365-2 (F1d adversary citation-verification scope — engine-level), DRIFT-006 (F5 multi-axis review gap for complexity issues).

**Process validation:** VSDD Feature Mode F1-F7 worked as designed for a 2-function feature with cross-cutting BC implications. The full audit trail (spec, BC catalog updates, adversarial reviews, Copilot rounds, lessons) is preserved in `.factory/cycles/cycle-001/adversarial-reviews/issue-365-search-issue-keys-dedupe/`.

_Recorded: cycle 3-feature-search-issue-keys-dedupe-365 close, 2026-05-15T17:51:09Z_
_Tagged: [codified] — cycle summary; applied lessons L-365-1..L-365-4 for future reference_

---

## 2026-05-15 — PR #369 / PG-365-1 Chore Process Post-Mortem

### L-PG365-1-process [codified] "Trivial change" rationalization bypasses VSDD discipline and produces predictable downstream defects

**Context:** PR #369 (PG-365-1 chore) was treated as a "trivial" change and shortcut to single-pass adversary + pr-manager — no F1 spec, no F2 story, no F3 red gate, no F5 multi-axis convergence, no F1d/F5 3-clean discipline.

**Result:** 7 Copilot rounds with 9 valid findings, including R4 catching the same `Source:` field coverage gap that the orchestrator's single adversary pass had explicitly deferred as NIT-2. The "trivial change" rationalization led to defects landing at Copilot rather than at F5.

**Root cause:** The orchestrator's MANDATORY STEPS list (Phase 1d / Phase 5 3-clean adversarial convergence) does not have a "trivial" exemption. When it was informally granted one, the predictable outcome was a multi-round Copilot session that reproduced the exact defects that F5 adversarial convergence was designed to surface.

**Specific failure sequence (R4):**
- Orchestrator's single adversary pass identified the Source-field numeric-count coverage gap but categorized it as NIT-2 ("acceptable to defer").
- PR was opened with that gap present.
- Copilot R4 re-surfaced it as a valid finding — 4 rounds of context accumulation later.
- The NIT-2 categorization on the adversary side vs. a real finding on the Copilot side is an inconsistency that 3-clean adversarial convergence would have resolved before the PR opened.

**Lesson:** Apply VSDD process to ALL PRs regardless of perceived size, OR formally codify a `workflows/maintenance.lobster` chore-mode workflow with explicit-but-reduced-but-still-mandatory checklist. The current ad-hoc shortcut pattern produces predictable round-2-through-N defects in F6 that should have been caught at F5 or earlier. Tracked in DRIFT-007.

**Quantified cost of shortcut:** 7 Copilot rounds with 9 valid findings vs. an estimated 1-2 Copilot rounds if F5 3-clean had been applied (extrapolating from PR #357 which converged in 2 rounds after a thorough F5 pass). The shortcut saved an estimated 1-2 adversarial passes but cost 5-6 extra Copilot rounds — net negative.

_Discovered: PR #369 post-merge retrospective, 2026-05-15T19:49:41Z_
_Tagged: [codified] — chore-PR process failure; VSDD "trivial" exemption anti-pattern_
_Related: DRIFT-007 (chore-mode workflow not formalized)_

---

## 2026-05-16 — S-340 Cycle Close-out Lessons

### L-S340-1 [novel-pg] Mutation-Red-Gate substitution pattern

When a story pins behavior that production code ALREADY satisfies (regression-pin / green-on-first-run), standard Red Gate (test fails before implementation) cannot be naturally achieved. The S-340 cycle substituted a mutation-based Red Gate: deliberately break the production behavior, confirm the test fails with the expected assertion, revert, confirm tests pass.

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Codify into per-story-delivery / test-writer prompt if seen again.
- Reference: `.factory/cycles/cycle-001/S-340/implementation/red-gate-log.md`

_Discovered: S-340 F3 delivery, 2026-05-15_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

### L-S340-2 [novel-pg] Machine-enforced red-gate verification for regression-pin stories

Pass 1 adversary `[process-gap]` finding: mutation-Red-Gate pattern was applied but not documented as a process pattern in the story or red-gate-log. Pass 4 adversary `[process-gap]` finding: red-gate verification for regression-pin stories should be machine-enforced (e.g., script that applies the mutation, runs the test, and confirms failure) rather than manually described in log prose.

Both are first occurrences in cycle-001. Logged as [novel-pg] only. Do NOT file follow-up issues for first-occurrence process-gaps; revisit if they recur in a future cycle.

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Reference: `.factory/cycles/cycle-001/S-340/implementation/red-gate-log.md`

_Discovered: S-340 F5 adversary passes 1 and 4, 2026-05-15_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

## 2026-05-16 — S-345 Cycle Close-out Lessons

### L-S345-1 [novel-pg] Evidence-staleness chase-your-tail on PRs that include demo evidence files

When iterating Copilot reviews on a PR that includes captured evidence files (e.g., test output transcripts, cargo output logs, snapshot captures), every code fix that shifts line numbers or output formatting causes evidence files to become stale. This produces a "chase-your-tail" pattern: each Copilot round that fixes a real finding also triggers a new "stale evidence" comment in the next round, which triggers a new evidence-regeneration commit, which shifts line numbers again.

**Resolution pattern observed in PR #371:** Rather than regenerating evidence incrementally per Copilot round, defer all evidence regeneration to a single consolidated commit at the FINAL HEAD — after all behavioral fixes have been applied and no further code changes are expected. A single `convergence batch` commit (9420f1b in this case) regenerated ALL evidence files atomically, eliminating the evidence-staleness feedback loop entirely.

**Rule:** When a PR includes demo evidence files AND Copilot review is expected to produce multiple rounds, do NOT regenerate evidence files after each round. Regenerate ALL evidence in one consolidated commit after the final behavioral fix, before requesting the last Copilot review pass.

**Anti-pattern name:** evidence-staleness chase-your-tail — each evidence update triggers another stale-evidence comment.

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Codify into demo-recorder prompt or pr-manager process if the pattern recurs in a future cycle.
- Reference: PR #371 convergence batch commit 9420f1b, S-345 F7 cycle close-out.

_Discovered: S-345 F7 Copilot review cycles, 2026-05-16_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

### L-S345-2 [novel-pg] Proptest helper filter_map masks malformed shapes — use map + assert instead

When writing a proptest that pins an exact JSON wire shape, use `iter().map()` combined with `assert!` (or `prop_assert!`) instead of `iter().filter_map()`. The `filter_map` combinator silently drops entries that do not match the expected shape — including malformed shapes that represent contract violations. A proptest that uses `filter_map` to extract "expected" structure will trivially pass even when the function under test returns a structurally wrong value, because the malformed entry is silently discarded from the iteration rather than triggering an assertion failure.

**Concrete manifestation (PR #371 / S-345):** An early proptest draft used `filter_map` to extract `"labelsAction"` keys from the returned JSON array. On a structurally wrong return value (no `"labelsAction"` key), `filter_map` would have returned `None` and silently skipped the entry — producing `0 assertions checked` rather than `1 assertion FAILED`. The proptest would have reported `256 cases: OK` on a broken implementation.

**Correct pattern:** `iter().map(|entry| entry["labelsAction"].as_str().expect("BC-3.4.006: labelsAction must be present"))` — uses `expect!` (or `prop_assert!`) to fail explicitly on malformed entries rather than silently skipping them.

**Rule:** In proptests that pin exact JSON or struct shapes, always use assertion-based extraction (`expect`, `unwrap_or_else`, `prop_assert!`) rather than silently-dropping combinators (`filter_map`, `find`, `flatten`). The discriminating power of the proptest depends on the assertions firing on malformed entries.

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Codify into test-writer prompt if the pattern recurs in a future proptest delivery.
- Reference: PR #371 / S-345 proptest for `build_labels_edited_fields` invariants, BC-3.4.006.

_Discovered: S-345 F5 adversary review / F6 Copilot review, 2026-05-16_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

## 2026-05-16 — S-346 Cycle Close-out Lessons

### L-S346-1 [novel-pg] Empirically verify adversary's schema/API claims before fixing

When an adversary produces a CRITICAL or BLOCKER finding about an external tool's API schema (e.g., "jq will silently produce nulls because the field doesn't exist at the path you think"), do NOT fix the code based on the claim alone. Verify directly by inspecting the actual tool output.

**Concrete instance (S-346 Pass 5 F1):** The adversary claimed cargo-mutants v27's `outcomes.json` uses a nested per-outcome array structure (`.outcomes[] | select(.kind=="caught")`) rather than top-level scalar keys, asserting that the existing jq queries `.caught // 0`, `.missed // 0` etc. would silently return null. This was presented with high confidence as a CRITICAL finding. Directly inspecting a locally produced `mutants.out/outcomes.json` via `jq 'keys'` showed the top-level keys `caught`, `missed`, `timeout`, `unviable`, `total_mutants` — matching the existing jq queries exactly. The adversary's claim was speculative, not evidence-based. REFUTED with zero code changes.

**Rule:** For any adversary finding that claims an external tool's output schema differs from what the code assumes — particularly when the code was written by inspecting the tool's actual output — verify by running the tool and examining the output with `jq 'keys'`, `jq '.'`, or equivalent before making any code change. Adversaries can produce confident-sounding speculative findings about external tool schemas; the cost of verification is one shell command; the cost of acting on a wrong claim is a spurious code change that passes CI but breaks empirical correctness.

**Generalization:** This applies beyond cargo-mutants. Any adversary claim about the JSON/YAML/TOML schema of an external tool (cargo, rustc, jq, yq, gh, etc.) should be treated as "unverified hypothesis until locally confirmed."

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Codify into adversarial-review SKILL.md or per-story-delivery if the "speculative schema" adversary pattern recurs.
- Reference: S-346 Pass 5 F1 REFUTED; `.factory/cycles/cycle-001/S-346/implementation/red-gate-log.md`.

_Discovered: S-346 F5 adversary Pass 5, 2026-05-16_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

### L-S346-2 [novel-pg] Doc-drift across reference docs is the main risk for CI-infrastructure stories

For CI-infrastructure stories (those that add/modify CI jobs, config files, and documentation without touching production code), documentation drift across multiple reference files is the dominant convergence challenge — not implementation correctness.

**Concrete instance (S-346):** The implementation required 5 fix rounds across 8 adversary passes. Every fix round triggered at least one back-sync cascade to keep multiple reference documents in lockstep with the iterating CI implementation:

- `.github/workflows/ci.yml` (the implementation)
- `.factory/cicd-setup.md` (the canonical CI spec — pre-updated by F2 architect)
- `docs/specs/cargo-mutants-policy.md` (the whitelist policy doc)
- `docs/demo-evidence/S-346/baseline-mutants-report.txt` (the baseline evidence)
- `CLAUDE.md` (the AI agent notes)
- `.factory/code-delivery/issue-346/story.md` (the story spec AC-1 implementation notes)

Each adversary pass that changed the CI YAML (kill-rate formula, diff generation, harness-health gate logic) required auditing all 5+ reference documents for stale descriptions of the old behavior.

**Rule:** For CI-infrastructure stories, plan the doc-fallout sweep as an explicit named step in the implementation checklist, not as a one-shot cleanup at the end. After each implementation revision, immediately grep all reference documents for terminology describing the OLD behavior and update them in the same commit. Failure to do this atomically produces back-sync churn in subsequent adversary passes.

**Anti-pattern name:** CI-infra doc-drift — when iterative refinements to a CI job cascade into stale descriptions across multiple reference docs, each requiring a separate back-sync commit.

- First occurrence in cycle-001. Logged as [novel-pg] only.
- Codify into story-writer template for CI-infrastructure stories if the pattern recurs.
- Reference: S-346 5-round convergence with doc-fallout at every pass; `.factory/code-delivery/issue-346/story.md`.

_Discovered: S-346 F5 adversary convergence retrospective, 2026-05-16_
_Tagged: [novel-pg] — first occurrence; monitor for recurrence before promoting to codified rule_

---

## L-288-04: Validate adversarial findings against actual risk profile before mechanizing them [process]

**Date:** 2026-05-18
**Cycle:** 3-feature-jsm-request-types-288 (F1d pass-01 → F3 scope simplification)
**Tag:** [codified] [process-gap]

### What happened

F1d pass-01 finding F10 (CONCERN) flagged the OAuth scope-addition "Developer Console coordination" as a HIGH-risk release gate with no enforced mechanism. The product-owner remediation added a PR-template release-gate clause to BC-1.3.023 requiring `.github/PULL_REQUEST_TEMPLATE.md` creation in S-288-pr3-scope. The next 9 adversarial passes accepted this without challenge; F1d converged 3/3.

During F3 human approval gate, the user questioned the PR-template mechanism. The orchestrator dispatched the research-agent to validate the actual risk profile (`.factory/research/issue-288-oauth-scope-coordination.md`):

- Failure mode is **loud and immediate** (`invalid_scope` redirect, not silent corruption)
- `jr` has one maintainer = one Atlassian Developer Console admin = no team-coordination problem
- Existing code comment at `src/api/auth.rs:46-51` + pin test already mitigate the implementer-visible failure
- Atlassian auto-handles the user-facing re-consent prompt
- Real-world precedent (`cli/cli`, ankitpokhrel/jira-cli) shows scope changes are infrequent (≤2/year) and don't warrant per-PR ceremony

Verdict: the F1d-added PR-template mechanism was disproportionate. BC-1.3.023 was simplified to "maintainer coordination" + existing code comment + CLAUDE.md note + CHANGELOG re-consent entry. S-288-pr3-scope was dropped; work absorbed into S-288-pr4-dispatch. 1 story removed; 1 PR cycle saved.

### Lesson

**Adversarial findings that propose NEW PROCESS MECHANISMS (PR templates, CI hooks, release gates) should be validated against actual failure-mode severity and existing safeguards before being mechanized in BC bodies.** The convergence loop optimizes for "no findings remain"; it does NOT optimize for "no over-engineering remains." A finding can be technically valid (a coordination risk DOES exist) while the proposed mechanism is disproportionate.

Pattern to watch for: adversary finding asserts a HIGH-risk condition, recommends a NEW process mechanism, and the remediation adds the mechanism without questioning whether existing safeguards already cover the failure path or whether the mechanism's friction outweighs the avoided risk.

### Application going forward

- Future cycles: when an adversary CONCERN proposes new process artifacts (CI jobs, PR templates, release-gate scripts, mandatory checklist items), the remediating agent (or orchestrator) should explicitly:
  1. Identify the failure mode and its detection signal (loud vs silent, immediate vs delayed)
  2. Inventory existing safeguards (doc comments, regression tests, runtime checks)
  3. Quantify mechanism overhead (per-PR friction, maintainer time, infrastructure cost)
  4. Use research-agent for external validation if the failure-mode framing depends on assumptions about external systems
  5. Only mechanize if existing safeguards are demonstrably insufficient AND mechanism overhead is proportional

- Orchestrator: when an adversary CONCERN proposes a new process mechanism, consider dispatching research-agent to validate the failure-mode framing BEFORE accepting the remediation
- BC authoring: distinguish "behavioral contracts" (what the code MUST do) from "process contracts" (what humans MUST do around the code). The latter belong in CONTRIBUTING.md / RELEASING.md / CLAUDE.md, not in BC bodies — BCs are testable in CI; process contracts are not

### Status

[codified] — this lesson is recorded and will inform future adversarial-remediation cycles. Suggests a `vsdd-factory:adversarial-review` skill enhancement: when a CONCERN finding proposes a new process mechanism, prompt the orchestrator to validate before accepting the remediation.

_Discovered: #288 F3 human approval gate → research-agent validation, 2026-05-18_

---

## L-288-pr1-01: Copilot catches what local-adversary misses on test-quality dimensions [codified]

**Date:** 2026-05-18
**Cycle:** 3-feature-jsm-request-types-288 (F4 pr1-api delivery)
**Tag:** [codified] [novel-pg]

### What happened

Per-story adversarial review for S-288-pr1-api converged at 3/3 CLEAN (0B/0C/3N) with NITs flagged as acceptable. PR was opened with all 10 CI checks green and pr-reviewer APPROVE. Copilot then ran 6 rounds and found:
- POST body shape was not asserted in AC-001 test (mock matched method+path only)
- `searchQuery` absence test was self-documented as soft (already flagged as F-03 NIT by per-story adversary — but adversary accepted as "non-blocker"; Copilot insisted on tightening with `query_param_is_missing`)
- `visible: bool` field in JSM API response was silently dropped by `RequestTypeField` struct (adversary verified 14 struct fields but did not cross-reference against full API response shape)
- AC-005 in evidence report cited the wrong test for `RequestTypeField` coverage (story.md had this drift; adversary did not catch)
- Doc-comment phrasing accuracy ("no data is lost" was over-stating reality)
- Positive searchQuery test omitted pagination param matchers (would match a request with searchQuery + extra params)

### Lesson

**Per-story adversary catches structural/semantic defects; Copilot catches test-precision defects.** The adversary verified the diff was internally consistent and matched the BCs/ACs — but it accepted F-03 ("AC-003 negative-test softness, self-documented") as a NIT rather than a fix-required CONCERN. Copilot didn't accept that — it required `query_param_is_missing`. Similarly, Copilot caught the missing `visible` field by reading the actual Atlassian API response shape (not just the test fixtures).

Pattern: when an adversarial NIT is "self-documented imperfection" (test author admits the test has a known weakness), a fresh-context Copilot pass will frequently insist on tightening it. Future cycles should treat self-documented test weaknesses as CONCERN-class, not NIT-class.

### Application going forward

- Per-story adversary should treat `// note: this test cannot strictly enforce X` comments in test code as a CONCERN finding (test author has documented an incompleteness), not a NIT — and ask whether the incompleteness should be fixed before merge or filed as a follow-up
- For new struct definitions, the adversary should cross-reference against ACTUAL API response shape (e.g., grep for all fields in the swagger/example responses) rather than just verifying the struct compiles and round-trips
- AC-trace fields in evidence reports / story files should be sanity-checked against the test names that ACTUALLY cover each AC — Copilot caught one drift here that 3 adversarial passes missed

_Discovered: S-288-pr1-api F4 delivery + 6 Copilot rounds → convergence, 2026-05-18_
_Tagged: [codified] [process-gap] — first occurrence; applicable to all future adversarial-remediation cycles_

---

## L-288-pr2-01: Budget 8-12 adversarial passes for stories with NEW BCs [codified]

**Date:** 2026-05-19
**Cycle:** 3-feature-jsm-request-types-288 (F4 pr2-cli delivery)
**Tag:** [codified] [process-level]

### What happened

S-288-pr2-cli required 11 adversarial passes before reaching 3/3 CLEAN convergence. Substantive findings were present in passes 01-08 (30+ total findings across BC↔impl gaps, spec-intra-document inconsistencies, test-precision issues, encapsulation problems, and doc-fallout). Passes 09-10-11 were all CLEAN.

Each successive pass found defects the previous missed because fresh-context revealed different surfaces:
- Pass 01: BC-string implementation drift (CRITICAL), output-channel violations (HIGH)
- Pass 02-03: spec-intra-document consistency, test-precision `||` disjunctions
- Pass 04-05: encapsulation, cross-profile cache discipline, POLICY compliance
- Pass 06-07: accept-either test hiding, cell-content `||`, numeric-bypass `||`
- Pass 08: CLAUDE.md call_site_label drift

### Lesson

**Budget 8-12 adversarial passes for stories that introduce NEW BCs (vs <5 for pure refactor stories).** New BCs create new contract surfaces that fresh-context passes continue to find violations of, even after earlier passes declared "converging." The 3/3 CLEAN convergence criterion is correct; the budget estimate must reflect the actual contract density.

Do not shortcut even when the trajectory looks "almost converged" — passes 06-07 found material issues (MEDIUM severity) on what felt like a near-clean trajectory after pass 05.

### Application going forward

- Story-writer: annotate estimated adversarial pass budget in story frontmatter for stories with ≥4 new BCs: `estimated_adv_passes: 8-12`
- Orchestrator: when dispatching per-story adversary for a new-BC story, pre-set the convergence expectation at 10+ passes rather than 5
- For pure refactor stories (no new BCs, only internal restructuring): <5 passes is still the correct budget

_Discovered: S-288-pr2-cli F4 per-story adversarial convergence, 2026-05-19_
_Tagged: [codified] — budget discipline for new-BC stories vs refactor stories_

---

## L-288-pr2-02: L-288-pr1-01 `||`/`.or_else()` test-precision recurrence rate is HIGH — elevate to MEDIUM [codified]

**Date:** 2026-05-19
**Cycle:** 3-feature-jsm-request-types-288 (F4 pr2-cli delivery)
**Tag:** [codified] [process-level] [policy]

### What happened

L-288-pr1-01 (codified 2026-05-18) states: do not use `||` in positive assertions because it accepts either disjunct and hides the case where one branch always passes while the other never fires. The lesson was codified in CLAUDE.md and lessons.md after pr1-api.

Despite being codified, the same pattern recurred FOUR times across pr2-cli adversarial passes:
- Pass 02: `.or_else()` escape in `require_service_desk` test assertion (accepted wrong exit code)
- Pass 03: case-sensitive ExactMultiple test hidden by `||` (case-variant was never exercised)
- Pass 06: cell-content assertion used `||` allowing either cell to satisfy the check
- Pass 07: numeric-bypass `||` — test passed on numeric input that should have been rejected

Every recurrence was found by fresh-context adversarial review, not by the test author or the implementing agent.

### Lesson

**Adversary should grep `tests/` for `||` and `.or_else(` in ALL new test code on EVERY pass and report any match as MEDIUM (not LOW per prior classification).** The recurrence rate (4× in a single story despite an explicit codified lesson) shows that the pattern is a systematic failure mode requiring grep-level enforcement, not documentation-level awareness.

The current policy (L-288-pr1-01) classifies `||`/`.or_else()` violations as LOW. This classification is wrong for repeated recurrences across a single story. MEDIUM is appropriate: the issue is not catastrophic but it has a demonstrated pattern of slipping through human review AND prior VSDD adversarial passes.

### Concrete grep rule

On every adversarial pass, as part of the L-288-pr1-01 audit section, run:

```bash
grep -n "\|\|" tests/<story-test-files>.rs
grep -n "\.or_else(" tests/<story-test-files>.rs
grep -n "\.or(" tests/<story-test-files>.rs
```

Any match in a positive assertion (not a negative compound test or a functional `or_else` for option handling) is a MEDIUM finding: "MEDIUM — accept-either disjunction in positive assertion. Hides case where one branch never fires."

### Application going forward

- Per-story adversary: include explicit `||`/`.or_else(` grep results in every pass report, under "L-288-pr1-01 test-precision audit" section
- Classify any hit in a positive test assertion as MEDIUM (not LOW)
- This rule applies to ALL stories, not just #288 follow-ons

_Discovered: S-288-pr2-cli F4 adversarial passes 02/03/06/07 (four recurrences of L-288-pr1-01 despite codification), 2026-05-19_

---

## PG-384-1: `check-bc-cumulative-counts.sh` does not cover the BC-INDEX `## Coverage Statistics` table [deferred — infrastructure gap]

**Date:** 2026-05-20
**Cycle:** issue-384 F7 convergence close-out
**Tag:** [deferred] [infrastructure-gap] [spec-guard]

### What happened

During #384 F2 (spec evolution), the BC-INDEX `## Coverage Statistics` table drifted — it showed stale section totals (569/337) while the canonical counts were 573/341. Both existing spec guards (`check-spec-counts.sh` and `check-bc-cumulative-counts.sh`) still exited 0; the drift was caught only by a fresh-context adversarial review pass.

Investigation showed that `check-bc-cumulative-counts.sh` validates BC-INDEX `## Section N:` header lines, the `total_bcs:` frontmatter value, and per-file frontmatter counts, but does NOT validate the `## Coverage Statistics` narrative table that appears below the section headers in BC-INDEX.md. That table is a redundant restatement of the already-guarded header data, but it drifts independently.

### Lesson

**The `## Coverage Statistics` table in BC-INDEX.md is a third sync surface that the existing guards do not cover.** When BCs are added and the section-header counts are updated, the Coverage Statistics prose table must also be updated manually — there is no automated check.

Two remediation paths:
1. Extend `check-bc-cumulative-counts.sh` to parse and validate the Coverage Statistics table rows against the canonical per-section counts.
2. Remove the Coverage Statistics table entirely (it is fully redundant with the guarded section headers).

### Status: DEFERRED

The drift class is low-impact (redundant table; arithmetic is correct in all load-bearing locations). A GitHub follow-up issue could not be auto-created at F7 close (per S-7.02 cycle-closing-checklist, deferral is the sanctioned alternative). User to file in next maintenance cycle or #392-successor issue.

_Discovered: #384 F2 adversarial spec review pass 1; recorded at F7 close-out, 2026-05-20_
_Tagged: [deferred] — needs GitHub issue; target: next maintenance sweep_

---

## PG-384-2: All three spec guard scripts must be run after ANY BC file edit — two-guard dispatch in F2/F3 is incomplete [codified]

**Date:** 2026-05-20
**Cycle:** issue-384 F7 convergence close-out
**Tag:** [codified] [process-level] [spec-guard]

### What happened

During #384 F2 and F3, the orchestrator instructed the product-owner to run only two of the three spec guard scripts after each BC file edit:
- `scripts/check-spec-counts.sh`
- `scripts/check-bc-cumulative-counts.sh`

The third guard — `scripts/check-bc-no-numeric-test-counts.sh` — was not included in the dispatch instructions. As a result, a BC body containing the phrasing "Basic-auth 401 integration tests" (a numeric-adjacent string pattern flagged by the guard) reached the PR and caused the `Spec Guards` CI job to fail on PR #394 in its first push. The failure required a fixup commit before CI went green.

The guard was already live in CI (added by #392 / PR #393). The gap was not in the guard itself — it was in the orchestration dispatch not including the guard in the run-locally pre-PR checklist.

### Lesson

**After ANY edit to `.factory/specs/prd/` BC files, ALL THREE guard scripts must be run locally before creating or updating a PR:**

```bash
scripts/check-spec-counts.sh
scripts/check-bc-cumulative-counts.sh
scripts/check-bc-no-numeric-test-counts.sh
```

This applies to:
- F2 spec evolution bursts (BC body authoring or modification)
- F3 story ACs referencing BC text
- Any chore/maintenance PR touching BC files
- Any spec-drift fix PR

The two-guard pattern (`check-spec-counts.sh` + `check-bc-cumulative-counts.sh`) is insufficient for any burst that modifies BC body text. Add `check-bc-no-numeric-test-counts.sh` to all three contexts.

### Status: CODIFIED

No code change needed. The guard already exists. The gap was not running it. Future orchestration dispatch instructions for any phase that touches `.factory/specs/prd/` BC files must explicitly list all three scripts.

_Discovered: #384 F3 implementation → CI failure on PR #394 Spec Guards job; recorded at F7 close-out, 2026-05-20_
_Tagged: [codified] — dispatch instructions must include all three guards_

---

## 2026-05-20 — Issue #385 Cycle Close-Out: 7 Process-Gap Deferred Items

Issue #385 (JSM input validation + UX polish) was delivered via PR #395 (squash-merge f7fc8c3,
2026-05-20). F1–F4 COMPLETE; F7 convergence CLOSED. The cycle surfaced 7 process-gap findings
(PG-385-1 through PG-385-7). All 7 are recorded as **JUSTIFIED DEFERRALS** in STATE.md Drift
Items. None are content defects; all are process/tooling improvement opportunities.

### Cycle Verdict: CLOSED

- F4 per-story adversarial: CONVERGED 3/3 CLEAN.
- Copilot: 3 rounds, converged to 0.
- CI: 10/10 green (Format, Clippy, Test ubuntu, Test macos, MSRV, Deny, Coverage, Secret Scan,
  Spec Guards, Mutation Testing).
- Security review: CLEAN.
- Issue #385: CLOSED / stateReason COMPLETED.

### PG-385-1 [deferred] F2 holdout template lacks mandatory `realized_by:` stub

**Finding:** M-02 class (missing `realized_by:` on new holdout entries) recurred for the third
time — first seen in #284 F2, first codified from #384 F2 pass 5, recurred in #385 F2. The
prd-delta-385.md template's NEW_HOLDOUT block does not include a required `realized_by:` stub,
so the field is omitted by the product-owner and must be added retroactively at F3.

**Justification for deferral:** Engine template gap, not solvable from the jira-cli repo.
No recurrence risk within jira-cli — the gap can only be fixed in the factory engine templates.
The impact is bounded: adds at most one F2 adversary pass per cycle where new holdouts are
created. No content correctness defect in any shipped spec.

**Target:** Next engine maintenance pass — add `realized_by: [TBD — to be filled at F3
story-creation time]` as a required stub to the F2 delta template NEW_HOLDOUT block.

_Discovered: #385 F2 adversarial review. Status: [deferred] — engine template gap._

---

### PG-385-2 [deferred] No CI guard for canonical-ordering or multi-copy-text consistency in BC files

**Finding:** During #385 F2, duplicate/inconsistently-ordered text across holdout-scenarios.md
required structural de-duplication. A future author could re-introduce out-of-order or duplicate
text without any CI failure catching it.

**Justification for deferral:** Scripts-improvement gap; no blocking impact. Not a content
defect. Should be bundled with the pre-existing spec-guard hardening follow-ups from issue #383
(DEFER-383-3, now resolved by #392) into a "spec-guard hardening phase 2" issue. The sandbox
classifier blocks autonomous GitHub issue creation; the human should file this when scheduling
the next maintenance sweep.

**Target:** Next scripts-maintenance PR — extend `check-spec-counts.sh` or add a new script
to lint for duplicate heading IDs and canonical ordering of holdout entries.

_Discovered: #385 F2 structural cleanup. Status: [deferred] — scripts improvement candidate for
future "spec-guard hardening" bundle (with PG-385-3/4/6)._

---

### PG-385-3 [deferred] No lint for `src/*.rs:NN-MM` micro-range citations in BC Source/Trace fields

**Finding:** BC Source/Trace fields that cite specific line-number ranges (e.g.,
`src/cli/issue/create.rs:45-67`) drift quickly as code evolves. No CI guard exists to detect
these micro-range citations and warn that they need updating.

**Justification for deferral:** Scripts-improvement gap. Not a content defect; the cited ranges
are informational and their staleness is low-impact. Should be bundled with PG-385-2/4/6 into
a single "spec-guard hardening phase 2" follow-up issue.

**Target:** Next scripts-maintenance PR — add `scripts/check-bc-no-line-range-citations.sh`
that errors on patterns like `src/*.rs:\d+-\d+` in BC Source/Trace fields.

_Discovered: #385 F2 adversarial review. Status: [deferred] — scripts improvement candidate for
future "spec-guard hardening" bundle._

---

### PG-385-4 [deferred] `check-spec-counts.sh` does not validate holdout prose count vs frontmatter

**Finding:** Adding H-NEW-JSM-RT-006/007 in #385 F2 required a manual prose update to the
holdout-scenarios.md "N holdout scenarios" body preamble line. The guard would not have caught
a missed prose update. Parallel to DEFER-383-3 (now resolved by #392 for BCs; same gap class
now for holdouts).

**Justification for deferral:** Scripts-improvement gap. Not a content defect. Should be bundled
with PG-385-2/3/6 into a single "spec-guard hardening phase 2" follow-up issue.

**Target:** Next scripts-maintenance PR — extend `check-spec-counts.sh` to grep
holdout-scenarios.md body preamble for the "N holdout scenarios" prose line and assert it
equals `total_holdouts` frontmatter. Analogous to DEFER-383-3 resolution.

_Discovered: #385 F2 manual prose update required. Status: [deferred] — scripts improvement
candidate for future "spec-guard hardening" bundle._

---

### PG-385-5 [deferred] Story-writer template lacks `bc_anchors` completeness rule

**Finding:** During #385 F3 adversarial story review, BCs referenced in story ACs were not
consistently mirrored in `bc_anchors`, creating traceability gaps detectable only by manual
review.

**Justification for deferral:** Engine template gap, not solvable from the jira-cli repo. No
recurrence risk within jira-cli. Impact is bounded to a single adversary pass per cycle where
BC anchors are incomplete.

**Target:** Next engine maintenance pass — add a mandatory rule to the story-writer template:
every BC cited in an AC Trace or test-deliverable table MUST appear in `bc_anchors` OR carry
an explicit `regression-only: true` annotation.

_Discovered: #385 F3 adversarial story review. Status: [deferred] — engine template gap._

---

### PG-385-6 [deferred] No STORY-INDEX count guard script

**Finding:** A pre-existing off-by-one (`total_stories: 44` when actual count is 43) survived
4+ feature-followup additions because no CI script validates the STORY-INDEX frontmatter count
against actual manifest rows. Corrected this cycle (44→43), but the guard is still missing.

**Justification for deferral:** Scripts-improvement gap. Should be bundled with PG-385-2/3/4
into a single "spec-guard hardening phase 2" follow-up issue. No blocking impact; off-by-one
cosmetic (only the frontmatter row count was wrong; all actual story rows were present).

**Target:** Next scripts-maintenance PR — add `scripts/check-story-index-counts.sh` to validate
`total_stories` frontmatter against actual story manifest rows + sprint-state.yaml story count.

_Discovered: #385 F3 story-index correction (44→43). Status: [deferred] — scripts improvement
candidate for future "spec-guard hardening" bundle._

---

### PG-385-7 [deferred] Story-writer line-range instructions under-scope governing comments

**Finding:** During #385 F3 story drafting, implementation line ranges for target functions were
specified without including the governing `///` rustdoc or `//` block-comment header. When
test-writers use those ranges to write assertion strings, missing the comment means missing
the contract statement.

**Justification for deferral:** Engine template / story-writer prompt gap, not solvable from
the jira-cli repo. No recurrence risk within jira-cli.

**Target:** Next engine maintenance pass — update story-writer instructions to specify that a
line range for a function MUST include the governing comment/rustdoc block (typically 1–10
lines above the `fn` keyword).

_Discovered: #385 F3 story drafting. Status: [deferred] — engine story-writer prompt gap._

---

### Cycle Summary: #385

- All 7 PG-385-1..7 findings are process/tooling improvements, not content defects.
- No finding has recurred 3+ times from within this cycle alone (PG-385-1 is the third
  instance of the M-02 class across the entire cycle-001 span, not within #385 alone).
- PG-385-2/3/4/6 should be bundled with the pre-existing deferred items from issue #383
  (DEFER-383-3, now RESOLVED) into a single future "spec-guard hardening phase 2" issue
  that the human should file (sandbox classifier blocks autonomous GitHub issue creation).
- PG-385-1/5/7 are engine template improvements for a future self-improvement cycle.
- Issue #385 F1–F7: COMPLETE. PR #395 squash-merged @ f7fc8c3, 2026-05-20. Cycle CLOSED.

_Recorded: F7 close-out, 2026-05-20_
_Tagged: [codified] [policy] — elevates accept-either classification from LOW to MEDIUM; mandates grep audit on every adversary pass_

---

## Issue #388 F2 — Deferred Process-Gap Findings (for F7/cycle-close codification)

_Recorded: F2 close-out, 2026-05-20. Source: #388 adversarial spec review, passes 1–10._
_Status: DEFERRED — for codification at F7 gate or cycle-close, not blocking F3._

---

### PG-388-1 [deferred] BC-authoring checklist: None/null branch assignment for Optional fields

**Finding:** For every `Option<T>` / nullable field that a BC branches on, the None/null branch
must be explicitly assigned a classification (e.g., `Errors:`, `Outputs/Effects:`, or a
designated fall-through). During #388 F2 adversarial review, missing None-branch classifications
on BC-3.4.010/011 were caught in passes 1–3 and required correction. No existing checklist item
codifies this rule.

**Recommendation:** Add to BC-authoring checklist: "For every `Option`/nullable field a BC
branches on, confirm the None/null branch is assigned a classification. Do not leave None-path
behavior implicit."

**Target:** Engine-level BC-authoring checklist / F2 product-owner prompt. Not solvable from
jira-cli repo. Target: next engine maintenance pass.

_Discovered: #388 F2 adversarial review passes 1–3. Status: [deferred] — engine template gap._

---

### PG-388-2 [deferred] No convention/CI check that "pinned verbatim" code blocks in BC bodies have a corresponding full-string assertion test

**Finding:** Several BC bodies in bc-3-issue-write.md use a "pinned verbatim" code block
(triple-backtick block with an exact error string or output literal) to express a required
output. There is no project convention and no CI guard requiring that each such pinned-verbatim
block has a corresponding full-string assertion in the test suite (as opposed to a `contains()`
or partial-match assertion). The gap was surfaced during #388 F2 passes 4–5 when reviewers
noted that new BC-3.4.010/011 verbatim strings lacked paired full-string pins.

**Recommendation:** Establish a convention: any code block in a BC body annotated or named
as "pinned verbatim" (or containing an exact error string literal) MUST have a corresponding
`assert_eq!` / `.stdout("...")` full-string assertion test. Consider a CI grep that flags
verbatim-pin blocks in BC files and cross-checks for a matching literal in the test corpus.

**Target:** Convention: codify in CLAUDE.md or BC-authoring guidelines. CI guard: new
`scripts/check-bc-verbatim-pins.sh` (future scripts-maintenance PR). Not blocking any current
delivery.

_Discovered: #388 F2 adversarial review passes 4–5. Status: [deferred] — convention gap; CI
guard would require new scripts/ work._

---

### PG-388-3 [deferred] Pre-existing L2↔L3 BC-count drift not gated by any guard script

**Finding:** The L2 domain spec files (`bc-02.md`, `bc-03.md`) have `bc_count` frontmatter
values that are approximately 20 BCs behind the L3 PRD values (DRIFT-009 in Drift Items).
This drift has accumulated across multiple cycles (#350, #365, #340, #288, and now #388)
because no guard script validates that L2 frontmatter counts stay in sync with L3 PRD
frontmatter counts. The #388 F2 adversarial review surfaced this again in pass 6.

**Important:** This drift was NOT introduced by #388. It is pre-existing (first recorded as
DRIFT-009 during #288 F1d). Including here for F7/cycle-close codification tracking.

**Recommendation:** Extend `scripts/check-bc-cumulative-counts.sh` (or add a sibling script)
to compare L2 domain-spec `bc_count` frontmatter values against the corresponding L3 PRD
`bc_count` / `total_bcs` values and emit a warning (not hard-fail, since L2 propagation
policy has not been decided) when they diverge by more than a configurable threshold.
Alternatively, decide the L2 propagation policy (DRIFT-009 target: v0.6) so the drift can
be closed systematically.

**Target:** Policy decision (DRIFT-009 → v0.6 / L2 propagation policy). Scripts improvement:
extend `check-bc-cumulative-counts.sh` or add `check-l2-l3-bc-alignment.sh`. Not solvable
until L2 propagation policy is decided.

_Discovered: #388 F2 adversarial review pass 6. Pre-existing: DRIFT-009. Status: [deferred] —
L2 propagation policy required first; DRIFT-009 owner: orchestrator._

---

### PG-388-4 [codified] Pull target branch to merged commit before dispatching any post-merge reviewer

**Finding (F5 — 2026-05-21):** The F5 scoped adversarial reviewer for S-388 was dispatched
against a stale local `develop` checkout — the orchestrator did not pull `develop` to the
merge commit (`e0ea24b`) before invoking the post-merge reviewer. This produced a false
"implementation absent" finding in the reviewer's first pass, because the reviewer saw a
pre-merge state of `src/cli/issue/create.rs` that lacked `is_cross_hierarchy_type_error` and
the `Classification` enum.

**Root cause:** The orchestrator dispatched the F5 reviewer immediately after PR #397 was
merged, without first confirming that the reviewer's working tree was on develop at `e0ea24b`.
Since the reviewer operates against the filesystem (not a remote fetch), any worktree or
branch checked out at an earlier SHA will produce false-absent findings for all new symbols.

**Rule:** Before dispatching any post-merge reviewer (F5 adversarial, F6 hardening verifier,
traceability auditor, or code-reviewer), the orchestrator MUST:
1. Run `git -C <repo> log --oneline -1` to confirm `develop` HEAD is the expected merge SHA.
2. If the reviewer uses a worktree or separate checkout, run `git -C <worktree> pull origin develop`
   (or `git -C <worktree> checkout <merge-sha>`) and confirm HEAD matches before dispatch.
3. Record the confirmed SHA in the reviewer dispatch prompt so the reviewer can self-validate
   ("I am reviewing develop @ e0ea24b — confirm this matches your working tree").

**Impact of violation:** False-absent findings cause one or more wasted adversary passes
(reviewer finds "not implemented" for already-shipped code), inflating pass count and
reducing convergence signal fidelity.

**Scope:** Applies to all post-merge reviewer dispatches in all VSDD Feature Mode phases.
Particularly important for F5 (which runs post-merge in Feature Mode) and for F7 traceability
audits that verify specific file contents.

_Discovered: #388 F5 scoped adversarial review, 2026-05-21. Status: [codified] — process
discipline; no follow-up story needed. Rule applies to orchestrator dispatch protocol._

---

## Lessons from Issue #398 — Changed-Fields Confirmation Echo (2026-05-22)

### L-398-01 [codified] BC scope-broadening discovered mid-convergence requires full re-convergence

**Finding (F2 — 2026-05-22):** During F2 Spec Evolution for #398, the human gate applied
a scope change after 13 adversary passes had already converged: BC-3.4.014 (confirmation-echo
`--output json` shape) was broadened from team-only to ALL-set-fields echo, mirroring BC-3.4.012.
This required resetting the convergence counter and running 3 additional re-convergence passes
(passes 14/15/16 CLEAN) before the F2 gate could be approved.

**Rule:** Any scope change to a BC that adds new required behavior — even if it "just mirrors"
an existing BC — must be treated as a substantive finding and reset the adversary convergence
counter. Do not attempt to approve the F2 gate while the counter is mid-reset.

**Impact:** +3 passes, +10 product-owner fix rounds. VP-398-005 scope broadened; VP-398-006 added.

_Discovered: #398 F2 adversarial review, passes 13-16. Status: [codified] — process discipline.
No follow-up story needed._

---

### L-398-02 [codified] Run ALL THREE guard scripts after any BC file edit (not just two)

**Finding (F2/F3 — 2026-05-22):** During #398 F2/F3, the product-owner was instructed to run
only `check-spec-counts.sh` and `check-bc-cumulative-counts.sh` — NOT the third guard
`check-bc-no-numeric-test-counts.sh`. This allowed a phrase containing numeric test counts to
reach CI and fail the Spec Guards job on PR #394. The same gap was recorded as PG-384-2 during
#384; it recurred in #398.

**Rule (repeat codification from PG-384-2):** After ANY edit to `.factory/specs/prd/` BC files,
ALL THREE guard scripts must be run locally before creating or updating a PR:
1. `scripts/check-spec-counts.sh`
2. `scripts/check-bc-cumulative-counts.sh`
3. `scripts/check-bc-no-numeric-test-counts.sh`

**Impact:** PG-384-2 recurred as PG-398-2 variant. Root cause: orchestrator dispatch prompt
omits the third script. Fix: orchestrator dispatch must explicitly list all three.

_Discovered: #398 F2/F3, recurrence of PG-384-2. Status: [codified] — orchestrator dispatch
prompt must be updated. Tracked in #400 (TH-398-3)._

---

### PG-398-4 [codified] Worktree-path mis-resolution is a recurring class (2 occurrences) — warrants sanity-gate

**Finding (F4 — 2026-05-22):** The F4 per-story adversary pass 3 for #398 returned a false
NOT-CLEAN because the adversary mis-resolved the worktree path and reviewed the develop
baseline instead of the `feature/issue-398-changed-fields-echo` worktree. Mitigated by
re-dispatch with an explicit sanity-gate (`git log` + `grep` for target commit) before review.

This is the SECOND occurrence of this class (first: PG-388-4, #388 F5 adversarial review,
2026-05-21). Two occurrences in consecutive cycles elevates this from a one-off to a
**recurring class** warranting a codified mandatory worktree sanity-gate.

**Rule (reinforced from PG-388-4):** Before dispatching any post-branch reviewer (F4 per-story
adversary, F5 scoped adversarial, F6 hardening verifier, traceability auditor), the orchestrator
MUST:
1. Run `git -C <repo> log --oneline -1` to confirm the working tree HEAD is the expected
   feature-branch or merge commit SHA.
2. Record the confirmed SHA in the reviewer dispatch prompt so the reviewer can self-validate.
3. If using a worktree, run `git -C <worktree> log -1 --format='%H'` and confirm HEAD before
   dispatch — do NOT assume the worktree reflects the expected state.

**Impact of violation:** False-absent findings waste one adversary pass per violation and
reduce convergence signal fidelity.

**Recurrence tracking:** 2 occurrences (PG-388-4 → PG-398-4). Codified worktree-sanity-gate
protocol tracked in follow-up #400 for engine-level codification in FACTORY.md / adversary
dispatch prompt.

_Discovered: #398 F4 per-story adversarial review, 2026-05-22. Recurring class from PG-388-4
(2026-05-21). Status: [codified] — process discipline. Tracked in #400._

---

### L-398-03 [codified] F6 mutation scope should cover only new/changed code paths, not full-codebase

**Finding (F6 — 2026-05-22):** For #398, F6 mutation testing was scoped to the delta (3/3
viable mutants caught in the confirmation-echo code path; 0 surviving). Full-codebase mutation
was not re-run (consistent with the per-PR diff-scoped `cargo mutants --in-diff` CI policy
established by #346). Kani + fuzz were JUSTIFIED-SKIP: no new unsafe code, no new numeric
boundary operations requiring formal proof.

**Rule:** For Feature Mode F6, mutation testing scope is always the PR diff (`cargo mutants
--in-diff`), not a full-codebase re-run. Formal verification tools (Kani, fuzz) require
explicit justification when skipped: acceptable skip reasons are (a) no new unsafe code, (b)
no new numeric overflow boundary operations, (c) no new cryptographic operations.

_Discovered: #398 F6 hardening, 2026-05-22. Status: [codified] — already implicit in
docs/specs/cargo-mutants-policy.md; surfaced explicitly here for F6 skip-justification
discipline._

---

### L-398-04 [codified] MAXIMUM_VIABLE_REFINEMENT declaration requires explicit human authorization

**Finding (F7 — 2026-05-22):** At F7 Delta Convergence for #398, MAXIMUM_VIABLE_REFINEMENT
was reached after all 5 dimensions PASS and the human explicitly authorized cycle-close.
The human authorization step is mandatory — the orchestrator cannot self-declare
MAXIMUM_VIABLE_REFINEMENT without human sign-off at the F7 gate.

**Rule:** F7 cycle-close requires explicit human authorization. The state-manager records the
authorization date and authorizing party (e.g., "human-authorized 2026-05-22") in STATE.md,
the Phase Progress row, and the cycle manifest. This creates an auditable record of the human
gate for every completed cycle.

_Discovered: #398 F7 Delta Convergence, 2026-05-22. Status: [codified] — applies to all
Feature Mode F7 cycle-close events._

---

### L-398-05 [codified] Process-gap findings must be tracked in a filed GitHub issue at cycle-close

**Finding (F7 cycle-closing checklist S-7.02 — 2026-05-22):** At #398 cycle-close, 5
process-gap findings (PG-398-1..5) and 4 test-hardening items (TH-398-1..4) were identified
via the S-7.02 cycle-closing checklist. These were filed as GitHub issue #400 (non-blocking
maintenance sweep, 2026-05-22) and recorded in the Drift Items table with disposition
"tracked in #400".

**Rule:** At every F7 cycle-close, the orchestrator must:
1. Run the S-7.02 cycle-closing checklist.
2. Collect all process-gap (PG-*) and test-hardening (TH-*) items surfaced.
3. File a GitHub issue for the collection before declaring the cycle closed.
4. Record each item in STATE.md Drift Items with disposition "tracked in #NNN".
5. Tag lessons as [codified] with the GitHub issue reference.

The issue may be non-blocking (future maintenance sweep). The cycle is not blocked on
resolving the items — only on filing them.

_Discovered: #398 F7 cycle-close, 2026-05-22. Recurrence check: #384 (PG-384-1/2 recorded
but no issue filed at cycle-close), #385 (PG-385-1..7 recorded), #388 (PG-388-1..4 recorded).
#398 is the first cycle where a dedicated follow-up issue (#400) was explicitly filed at
cycle-close per S-7.02 discipline. Status: [codified]._

---

## 2026-05-25 — Issue #396 Cycle-Close Lessons (PG-396-1..5)

### PG-396-1 [codified] [recurring 2×] Silent-drop class: flag-combination conflict blocks require an update whenever a new flag is added

**Finding (F5 pass 1 — 2026-05-25):** The `--label` conflict block in `handle_edit` guards against
flag combinations (labels + other flags) that would produce a silent partial edit on the bulk-labels
path. When `--field` was implemented in #396, the conflict block was not extended to include `--field`.
A user passing `jr issue edit FOO-1 --label add:foo --field priority=High` would silently drop
`--field`. FIX-F5-001 (PR #406 @ `699a5fd`) added `--field` to the conflict block with an exit-64
guard and integration test.

This is a recurring class: in #110-pr2, the adversarial review found the same pattern — adding a
non-label flag without extending the conflict block. The structural root cause is that the conflict
block has no CI-enforced structural invariant tying its entries to the set of flags that could
silently interact with the bulk-labels path.

**Rule:** Whenever a new flag is added to `handle_edit` that modifies issue fields on the
platform-write path, the implementer MUST check the `--label` conflict block and add the new flag
if it is not already included. This is currently a manual discipline; a structural meta-test that
mechanizes the invariant has been filed as issue #407.

**Recurrence tracking:** 2 occurrences (#110-pr2 → #396). Status: [codified]. Mitigation: #407
files a structural meta-test to make the invariant mechanically enforceable at CI time.

_Discovered: #396 F5 adversarial pass 1, 2026-05-25. Recurring class, 2× occurrences._

---

### PG-396-2 [codified] [process-gap] Line-anchor citations in spec files and CLAUDE.md drift as code evolves

**Finding (F5 passes 2/3 — 2026-05-25):** The F5 adversarial review found EC-3.4.017-13 (the newly
inserted spec entry) referenced a specific line number in `bc-3-issue-write.md:1529` that will drift
as the file is edited. Additionally, 2 other stale line-anchor citations were found in
`.factory/specs/prd/*.md` and `CLAUDE.md` (`src/file.rs:NN-MM` references). This is the same class
as PG-385-3 (line-range citations in BC Source/Trace fields), now extended to include spec prose and
CLAUDE.md gotcha entries.

**Rule:** BC entries, spec prose, and CLAUDE.md entries MUST NOT cite absolute line numbers in source
files. The correct citation form is function name, type name, or a stable identifier (e.g.,
`handle_edit` in `src/cli/issue/create.rs`) — not `src/cli/issue/create.rs:NN-MM`. The existing
`scripts/check-bc-no-line-range-citations.sh` proposed in PG-385-3 would cover this class. Filed as
issue #408 to track a systematic guard or sweep process.

_Discovered: #396 F5 adversarial passes 2/3, 2026-05-25. Status: [codified]. Tracked in #408._

---

### PG-396-3 [codified] Test isolation discipline: production code follows the spec; fragile tests get isolation infrastructure, not production workarounds

**Finding (F4 per-story adversary, commit 32f60a0 revert — 2026-05-23):** The F4 implementer
initially used `jr_cmd` (real disk, `~/.cache/jr/v1/default/fields.json`) for cache-touching tests
and then added a "strictly-larger guard" in production `resolve_edit_fields` to avoid test
interference: if the cache file exists but is zero-size or older, skip it. This production
guard existed solely to make fragile tests pass — it was not in the spec.

The adversarial review caught the pattern. The revert (commit `32f60a0`) removed the production
workaround and replaced the tests with `jr_cmd_with_xdg` + per-test `tempfile::TempDir`, which
provides an isolated `XDG_CACHE_HOME` per test invocation.

**Rule:** When tests are fragile because they share real-disk state (caches, config files, keychains),
the fix is test isolation, not production code hardening. Acceptable isolation patterns in this
codebase:
- `jr_cmd_with_xdg(env_vars)` with `tempfile::TempDir` for `XDG_CACHE_HOME` + `XDG_CONFIG_HOME`
- `temp_env::with_var` for environment-variable overrides
- `#[ignore]` + `JR_RUN_KEYRING_TESTS=1` for keychain-touching tests

Production code must not contain guards whose sole purpose is to accommodate test fragility.

_Discovered: #396 F4 per-story adversarial review, 2026-05-23. Revert commit 32f60a0. Status: [codified]._

---

### PG-396-4 [soft-codified] Best-effort cache writer style consistency — `let _ = ...` vs `?` for always-Ok Results

**Finding (F4 PR review + Copilot — 2026-05-23):** The #396 implementer used `let _ = ...` to
discard the `Result<()>` from one best-effort cache writer and `?` on another best-effort writer
(both return `Ok(())` unconditionally — the "best-effort writer" pattern documented in CLAUDE.md).
The inconsistency was flagged by the PR reviewer and Copilot review.

The CLAUDE.md documents two patterns for cache writers (propagate via `?` if correctness-critical;
swallow + warn if purely a read-acceleration shortcut), but does not prescribe a single canonical
spelling for the swallow case.

**Observation (not a hard rule):** For best-effort writers that always return `Ok(())`, prefer
`let _ = ...` (intention-revealing, shows the discard is deliberate) over `?` (which implies the
caller should handle a possible error). Consider updating CLAUDE.md to specify `let _ = ...` as
the canonical spelling for the always-Ok best-effort writer pattern.

**Status:** soft-codified as an observation. No follow-up issue filed. Decide at next cache writer
touch whether to promote to a CLAUDE.md gotcha.

_Discovered: #396 F4 PR review + Copilot R2, 2026-05-23. Status: [soft-codified]._

---

### PG-396-5 [codified] [recurring 3×] Tautological tests — implementer-written tests that reimplementing production logic inline rather than exercising the production path

**Finding (F4 Copilot R2 C4 — 2026-05-23):** Test 38 of the S-396 deliverable
(`test_bc_3_4_015_number_resolver_integer_is_i64_not_f64`) reconstructed the production
wire-serialization logic inline inside the test body. The test's assertion was computed by the
same algorithm as the production code, meaning the test would pass even if both the production code
and the test logic contained the same bug. Tests 26 and 27 already exercise the same contract
end-to-end via wiremock; test 38 added no new coverage and degraded test fidelity.

This is a recurring class: same pattern recorded in TH-398-1 (#398 cycle) and TH-398-2 (#398 cycle)
following the same root cause (implementer writes the test while typing the production algorithm
and re-expresses that algorithm in the assertion).

**Rule:** If a test's assertion is computed by reimplementing the production logic inline, extract
a named production helper instead and write the test against that helper. The test should exercise
the production path, not duplicate it. If the test cannot exercise the production path (because the
path has no callable helper surface), that is a design signal to extract one.

CLAUDE.md TDD anti-pattern candidate: "If your test re-implements the production logic you are
testing, extract a helper instead. A tautological assertion (produced by the same algorithm being
tested) has zero defect-catching value."

**Recurrence tracking:** 3 occurrences (TH-398-1 → TH-398-2 → PG-396-5). Filed as issue #409
(S-396 specific instance: extract `parsed_number_to_wire_value` helper). Status: [codified].

_Discovered: #396 F4 Copilot R2 finding C4, 2026-05-23. Recurring class, 3× occurrences. Tracked in #409._

---

## 2026-05-25 — Issue #407 Cycle-Close Lessons (PG-407-1..3)

### PG-407-1 [codified] Validate-before-acting on Copilot review findings: ALL THREE R1 findings on PR #411 were REFUTED

**Context (PR #411 Copilot R1 — 2026-05-25):** Copilot Round 1 on PR #411 posted 3 findings.
All 3 appeared reasonable at face value:
1. Refactor `expected` sets in the meta-test to named `const` slices at module scope (DRY).
2. Replace the guard comment above the global-source-text extraction with brace-matched extraction
   to make the invariant structurally self-enforcing.
3. Share a helper function to deduplicate the expected-set construction across subtests.

Per DEC-018, a research agent validated each finding against the locked F1/F2 design decisions
before acting. All 3 were REFUTED:

1. **`const` slice refactor REFUTED by AC-016 + F1 Q1.** The spec deliberately chose manual
   enumeration per AC-016 ("manually enumerate the expected set") to keep the meta-test as a
   direct human-readable declaration of the contract. Extracting to a shared module-level `const`
   would reduce reviewability and obscure which test asserts which invariant. The F1 design note
   (Q1: "Why manual enumeration? Because each test is an independent witness…") explicitly
   rejected shared constants.

2. **Guard comment removal REFUTED by EC-3.4.017-14.** The spec extension EC-3.4.017-14 explicitly
   documents the guard comment as a load-bearing anchor: it marks the extraction site in the
   source text, enabling the `include_str!` scan to locate the conflict block reliably. Replacing
   it with brace-matched extraction would have required a different (more fragile) parsing strategy.

3. **Shared expected-set helper REFUTED by AC-013.** AC-013 requires each subtest to be an
   "independent witness" — tests must not share state that could mask an error in one path while
   another passes. A shared helper introduces a common-mode failure point: if the helper were
   wrong, all subtests would pass while the invariant was violated.

After citing F1 Q1, AC-013, AC-016, and EC-3.4.017-14 in the R1 replies, Copilot converged
on R2 with zero new comments.

**Rule (reinforcement of DEC-018):** ALWAYS research-validate Copilot findings before acting —
especially when the finding suggests an "obvious" refactor. The danger class is findings that
sound like best-practice improvements (DRY, structured parsing, shared helpers) but contradict
human-gated spec decisions. The validation cost is one research pass; the cost of acting on a
refuted finding is reverting production code that contradicted the spec.

**Quantified benefit:** Without validation, 3 wrong changes would have shipped: module-scope
`const` (contradicts AC-016), guard comment removal (contradicts EC-3.4.017-14), shared helper
(contradicts AC-013). 0 rounds of rework needed after research-validation. Copilot converged
in 2 rounds (R1 with 3 findings → R2 with 0).

_Discovered: #407 PR #411 Copilot R1, 2026-05-25. Status: [codified]. Reinforces DEC-018._

---

### PG-407-2 [codified] Structural meta-tests mechanize invariants that previously relied on developer discipline

**Context (S-407 delivery — 2026-05-25):** The `--label` conflict-block silent-drop bug
(FIX-F5-001, post-#396) existed because there was no test enforcing that the conflict block
listed every relevant flag. The bug recurred twice (#110-pr2 → #396) before being fixed,
because the invariant was maintained only by developer discipline (manual checklist, code review,
spec prose) — none of which fired at CI time.

S-407 delivered `test_label_conflict_block_lists_every_relevant_flag`, which mechanically
enforces the invariant:
- Uses `include_str!("../create.rs")` to extract the full source text of `create.rs` at
  compile-verification time.
- Parses the `--label` conflict block from the source text (no mocking, no indirection).
- Asserts via `BTreeSet` difference that EVERY entry in `BULK_SUPPORTED` /
  `REJECTED_IN_BULK` appears in the conflict block.
- On failure: emits actionable diff — "Flags in expected but NOT in conflict block: [\"--newflag\"]"

A future developer adding a new flag to `BULK_SUPPORTED`/`REJECTED_IN_BULK` will see
`cargo test` fail with a deterministic, named diff — not a silent drop in production.

**Rule:** When a recurring silent-drop class is identified (same invariant violated 2+ times
because it depended on developer discipline), the correct response is a structural meta-test
that mechanizes the invariant at CI time. The meta-test should:
1. Parse or read the production artifact directly (no mock, no fixture copy that can drift).
2. Assert the invariant as a set/count comparison with actionable error output.
3. Fail deterministically — the failure message must identify exactly which entry was missing.

**Pattern name:** structural meta-test. Distinct from a unit test (which tests a function's
output) and an integration test (which tests end-to-end behavior). A structural meta-test
tests a structural property of the source code or spec artifact itself.

_Discovered: #407 cycle-close analysis, 2026-05-25. Status: [codified]._

---

### PG-407-3 [observation] Test-only cycles benefit from full F1→F7 VSDD discipline even at 1 SP

**Context (S-407 pipeline — 2026-05-25):** S-407 was a 1-SP test-hardening cycle with no new
BCs, no new VPs, and a net change of +1 EC (EC-3.4.017-14). Despite the small scope, the full
F1→F7 pipeline was applied.

The pipeline surfaced real value at every stage despite the minimal footprint:
- **F2 pass 1 (HIGH design-trap):** The adversary identified that `issue_type` vs `--type`
  clap rename would have caused a `clap::Error` on first run (flag name mismatch). Caught
  before implementation — zero cost to fix.
- **F2 passes 2–4 (MEDIUM clarity findings):** Cross-reference language in EC-3.4.017-14,
  trace frontmatter, invariant wording — all improved before the story was handed to F3.
- **F5 passes 1–3 (all CLEAN):** No fix-PRs needed, confirming the implementation was
  spec-faithful. The meta-test approach (EC-3.4.017-14) mechanically enforces the invariant
  without ambiguity.
- **F6 (100% mutation kill on in-diff mutant):** Confirmed the test catches real faults,
  not just structural conformance.
- **F7 (5/5 PASS):** Traceability confirmed end-to-end.

**Observation:** The compounding value of adversarial convergence applies even to small deltas.
The F2 HIGH finding alone (design-trap that would have caused first-run failure) justified the
pipeline cost for a 1-SP story. The pattern "this is too small for full VSDD" is a rationalization
that should be resisted unless the orchestrator has strong reason to believe F2 will be trivially
CLEAN.

**Status:** [observation] — not a hard rule. The orchestrator may exercise judgment on whether
to apply abbreviated F1d (single-pass) or skip-with-justification on truly mechanical refactors.
But for any cycle involving spec extension (even +1 EC), F2 adversarial review adds value.

_Discovered: #407 F7 cycle-close, 2026-05-25. Status: [observation]._

---

## 2026-05-26 — Lessons from issue #327 (rand 0.9 → 0.10 migration)

### L-327-1 [codified] Empirical-first beats prediction for supply-chain changes

**Context (S-327 — F1/F2/F3 predicted `deny.toml` skip entries; F4 cargo-deny exit 0 without
them; F5 adversary inferred a defect from static analysis that didn't exist):**

The F1 delta analysis and F3 story spec (AC-5) both anticipated that `[[bans.skip]]` entries
would be required in `deny.toml` for the `rand 0.9` / `rand 0.10` transitive dual-presence.
This prediction was reasonable — the deny.toml pattern for getrandom (0.2/0.3/0.4 triple-skip)
and toml (1.x/2.x dual-skip) both use explicit `[[bans.skip]]` entries for similar situations.

However, at F4 implementation time, `cargo deny check` exited 0 WITHOUT any skip entries.
The reason: `cargo-deny` does not flag crates that are dev-dep-only transitive duplicates
when the duplicate path is not in the production dependency graph of the current target platform.
`rand 0.9.4` enters via `proptest 1.x` (dev-dep) and `quinn-proto` (reqwest/rustls transitive —
platform-specific). On the standard `x86_64-unknown-linux-gnu` target, `cargo-deny` saw only
one `rand` instance in the active build graph.

The F5 adversary (pass 1) then inferred from static analysis of the lockfile that `cargo deny
check` MUST fail — rated this HIGH severity. The orchestrator ran `cargo deny check` live and
observed exit 0, empirically refuting the finding. Passes 2/3 were CLEAN.

**Lesson:** For supply-chain and tooling changes (Cargo.toml bumps, deny.toml changes, lockfile
touches), run the tools empirically FIRST before writing spec narrative that predicts what the
tools will report. Prediction based on "similar situations" can be wrong because tool behavior
depends on the exact dependency graph shape at the time of the change. Empirical-first:
1. Apply the change.
2. Run the tools.
3. Observe the actual exit code and output.
4. Write spec narrative to match observed behavior.

Codify as a story-template convention for dependency-migration stories.

_Discovered: S-327 F5 pass-1 resolution, 2026-05-26. Status: [codified]._

---

### L-327-2 [observation] Perplexity verification adds zero value for well-cited primary-source research

**Context (S-327 research verification pass — verdict `PERPLEXITY-CONFIRMS-PRIOR-ASSESSMENT`):**

The Perplexity verification pass for this story found zero divergences from the prior assessment.
All claims were already grounded in primary sources:
- `docs.rs/rand/0.10.x` changelog (rename motivation and behavior equivalence)
- `GHSA-cq8v-f236-94qc` advisory text (affected path: `Rng::sample` with `ThreadRng` + custom logger)
- `rand` 0.10.x release notes and the `getrandom` crate documentation

When 3+ primary sources already cover the claim space, Perplexity duplicates effort without
adding new information.

**Suggestion:** Consider a "skip Perplexity when 3+ primary sources are already cited in the
research doc" heuristic for future research-validation passes. This is not a hard rule —
Perplexity adds value when claims are second-hand (documentation comments, third-party blog
posts, secondary spec citations). For first-party crate documentation + advisory text,
primary sources dominate.

**Status:** [observation] — not a policy change. The existing DEC-018 Perplexity-validation
discipline remains in force for Copilot review findings. This observation applies only to
optional research-verification passes driven by F2/F5 dispatch.

_Discovered: S-327 research verification (`.factory/research/rand-0.10-perplexity-verification.md`), 2026-05-26. Status: [observation]._

---

### L-327-3 [codified] Embed live tool output in F5 dispatch packets to prevent static-analysis false positives

**Context (S-327 F5 pass-1 HIGH false positive F-327-P1-001 — resolved in ~2 hours):**

F5 adversary (pass 1) rated the deny.toml change HIGH severity based on static analysis of the
lockfile: `rand 0.9.4` entry visible → `multiple-versions = "deny"` in deny.toml → `cargo deny
check` must fail → skip entries missing → HIGH defect.

The chain of reasoning was logically sound given the inputs. But the F5 adversary `read-only`
profile cannot run `cargo deny check` to reproduce the claim. The orchestrator had to run the
tool live, observe exit 0, and communicate the resolution back — a round-trip that cost ~2
hours of investigation.

The same pattern will recur for every code-implementation-review F5 cycle where the adversary's
inference is about tool behavior (`cargo build`, `cargo test`, `cargo clippy`, `cargo deny check`,
`cargo fmt`). The adversary cannot disprove their own inference without tool access.

**Mitigation options (in priority order):**
1. **Embed live tool output in F5 dispatch packets by default** — before dispatching F5, run
   `cargo deny check --message-format json 2>&1` (or the relevant tool), capture the full output,
   and include it in the dispatch packet as a `tool_outputs` block. The adversary can then cite
   "verified by cargo-deny exit 0 with output: ..." rather than inferring from lockfile structure.
2. **Grant F5 adversary Bash access scoped to read-only cargo subcommands** — `cargo build`,
   `cargo test`, `cargo clippy`, `cargo deny check`, `cargo audit`, `cargo fmt --check`. This is
   the more powerful fix but requires a profile configuration change.

Codifies PG-327-2. The mitigation is especially high-value for dependency-bump and deny.toml
stories where the adversary's primary axis is supply-chain hygiene.

_Discovered: S-327 F5 pass-1 back-and-forth, 2026-05-26. Status: [codified]._

---

## L-410-1 [codified] F1 architect per-test audit requires grep cross-check to prevent undercounting

**Context (S-410 / issue #410 — 2026-05-27):**

During the F1 delta analysis for S-410, the F1 architect produced a per-test classification
table for `tests/multi_cloudid_disambiguation.rs`. The table listed 11 tests as subject to
gating behind `JR_RUN_KEYRING_TESTS=1`. The actual test count (determined by
`grep -c "^async fn test_\|^fn test_"`) was 12. The missed test was
`test_interactive_render_shows_name_url_and_id` — a full OAuth login flow that asserts
exit-0 and is keychain-transitive.

The undercount was caught by the pr-reviewer's removed-behavior audit during PR review.
A followup commit (211265a) gated the missed test. The PR description count mismatch
(5→6 gated in multi_cloudid, 12→13 total) was then caught by Copilot on pass 1.
Copilot pass 2 was clean.

**Root cause:** The F1 architect classified tests by reading function names and test
docstrings. It did not cross-check the classification table row count against a mechanical
count of test function definitions.

**Process improvement:** Whenever the F1 architect produces a per-test classification
table for a test file, it MUST cross-check the table row count against:
```
grep -c "^async fn test_\|^fn test_" <test_file>
```
If the counts do not match, the table is incomplete and must be revisited before sign-off.

**Disposition:** DEFERRED drift item in STATE.md (single instance, low recurrence risk per
established PG-NNN precedent for single-occurrence process gaps). No follow-up story
created. Target: next maintenance sweep.

_Discovered: S-410 PR review, 2026-05-27. Status: [codified]._

---

## L-408-1 [codified] Copilot caught within-document convention consistency gap missed by internal multi-angle pre-PR review

**Context (S-408 / issue #408 — 2026-05-27):**

S-408 re-anchored 5 stale line-number citations to symbol-form (`<file>::<function>` or
`<file>::<function> § "<comment>"`). The internal pr-reviewer ran its standard three-angle
scan (line-by-line diff, removed-behavior check, cross-file consistency) before the PR opened.
Copilot round 1 then caught a path-prefix consistency gap: line 336 used `create.rs::handle_edit`
(bare module-relative form) while line 334 immediately above used `src/cli/issue/create.rs::handle_edit`
(full src-relative form). The inconsistency was within the same paragraph of the same file. The
fix was a one-line update in bfa333d. Copilot re-review was clean.

**Why the internal reviewer missed it:** The internal pr-reviewer's three angles focus on
correctness, behavior change, and cross-file propagation. "Within-document convention
consistency" is not an explicit axis — the reviewer treats each citation independently rather
than comparing adjacent citations for format uniformity.

**Disposition:** Opportunistic adoption rather than a new agent prompt edit this cycle.
Adding "within-document convention consistency" as an explicit internal review angle would
catch this class of gap, but the class is low-frequency (emerges mainly in docs/spec PRs that
adopt a new formatting convention mid-document) and the Copilot fallback caught it with
minimal cost (one round, one line). No follow-up story created.

_Discovered: S-408 Copilot round 1, 2026-05-27. Status: [codified]._

---

## L-409-1 [codified] Byte-identical refactors surface inherited bugs — extraction PR is a low-stakes place to flag latent issues

**Context (S-409 / issue #409 — 2026-05-27):**

S-409 extracted the existing inline `parsed_number_to_wire_value` conversion logic from
`src/cli/issue/field_resolve.rs` into a named helper function and replaced tautological
integration test 38 with 6 discriminating inline unit tests. The production behavior was
byte-identical; no BC changes were in scope.

During the Copilot review cycle, Copilot caught 2 pre-existing precision bugs at the
f64→i64 boundary in the extracted helper: edge-case values near `i64::MAX` and `i64::MIN`
can silently truncate when the f64 representation is not exact. These bugs existed before
S-409 in the original inline code; the extraction did not introduce them.

The internal pr-reviewer had noticed the boundary behavior but correctly classified it as
"pre-existing, not introduced by this PR" and did not flag it as a S-409 blocker.
Perplexity-validation confirmed the technical claims were accurate (f64 can represent
integers up to 2^53 exactly; beyond that, rounding occurs before the cast). Filed as
follow-up issue #421 for the next maintenance sweep.

**Why this pattern matters:** Byte-identical refactors that extract a function are an
inherently low-risk delivery — no behavioral change means no regression risk. This makes
the extraction PR a natural place to opportunistically surface latent bugs in the
to-be-extracted block. The extractor can scan for precision/overflow/edge-case
issues in the original code and either (a) file follow-ups scoped to the next maintenance
sweep, or (b) include the fix in the refactor scope if the change is trivially contained.
The cost of surfacing is near-zero; the cost of a silent truncation reaching production
is non-trivial.

**Disposition:** Opportunistic — when the next refactor lands that extracts an existing
function, the implementer can scan the block for latent issues and queue them as
follow-ups or include trivial fixes in scope. No new agent prompt edit this cycle.

---

## L-421-1 [codified] F1 architect's full rationale section must be re-read before implementing

**Context (S-421 / issue #421 — 2026-05-28):**

The F1 delta analysis for S-421 explicitly walked through Option A vs Option B vs Option C
trade-offs for the two-stage i64-first parser. The architect's rationale section flagged that
Option B alone would not fix the bug — Option C (i64-first with strict inequalities) was the
correct choice. The BLOCKING bug Copilot R2 caught (precision regression in the initial fix)
occurred because the implementer implemented a version closer to Option B without re-reading
the full rationale section before coding.

**Lesson:** When an F1 delta-analysis explicitly walks through multiple options and selects one
with a documented rationale for why the rejected options are insufficient, the implementer MUST
re-read the full rationale section (not just the recommendation) before implementing. The
recommendation is incomplete without the rationale for why alternatives were rejected.

**Disposition:** Codified. Add to implementer playbook: "Before implementing any Option X
recommendation from an F1 delta analysis, re-read the 'why the other options were rejected'
rationale. The rejection criteria are the safeguards against implementing a near-miss variant."

_Discovered: S-421 Copilot round 2, 2026-05-28. Status: [codified]._

---

## L-421-2 [codified] Rustdoc rewrites touching enumeration labels must grep for stale cross-references before pushing

**Context (S-421 / issue #421 — 2026-05-28):**

Copilot rounds R6-R8 caught 4 separate stale-cross-reference issues introduced during
rustdoc rewrites in rounds R3-R7. Each rewrite touched an enumeration or case label in
a multi-section rustdoc that had internal cross-references (bullet labels, case names,
paragraph back-references). The rewrite systematically leaked stale references pointing
at the OLD structure. The pattern repeated across 4 rounds because each round's fix
introduced new stale references in the sections that were rewritten to fix prior issues.

**Lesson:** After any rustdoc rewrite that touches an enumeration or case label, grep the
file for all references to the OLD label names BEFORE pushing. Internal cross-references
in rustdoc are invisible to the compiler (unlike Rust symbol references) and silently
diverge on any structural rename.

**Disposition:** Codified. Implementer playbook addition: "When rewriting a multi-section
rustdoc that contains enumeration labels (e.g., 'Stage 1', 'Case A', 'Form X'), before
pushing: grep the file for all occurrences of each old label name to identify stale
back-references. Treat stale rustdoc cross-references with the same seriousness as stale
code cross-references — they mislead maintainers."

_Discovered: S-421 Copilot rounds 6-8, 2026-05-28. Status: [codified]._

---

## L-421-3 [codified] Library-behavior claims in docs must be empirically verified before asserting

**Context (S-421 / issue #421 — 2026-05-28):**

Copilot R6 caught a doc claim about serde_json's serialization behavior that was empirically
false: the rustdoc asserted that "serde_json serializes integer-valued f64s as bare integer
literals" (e.g., `1.0` → `1`). This claim was carried through R3 and R4 doc rewrites without
verification. The claim contradicts serde_json's actual behavior (it serializes `1.0_f64` as
`1.0`, preserving the fractional part).

**Lesson:** Any doc claim about a library's runtime behavior (serialization, formatting,
parsing semantics, encoding conventions) MUST be empirically verified by a small test
program or REPL check before being asserted in rustdoc. Library behavior can change across
versions, and training-data knowledge is not a substitute for a runtime test.

**Disposition:** Codified. Mirrors the existing "Perplexity-validate any external-tracker
citation" rule in CLAUDE.md. Generalized rule: "Empirically verify any library-behavior
claim before asserting it in docs." Consider adding to CLAUDE.md AI Agent Notes as a
standing rule alongside the Perplexity-validation rule.

_Discovered: S-421 Copilot round 6, 2026-05-28. Status: [codified]._

---

## L-421-4 [codified] S-410 architect-miscount extends — 'follows exit path' reasoning incomplete for subprocess keychain classification

**Context (S-421 / issue #421 — 2026-05-28):**

During the S-421 PR cycle, 3 'NO-KEYCHAIN' tests flaked on parallel CI runs:
- `test_no_input_multi_org_exits_64_with_actionable_error`
- `test_cloud_id_flag_value_not_in_response_exits_64`
- `test_no_input_multi_org_lists_available_cloud_ids_in_error`

The S-410 F1 architect had classified these as no-keychain because "the exit-64 path doesn't
reach `store_oauth_tokens`". But the subprocess setup happens early enough that
`JR_SERVICE_NAME` contamination still occurs during parallel test execution, causing
contention even on the exit-64 path. Filed as #428.

**Lesson:** The architect's "follows exit path" reasoning for keychain classification was
incomplete — it considered only the explicit code path to keychain write, not the full
subprocess lifecycle (where JR_SERVICE_NAME is set at subprocess spawn time, before any
exit-64 branch is reached). Future keychain-isolation audits must check whether the test
subprocess sets `JR_SERVICE_NAME` at all, regardless of whether the code path reaches an
explicit keychain call.

**Disposition:** Codified. Filed as #428 (3 more tests need gating behind JR_RUN_KEYRING_TESTS=1).
Future F1 audits for keychain test isolation should use the question: "Does this test's subprocess
set JR_SERVICE_NAME at any point during its lifecycle?" not "Does the code path reach a keychain
call?".

_Discovered: S-421 PR CI flakes (3 occurrences), 2026-05-28. Status: [codified]. Follow-up: #428._

---

## L-421-5 [codified] Copilot review diminishing-returns heuristic — stop when findings transition from 'bugs in fix' to 'imprecision in my own doc cleanup'

**Context (S-421 / issue #421 — 2026-05-28):**

The S-421 PR cycle used 9 Copilot review rounds (R1-R9), the deepest of the project. The 15
distinct findings followed a clear pattern:
- R1: deferred to follow-up (out of scope)
- R2: BLOCKING precision regression in initial fix
- R3-R5: docs imprecision + contract-vs-impl mismatch (`trim_start_matches` multi-sign) + minor API contract issues
- R6-R8: stale cross-references I introduced in my own R3-R7 rewrites + empirically-false serde_json claim
- R9: design-intent disagreement (accepted as documented Option C trade-off)

Rounds R6-R8 were in response to issues I introduced during my own doc cleanup. R9 was a
design debate rather than a bug.

**Lesson:** Once Copilot's findings transition from "bugs in the fix" to "imprecision in my
own doc cleanup" (i.e., the fix has stabilized but my rewrite work keeps introducing new
doc issues), that is the diminishing-returns inflection point. The correct response at that
point is: (a) step back from incremental doc rewrites, (b) do one complete top-down reread
of the changed rustdoc looking for internal consistency, then (c) push and request one final
Copilot pass. Doing incremental rewrites per-round causes each round's fix to potentially
introduce new stale references in the rewritten sections.

**Disposition:** Heuristic codified. Suggested rule: "If two consecutive Copilot rounds find
only doc nits introduced by my own previous-round rewrites, stop incremental rewrites. Do
a single top-down consistency pass of the full changed doc, then close." Rounds R8/R9 in
this cycle could likely have been replaced by this approach.

_Discovered: S-421 9-round Copilot cycle retrospective, 2026-05-28. Status: [codified]._

_Discovered: S-409 Copilot review, 2026-05-27. Status: [codified]._

---

## L-428-1 [codified] `pub(crate)` is invisible to integration-test crates — use `#[doc(hidden)] pub` for test-reachable non-public items

**Context (S-428 / issue #428 — 2026-05-28):**

The F1 delta analysis for S-428 locked `AccessibleResource` and `resolve_cloud_id` as
`pub(crate)` visibility (decision DEC-028). During F3 implementation, the implementer
discovered that `pub(crate)` is not reachable from `tests/` — integration test crates link
the non-test build of the library as an external crate, which sees only `pub` items from
the library's API surface. `pub(crate)` restricts visibility to within the current crate;
from the perspective of an external crate (like the integration-test binary), `pub(crate)`
items are completely invisible.

The correct visibility for items that must be reachable from `tests/` but are not intended
as a supported public API is `#[doc(hidden)] pub`:
- `pub` makes the item reachable from external crates (including integration tests).
- `#[doc(hidden)]` suppresses the item from rustdoc output, signaling "not a supported API."

This deviation was validated via research-agent + Perplexity; the story ACs were corrected
accordingly before F4 sign-off.

**Rule:** F1 design decisions that prescribe `pub(crate)` visibility for items intended to be
called from `tests/` are technically impossible — `pub(crate)` is invisible to integration
test crates. The F1 architect should ask: "Does this item need to be reachable from `tests/`
(integration test crate)?" If yes, `pub(crate)` is wrong; use `#[doc(hidden)] pub` instead.
If the item only needs to be reachable from inline `#[cfg(test)]` blocks within the same
file, `pub(crate)` is correct.

**Summary:** `pub(crate)` = reachable from any module within the same compiled crate
(including inline `#[cfg(test)]` blocks). `pub` = reachable from any crate. Integration
test files in `tests/` are compiled as separate crates — they see only `pub` items.

_Discovered: S-428 F3 implementation, 2026-05-28. Source: research-agent + Perplexity validation. Status: [codified]._

---

## L-428-2 [codified] [process-gap] Story AC verification greps must anchor on stable code-arm patterns, not speculative implementations

**Context (S-428 / issue #428 — 2026-05-28):**

During the S-428 cycle, story ACs were written with literal grep verification commands
before the implementation existed. Three verification commands drifted from the
as-built code:

1. AC visibility grep: `grep "pub(crate) fn resolve_cloud_id"` — the implementation used
   `#[doc(hidden)] pub fn resolve_cloud_id` (pub(crate) invisible to integration tests).
2. AC test-attribute grep: `grep '#\[ignore\]'` — the rewritten tests used
   `#[ignore = "..."]` with an explanatory string, not bare `#[ignore]`.
3. AC resource-ID count: a grep that counted `resources[0].id.clone()` occurrences also
   matched a rustdoc example line, producing a count one higher than expected.

All three were resolved before F7 close, but each required a mid-cycle spec correction pass.

**Root cause:** AC verification greps were written speculatively based on imagined
implementation structure, not derived from actual code patterns. The greps were
"correct in spirit" but incorrect in exact syntax.

**Rule:** When writing story ACs that include grep-based verification commands:
1. Write the grep AFTER the implementation exists (or at minimum, after the implementer
   confirms the exact code form).
2. Anchor greps on stable semantic patterns (e.g., function name, struct name) rather
   than assumed exact syntax (e.g., exact attribute spelling, exact attribute argument form).
3. Validate the grep command against the actual code before including it in the AC.
4. For count-based greps (e.g., "appears exactly N times"), add a non-code-file exclusion
   (e.g., `--include="*.rs" --exclude-dir=target`) to avoid false matches from docs or tests.

**Scope:** Applies to all future story ACs that include literal grep verification commands.
Consider whether the story-writer agent prompt should require verification-grep validation
against actual code before sign-off.

**Disposition:** DEFERRED drift item (L-428-2-PG) added to STATE.md Drift Items table —
target: next maintenance sweep; reason: low-severity doc-mechanics gap, no runtime impact.
No follow-up story created.

_Discovered: S-428 F3–F5 cycle (3 AC grep drift instances), 2026-05-28. Status: [codified] [process-gap]._

---

## L-400-1 [codified] [receiving-code-review] Validate Copilot's stated causal mechanism by code trace before acting — hardening that documents intent is still a valid outcome even when the mechanism is wrong

**Context (S-400-A / issue #400 — 2026-05-28):**

During the S-400-A cycle (4 Copilot review rounds on TH-398-1..4 test hardening),
round-3 surfaced the following finding: "the `--output table` flag addition to the dry-run
echo test is unnecessary because `config.defaults.output` would flip the output branch
regardless, making the test environment-dependent."

Code trace refutation:
- `config.defaults.output` is read in `src/config.rs` as a config-file default.
- The runtime output decision in `main.rs` reads `cli.output`, which is the clap-parsed
  value from the command line — clap-defaulted to `"table"` regardless of config file.
- `config.defaults.output` is therefore NOT wired into the runtime output path for the
  binary under test. The claimed branch-flip mechanism does not exist.

**Outcome:** The `--output table` flag was retained as **defensive hardening** that makes
the test's intent explicit, not as a fix for a real bug. The distinction matters: if the
fix had been applied as a real bug fix, a future reader might infer that the test was
previously broken — misleading audit trail. Framing it as defensive hardening is accurate.

**Rule (per DEC-018 / receiving-code-review discipline):**
1. When a Copilot review finding identifies a causal mechanism (e.g., "X causes Y"),
   trace the mechanism in the actual code before accepting the finding as a bug.
2. If the mechanism is false but the suggested change is still beneficial (e.g., makes
   intent explicit, adds a defensive assertion), apply it as hardening — not as a bug fix.
3. Document the refutation in the DEC log so future readers understand the real rationale.

**Scope:** Applies to all future Copilot review cycles on this project and validates the
DEC-018 standing rule (established 2026-05-11) that Perplexity/code-trace validation
should precede action on any Copilot finding.

_Discovered: S-400-A round-3 Copilot review, 2026-05-28. Status: [codified] [receiving-code-review]._

---

## 2026-05-31 — E2E Test-Enhancements Feature (S-E2E-3/4/5) — Session Review

**Arc summary:** Full VSDD Feature-Mode F1→F7 cycle delivering 3 stories (S-E2E-3 M1+foundation,
S-E2E-4 M2 coverage, S-E2E-5 M3 ops) that hardened the live-Jira E2E test suite. Brainstormed
design → Perplexity-backed research → F1 delta → F2 spec adversarial (7 passes, 3-clean) →
F3 stories → F4 per-story TDD delivery (5 PRs #435-#439 onto an integration branch) → F5
combined-delta adversarial (3-clean) → F6 hardening (scoped zero-src) → F7 convergence →
merged via PR #440 onto develop. Zero src/ changes throughout. 38 ACs, ~14 new gated live
tests, 18 always-run unit tests. Shipped to develop @ 8f3e2a1; live e2e.yml 30/0.

**Decision refs:** DEC-037 (F1 approval), DEC-038 (F2 convergence), DEC-039 (F3 stories),
DEC-040 (S-E2E-3 merged), DEC-041 (S-E2E-4 merged), DEC-042 (S-E2E-5 merged), DEC-043 (F5
converged), DEC-044 (F6+F7 converged, merge ready).

---

### L-E2E-1 [codified] Combined-delta F5 is essential and structurally irreplaceable

The F5 combined-delta adversarial pass (reviewing the full feature delta as a single unit)
caught 2 HIGH cross-story integration defects that per-story reviews AND all automated gates
structurally could not catch:

- **F-1 (portability gap):** The `issue_type()` env-parametric helper (making tests run on
  any Jira instance, not just one that has "Task") was used in only 1 of 10 issue-creating
  tests. The other 9 hardcoded `"Task"`, defeating the whole-instance-portability requirement.
  Per-story review saw S-E2E-3 in isolation — the helper was present in that story, so it
  looked correct. Only the combined view revealed the inconsistency across S-E2E-3 and S-E2E-4.

- **F-2 (teardown orphan):** Dedup-test issues carried only the unique run label in their
  label set. The teardown's `labels = e2e-<run_id>` exact-match filter could not find them
  (they only had the unique label), guaranteeing per-run orphans that accumulate in the live
  Jira project. Again: per-story review could not see the teardown label contract across stories.

**Why automated gates cannot substitute:** Gated `#[ignore]` tests do not execute without
`JR_RUN_E2E=1`. The entire live-execution path is invisible to CI. Source-verified adversarial
review of assertions is the only practical defense for this code class.

**Rule:** Never skip or compress F5 for zero-src test/CI stories on the grounds that "there
is no production surface to review." The production surface is the live Jira project — the
tests run against it, and cross-story integration defects accumulate there.

_Discovered: E2E-enh F5 combined-delta pass, 2026-05-31. Status: [codified]._

---

### L-E2E-2 [codified] Never pre-write a gate or convergence verdict before the pass actually returns

Twice during this feature the orchestrator dispatched state-manager to record "CLEAN" or
"converged" verdicts before the underlying review pass had completed or been read:

1. A "P3 CLEAN" verdict was written for a pass that had not been run (the pass had been
   cancelled by a model outage that wiped unsaved edits; the on-disk state reflected the
   pre-P3 content, not any P3 result).
2. A speculative batch of "P4/P5/P6 converged" records — including fixes and commit SHAs —
   was written prospectively. A model outage exposed this; a referenced commit (d6f0826)
   never existed.

Both required correction passes and left a temporarily misleading audit trail.

**Root cause:** The orchestrator conflated "planning the next N steps" with "recording
completed steps." These are different operations with different preconditions.

**Rule:** A state-manager write for a review pass MUST be triggered by reading the actual
pass output, not by predicting what that output will say. The flow is:
1. Dispatch the review agent.
2. Read the full result.
3. Dispatch state-manager with the real verdict.

Never combine steps 2 and 3 speculatively before step 1 has returned.

_Discovered: E2E-enh F2 and F5 cycles, 2026-05-31. Status: [codified]._

---

### L-E2E-3 [codified] Never batch speculative future review passes with their fixes and commits

Closely related to L-E2E-2 but a distinct failure mode: the orchestrator planned and
partially executed a batch of "pass N → find X → fix → commit → pass N+1 → clean" in a
single planning step, without running each pass sequentially and waiting for the real result.

This produces phantom state: fix commits that address findings that weren't confirmed,
state records that describe a clean pass that didn't happen, and convergence declarations
that are ahead of reality.

**Rule:** One pass at a time. Run the pass, read the result, fix what was found, commit,
then run the next pass. Do not plan past the next pass boundary.

_Discovered: E2E-enh F2 cycle, 2026-05-31. Status: [codified]._

---

### L-E2E-4 [codified] Fix edits must themselves be surface-validated against source

During F2 adversarial convergence, pass-2 introduced a fix that added a `to_category` field
to a transition assertion. This field does not exist on the `Transition` serde type in the
handlers. The phantom field was caught in pass-3 as a CRITICAL.

The pass-2 fix was generated from memory/assumption about the JSON shape, not derived from
reading the actual serde struct definition and handler code. Every JSON-shape claim —
including corrections — must be derived from the actual source, not from the reviewer's
mental model of what the shape should be.

**Rule:** When a review pass finds a wrong JSON shape and a fix is authored, that fix must
be validated against the real serde type + handler before being committed. The correction
can introduce a new error as easily as the original code did.

_Discovered: E2E-enh F2 pass-3 CRITICAL (to_category nonexistent field), 2026-05-31. Status: [codified]._

---

### L-E2E-5 [codified] Measure before escalating — never base a diagnosis on a number you haven't actually measured

During F6 regression verification, the orchestrator misdiagnosed a 4130-line file as a
"10001-line runaway" — a figure that was not measured but was stated with apparent confidence.
Based on this phantom measurement, the orchestrator asked the human a loaded "corruption
recovery" question that implied the working directory was in a bad state.

The file was never corrupt. The correct line count (`wc -l`) would have taken one shell
command and two seconds to run.

**Rule:** Before escalating any size, count, line-number, or state diagnosis to the human,
run the measurement command that would confirm or refute it. "I believe the file is N lines"
is not a measurement. `wc -l <file>` is a measurement. Escalate only after measuring.

**Related failure:** This same session also saw an interim test run report "8 failed" when
the real cause was a force-removed worktree destroying the working directory mid-run (the
background `cargo test` process was still running in the worktree when `git worktree remove
--force` deleted it). A clean re-run immediately produced 1521/0/58. The phantom failure
report caused unnecessary concern. See also L-E2E-8.

_Discovered: E2E-enh F6 regression verification, 2026-05-31. Status: [codified]._

---

### L-E2E-6 [codified] Partial-fix propagation discipline — grep ALL sites when fixing a value or pattern

Three instances of partial-fix propagation gaps occurred in this feature cycle:

1. **F-1 cross-story (the major one):** `issue_type()` helper propagated to only 1 of 10
   create-test call sites. The other 9 were found and fixed by the F5 combined-delta pass.
2. **Line-budget doc inconsistency:** The line-budget constant was updated from 400 to 500
   in the code and in one of two documentation sites. The second doc site still said 400.
   Caught by the F5 adversarial re-review.
3. **create-JSON shape:** A fix to the create test JSON shape corrected the field at one
   assertion site but not all sites where the same shape assumption was present.

**Rule:** When fixing a value, pattern, or helper that appears at multiple call/reference
sites, grep ALL sites BEFORE committing the fix. The fix is not complete until every site
is updated. Use: `grep -rn <pattern> tests/ docs/` and review every match before finalizing.

This is a specific application of the existing "Perplexity validates APPROACH; grep validates
SURFACE AREA" lesson (PR #357 R1) — extended now to cover test/doc fix propagation, not just
security-sensitive gating.

_Discovered: E2E-enh F5 combined-delta pass and F2 review, 2026-05-31. Status: [codified]._

---

### L-E2E-7 [codified] Security review is mandatory for CI/secret-handling stories even when zero src/ changes

Story S-E2E-5 (M3 ops) was the only F4 delivery in this feature that required a security
review in addition to a code review, because it touched CI workflow files and secret-handling
logic (the leak-guard test, e2e-sweeper.yml, and the 401-vs-connection classifier in e2e.yml).

The security review found 2 MEDIUM + 1 LOW CWE-532 issues:
- The leak-guard test's own `assert!` failure message echoed the email it was trying to guard
  against leaking — the security test leaked on failure.
- The sweeper workflow interpolated secrets into a text string that was echoed to the log.
- The `UNKNOWN` branch in e2e.yml dumped raw probe output containing the full HTTP response.

All three were fixed before merge. None would have been caught by the standard code review
(which focuses on correctness, not secret-handling patterns).

**Rule:** Any story that touches CI workflow files (`.github/workflows/`), secret composition,
or logging of probe outputs requires a security review, regardless of whether `src/` changes.
The security surface of a CI workflow can be substantial even with zero production-code changes.

_Discovered: E2E-enh F4 S-E2E-5 security review, 2026-05-31. Status: [codified]._

---

### L-E2E-8 [codified] Do not force-remove a verify worktree while a background job runs in it

During F6 regression verification, the orchestrator force-removed a worktree
(`git worktree remove --force`) while a background `cargo test` process was still executing
inside it. The removal destroyed the working directory mid-run. The in-progress `cargo test`
saw its source tree vanish, and the run output erroneously reported 8 failed tests.

A clean re-run immediately after the force-remove produced 1521 passed / 0 failed / 58
ignored — confirming the "failures" were filesystem destruction artifacts, not real regressions.

**Rule:** Before force-removing a worktree, verify no background processes are running in it.
Use `jobs` or `ps aux | grep cargo` to check. If a background `cargo test` or other build
process is running, wait for it to finish or kill it explicitly before removing the worktree.

_Discovered: E2E-enh F6 regression verification, 2026-05-31. Status: [codified]._

---

### L-E2E-9 [codified] State-manager writes require absolute file paths; read back to verify the write landed

Multiple state-manager dispatches during this feature produced silently mis-landed or absent
writes when relative paths were used as the write target. Because the agent tool call returns
success regardless of whether the content reached the intended location, there is no automatic
signal of failure.

**Rule:** Every state-manager write MUST:
1. Use an absolute path (e.g., `/Users/zious/Documents/GITHUB/jira-cli/.factory/...`),
   never a relative path.
2. Be followed immediately by a read-back that confirms the expected section header (or
   a key sentinel line) is present in the file at that absolute path.

If the read-back does not find the expected content, the write failed or landed in the wrong
location. Investigate before proceeding.

_Discovered: E2E-enh F2/F5 state-manager dispatch sessions, 2026-05-31. Status: [codified]._

---

### L-E2E-10 [process-gap] Mechanical jr-invocation-vs-clap-tree guard — follow-up candidate

**Background:** The assumed-CLI-surface defect class (writing test/spec assertions against
`jr <subcommand>` invocations that do not actually exist in the clap command tree) recurred
approximately 10 times across this feature and the prior E2E story (S-E2E-1/2). Examples
from F2 adversarial:
- `jr project view` referenced as a test scenario — no such subcommand exists.
- `jr auth status --output json` assumed a JSON output arm that was not implemented.
- `jr project fields` with assumed argument shapes that diverged from the real handler.

The F2 design-spec adversarial review (7 passes, 3-clean) was specifically motivated by the
prior E2E story's 6 CRITICAL assumed-surface findings. The adversarial review is effective at
catching these, but it is a human-in-the-loop process — each instance costs a full adversary
pass and a fix round.

**The gap:** No mechanical guard exists that extracts every `jr <command> <subcommand>` token
from spec and test files and validates each against the live clap command tree (or `jr --help`
output) at authoring time. Such a guard would catch this class at near-zero marginal cost.

**Partial mitigations already in place:**
- The always-run line-budget meta-test (`test_no_test_function_exceeds_line_budget`) prevents
  gated-dead-code bloat from accumulating undetected.
- Source-verified adversarial review of assertions (F5) catches surface defects, though not
  at authoring time.

**Recommendation:** File as a follow-up story (candidate story: "CLI surface smoke-check
script — extract jr invocations from test/spec files and validate against `jr --help` tree").
Scope: a shell script (or Rust integration test) that:
1. Greps for `jr <word>` patterns in `tests/`, `docs/specs/`, and `.factory/stories/`.
2. Runs `jr <word> --help` (or `jr --help | grep <word>`) to confirm the subcommand exists.
3. Exits non-zero and prints the failing invocation if any subcommand is not found.
4. Runs in CI as an always-run check (not gated behind `JR_RUN_E2E`).

This is a LOW-effort story (the script is ~30 lines of shell) with HIGH recurrence-prevention
value given the ~10 occurrences across two feature cycles.

**Disposition:** Recommend filing as a follow-up story rather than deferring as justified.
The recurrence frequency (~10 hits in 2 features) and the low implementation cost make this
worth scheduling. A justified deferral would be appropriate only if the adversarial review
gate is considered a sufficient control — but the F2/F5 evidence shows it catches instances
AFTER spec/test authoring, not at authoring time.

_Discovered: E2E-enh F2 adversarial convergence + recurrence analysis, 2026-05-31. Status: [process-gap]._
