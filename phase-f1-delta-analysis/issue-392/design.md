---
document_type: design
issue: 392
intent: enhancement
feature_type: infrastructure
scope_class: ci-tooling
created: "2026-05-19"
---

# Design — Extend spec-count CI guard for cumulative total_bcs + BC-INDEX headers

## Summary

`scripts/check-spec-counts.sh` guards `definitional_count` per bc-*.md, `total_nfrs`, and
`total_holdouts`. It does NOT guard the cumulative `total_bcs` frontmatter, the grand-total
sum, or any BC-INDEX.md section header counts. These same numbers are restated across 6+
locations with no machine check. A live drift exists today: `bc-2-issue-read.md` frontmatter
says `total_bcs: 93` but its body preamble (line 17) still reads "92 behavioral contracts".
This design adds a sibling script `check-bc-cumulative-counts.sh` to catch that class.

---

## Q1 — Extend vs sibling script

**Decision: add a sibling script `scripts/check-bc-cumulative-counts.sh`.**

Rationale:

- The existing script (62 lines) is single-purpose: per-file body-heading counts vs
  frontmatter. The new logic is qualitatively different: cross-file cumulative
  reconciliation, grand-total arithmetic, multi-file parse of BC-INDEX.md sections, and
  PENDING-row exclusion. Mixing them would push `check-spec-counts.sh` past ~150 lines and
  make failure triage ambiguous (a developer seeing "FAIL" would not know which concern
  tripped).
- Blast-radius isolation: if the new script has a bug (false positive), it blocks PRs but
  does not invalidate the definitional-count guard. A sibling can be hotpatched or
  temporarily disabled in CI without touching the existing guard.
- CI step naming maps cleanly: `check-spec-counts (DRIFT-001)` stays; the new step gets
  `check-bc-cumulative-counts (DRIFT-002)`.
- Convention is already established: `check-bc-no-numeric-test-counts.sh` is a sibling for
  PG-365-1. A third sibling is consistent.

---

## Q2 — Must-agree invariant set

### The four per-BC surfaces that must match

For each bounded context N (1..7 plus X=cross-cutting):

**Surface A — `bc-N.md` frontmatter `total_bcs:`**

Example verbatim (bc-2-issue-read.md line 4):
```
total_bcs: 93   # cumulative claim (incl. range-collapsed); ...
```
Parse rule: `grep '^total_bcs:' "$file" | awk '{print $2}'`
(same pattern the existing script uses for `definitional_count`).

**Surface B — `BC-INDEX.md` `## Section N:` header cumulative count**

Example verbatim (BC-INDEX.md line 50):
```
## Section 1: Auth & Identity (bc-1-auth-identity.md) — 57 BCs cumulative; 46 individually-bodied
```
Example verbatim (BC-INDEX.md line 524):
```
## Section X: Cross-Cutting Utilities (cross-cutting.md) — 138 BCs cumulative; 72 individually-bodied
```
Parse rule: `grep '^## Section [0-9X]:' BC-INDEX.md` then extract the integer before
`BCs cumulative`:
```bash
sed 's/.*— \([0-9]*\) BCs cumulative.*/\1/'
```

**Surface C — `BC-INDEX.md` frontmatter `sections:` lines**

Example verbatim (BC-INDEX.md lines 8-15):
```yaml
sections:
  - bc-1-auth-identity.md (57 BCs cumulative; 46 individually-bodied)
  - bc-2-issue-read.md (93 BCs cumulative; 51 individually-bodied)
  ...
  - cross-cutting.md (138 BCs cumulative; 72 individually-bodied)
```
Parse rule: for each bc-N file, grep the sections block for the matching filename and
extract the leading integer: `grep "bc-N-" BC-INDEX.md | sed 's/.* (\([0-9]*\) BCs.*/\1/'`

**Surface D — `CANONICAL-COUNTS.md` per-file `total_bcs` table**

