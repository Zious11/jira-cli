---
context: bc-2
title: "Issue Read (list/view/comments/changelog)"
total_bcs: 20   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 17   # count of `#### BC-` headings in this file
last_updated: 2026-01-01
source_pass: 3
---

# BC-2 — Issue Read (list / view / comments / changelog)

20 behavioral contracts across 3 subdomains: JQL composition (2.1), Issue list behavior (2.2), Issue view (2.3).

---

## Subdomains

### 2.1 JQL Composition (8 BCs: BC-2.1.001..008)

| L3 BC ID | Summary |
|---|---|
| BC-2.1.001 | Example JQL contract one |
| BC-2.1.002 | Example JQL contract two |

#### BC-2.1.001: Example JQL contract one

**Precondition:** JQL string is valid.
**Postcondition:** query executes successfully.

#### BC-2.1.002: Example JQL contract two

**Precondition:** JQL string is empty.
**Postcondition:** default filter applied.

### 2.2 Issue List (7 BCs: BC-2.2.009..015)

| L3 BC ID | Summary |
|---|---|
| BC-2.2.009 | Example list contract one |

#### BC-2.2.009: Example list contract one

**Precondition:** project exists.
**Postcondition:** list returned.

### 2.3 Issue View (5 BCs: BC-2.3.016..020)

| L3 BC ID | Summary |
|---|---|
| BC-2.3.016 | Example view contract one |

#### BC-2.3.016: Example view contract one

**Precondition:** issue key valid.
**Postcondition:** issue details returned.

#### BC-2.3.017: Example view contract two

**Precondition:** issue key invalid.
**Postcondition:** error returned.

#### BC-2.3.018: Example view contract three

**Precondition:** issue has no comments.
**Postcondition:** empty comment list returned.

#### BC-2.3.019: Example view contract four

**Precondition:** issue has attachments.
**Postcondition:** attachments listed.

#### BC-2.3.020: Example view contract five

**Precondition:** issue has labels.
**Postcondition:** labels displayed.

#### BC-2.3.021: Example individually-bodied extra A

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.022: Example individually-bodied extra B

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.023: Example individually-bodied extra C

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.024: Example individually-bodied extra D

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.025: Example individually-bodied extra E

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.026: Example individually-bodied extra F

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.027: Example individually-bodied extra G

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.028: Example individually-bodied extra H

**Precondition:** N/A.
**Postcondition:** N/A.

#### BC-2.3.029: Example individually-bodied extra I

**Precondition:** N/A.
**Postcondition:** N/A.

## Appendix: Coverage Notes

999 behavioral contracts covering some subsection of issue-read behavior at the
cross-cutting level — this line is a DECOY for the body-decoy-prose fixture.
The guard's `sed '/^## /q'` truncation excludes this line; only the preamble
(line 12: "20 behavioral contracts") is passed to grep. NOTE: because the
preamble is file-order-first and grep uses `-m1`, even a bare grep without the
sed truncation would short-circuit on line 12 and never reach this decoy at
line 124. The sed truncation is correct defensive code, but this file's
preamble-first layout means the decoy is structurally unreachable by -m1
regardless of whether sed is present.
