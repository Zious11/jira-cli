---
decision_id: po-decision-bc-parameterization
issue: "#382"
date: 2026-05-19
decider: product-owner
adversary_pass: adversary-pass-01
findings_addressed: F-02, F-03
---

# PO Decision: BC-1.6.042 Parameterization Strategy

## Decision

**Option (a): Parameterize substring #3 in BC-1.6.042 in-place.**

The contract heading, ID, and position in BC-INDEX are unchanged. Only the
Behavior line is updated to reflect that the required scope name is
runtime-resolved from `JrError::InsufficientScope { required_scope:
Option<String> }` rather than hardcoded as `write:jira-work`.

## Rationale

1. **Semantic unity.** Both the `write:jira-work` path and the
   `write:servicedesk-request` path describe the same behavior: "display the
   scope that was missing." They are not distinct behaviors; they are
   instantiations of one parameterized behavior. A single BC with a
   parameterized field accurately captures this — two BCs would overstate the
   distinction.

2. **Backward compatibility.** The `None` branch of `Option<String>` falls back
   to the historical literal `write:jira-work`, so existing tests that assert
   the literal string remain green without modification. No test-file churn
   results from this decision.

3. **Minimal index propagation.** BC count is stable (57 cumulative in bc-1),
   BC-INDEX title and row are unchanged, CANONICAL-COUNTS.md is unchanged.
   Adversary finding F-02 (which was contingent on a count change from option
   (b) or (c)) is moot.

4. **Precedent in this codebase.** `JrError::NotAuthenticated { hint: String }`
   (`src/error.rs:5-6`) already uses a structured-hint-field pattern interpolated
   into thiserror Display, with multiple construction sites passing different hint
   text per call path. Parameterizing `InsufficientScope` with `Option<String>`
   follows the same template, with `Option<String>` (vs plain `String`) chosen to
   preserve None-fallback for test back-compat (T-2 at `tests/api_client.rs:100-144`
   passes unmodified under the None branch).

## Rejected Alternatives

### Option (b): Split into BC-1.6.042 (platform-write) + BC-1.6.047 (JSM-write)

Rejected. The two "behaviors" are not semantically distinct — both are
"show the missing scope name." Splitting would create two contracts with
identical structure differing only in a string constant, inflating BC count
for no analytical gain. BC-INDEX, CANONICAL-COUNTS.md, and any story body
that references BC-1.6.042 would all require updates. Adversary F-02
(index-count drift) would become load-bearing, requiring additional churn.

### Option (c): Add BC-1.6.047 as supplementary contract

Rejected. Would create two contracts with overlapping postconditions, requiring
synchronized maintenance for no testable behavior difference. Both contracts
would assert the same Display substring set; tests for either would satisfy the
other. Adds BC-INDEX/CANONICAL-COUNTS propagation cost without analytical gain.
Parameterization (option a) captures the single parameterized behavior with one BC.

## Files Modified

| File | Change |
|------|--------|
| `.factory/specs/prd/bc-1-auth-identity.md` (line ~472) | BC-1.6.042 Behavior line updated; Change note appended |
| `.factory/phase-f1-delta-analysis/issue-382/po-decision-bc-parameterization.md` | This file (created) |

## Files NOT Modified (confirmed in-scope exclusions)

- `BC-INDEX.md` — no BC count or ID change
- `CANONICAL-COUNTS.md` — no count change
- Any story body file — no F3 story exists yet (quick-dev route)
- Any source code or test file — PO scope only
