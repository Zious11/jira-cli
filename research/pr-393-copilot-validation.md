# PR #393 — Copilot Review Validation (S-392 cumulative spec-count guard)

**Date:** 2026-05-19
**Scope:** `scripts/check-bc-cumulative-counts.sh` + `tests/spec-count-fixtures/` (worktree `.worktrees/S-392`)
**Findings validated:** A/B, C, E (D confirmed already-fixed in `978250d`)

---

## Finding A/B — `set -euo pipefail` + failed `grep | awk` pipeline in Surface A / E assignment

### VERDICT: **Copilot CORRECT** — the prior adversary review was WRONG. FIX required.

### Exact code structure (verbatim, with line numbers)

Surface A — line 42, inside the `for f in ...` loop (lines 36–145), NOT a `local`, plain assignment:

```bash
42:  surface_a=$(grep '^total_bcs:' "$f" | awk '{print $2}')
43:  if [ -z "$surface_a" ]; then
44:    echo "ERROR: $basename_f: total_bcs frontmatter not found"
```

Surface E — line 150, top-level (not in a function, not `local`), plain assignment:

```bash
150:surface_e=$(grep '^total_bcs:' "$BC_INDEX" | awk '{print $2}')
151:if [ -z "$surface_e" ]; then
152:  echo "ERROR: BC-INDEX.md: total_bcs frontmatter not found"
```

Script header: line 2 is `set -euo pipefail` (errexit + nounset + pipefail all active).

### `|| true` audit — A and E are the ONLY two extractions missing it

| Surface | Line | Pipeline | `\|\| true`? |
|---------|------|----------|----------|
| A (per-file frontmatter) | 42 | `grep \| awk` | **NO** |
| B (Section header) | 57–58 | `grep \| sed` | YES (line 58) |
| C (sections: line) | 77–78 | `grep \| sed` | YES (line 78) |
| D (canonical table) | 96–98 | `awk \| grep \| awk` | YES (line 98) |
| Body prose | 129 | `sed \| grep \| sed` | YES (line 129) |
| E (BC-INDEX frontmatter) | 150 | `grep \| awk` | **NO** |
| F (**Sum** row) | 163 | `grep \| sed` | YES (line 163) |
| G (grand-total prose) | 176 | `grep \| sed` | YES (line 176) |

The prior fixes deliberately added `|| true` to B/C/D/F/G and the body-prose line. A and E were missed. This asymmetry is itself strong evidence the omission is a bug, not a deliberate choice — the author already knew the pattern was needed everywhere else.

### Why the prior adversary review is WRONG

The prior adversary claimed `var=$(failing-cmd)` in a "simple assignment is EXEMPT from `set -e`." This conflates two distinct Bash rules:

1. **Inside the substitution subshell:** `set -e` is unset within the `$(...)` subshell by default (non-POSIX, no `inherit_errexit`). This means commands *inside* `$(...)` don't abort the subshell early. TRUE — but irrelevant here.
2. **The exit status of the assignment statement itself:** For a simple command that is a variable assignment whose value comes from command substitution, the exit status of the assignment **is** the exit status of the command substitution (Bash manual, "Simple Command Expansion"). Under `set -e`, that non-zero status terminates the script. The assignment is NOT exempt.

The adversary's "exempt" intuition is the well-known case where the substitution is an *argument* to another command (e.g. `echo "$(false)"` — exit status is `echo`'s 0, masks the failure), OR the `local var=$(...)` case where `local`'s own 0 exit status masks the failure. Neither applies: Surface A/E are **plain `var=$(...)` assignments with the substitution as the entire RHS** — exactly the construct where the failure DOES propagate.

### Behavior trace (the failure path Copilot describes)

With `set -euo pipefail`, when `BC-INDEX.md` has no `^total_bcs:` line (the exact condition the `[ -z "$surface_e" ]` check exists to catch):

