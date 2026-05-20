# Issue #392 Delivery Lessons

## L-392-01 — DRIFT-002 guard closes the process-gap that caused #383's 11-pass F2 churn
S-392 implements the cumulative spec-count CI guard recommended after issue #383's F2
phase required 11 adversary passes, most of them sweeping stale-count partial-fix-regression
drift across BC-INDEX / CANONICAL-COUNTS / README. The new `check-bc-cumulative-counts.sh`
(DRIFT-002) now catches that drift class at CI time. Meta-win: a process-gap surfaced by
one cycle's adversary review was converted into a permanent automated guard.

## L-392-02 — Adversary bash-semantics claims must be empirically verified, not asserted
During S-392 per-story adversarial review, a pass asserted that `var=$(grep|awk)` command
substitution in a simple assignment is EXEMPT from `set -e` (so Surfaces A/E needed no
`|| true`). Copilot's PR review contradicted this. Research-agent validation confirmed
Copilot CORRECT: a *bare* `var=$(...)` assignment IS errexit-fatal under `set -euo pipefail`
when the pipeline fails — only `local var=$(...)` is exempt (because `local`'s own exit
status masks it). The adversary conflated the in-subshell `set -e` disabling with the
assignment statement's own exit status. LESSON: when an adversary (or any agent) makes a
claim about shell/language edge-case semantics, it must be EMPIRICALLY verified (write and
run the construct) — not asserted from memory. The research-agent's empirical-test protocol
is the model. Codify into adversary review guidance: bash `set -e`/`pipefail` claims require
a runnable proof.

## L-392-03 — Copied fixtures propagate a template defect N-fold
19 Copilot comments collapsed to 5 root issues; the largest (finding E) was ONE defect — a
wrong `definitional_count` — replicated across 7 fixture trees because the test-writer
copied a single `bc-2-issue-read.md` to all trees. A template defect in copied fixtures
yields N-fold review noise. LESSON: when fixtures share a copied base file, validate the
base ONCE before propagation; or generate fixtures from a single source rather than copying.

## L-392-04 — Parallel adversary passes diverge; treat as a combined evidence pool
Consistent with L-383-03: in S-392's convergence round, parallel pass A flagged Surface D's
missing numeric-validation as MEDIUM (DIRTY) while parallel passes B and C graded it LOW or
missed it. Parallel fresh-context passes find non-overlapping defects and disagree on
severity. The orchestrator must treat ALL parallel outputs as one evidence pool and apply
the strictest applicable verdict — a CLEAN from one pass does not override a DIRTY from its
sibling.

## L-392-05 — Infrastructure stories: ACs trace to a design doc, not BCs; Red Gate adapts
S-392 is CI tooling — no behavioral contracts. ACs traced to the issue-392 design doc
(Q1-Q6) instead of BC-S.SS.NNN. The Red Gate was "fixture harness exists and fails because
the guard script is absent" (a bash-tooling analog of "tests fail before implementation").
Step 2 (cargo-check stubs) was N/A. This is a valid VSDD adaptation for infrastructure work
and parallels how S-383 (pure-addition) skipped Step 2 — document as the standard pattern
for tooling/CI stories.
