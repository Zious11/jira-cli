# Demo Evidence Report — S-BC-CITATION-GUARD-1

**Story:** CITATION-GUARDS Story B: BC-body Trace/Source file::symbol citation guard (DEC-148)
**Story ID:** S-BC-CITATION-GUARD-1
**Branch:** ci/bc-citation-guard
**Recorded:** 2026-07-06

---

## Coverage Map

| AC | Description | Evidence | Success Path | Error Path |
|----|-------------|----------|-------------|------------|
| AC-001 | Guard passes GREEN on develop HEAD | VHS recording | `AC-001-canonical-green.gif/.webm` | — (failure path is AC-003) |
| AC-002 | Self-test fixture table (10/10) | VHS recording | `AC-002-self-test.gif/.webm` | — (all 10 fixtures run internally) |
| AC-003 | Error output format — dead symbol | VHS recording | — | `AC-003-dead-symbol-failure.gif/.webm` |
| AC-003b | Error output format — tier-ii .snap missing | VHS recording | — | `AC-003b-tier-ii-snap-missing.gif/.webm` |
| AC-004 | Scope restriction: Trace/Source lines only | Transcript (inline) | See §AC-004 below | — |
| AC-005 | Coverage floor: FLOOR=231 declaration | VHS recording | `AC-005-floor-declaration.gif/.webm` | Fixture G in AC-002 self-test |
| AC-006 | CI wiring, job name, CLAUDE.md | VHS recording | `AC-006-ci-wiring.gif/.webm` | — |
| AC-007 | CHANGELOG entry | Transcript (inline) | See §AC-007 below | — |

---

## VHS Recordings

### AC-001 — Canonical GREEN (CI topology replicated)

**Files:** `AC-001-canonical-green.gif`, `AC-001-canonical-green.webm`, `AC-001-canonical-green.tape`

**Demonstrated:**
1. `git worktree add --detach .factory origin/factory-artifacts` — mounts factory-artifacts branch at `.factory/`
2. `bash scripts/check-bc-citation-symbols.sh` — exits 0, emits `Check passed: 309 citations checked`
3. `git worktree remove --force .factory && git status --short` — mount removed, working tree clean

The recording mirrors the exact CI topology: the spec-guard job checks out develop (`src/` tree) then mounts `origin/factory-artifacts` at `.factory/` so both `src/` and `bc-*.md` files are simultaneously on-disk.

**Post-tape verification (confirmed clean):**
```
.factory removed - CLEAN
git status: CLEAN (nothing to commit, working tree clean)
```

---

### AC-002 — Self-test (10/10 fixtures)

**Files:** `AC-002-self-test.gif`, `AC-002-self-test.webm`, `AC-002-self-test.tape`

**Demonstrated:** `bash scripts/check-bc-citation-symbols.sh --self-test` exits 0 and emits:
```
All self-test fixtures passed (10/10)
```

The `--self-test` block runs all 10 hermetic fixtures (A–K) plus 5 post-fixture self-assertions. Fixtures exercise:
- Fixture A: dead-symbol (fn-grep NO-MATCH) → rc=1 expected
- Fixture B: dead-file + tier-ii .snap sub-probes → rc=1 / rc=0 as appropriate
- Fixture C: import-only false-green protection → rc=1 expected
- Fixture D: Source-field extraction → rc=1 expected
- Fixture E: two-pass extraction (§-form) → rc=0, "1 citations checked"
- Fixture F: success path + pub(crate) const + fn-with-paren strip → rc=0
- Fixture G: coverage-floor RED probe (1 citation + 100 citations, both < FLOOR=231) → rc=1
- Fixture I: `::tests` module-path ALIVE → rc=0
- Fixture J: `::tests` module-path DEAD (no permissive fallback) → rc=1
- Fixture K: standalone CamelCase type ALIVE → rc=0

---

### AC-003 — Error path: dead symbol

**Files:** `AC-003-dead-symbol-failure.gif`, `AC-003-dead-symbol-failure.webm`, `AC-003-dead-symbol-failure.tape`

**Demonstrated:** A bc stub with `**Trace**: src/cli/issue/edit.rs::handle_edit_nonexistent_fn_demo` triggers:
```
DEAD: handle_edit_nonexistent_fn_demo not found in src/cli/issue/edit.rs
1 stale citation(s) found in bc-*.md Trace/Source fields
exit:1
```

