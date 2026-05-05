---
document_type: canonical-counts
product: jr (jira-cli)
generated: "2026-05-04"
last_verified: "Pass 17 fixes (ADV-P17-003 L2 bc_count sync)"
---

# Canonical Counts — jr (jira-cli) L3 PRD

This file is the single source of truth for all count claims across PRD and
architecture files. Every count is backed by a shell command that can be
re-run to verify. Disputes go here first.

---

## BC Counts

### Per-file definitional counts (actual `#### BC-` headings)

| File | Actual `#### BC-` count | Frontmatter `definitional_count` | Match? |
|------|------------------------|----------------------------------|--------|
| bc-1-auth-identity.md | 46 | 46 | YES |
| bc-2-issue-read.md | 49 | 49 | YES |
| bc-3-issue-write.md | 48 | 48 | YES |
| bc-4-assets-cmdb.md | 22 | 22 | YES |
| bc-5-boards-sprints.md | 17 | 17 | YES |
| bc-6-config-cache.md | 29 | 29 | YES |
| bc-7-output-render.md | 34 | 34 | YES |
| cross-cutting.md | 64 | 64 | YES |
| **Total individually-bodied** | **309** | — | — |

Verification command:
```bash
for f in .factory/specs/prd/bc-*.md .factory/specs/prd/cross-cutting.md; do
  echo -n "$(basename $f): "; grep -c '^#### BC-' "$f"
done
```

### Per-file total_bcs (cumulative claim: individually-bodied + range-collapsed)

| File | Frontmatter `total_bcs` |
|------|------------------------|
| bc-1-auth-identity.md | 57 |
| bc-2-issue-read.md | 91 |
| bc-3-issue-write.md | 77 |
| bc-4-assets-cmdb.md | 32 |
| bc-5-boards-sprints.md | 35 |
| bc-6-config-cache.md | 39 |
| bc-7-output-render.md | 80 |
| cross-cutting.md | 130 |
| **Sum** | **541** |

### Grand total

**Canonical grand total: 541**

Breakdown:
- 541 = sum of per-file `total_bcs` values
- 309 of 541 are individually-bodied (have a `#### BC-` heading)
- 232 are range-collapsed (counted in cumulative claim, no individual heading)
- BC-X.4.009 (ADV-P1-029) is a `#### BC-` heading in cross-cutting.md; it is
  included in cross-cutting's `total_bcs: 130` and in the 541 sum.
  It does NOT add +1 beyond the 541.

**History of the 542 confusion:**
- Pass 10 added BC-X.4.009 as a new `#### BC-` heading in cross-cutting.md
- cross-cutting.md `total_bcs` was correctly updated to 130 (including BC-X.4.009)
- The 541 sum already includes this
- BC-INDEX.md and README.md incorrectly added another +1 claiming 542
- Pass 13 fix: corrected all 542 claims to 541

### L2 domain-spec bc_count vs L3 total_bcs alignment (ADV-P17-003)

L2 frontmatter `bc_count` values are now aligned to match L3 `total_bcs` values.
bc_count in L2 represents the same cumulative claim (individually-bodied + range-collapsed).

| L2 File | L2 bc_count (after P17 fix) | L3 File | L3 total_bcs | Aligned? |
|---------|----------------------------|---------|--------------|----------|
| bc-01-auth-identity.md | 57 | bc-1-auth-identity.md | 57 | YES |
| bc-02-issue-read.md | 91 | bc-2-issue-read.md | 91 | YES |
| bc-03-issue-write.md | 77 | bc-3-issue-write.md | 77 | YES |
| bc-04-assets-cmdb.md | 32 | bc-4-assets-cmdb.md | 32 | YES (was 44) |
| bc-05-boards-sprints.md | 35 | bc-5-boards-sprints.md | 35 | YES |
| bc-06-config-cache.md | 39 | bc-6-config-cache.md | 39 | YES (was 38) |
| bc-07-output-render.md | 80 | bc-7-output-render.md | 80 | YES (was 126) |

Note: bc-01/02/03/05 were already aligned pre-Pass 17. bc-04/06/07 corrected at Pass 17.

