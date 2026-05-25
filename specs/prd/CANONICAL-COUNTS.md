---
document_type: canonical-counts
product: jr (jira-cli)
generated: "2026-05-04"
last_verified: "2026-05-25 (F2 delta issue #407; EC-3.4.017-14 added to BC-3.4.017; BC counts unchanged: 583 total, bc-3 103/74)"
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
| bc-2-issue-read.md | 51 | 51 | YES |
| bc-3-issue-write.md | 74 | 74 | YES |
| bc-4-assets-cmdb.md | 22 | 22 | YES |
| bc-5-boards-sprints.md | 17 | 17 | YES |
| bc-6-config-cache.md | 29 | 29 | YES |
| bc-7-output-render.md | 38 | 38 | YES |
| cross-cutting.md | 74 | 74 | YES |
| **Total individually-bodied** | **351** | — | — |

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
| bc-2-issue-read.md | 93 |
| bc-3-issue-write.md | 103 |
| bc-4-assets-cmdb.md | 32 |
| bc-5-boards-sprints.md | 35 |
| bc-6-config-cache.md | 39 |
| bc-7-output-render.md | 84 |
| cross-cutting.md | 140 |
| **Sum** | **583** |

### Grand total

**Canonical grand total: 583** (+4 BC-7.4.013-016 added 2026-05-08 via Fix-PR A `28b0f35`; +1 BC-2.6.050 added 2026-05-13 via issue #350; +1 BC-2.6.051 added 2026-05-14 via issue #365; +1 BC-3.4.009 added 2026-05-15 via issue #340 F2; +18 BC-3.8.001..010 + BC-X.12.001..008 added 2026-05-18 via issue #288 F2+F1d; +3 BC-3.8.011..013 added 2026-05-19 via issue #288 F1d + issue #383 F2; +4 BC-3.8.014..015 + BC-X.8.006..007 added 2026-05-19 via issue #384 F2; +2 BC-3.8.016..017 added 2026-05-20 via issue #385 F2; +2 BC-3.4.010..011 added 2026-05-20 via issue #388 F2; +3 BC-3.4.012..014 added 2026-05-21 via issue #398 F2; +3 BC-3.4.015..017 added 2026-05-22 via issue #396 F2)

_Note: BC-INDEX.md `total_bcs` header updated to 583 to match this file. CANONICAL-COUNTS.md carries the per-file sum as the primary source of truth. (+0 BC count change since last verified — issue #407 F2 added EC-3.4.017-14 only; the prior +3 was BC-3.4.015..017 from #396 F2 2026-05-22)_

Breakdown:
- 583 = sum of per-file `total_bcs` values (canonical; see per-file table above)
- 351 of 583 are individually-bodied (have a `#### BC-` heading)
- 232 are range-collapsed (counted in cumulative claim, no individual heading)
- BC-X.4.009 (ADV-P1-029) is a `#### BC-` heading in cross-cutting.md; it is
  included in cross-cutting's `total_bcs: 140` and in the **583 sum**.
  It does NOT add +1 beyond the 583.

_Historical note (archived; historical total was 566; current canonical is 583): Passes 10-13 involved a 541/542 count confusion around BC-X.4.009. All 542 claims were corrected to 541 at Pass 13. Subsequent additions (BC-7.4.013-016, BC-2.6.050-051, BC-3.4.009, BC-3.8.001-010, BC-X.12.001-008) brought the total to 566. See git history for the full audit trail._

### L2 domain-spec bc_count vs L3 total_bcs alignment (ADV-P17-003)

L2 frontmatter `bc_count` values are now aligned to match L3 `total_bcs` values.
bc_count in L2 represents the same cumulative claim (individually-bodied + range-collapsed).

| L2 File | L2 bc_count (after P17 fix) | L3 File | L3 total_bcs | Aligned? |
|---------|----------------------------|---------|--------------|----------|
| bc-01-auth-identity.md | 57 | bc-1-auth-identity.md | 57 | YES |
| bc-02-issue-read.md | 92 | bc-2-issue-read.md | 93 | PENDING (L2 bc_count not yet bumped; L3 +1 BC-2.6.051 added 2026-05-14) |
| bc-03-issue-write.md | 77 | bc-3-issue-write.md | 103 | PENDING (L2 bc_count not yet bumped; L3 +1 BC-3.4.009 2026-05-15; +10 BC-3.8.001-010 2026-05-18; +3 BC-3.8.011-013 2026-05-19; +2 BC-3.8.014-015 2026-05-19; +2 BC-3.8.016-017 2026-05-20; +2 BC-3.4.010-011 2026-05-20; +3 BC-3.4.012-014 2026-05-21; +3 BC-3.4.015-017 2026-05-22) |
| bc-04-assets-cmdb.md | 32 | bc-4-assets-cmdb.md | 32 | YES (was 44) |
| bc-05-boards-sprints.md | 35 | bc-5-boards-sprints.md | 35 | YES |
| bc-06-config-cache.md | 39 | bc-6-config-cache.md | 39 | YES (was 38) |
| bc-07-output-render.md | 84 | bc-7-output-render.md | 84 | YES (+4 BC-7.4.013-016 added 2026-05-08) |

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
- MEDIUM: 15 (NFR-R-C, NFR-R-F, NFR-S-A, NFR-S-C, NFR-O-A, NFR-O-B, NFR-O-D, NFR-O-F, NFR-O-J, NFR-O-L, NFR-O-M, NFR-O-O, NFR-O-S, NFR-O-W, NFR-P-NEW-1) — check nfr-catalog.md §Summary for exact split
- LOW: 19 (remainder)
- **Total: 41** (confirmed by grep count above)

Note: NFR-O-K was merged into NFR-S-D at adversary Pass 7 (no net change). NFR-S-F added at ADV-P3-007 (+1). NFR-S-E severity promoted LOW→HIGH at ADV-P2-004 (no net count change).

---

## Holdout Scenarios

**Canonical holdout total: 57**

Verification command:
```bash
grep -c '^### H-' .factory/specs/prd/holdout-scenarios.md
```

Expected: 57 (H-001..H-047 + H-NEW-MP-001 + H-NEW-VERBOSE-001 + H-NEW-VERBOSE-002 + H-NEW-AUTH-002 + H-NEW-JSM-RT-001 + H-NEW-JSM-RT-002 + H-NEW-JSM-RT-003 + H-NEW-JSM-RT-004 + H-NEW-JSM-RT-005 + H-NEW-JSM-RT-006 + H-NEW-JSM-RT-007)

_Note: holdout-scenarios.md frontmatter `total_holdouts: 57` counts all holdout entries including ones without `### H-` headings; the grep count of `### H-` headings is 57 because H-NEW-* holdouts use the extended format. The frontmatter count (57) is authoritative. (+2 since last verified: H-NEW-JSM-RT-006 + H-NEW-JSM-RT-007 added 2026-05-20 via issue #385 F2)_

---

## Risk Register

**Canonical risk total: 36**

Verification command:
```bash
grep -c '^| \*\*R-[CHML]' .factory/architecture/risk-register.md
```

Severity distribution (per risk-register.md §Risk Summary):
- CRITICAL: 1 (R-C1)
- HIGH: 7 (R-H1..R-H6 baseline + R-H288-1 from issue #288)
- MEDIUM: 11 (R-M0..R-M8 baseline + R-NEW-AR-1, R-NEW-AR-4 from S-3.03 + R-M288-1 from issue #288)
- LOW: 17 (R-L1..R-L13 baseline + R-NEW-AR-2, R-NEW-AR-3, R-NEW-AR-5 from S-3.03 + R-NEW-S307-1 from S-3.07)
- **Total: 36**

Note: R-M3 was merged into R-L11 at Pass 8 (net -1). R-L12 + R-L13 added at CV-003 gate prep. 5 auto-refresh risks added S-3.03 v2 (2 MEDIUM, 3 LOW). 1 search anti-loop risk added S-3.07 v2 (1 LOW). 2 risks added issue #288 (1 HIGH, 1 MEDIUM). risk-register.md §Risk Summary is authoritative.

Last reconciled: 2026-05-18 (post-#288 F2 delta; previous reconciliation pre-S-3.03)

---

## ADRs

**Canonical ADR count: 13** (ADR-0001..ADR-0013)

- ADR-0001..0006: source in `.reference/jira-cli/docs/adr/`
- ADR-0007..0013: in `.factory/architecture/adr/`
- ADR-0002: Superseded by ADR-0006 (still counted — superseded is a valid status)
- ADR-0013: PKCE deferral for OAuth 2.0 authorization code flow (Phase 1→2 gate, 2026-05-04)

Verification: count rows in adr-index.md Summary Table (both `[ADR-NNNN]` link rows and plain `ADR-NNNN` rows).

---

## Security Decisions

**Canonical SD count: 3** (SD-001, SD-002, SD-003)

Location: `.factory/architecture/security-decisions/`

---

## Cache Types

**7 distinct cache files** (per cache.rs):
1. team list
2. project meta
3. workspace ID (hybrid: reads env + cache)
4. CMDB fields
5. object-type attributes
6. resolutions
7. fields list (`fields.json` — `FieldsCache`; added issue #396 F2 for `--field` name resolution; best-effort writer; 7-day TTL)

All use 7-day TTL. Root path: `~/.cache/jr/v1/<profile>/`.

---

## Other Counts

| Claim | Canonical Value | Source |
|-------|----------------|--------|
| Bounded contexts | 7 (bc-1..bc-7) + 1 cross-cutting | README.md Document Map |
| HTTP method types | 11 (Pass 2 R1 verified) | Pass 2 deep R1 §inventory |
| API resource files | 18 (`api/jira/*`, `api/jsm/*`, `api/assets/*`) | adr-index.md ADR-0001 harmonization |
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