Example verbatim (CANONICAL-COUNTS.md lines 41-51):
```
| bc-1-auth-identity.md | 57 |
| bc-2-issue-read.md | 93 |
...
| cross-cutting.md | 138 |
| **Sum** | **569** |
```
Parse rule: `grep "| $basename |" CANONICAL-COUNTS.md | awk -F'|' '{gsub(/ /,"",$3); print $3}'`
Skip the `**Sum**` bold row (it is the grand-total, handled separately).

### Grand-total surfaces that must equal `sum(A[1..8])`

**Surface E — `BC-INDEX.md` frontmatter `total_bcs:`**
Verbatim (BC-INDEX.md line 4, truncated):
```
total_bcs: 569  # cumulative claim ...
```
Parse rule: `grep '^total_bcs:' BC-INDEX.md | awk '{print $2}'`

**Surface F — `CANONICAL-COUNTS.md` Sum row**
Verbatim (CANONICAL-COUNTS.md line 51):
```
| **Sum** | **569** |
```
Parse rule: `grep '^\| \*\*Sum\*\*' CANONICAL-COUNTS.md | sed 's/.*\*\*\([0-9]*\)\*\*.*/\1/'`

**Surface G — `CANONICAL-COUNTS.md` grand-total prose line**
Verbatim (CANONICAL-COUNTS.md line 55):
```
**Canonical grand total: 569** (+4 BC-7.4.013-016 ...
```
Parse rule: `grep '^\*\*Canonical grand total:' CANONICAL-COUNTS.md | sed 's/.*: \([0-9]*\)\*\*.*/\1/'`

### Body-preamble prose counts — IN scope

The bc-2 93-vs-92 live drift confirms these drift. Include them.

Each bc-N.md has a prose line of the form:
```
N behavioral contracts across M subdomains: ...
```
Verbatim (bc-2-issue-read.md line 17):
```
92 behavioral contracts across 6 subdomains: JQL composition (2.1), ...
```
Parse rule: `grep -m1 'behavioral contracts' "$file" | sed 's/^\([0-9]*\) behavioral.*/\1/'`
Compare against Surface A (`total_bcs:` frontmatter) for the same file.

### Surfaces explicitly OUT of scope

- `BC-INDEX.md` Coverage Statistics table (lines 674-684) — duplicate of Section headers;
  parsing it adds complexity with no new signal (any drift there also shows in Surface B/E).
- `BC-INDEX.md` per-subdomain `### N.M` headers — e.g.,
  `### 1.2 Profile Lifecycle Management (6 BCs: BC-1.2.013..018)`. The subdomain-level
  count-vs-reality reconciliation is a separate concern (and requires counting rows per
  subdomain table — much more fragile). Defer to a follow-up.
- `README.md` grand total — human-facing prose; changes infrequently and is covered by
  existing maintainer discipline. Adding a grep against README is fragile (format can vary).

---

## Q3 — PENDING carve-out

The L2-vs-L3 alignment table in CANONICAL-COUNTS.md (lines 74-82) must be skipped
entirely. These rows represent documented, intentional divergence between L2 domain-spec
`bc_count` and L3 `total_bcs`. A naive check would false-positive on them.

Verbatim PENDING rows (CANONICAL-COUNTS.md lines 77-78):
```
| bc-02-issue-read.md | 92 | bc-2-issue-read.md | 93 | PENDING (L2 bc_count not yet bumped; L3 +1 BC-2.6.051 added 2026-05-14) |
| bc-03-issue-write.md | 77 | bc-3-issue-write.md | 91 | PENDING (L2 bc_count not yet bumped; L3 +1 BC-3.4.009 2026-05-15; +10 BC-3.8.001-010 2026-05-18; +3 BC-3.8.011-013 2026-05-19) |
```

**Skip rule:** when parsing CANONICAL-COUNTS.md, skip the entire "L2 domain-spec bc_count
vs L3 total_bcs alignment" table. The safest implementation: identify the table by its
section header and stop parsing at the next `---` separator.