The file `src/cli/issue/edit.rs` exists (file-existence passes), but the function `handle_edit_nonexistent_fn_demo` is not defined in it (definition-anchored grep fails → DEAD). This is the exact DEC-148 class: a function that moved to a different file leaves a stale citation.

**Repo safety:** Only a temp directory (`$(mktemp -d)`) was used as `BC_DIR`. No repo files were modified. Verified with `git status --porcelain` → empty.

---

### AC-003b — Error path: tier-ii .snap file missing

**Files:** `AC-003b-tier-ii-snap-missing.gif`, `AC-003b-tier-ii-snap-missing.webm`, `AC-003b-tier-ii-snap-missing.tape`

**Demonstrated:** A bc stub with `**Trace**: src/cli/auth/tests/snapshots/jr__demo_nonexistent.snap` triggers:
```
DEAD: src/cli/auth/tests/snapshots/jr__demo_nonexistent.snap not found
1 stale citation(s) found in bc-*.md Trace/Source fields
exit:1
```

Non-`.rs` tokens (tier ii) receive file-existence check only — no symbol grep. The `.snap` extension passes the any-extension shape guard (`\.[a-zA-Z0-9]+$`). The file simply does not exist on disk → `DEAD: … not found`. Tier-ii tokens count toward N (total_citations) identically to `.rs` tokens.

---

### AC-005 — FLOOR=231 declaration + coverage floor guard

**Files:** `AC-005-floor-declaration.gif`, `AC-005-floor-declaration.webm`, `AC-005-floor-declaration.tape`

**Demonstrated:**
```bash
$ grep -n '^FLOOR=' scripts/check-bc-citation-symbols.sh
32:FLOOR=231       # floor(0.75 × N); N=309 (304 .rs + 5 .snap, measured on factory-artifacts 2b09313).

$ grep -n 'BC-CITE-COVERAGE-FLOOR\|expected >= ' scripts/check-bc-citation-symbols.sh
241:        echo "BC-CITE-COVERAGE-FLOOR: expected >= ${FLOOR} src/ citations, ..."

$ grep -n 'CANONICAL_MODE=1' scripts/check-bc-citation-symbols.sh | grep -v '#'
297:if [ "$self_test" = "0" ] && [ -z "${BC_DIR+x}" ]; then CANONICAL_MODE=1; fi
479:    CANONICAL_MODE=1   # toggle floor guard ON (script-scope variable)
```

`FLOOR=231` is a script-scope variable (NOT `local` inside `run_check`). The floor guard fires only in `CANONICAL_MODE=1` (automatic when no `--self-test` or `BC_DIR` override). Fixture G in the AC-002 self-test directly verifies the floor guard fires at rc=1 for 1 citation and 100 citations (both below FLOOR=231).

---

### AC-006 — CI wiring: spec-guard job + CLAUDE.md

**Files:** `AC-006-ci-wiring.gif`, `AC-006-ci-wiring.webm`, `AC-006-ci-wiring.tape`

**Demonstrated:**

Job name updated (segment `"citation checks"` inserted):
```
110:  spec-guard:
111:    name: Spec Guards (BC counts, numeric-count lint, citation checks, mutants policy scope)
```

