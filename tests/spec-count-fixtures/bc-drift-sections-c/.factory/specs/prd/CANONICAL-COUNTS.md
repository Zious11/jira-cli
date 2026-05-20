---
document_type: canonical-counts
product: jr-fixture (bc-drift-sections-c)
generated: "2026-01-01"
last_verified: "2026-01-01 (fixture for spec-count-fixtures test suite)"
---

# Canonical Counts — fixture (bc-drift-sections-c)

This fixture drifts ONLY Surface C: the BC-INDEX.md frontmatter sections: line for
bc-2-issue-read.md says "15 BCs cumulative" while all other surfaces agree on 20.

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
| **Sum** | **30** |

### Grand total

**Canonical grand total: 30** (fixture: 10 bc-1 + 20 bc-2 = 30 total)

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
