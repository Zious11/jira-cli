# S-3.06 Demo Evidence Report

Story: Codify pre-merge numeric claim checker for BC heading/body drift (DRIFT-001)
Branch: chore/s-3-06-spec-counts-checker
Type: FACADE-MODE (process tooling + docs)

## Per-AC Evidence

### AC-001 — Script exits 0 on current spec corpus

- VHS GIF: AC001_script_exits_clean.gif (31 KB)
- VHS WebM: AC001_script_exits_clean.webm (27 KB)
- Tape: AC-001-script-exits-clean.tape
- Captured output:
  ```
  OK: all spec counts verified.
  ```
- Exit code: 0
- Verdict: PASS

---

### AC-002 — Script exits 1 on corrupted frontmatter

- VHS GIF: AC002_corrupted_frontmatter_detected.gif (137 KB)
- VHS WebM: AC002_corrupted_frontmatter_detected.webm (197 KB)
- Tape: AC-002-corrupted-frontmatter-detected.tape
- Recording captures:
  1. `sed -i.bak` corrupts `definitional_count: 46` → `definitional_count: 9999`
  2. Script runs and exits 1 with error output:
     ```
     ERROR: .factory/specs/prd/bc-1-auth-identity.md: actual #### BC- count=46, frontmatter definitional_count=9999
     FAIL: 1 spec count mismatch(es). Fix frontmatter or body before merging.
     ```
  3. `mv .bak` restores the original file
  4. Script re-runs and exits 0: `OK: all spec counts verified.`
- Exit code: 1 (during corruption), 0 after restoration
- Verdict: PASS

---

### AC-003 — Script verifies nfr-catalog total_nfrs (and holdout-scenarios total_holdouts)

Bundled with AC-001. The script checks all three corpora in a single invocation:
1. All `bc-*.md` files (7 files, definitional_count vs actual `#### BC-` headings)
2. `nfr-catalog.md` (total_nfrs vs actual `| **NFR-` table rows)
3. `holdout-scenarios.md` (total_holdouts vs actual `### H-` headings)

The AC-001 recording (`AC001_script_exits_clean.gif`) demonstrates the full corpus
passing all three check categories. See AC-001 evidence above.

Verdict: PASS

---

### AC-004 — lessons-codification.md exists with all required sections

Location: factory-artifacts branch, commit 4194611
Path: `.factory/rules/lessons-codification.md` (81 lines)

Required sections present (from `git show 4194611 -- rules/lessons-codification.md`):

```
## Pattern
## Root Cause
## Mitigation
## When to Run
## Escalation
## Cross-Reference
```

Full diff as delivered:

```diff
diff --git a/rules/lessons-codification.md b/rules/lessons-codification.md
new file mode 100644
index 0000000..0e12ae3
--- /dev/null
+++ b/rules/lessons-codification.md
@@ -0,0 +1,81 @@
+---
+document_type: lessons-codification
+rule_id: DRIFT-001
+last_updated: 2026-05-09
+status: active
+producer: story-writer
+related_story: S-3.06
+mitigation: scripts/check-spec-counts.sh
+---
+
+# DRIFT-001: BC Heading vs Body Count Drift
+
+## Pattern
+
+During Phase 1d adversarial spec review, the same finding class recurred 4
+times across passes P21, P22, P23, P24:
+
+- Pass 21: H-044 + L2 — BC heading count mismatch
+- Pass 23: reaffirms same pattern with different BC file
+- Pass 24: BC-2.1.006 12 vs 13 discrepancy
+- (Plus the original P21 instance)
+
+Each instance involved a numeric count claim in a BC file's YAML frontmatter
+(`definitional_count: N`) drifting from the actual `#### BC-` heading count
+in the body. The same pattern affected `total_nfrs:` in `nfr-catalog.md` and
+`total_holdouts:` in `holdout-scenarios.md`.
+
+## Root Cause
+
+Edits to a spec file that change the body BC heading count update either:
+- (a) the frontmatter declaration, OR
+- (b) the body content,
+
+but rarely both atomically. The drift is invisible to git diff review (which
+sees a coherent change in either file location), and only surfaces when
+adversarial-pass token counters or the canonical-counts document are recomputed.
+
+## Mitigation
+
+`scripts/check-spec-counts.sh` (introduced in S-3.06) is a pre-merge bash
+script that:
+
+1. Walks each `bc-*.md` file in `.factory/specs/prd/`, counts `#### BC-`
+   headings, compares to `definitional_count:` frontmatter. Mismatch → exit 1.
+2. Walks `nfr-catalog.md`, counts `^| \*\*NFR-` table rows, compares to
+   `total_nfrs:`. Mismatch → exit 1.
+3. Walks `holdout-scenarios.md`, counts `^### H-` headings, compares to
+   `total_holdouts:`. Mismatch → exit 1.
+4. Exits 0 if all counts match.
+
+## When to Run
+
+Run `scripts/check-spec-counts.sh` from the repo root:
+
+- After any adversarial pass that adds or removes a BC, NFR, or holdout.
+- Before declaring spec convergence at any phase gate.
+- Before merging any PR that touches `.factory/specs/prd/bc-*.md`,
+  `nfr-catalog.md`, or `holdout-scenarios.md`.
+- Optionally: as a `lefthook` pre-commit or pre-push hook (deferred — see
+  story's Out of Scope).
+
+## Escalation
+
+If the script reports drift:
+
+1. Identify the affected file(s) from script output.
+2. Determine which side is correct: did a recent edit add/remove a heading
+   without updating the count, or vice versa?
+3. Fix the inconsistent side. Do NOT auto-fix via the script — the script
+   intentionally reports only and does not modify content (auto-fix could
+   mask the root cause).
+4. Re-run the script to verify exit 0.
+
+## Cross-Reference
+
+- `CANONICAL-COUNTS.md` (`.factory/specs/prd/CANONICAL-COUNTS.md`, if present)
+  is the source of truth for grand totals across all spec files.
+  `check-spec-counts.sh` validates per-file consistency; it does NOT validate
+  the canonical grand total. That's a separate (deferred) verification.
+- `STATE.md` Drift Items table tracks DRIFT-NNN findings as they are
+  identified during pipeline operation.
```

Verdict: PASS

---

### AC-005 — CLAUDE.md AI Agent Notes references the script

From `grep -A 3 "check-spec-counts.sh" CLAUDE.md` (in worktree):

```
- Run `scripts/check-spec-counts.sh` after any edit to .factory/specs/prd/ BC files,
  nfr-catalog.md, or holdout-scenarios.md. Exits 0 if frontmatter counts match body counts.
  Exits 1 with specific mismatch details if drift is detected (DRIFT-001 mitigation).
```

Location: `CLAUDE.md` — AI Agent Notes section (commit df476a3 on chore/s-3-06-spec-counts-checker).

Verdict: PASS

---

## Summary

All 5 ACs have at least one demo artifact. Story delivers:
- 1 new shell script (`scripts/check-spec-counts.sh`, 61 lines, POSIX-portable bash)
- 1 new lessons-codification document (`.factory/rules/lessons-codification.md`, 81 lines)
- 1 CLAUDE.md addition (3-line bullet in AI Agent Notes)

Two-branch delivery:
- develop-side: commits f44b8ca (script) + df476a3 (CLAUDE.md) on `chore/s-3-06-spec-counts-checker`
- factory-artifacts: companion commit 4194611 (`rules/lessons-codification.md`)

All ACs verified via direct script invocation + content inspection.

## Artifact Index

| Artifact | AC | Size |
|----------|----|------|
| AC001_script_exits_clean.gif | AC-001, AC-003 | 31 KB |
| AC001_script_exits_clean.webm | AC-001, AC-003 | 27 KB |
| AC-001-script-exits-clean.tape | AC-001 | 376 B |
| AC002_corrupted_frontmatter_detected.gif | AC-002 | 137 KB |
| AC002_corrupted_frontmatter_detected.webm | AC-002 | 197 KB |
| AC-002-corrupted-frontmatter-detected.tape | AC-002 | 751 B |
| evidence-report.md (this file) | AC-003, AC-004, AC-005 | — |