---

## NFR Counts

**Canonical NFR total: 41**

Verification command:
```bash
grep -c '^| \*\*NFR-' .factory/specs/prd/nfr-catalog.md
```

Severity distribution per nfr-catalog.md routing table:
- CRITICAL: 1 (NFR-R-D)
- HIGH: 6 (NFR-R-A, NFR-R-B, NFR-R-E, NFR-S-B, NFR-S-E, NFR-S-F)
- MEDIUM: 15 (NFR-R-C, NFR-R-F, NFR-R-NEW-1, NFR-R-NEW-2, NFR-S-A, NFR-S-C, NFR-O-A, NFR-O-B, NFR-O-D, NFR-O-F, NFR-O-J, NFR-O-L, NFR-O-M, NFR-O-O, NFR-O-S, NFR-O-W, NFR-P-NEW-1) — check nfr-catalog.md §Summary for exact split
- LOW: 19 (remainder)
- **Total: 41** (confirmed by grep count above)

Note: NFR-O-K was merged into NFR-S-D at adversary Pass 7 (no net change). NFR-S-F added at ADV-P3-007 (+1). NFR-S-E severity promoted LOW→HIGH at ADV-P2-004 (no net count change).

---

## Holdout Scenarios

**Canonical holdout total: 48**

Verification command:
```bash
grep -c '^### H-' .factory/specs/prd/holdout-scenarios.md
```

Expected: 48 (H-001..H-047 + H-NEW-MP-001)

---

## Risk Register

**Canonical risk total: 26**

Verification command:
```bash
grep -c '^| \*\*R-[CHML]' .factory/architecture/risk-register.md
```

Severity distribution:
- CRITICAL: 1 (R-C1)
- HIGH: 6 (R-H1..R-H6)
- MEDIUM: 8 (R-M0..R-M5 + R-M7 + R-M8 — check risk-register.md for exact IDs; R-M3 merged into R-L11 at Pass 8)
- LOW: 11 (R-L1..R-L11)
- **Total: 26**

Note: R-M3 was merged into R-L11 at Pass 8 (net -1). R-H7 was added, then check for final state. risk-register.md header says "26 total" — use that as authority.

---

## ADRs

**Canonical ADR count: 12** (ADR-0001..ADR-0012)

- ADR-0001..0006: source in `.reference/jira-cli/docs/adr/`
- ADR-0007..0012: in `.factory/architecture/adr/`
- ADR-0002: Superseded by ADR-0006 (still counted — superseded is a valid status)

Verification: count rows in adr-index.md Summary Table (both `[ADR-NNNN]` link rows and plain `ADR-NNNN` rows).

---

## Security Decisions

**Canonical SD count: 3** (SD-001, SD-002, SD-003)

Location: `.factory/architecture/security-decisions/`

---

## Cache Types

**6 distinct cache files** (per cache.rs):
1. team list
2. project meta
3. workspace ID (hybrid: reads env + cache)
4. CMDB fields
5. object-type attributes
6. resolutions

All use 7-day TTL. Root path: `~/.cache/jr/v1/<profile>/`.

---

## Other Counts

| Claim | Canonical Value | Source |
|-------|----------------|--------|
| Bounded contexts | 7 (bc-1..bc-7) + 1 cross-cutting | README.md Document Map |
| HTTP method types | 11 (Pass 2 R1 verified) | Pass 2 deep R1 §inventory |
| API resource files | 17 (`api/jira/*`, `api/jsm/*`, `api/assets/*`) | adr-index.md ADR-0001 harmonization |
| list.rs LOC (post-split) | 1,083 | `wc -l src/cli/issue/list.rs` |
| auth.rs LOC | 1,397 | `wc -l src/api/auth.rs` |

---

## How to Update This File

When a pass adds or removes BCs, NFRs, holdouts, or risks:
1. Run the verification command for the affected category
2. Update the table in this file
3. Update the corresponding `total_bcs`/`definitional_count` frontmatter in the affected body file
4. Update BC-INDEX.md and README.md if BC grand total changes
5. Reference this file from any new count claim: "per CANONICAL-COUNTS.md"
