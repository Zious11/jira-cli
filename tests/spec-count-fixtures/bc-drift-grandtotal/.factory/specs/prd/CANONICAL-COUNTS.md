---
document_type: canonical-counts
product: jr-fixture (bc-drift-grandtotal)
generated: "2026-01-01"
last_verified: "2026-01-01 (fixture for spec-count-fixtures test suite)"
---

# Canonical Counts — fixture (bc-drift-grandtotal)

DRIFT INJECTED: **Sum** row and grand-total prose say 25 but sum of per-file total_bcs (10+20=30).
Expected guard behavior: exits 1 with ERROR about CANONICAL-COUNTS.md Sum row and grand-total prose mismatch.

---

## BC Counts

### Per-file definitional counts (actual `#### BC-` headings)

| File | Actual `#### BC-` count | Frontmatter `definitional_count` | Match? |
|------|------------------------|----------------------------------|--------|
| bc-1-auth-identity.md | 8 | 8 | YES |
| bc-2-issue-read.md | 17 | 17 | YES |
| **Total individually-bodied** | **25** | — | — |

### Per-file total_bcs (cumulative claim: individually-bodied + range-collapsed)

| File | Frontmatter `total_bcs` |
|------|------------------------|
| bc-1-auth-identity.md | 10 |
| bc-2-issue-read.md | 20 |
| **Sum** | **25** |

### Grand total

**Canonical grand total: 25** (DRIFTED — should be 30; fixture intentionally wrong to test guard detection)

### L2 domain-spec bc_count vs L3 total_bcs alignment

L2 frontmatter `bc_count` values vs L3 `total_bcs`. This section is skipped by
the cumulative-counts guard (PENDING rows document intentional, tracked divergence).

| L2 File | L2 bc_count | L3 File | L3 total_bcs | Aligned? |
|---------|------------|---------|--------------|----------|
| bc-01-auth-identity.md | 10 | bc-1-auth-identity.md | 10 | YES |
| bc-02-issue-read.md | 99 | bc-2-issue-read.md | 20 | PENDING (intentional L2-vs-L3 divergence for fixture; guard must skip this row) |

---

## NFR Counts

(Not relevant to this fixture — omitted.)