1. `grep '^total_bcs:' "$BC_INDEX"` → no match → exit status **1**.
2. `awk '{print $2}'` → receives empty stdin, runs fine → exit status **0**.
3. `set -o pipefail` → pipeline exit status = rightmost non-zero = **1** (grep's).
4. `surface_e=$(...)` plain assignment → assignment exit status = substitution exit status = **1**.
5. `set -e` → simple command (the assignment) returned non-zero, not in a conditional, not negated, not followed by `||` → **script terminates immediately at line 150**.
6. Line 151's `if [ -z "$surface_e" ]; then echo "ERROR: ..."` is **never reached** — the intended diagnostic is unreachable. The script exits non-zero with NO message, instead of printing `ERROR: BC-INDEX.md: total_bcs frontmatter not found`.

Surface A (line 42) is identical, with the additional consequence that the `continue` on line 46 is also unreachable — the whole loop dies on the first malformed bc-*.md file.

### Bash semantics — sourcing

The simple-assignment-vs-`local` distinction is documented and confirmed across multiple independent sources (see Research Methods). The Bash manual is authoritative: the exit status of a variable assignment with command substitution is the exit status of the last command substitution; `local`/`declare`/`readonly`/`export` are themselves *commands* whose own (usually 0) exit status masks the substitution. The flagged sites use **neither** `local` nor argument-position substitution — they are the bare-assignment case where errexit fires.

**Empirical-equivalence note:** Bash tool access was unavailable in this environment, so a throwaway script could not be executed. The verdict instead rests on the Bash reference manual semantics cross-validated against four independent secondary sources, plus the internal evidence that the same codebase author already applied `|| true` to the 6 sibling extractions (B/C/D/F/G + body-prose) — i.e. the project itself already treats this exact pipeline construct as errexit-fatal. A confirming local run would be: `set -euo pipefail; v=$(grep zzz /etc/hostname | awk '{print}'); echo "reached: [$v]"` → script exits before printing `reached:`. The codebase's own B–G fixes are the de-facto empirical proof.

### FIX (recommended)

Append `|| true` to Surface A and Surface E pipelines, matching B/C/D/F/G:

```bash
# line 42
surface_a=$(grep '^total_bcs:' "$f" | awk '{print $2}' || true)
# line 150
surface_e=$(grep '^total_bcs:' "$BC_INDEX" | awk '{print $2}' || true)
```

After the fix, a missing `^total_bcs:` line yields `surface_a`/`surface_e` empty, the
`[ -z ... ]` guard fires, and the intended `ERROR:` message + `ERRORS=$((ERRORS+1))` path runs as designed.

> Note: `grep -m1 ... || true` style is fine; placing `|| true` at the end of the
> pipeline neutralises the pipeline's aggregate (pipefail-computed) exit status,
> which is the correct scope — it covers both the `grep`-no-match (1) and any
> `awk` failure (2). This matches the existing B/C/D/F/G pattern exactly.

---

## Finding C — `$BC_INDEX` / `$CANONICAL` not existence-checked

### VERDICT: **Copilot PARTIALLY correct** — defensible improvement, low priority. FIX (lightweight) recommended.

### Current guards

- Line 25: repo-root guard exists — `[ -d ".factory" ] || { echo "ERROR: Run from repo root ..."; exit 1; }`. This only proves a `.factory/` directory is present, NOT that `.factory/specs/prd/BC-INDEX.md` or `CANONICAL-COUNTS.md` exist.
- Line 37: `[ -f "$f" ] || continue` checks each `bc-*.md` glob match (a no-op safety for the glob, since the glob already only yields existing files).
- **No `-f` check exists for `$BC_INDEX` (line 28) or `$CANONICAL` (line 29).**

### Behavior if a file is missing

If `BC-INDEX.md` is absent, line 57's `grep "..." "$BC_INDEX"` fails with `grep: .../BC-INDEX.md: No such file or directory` (exit 2). That pipeline ends in `|| true` (line 58), so `set -e` does NOT fire — `surface_b` ends up empty, and the existing `[ -z "$surface_b" ]` check on line 59 prints `ERROR: <file>: no ## Section header found in BC-INDEX.md for this file` and increments `ERRORS`. The script still exits 1, but the message **misdiagnoses** the cause: it blames a missing Section header when the real problem is the entire index file is absent. The raw `grep: No such file` line also leaks to stderr once per loop iteration (noisy).

For `$CANONICAL` (Surface D, F, G) the same misdiagnosis applies.

So Copilot is right that the omission produces a confusing failure mode — but it is NOT a correctness hole: the guard still exits non-zero, it just gives a misleading message. This is why the verdict is PARTIALLY (not fully) correct: the suggested fix improves diagnostics, it does not fix a broken gate.

### FIX (recommended, lightweight)

Add two `-f` checks immediately after line 29, before the loop:

```bash
[ -f "$BC_INDEX" ]  || { echo "ERROR: $BC_INDEX not found — expected BC-INDEX.md in $FACTORY"; exit 1; }
[ -f "$CANONICAL" ] || { echo "ERROR: $CANONICAL not found — expected CANONICAL-COUNTS.md in $FACTORY"; exit 1; }
```

Rationale to FIX rather than SKIP: it is two lines, it converts a misleading multi-line
error storm into one clear actionable message, and it matches the script's own
established defensive style (the line-25 repo-root guard). Low risk, clear benefit.

---

## Finding E — fixture `definitional_count: 15` vs actual `#### BC-` heading count

### VERDICT: **Copilot CORRECT on the count claim** — fixtures ARE internally inconsistent. But it is a **cosmetic fixture-hygiene issue**: no guard reads `definitional_count`. SKIP-or-FIX (low priority); FIX recommended for fixture self-consistency.

### 1. Actual count in `known-good/bc-2-issue-read.md`

`grep '^#### BC-'` yields **17** heading lines (verified, with line numbers):

```
BC-2.1.001 (L25), BC-2.1.002 (L30),
BC-2.2.009 (L41),
BC-2.3.016 (L52), .017 (L57), .018 (L62), .019 (L67), .020 (L72),
BC-2.3.021..029 (L77,82,87,92,97,102,107,112,117) — 9 "individually-bodied extra" headings
```

Total = 2 + 1 + 5 + 9 = **17**. Frontmatter declares `definitional_count: 15` (line 5). **Mismatch: declared 15, actual 17.** Copilot's count is accurate.

(Note: `total_bcs: 20` on line 4 is a *different* number — the cumulative claim including range-collapsed BCs — and is internally consistent with the rest of the fixture. Only `definitional_count` is wrong.)

### 2. Other fixture trees — same 17 vs 15

`bc-drift-total/.factory/specs/prd/bc-2-issue-read.md` — identical: `definitional_count: 15` (line 5), 17 `#### BC-` headings (BC-2.3.021..029 present, same line numbers). The test-writer copied one file across trees as suspected; the `definitional_count: 15` / 17-heading inconsistency is replicated in every fixture tree that has a `bc-2-issue-read.md`. The `known-good` `CANONICAL-COUNTS.md` per-file definitional table (line 21) also says `15` and `Match? YES`, which is itself wrong (should be 17).

### 3. Does the DRIFT-002 guard read `definitional_count`?

**No.** `check-bc-cumulative-counts.sh` greps only `^total_bcs:` (Surfaces A, E), `## Section` headers, `  - FILENAME` sections lines, the CANONICAL per-file `total_bcs` table, the `**Sum**` row, the grand-total prose, and the body `behavioral contracts` preamble. The string `definitional_count` does not appear anywhere in the guard. The guard validates **cumulative `total_bcs`** only — never `definitional_count`, never raw `#### BC-` heading counts.

### 4. Does DRIFT-001 (`check-spec-counts.sh`) run against the fixtures?

**No.** `check-spec-counts.sh` uses `FACTORY=".factory/specs/prd"` resolved relative to CWD and loops `for f in "$FACTORY"/bc-*.md`. It does NOT descend into `tests/spec-count-fixtures/`. It would only see the fixture files if it were *invoked with CWD set inside a fixture tree* — which nothing does. The fixture harness (`run-tests.sh`) invokes ONLY `check-bc-cumulative-counts.sh` (DRIFT-002), never `check-spec-counts.sh` (DRIFT-001). So DRIFT-001's `definitional_count` vs `#### BC-` comparison never runs against these fixtures.

### Conclusion for E

The fixture IS internally inconsistent (`definitional_count: 15` and the CANONICAL definitional table's `15`/`YES` are both wrong — actual is 17). **But the inconsistency affects ZERO guard behavior:**
- DRIFT-002 (the guard these fixtures test) never reads `definitional_count`.
- DRIFT-001 (the only guard that *would* compare `definitional_count` to `#### BC-`) never runs against the fixture trees.

So this is purely a fixture-hygiene / cosmetic defect. It is NOT a test-correctness bug — every fixture still exercises the DRIFT-002 surfaces exactly as the README documents, and the 7 pass/fail expectations are unaffected.

### Recommendation: **FIX (low priority, fixture hygiene)**

Make the fixtures self-consistent so a future maintainer (or a future DRIFT-001 fixture
extension) is not misled. Two options, either acceptable:

- **(a) Adjust the declared count to reality:** set `definitional_count: 17` in every
  fixture `bc-2-issue-read.md`, and update `known-good/CANONICAL-COUNTS.md` line 21 to
  `| bc-2-issue-read.md | 17 | 17 | YES |` (and the `**Total individually-bodied**`
  row from 23 to 25: 8 + 17).
- **(b) Trim the fixture body** to 15 `#### BC-` headings (delete BC-2.3.028 and .029)
  so the declared 15 becomes true.

Option (a) is lower-risk (no body edits, no chance of disturbing the prose-preamble /
`sed '/^## /q'` decoy logic the fixtures rely on). Recommend (a).

This is a SHOULD-FIX, not a MUST-FIX — it can ship in a follow-up if PR #393 is
time-boxed, since no guard or test outcome depends on it.

---

## Finding D — confirmed already fixed in `978250d`

Spot-checked both flagged locations in the worktree:

- `tests/spec-count-fixtures/run-tests.sh` — line 15 region now reads
  `PASS: Surface-D canonical table row drift exits 1` / line 19
  `Results: 7 passed, 0 failed`. The harness has 7 `run` calls (lines 73–79).
  No stale "4 fixtures" wording remains.
- `tests/spec-count-fixtures/README.md` — line 101–103 uses count-agnostic wording
  ("create a new `<name>/` directory alongside the existing fixture directories");
  the directory-layout block (lines 9–53) enumerates all 7 fixture trees. No stale
  "4 fixtures" claim remains.

Finding D requires no action — confirmed resolved.

---

## Summary table

| Finding | Verdict | Action |
|---------|---------|--------|
| A/B — Surface A/E missing `\|\| true` | **Copilot CORRECT** (prior adversary wrong) | **FIX** — add `\|\| true` to lines 42 and 150 |
| C — `$BC_INDEX`/`$CANONICAL` not `-f`-checked | **Copilot PARTIALLY correct** (diagnostic, not correctness) | **FIX** — add 2 `-f` guards after line 29 |
| E — fixture `definitional_count: 15` vs actual 17 | **Copilot CORRECT on count** (but cosmetic — no guard reads it) | **FIX (low priority)** — set `definitional_count: 17` + CANONICAL table |
| D — stale "4 fixtures" docs | Already fixed in `978250d` | None — confirmed |

**Highest-value item (A/B):** the prior adversary review was definitively wrong. A plain
`var=$(grep|awk)` assignment under `set -euo pipefail` DOES terminate the script when
grep finds no match, because the assignment statement's own exit status equals the
pipeline's pipefail-aggregated exit status. The `local`-masks-errexit exemption does not
apply because these are bare assignments, not `local` declarations. The fix is the same
`|| true` the codebase author already correctly applied to all 6 sibling extractions.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Read | 6 | Full guard script, `check-spec-counts.sh`, fixture `bc-2`/`CANONICAL`/README, run-tests.sh |
| Grep | 5 | `#### BC-` heading counts, `definitional_count`/`total_bcs` extraction, `\|\| true` audit |
| Glob | 2 | Enumerate fixture trees |
| WebSearch | 3 | Bash `set -e` + command-substitution-assignment vs `local` semantics |
| Bash (empirical run) | 0 | **UNAVAILABLE** — Bash tool denied for research agent; empirical run substituted by manual + internal-evidence proof (see Finding A/B note) |
| Training data | 1 area | Bash errexit / pipefail simple-command semantics — cross-validated against 4 web sources + Bash manual; flagged explicitly |

**Total external tool calls:** 16 (6 Read + 5 Grep + 2 Glob + 3 WebSearch)
**Training data reliance:** low — every Bash-semantics claim is corroborated by at least
two independent web sources AND by internal codebase evidence (the author's own B–G `|| true` fixes).
**Verification gap flagged:** the requested throwaway empirical Bash run could not be
executed (no shell access in research-agent tool profile). Verdict confidence remains
HIGH because (1) the Bash manual is unambiguous on assignment exit status, (2) four
independent sources agree on the `local`-vs-plain distinction, and (3) the codebase
itself already treats this construct as errexit-fatal in 6 of 8 sibling sites.

Sources:
- [Bash: errexit and Command Substitution — Pavel Saman (Medium)](https://samanpavel.medium.com/bash-errexit-and-command-substitution-32edaeaae36d)
- [ShellCheck SC2311 — set -e disabled inside command substitution](https://www.shellcheck.net/wiki/SC2311)
- [[Help-bash] errexit and local var=$(cmd) broken?](https://help-bash.gnu.narkive.com/dmmvsN9q/errexit-and-local-var-cmd-broken)
- [Bash: Error handling — FVue](https://fvue.nl/wiki/Bash:_Error_handling)
- [How to Handle Error Handling with set -e in Bash — OneUptime](https://oneuptime.com/blog/post/2026-01-24-bash-set-e-error-handling/view)
