# Spec Count Fixtures — check-bc-cumulative-counts.sh self-test

This directory contains a fixture-based test harness for `scripts/check-bc-cumulative-counts.sh`
(DRIFT-002 mitigation). The harness verifies that the guard correctly detects — and only
detects — the count-surface drifts it is designed to catch.

## Directory layout

```
tests/spec-count-fixtures/
  run-tests.sh                          — test harness (run from repo root)
  README.md                             — this file
  known-good/                           — all count surfaces consistent; guard exits 0
    .factory/specs/prd/
      bc-1-auth-identity.md             — total_bcs: 10, body prose "10 behavioral contracts"
      bc-2-issue-read.md                — total_bcs: 20, body prose "20 behavioral contracts"
      BC-INDEX.md                       — frontmatter total_bcs: 30; Section 1 → 10; Section 2 → 20
      CANONICAL-COUNTS.md               — per-file rows (10, 20), Sum 30, grand-total 30
                                          NOTE: L2-alignment table contains an intentional PENDING
                                          row (L2 bc_count=99 vs L3 total_bcs=20) to verify the
                                          carve-out: the guard must skip the L2-alignment section
                                          entirely and exit 0.
  bc-drift-total/                       — BC-INDEX Section 2 header says "15 BCs cumulative"
    .factory/specs/prd/                   but bc-2 frontmatter says total_bcs: 20 → guard exits 1
  bc-drift-prose/                       — bc-2 body preamble says "15 behavioral contracts"
    .factory/specs/prd/                   but frontmatter says total_bcs: 20 → guard exits 1
  bc-drift-grandtotal/                  — CANONICAL-COUNTS.md Sum row and grand-total prose say 25
    .factory/specs/prd/                   but sum of per-file values (10+20) = 30 → guard exits 1
  bc-drift-sections-c/                  — BC-INDEX.md frontmatter sections: line for bc-2 says
    .factory/specs/prd/                   "15 BCs cumulative" but bc-2 frontmatter total_bcs: 20
                                          (Surfaces A and B agree on 20; only Surface C drifted)
                                          → guard exits 1
  bc-drift-canonical-d/                 — CANONICAL-COUNTS.md per-file total_bcs table row for
    .factory/specs/prd/                   bc-2 says 15 but bc-2 frontmatter total_bcs: 20
                                          (Surfaces A, B, C all agree on 20; only Surface D drifted)
                                          → guard exits 1
  bc-drift-decoy-prose-ok/              — all count surfaces agree (bc-1: 10, bc-2: 20, total: 30),
    .factory/specs/prd/                   but bc-2-issue-read.md body contains a DECOY line after a
                                          `## ` heading: "999 behavioral contracts covering some
                                          subsection ...". The guard's `sed '/^## /q'` truncation
                                          excludes body lines, so it reads only the correct preamble
                                          (20) and exits 0.
                                          NOTE: because the correct preamble (line 12) is always
                                          file-order-first and `grep -m1` short-circuits on the
                                          first match, a bare `grep -m1 'behavioral contracts'`
                                          WITHOUT the `sed` truncation would also return 20 — the
                                          body decoy at line 124 is unreachable regardless. This
                                          fixture therefore confirms the intended exit-0 behavior on
                                          a decoy-containing file and documents the `sed` truncation
                                          as defensive code, but cannot, by construction, fail a
                                          `-m1`-based regression where the preamble-first file
                                          convention holds. → guard exits 0
```

## Fixture design principles

- **Minimal**: each fixture tree contains only 2 bc-*.md files (bc-1, bc-2), BC-INDEX.md,
  and CANONICAL-COUNTS.md. No real spec content — EXAMPLE/FAKE bodies only.
- **Hermetic**: no network access, no real repo dependency. Each fixture is a self-contained
  mini-tree. The guard script resolves `.factory/specs/prd/` relative to its working directory.
- **One drift per fixture**: each failing fixture introduces exactly ONE surface mismatch,
  making failure diagnosis unambiguous.
- **PENDING carve-out in known-good**: the `known-good` CANONICAL-COUNTS.md intentionally
  includes a PENDING row in the L2-alignment table with a wildly different value (99 vs 20).
  The guard must exit 0 because it skips that section entirely (AC-2 per S-392).

## How to run

From the repo root:

```bash
bash tests/spec-count-fixtures/run-tests.sh
```

Expected output when guard script is implemented (Green Gate):

```
PASS: known-good exits 0
PASS: total_bcs drift exits 1
PASS: prose count drift exits 1
PASS: grand-total drift exits 1
PASS: Surface-C sections: line drift exits 1
PASS: Surface-D canonical table row drift exits 1
PASS: body decoy prose ignored; guard reads preamble only exits 0

Results: 7 passed, 0 failed
```

Expected output before guard script exists (Red Gate):

```
ERROR: guard script not found at scripts/check-bc-cumulative-counts.sh
       This is the expected Red Gate state — the guard has not been implemented yet.
       Run the fixture tests again after scripts/check-bc-cumulative-counts.sh is created.

Results: 0 passed, 7 failed (guard absent)
```

## Adding new fixtures

When adding a new drift category, create a new `<name>/` directory alongside the existing
fixture directories with the same `.factory/specs/prd/` subpath, add a `run` call in
`run-tests.sh`, and document the drift type in this README.
