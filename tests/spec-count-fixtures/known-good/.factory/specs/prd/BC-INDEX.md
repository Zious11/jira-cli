---
context: bc-index
title: "BC Master Index"
total_bcs: 30   # cumulative claim — sum of per-file total_bcs (10+20=30)
last_updated: 2026-01-01
source_pass: 3
sections:
  - bc-1-auth-identity.md (10 BCs cumulative; 8 individually-bodied)
  - bc-2-issue-read.md (20 BCs cumulative; 15 individually-bodied)
---

# BC Master Index — fixture (known-good)

Master traceability index for the spec-count fixture suite.

---

## Preamble: Ranged vs. Anchored BCs

Two kinds of BC entries exist in this index:

1. **Individually-anchored** — has a `#### BC-S.SS.NNN:` heading in the body file.
2. **Range-collapsed** — single index row covers multiple BCs; counted in `total_bcs` but no individual heading.

---

## Section 1: Auth & Identity (bc-1-auth-identity.md) — 10 BCs cumulative; 8 individually-bodied

### 1.1 OAuth Flow (6 BCs: BC-1.1.001..006)

| L3 BC ID | Summary |
|---|---|
| BC-1.1.001 | Example contract one |
| BC-1.1.002 | Example contract two |

### 1.2 Profile Management (4 BCs: BC-1.2.007..010)

| L3 BC ID | Summary |
|---|---|
| BC-1.2.007 | Example profile contract one |
| BC-1.2.008 | Example profile contract two |

---

## Section 2: Issue Read (bc-2-issue-read.md) — 20 BCs cumulative; 15 individually-bodied

### 2.1 JQL Composition (8 BCs: BC-2.1.001..008)

| L3 BC ID | Summary |
|---|---|
| BC-2.1.001 | Example JQL contract one |
| BC-2.1.002 | Example JQL contract two |

### 2.2 Issue List (7 BCs: BC-2.2.009..015)

| L3 BC ID | Summary |
|---|---|
| BC-2.2.009 | Example list contract one |

### 2.3 Issue View (5 BCs: BC-2.3.016..020)

| L3 BC ID | Summary |
|---|---|
| BC-2.3.016 | Example view contract one |
