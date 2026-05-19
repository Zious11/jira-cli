# Issue #383 Delivery Lessons

## L-383-01 — Pre-implementation research-agent validation (L-288-pr4-06 applied)
The pre-F2 research-agent design-validation confirmed gh CLI PR #12039 as a near-exact
precedent for the inverse-warning fix pattern (warn-and-continue to stderr). This de-risked
the F2 BC codification before any spec was written. L-288-pr4-06 (research-agent validates
BEFORE implementation) is working as intended.

## L-383-02 — BC-INDEX is the highest-blast-radius partial-fix-propagation site
The 11-pass F2 convergence cycle was dominated by partial-fix-regression findings (S-7.01):
stale numeric counts and missing amendment markers in BC-INDEX.md, CANONICAL-COUNTS.md, and
README.md. Adding a cumulative-sum CI guard to check-spec-counts.sh (DEFER-383-3) would have
caught ~80% of these at commit time, collapsing 11 passes to ~3.

## L-383-03 — Parallel fresh-context adversary dispatches surface non-overlapping defects
When two adversary passes ran in parallel on the same artifact state, they caught DIFFERENT
defects (pass-05A found README:38 bc-2 drift; pass-05B missed it; pass-07A found BC-INDEX:215;
pass-07B missed it). Parallel dispatch is efficient but BOTH outputs must be treated as a
combined evidence pool — a CLEAN from one does not override a DIRTY from its parallel sibling.

## L-383-04 — Adversary path-scoping error (orchestrator dispatch bug)
Per-story adversary pass-02 initially returned a FALSE "implementation absent" verdict because
the dispatch prompt used main-repo-relative path phrasing while the adversary (Read/Grep/Glob
only, no Bash) resolved paths against the main repo, not the worktree. FIX: when dispatching
adversary/review agents against worktree code, ALWAYS specify fully-qualified worktree-absolute
paths (`/repo/.worktrees/S-NNN/src/...`) AND add an explicit pre-flight grep step that
fail-fasts if the expected symbols are absent. Codified for future per-story adversary dispatches.
