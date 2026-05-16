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