```bash
# Extract only the "Per-file total_bcs" table (lines between the two section anchors)
# Use awk to parse between the known section header and "### Grand total"
awk '/^### Per-file total_bcs/,/^### Grand total/' CANONICAL-COUNTS.md \
  | grep '^| bc-\|^| cross-' \
  | grep -v '^\| \*\*'
```

This never touches the L2-alignment table, which lives under `### L2 domain-spec bc_count`.

---

## Q4 — Test strategy

**Recommendation: fixture-based shell test in `tests/spec-count-fixtures/`.**

Rationale: the repo uses Rust integration tests exclusively; there are no existing bash
tests, bats is not installed, and adding bats introduces a new CI dependency. A
fixture-based approach requires only bash and the script itself — zero new dependencies.

### Fixture layout

```
tests/spec-count-fixtures/
  known-good/
    .factory/specs/prd/
      bc-1-auth-identity.md    # minimal frontmatter + 2 '#### BC-' headings
      bc-2-issue-read.md       # frontmatter total_bcs matches body prose + BC headings
      BC-INDEX.md              # sections: + ## Section headers match body files
      CANONICAL-COUNTS.md      # per-file table + Sum + grand-total all consistent
  bc-drift-total/              # bc-N.md total_bcs: != BC-INDEX Section header
    .factory/specs/prd/ ...
  bc-drift-prose/              # body prose "N behavioral contracts" != total_bcs:
    .factory/specs/prd/ ...
  bc-drift-grandtotal/         # CANONICAL-COUNTS.md Sum != sum of per-file values
    .factory/specs/prd/ ...
```

### Test runner script

`tests/spec-count-fixtures/run-tests.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT="scripts/check-bc-cumulative-counts.sh"
PASS=0; FAIL=0
run() {
  local fixture=$1 expect=$2 label=$3
  result=$(cd "$fixture" && bash "$(pwd)/../../../../$SCRIPT" 2>&1; echo "EXIT:$?")
  exit_code=${result##*EXIT:}
  if [ "$exit_code" = "$expect" ]; then PASS=$((PASS+1)); echo "PASS: $label"
  else FAIL=$((FAIL+1)); echo "FAIL: $label (expected exit $expect, got $exit_code)"; fi
}
run tests/spec-count-fixtures/known-good       0 "known-good exits 0"
run tests/spec-count-fixtures/bc-drift-total   1 "total_bcs drift exits 1"
run tests/spec-count-fixtures/bc-drift-prose   1 "prose count drift exits 1"
run tests/spec-count-fixtures/bc-drift-grandtotal 1 "grand-total drift exits 1"
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
```

This runner is added as a CI step that executes BEFORE the real script guard, ensuring the
guard itself is verified on every PR.

---

## Q5 — CI wiring

Add one step to the existing `spec-guard` job, and add the fixture test step before it.
Exact YAML diff (insert after line 90 of `.github/workflows/ci.yml`):

```yaml
      - name: check-bc-cumulative-counts self-test (fixture suite)
        run: bash tests/spec-count-fixtures/run-tests.sh
      - name: check-bc-cumulative-counts (DRIFT-002)
        run: bash scripts/check-bc-cumulative-counts.sh
```

The job already has `timeout-minutes: 5` and the `.factory` worktree already checked out —
no additional checkout step needed. The fixture test runs first so a false-positive in the
guard fails at the self-test before blocking on real spec files.

Updated full job block for reference:
```yaml
  spec-guard:
    name: Spec Guards (BC counts + no numeric test counts)
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd  # v6
      - name: Fetch factory-artifacts branch (.factory/specs/prd/)
        run: |
          git fetch origin factory-artifacts
          git worktree add .factory origin/factory-artifacts
      - name: check-spec-counts (DRIFT-001)
        run: bash scripts/check-spec-counts.sh
      - name: check-bc-no-numeric-test-counts (PG-365-1)
        run: bash scripts/check-bc-no-numeric-test-counts.sh
      - name: check-bc-cumulative-counts self-test (fixture suite)
        run: bash tests/spec-count-fixtures/run-tests.sh
      - name: check-bc-cumulative-counts (DRIFT-002)
        run: bash scripts/check-bc-cumulative-counts.sh
```