Two new Guard 1 steps appended after Guard 2 (Story A's last step), preserving per-guard self-test-before-canonical sequencing:
```
133:      - name: check-cargo-mutants-policy-citations self-test (Guard 2)
134:        run: bash scripts/check-cargo-mutants-policy-citations.sh --self-test
135:      - name: check-cargo-mutants-policy-citations (Guard 2, DEC-150)
136:        run: bash scripts/check-cargo-mutants-policy-citations.sh
137:
138:      - name: check-bc-citation-symbols self-test (BC-CITE-001)
139:        run: bash scripts/check-bc-citation-symbols.sh --self-test
140:
141:      - name: check-bc-citation-symbols (BC-CITE-001)
142:        run: bash scripts/check-bc-citation-symbols.sh
```

`ci-gate.needs` is unchanged (per AC-006(c) and DEC-096/097 — `spec-guard` was already in `ci-gate.needs`).

**CLAUDE.md entry (AC-006(d)):** Line 355 in `CLAUDE.md`:
```
- `scripts/check-bc-citation-symbols.sh` — runs in spec-guard CI job; validates `src/` file
  and symbol citations in `**Trace**:`/`**Source**:` fields of `.factory/specs/prd/bc-*.md`
  bodies; exits 1 with `BC-CITE-001` offender list if any citation is stale. `--bc-dir`
  (designed-to-support) + `--src-root` (self-test only) + `--self-test` flags for offline
  verification. (DEC-148 Guard 1)
```

---

## Transcript Evidence

### AC-004 — Scope restriction: Trace/Source lines only; src/ paths only

**Command run:**

```bash
# bc stub with a src/ citation in PROSE (not a Trace/Source field)
# plus a valid Trace: line with a real function
cat > "$TMP/bc-scope-test.md" << 'EOF'
## Some BC body

In the prose of this BC, we mention `src/cli/issue/edit.rs::nonexistent_fn_in_prose_not_trace`.

This mention is in prose, not a **Trace**: or **Source**: field.

**Trace**: `src/cli/issue/edit.rs::handle_edit`
EOF

BC_DIR="$TMP" bash scripts/check-bc-citation-symbols.sh
```

**Output:**
```
Check passed: 1 citations checked
```

**Interpretation:** Only 1 citation was checked (the `handle_edit` on the `**Trace**:` line). The prose mention of `nonexistent_fn_in_prose_not_trace` was silently ignored — it is not on a `^\*\*(Trace|Source)\*\*:` anchored line. Exit 0.

The anchor `^\*\*(Trace|Source)\*\*:` (line 59 of the script) enforces scope mechanically, preventing false positives from prose, frontmatter, or other BC sections.

---

### AC-007 — CHANGELOG entry

**Location:** `CHANGELOG.md` `## [Unreleased]` → `### Added`

**Verified keywords present:**
- Topic prefix: `**CI: BC-body Trace/Source citation guard (Guard 1) (DEC-148):**`
- Script path: `scripts/check-bc-citation-symbols.sh`
- Error code: `BC-CITE-001`
- Field types: `**Trace**:`/`**Source**:`
- Files targeted: `bc-*.md`
- Capabilities: `definition-anchored symbol grep`, `coverage-floor guard`
- Origin: `DEC-148`

**Exact entry text:**
```
- **CI: BC-body Trace/Source citation guard (Guard 1) (DEC-148):** adds
  `scripts/check-bc-citation-symbols.sh` (BC-CITE-001; validates `src/` file and symbol
  citations in `**Trace**:`/`**Source**:` fields of all `bc-*.md` bodies; definition-anchored
  symbol grep; self-test fixtures; coverage-floor guard) as a step in the `spec-guard` CI job.
  Prevents the Seam-extraction citation-drift class (DEC-147/148/149).
  Calibration: measured N=309 citations (304 `.rs` + 5 `.snap`) on factory-artifacts @ 2b09313;
  FLOOR=231 = floor(0.75 × 309); non-.rs `src/` citations receive file-existence-only
  validation (tier ii).
```

All required keywords from AC-007 are present.

---

## Files Produced

```
docs/demo-evidence/S-BC-CITATION-GUARD-1/
├── evidence-report.md                         ← this file
├── AC-001-canonical-green.gif                 ← VHS recording
├── AC-001-canonical-green.webm                ← VHS recording
├── AC-001-canonical-green.tape                ← VHS script source
├── AC-002-self-test.gif                       ← VHS recording
├── AC-002-self-test.webm                      ← VHS recording
├── AC-002-self-test.tape                      ← VHS script source
├── AC-003-dead-symbol-failure.gif             ← VHS recording
├── AC-003-dead-symbol-failure.webm            ← VHS recording
├── AC-003-dead-symbol-failure.tape            ← VHS script source
├── AC-003b-tier-ii-snap-missing.gif           ← VHS recording
├── AC-003b-tier-ii-snap-missing.webm          ← VHS recording
├── AC-003b-tier-ii-snap-missing.tape          ← VHS script source
├── AC-005-floor-declaration.gif               ← VHS recording
├── AC-005-floor-declaration.webm              ← VHS recording
├── AC-005-floor-declaration.tape              ← VHS script source
├── AC-006-ci-wiring.gif                       ← VHS recording
├── AC-006-ci-wiring.webm                      ← VHS recording
├── AC-006-ci-wiring.tape                      ← VHS script source
├── setup-ac003-demo.sh                        ← helper (creates temp bc-dead.md for tape)
└── setup-ac003b-demo.sh                       ← helper (creates temp bc-snap.md for tape)
```

AC-004 and AC-007 evidence is captured as inline transcripts in this report (no dedicated recording — scope restriction and CHANGELOG verification are grep/cat operations whose output is fully captured above).