---

## Q6 — Failure-message format

Each error line must identify: which surface, which file, expected value, actual value.

```
ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but BC-INDEX.md Section 2 header=92
ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but body prose="92 behavioral contracts"
ERROR: bc-2-issue-read.md: total_bcs frontmatter=93 but CANONICAL-COUNTS.md table row=92
ERROR: BC-INDEX.md frontmatter total_bcs=568 but computed sum of per-file total_bcs=569
ERROR: CANONICAL-COUNTS.md **Sum** row=568 but computed sum of per-file total_bcs=569
ERROR: CANONICAL-COUNTS.md grand-total prose=568 but computed sum of per-file total_bcs=569
FAIL: 2 spec cumulative count mismatch(es). Fix frontmatter, BC-INDEX.md, or CANONICAL-COUNTS.md before merging.
```

On success:
```
OK: all cumulative BC counts verified (569 total across 8 files).
```

---

## File list

Files the implementing story will CREATE:
- `scripts/check-bc-cumulative-counts.sh` — the new guard script
- `tests/spec-count-fixtures/run-tests.sh` — fixture test runner
- `tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-1-auth-identity.md`
- `tests/spec-count-fixtures/known-good/.factory/specs/prd/bc-2-issue-read.md`
- `tests/spec-count-fixtures/known-good/.factory/specs/prd/BC-INDEX.md`
- `tests/spec-count-fixtures/known-good/.factory/specs/prd/CANONICAL-COUNTS.md`
- `tests/spec-count-fixtures/bc-drift-total/.factory/specs/prd/` (same 4 files, drifted)
- `tests/spec-count-fixtures/bc-drift-prose/.factory/specs/prd/` (drifted prose)
- `tests/spec-count-fixtures/bc-drift-grandtotal/.factory/specs/prd/` (drifted grand total)

Files the implementing story will MODIFY:
- `.github/workflows/ci.yml` — add 2 steps to `spec-guard` job
- `CLAUDE.md` — add DRIFT-002 entry in the "Run scripts/check-spec-counts.sh after any
  edit" note (parallel to the existing DRIFT-001 mention)

Files the implementing story should ALSO fix (one-line drift, in scope):
- `.factory/specs/prd/bc-2-issue-read.md` line 17: change "92 behavioral contracts" to
  "93 behavioral contracts" (the live drift the new guard would catch)

---

## Risk assessment

**Primary risk: false positive blocks all PRs.**

If the script has a parse bug (e.g., grep extracts the wrong integer from a Section header
due to a format variation), every PR touching any file in the spec-guard job fails. The
mitigations:

1. **Fixture self-test runs first in CI.** A false positive that trips on real spec files
   but not fixtures indicates a fixture coverage gap — detectable because the fixture test
   passes and the real guard fails. Developer can diff fixture vs real files to isolate.
   A parse bug that also trips the fixture → both steps fail → immediately obvious the
   script itself is broken.
2. **Sibling isolation (from Q1).** Disabling one step in the YAML unfreezes all PRs
   without touching the working `check-spec-counts` guard.
3. **PENDING carve-out is narrow and anchor-based** (section header + stop at `---`). It
   does not rely on line numbers, which shift as CANONICAL-COUNTS.md grows. Robust to
   future additions to the L2-alignment table.

**Secondary risk: coverage gap in fixture set.**

Fixtures only cover 2 bc-*.md files. A parsing assumption that holds for bc-1/bc-2 might
break on a file with unusual frontmatter comment syntax. Mitigation: the real guard runs
against all 8 body files on every PR — any undetected fixture gap surfaces on first real
drift. Add more fixture variants if a false positive is reported.

**Not a risk: PENDING rows causing false positives.** The carve-out skips the entire L2
alignment table by section anchor; it does not rely on detecting the word "PENDING" in
individual rows. Even if new PENDING rows are added, the skip is structural.
