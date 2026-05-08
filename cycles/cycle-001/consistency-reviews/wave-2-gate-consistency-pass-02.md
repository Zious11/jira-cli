---
document_type: consistency-review
wave: 2
pass: 02
producer: consistency-validator
date: 2026-05-08
diff_range_factory: 7fd17bf..28b0f35
diff_range_develop: ca22be0..6cb9994
verdict: DRIFT-FOUND
findings_count: 4
gate_close_recommendation: GATE-PASSES
---

# Consistency Review — Wave 2 Gate Pass 02

## Verdict: DRIFT-FOUND — 0 BLOCKING + 4 DRIFT (2 new, 2 carried from pass-01)

All 4 BLOCKING findings from pass-01 (WV2-CV-01, WV2-CV-02, WV2-CV-07 +
confirmed adversary findings WV2-ADV-01/ADV-02/ADV-03) are resolved. The
fix-PRs introduced one new propagation gap: Fix-PR A updated bc-7-output-render.md
frontmatter and BC-INDEX.md frontmatter for the +4 BC-7.4.013-016 additions, but
did not update three downstream count documents (BC-INDEX body section header,
BC-INDEX body summary table, and CANONICAL-COUNTS.md). WV2-SEC-01 (develop-side
fix, PR #310 at 6cb9994) is confirmed implemented but the security review doc
and STATE.md carry no RESOLVED notation. Two pass-01 DRIFT items persist
(WV2-CV-03, WV2-CV-05).

The gate may close: no blocking mis-anchors remain, no spec claims an unresolved
security finding, and the remaining drift items are low-impact count propagation
and cosmetic status mismatches.

---

## 1. BLOCKING Resolution Status

| Pass-01 ID | Description | Pass-02 Status | Verification |
|------------|-------------|----------------|--------------|
| WV2-CV-01 | BC-X.5.005 H1 heading named deprecated 3-arg calculator | RESOLVED | `.factory/specs/prd/cross-cutting.md:316` — H1 now reads `parse_duration_validate("1w2d3h30m")` as primary, with deprecated calculator described parenthetically. |
| WV2-CV-02 | WAVE-PLAN.md Wave 2 ACTIVE/draft, S-3.10 absent, Wave 3 = 9 stories | RESOLVED | `WAVE-PLAN.md:9` — `wave_2_status: COMPLETE`; `WAVE-PLAN.md:73` — `COMPLETE (2026-05-08)`; `WAVE-PLAN.md:77-83` — all 7 Wave 2 rows show `merged (#NNN)`; `WAVE-PLAN.md:104` — S-3.10 row present; `WAVE-PLAN.md:91` — Wave 3 header reads `(10 stories)`. |
| WV2-CV-07 | S-2.02 SHA `75289600` typo in STORY-INDEX and STATE.md | RESOLVED | `STORY-INDEX.md:108` — `merged (PR #304 / 7528960)`; `STATE.md:61` — `S-2.02 MERGED at 7528960`. Both corrected; no trailing zero. |
| WV2-ADV-01 (confirmed) | BC-7.3.004 mis-anchor in S-2.07 spec | RESOLVED (spec portion) | `S-2.07-json-output-policy-and-test-naming.md:11-12` — frontmatter now `BC-7.1.001` + `BC-7.4.013..016`; body ACs trace to BC-7.1.001 and BC-7.4.013. Develop-side test docstrings deferred as WV2-FIX-A-FOLLOWUP-01, confirmed in STATE.md:150. |
| WV2-ADV-02 (confirmed) = WV2-CV-08 | nfr-catalog.md body rows + Summary Table not updated | RESOLVED | NFR Summary Table has 11 RESOLVED rows: NFR-R-C, NFR-R-F, NFR-O-H, NFR-O-L, NFR-O-M, NFR-O-O, NFR-O-R, NFR-O-V, NFR-O-F, NFR-O-J, NFR-O-W. Phase 3 routing summary at `nfr-catalog.md:190` correctly states `RESOLVED: 11`. |
| WV2-ADV-03 (confirmed) | BC-6.2.013 mis-anchor in S-2.06 spec | RESOLVED (spec portion) | `S-2.06-worklog-duration-and-cmdb-cache-tuple.md:12` — frontmatter now `BC-6.2.006`; body ACs trace to BC-6.2.006. Develop-side test function name rename deferred as WV2-FIX-A-FOLLOWUP-02, confirmed in STATE.md:151. |

---

## 2. Deferred Items — Verified as Intentional

| Item | Status in STATE.md | Develop-side confirmation |
|------|--------------------|--------------------------|
| WV2-FIX-A-FOLLOWUP-01: 11 auth test docstrings cite BC-7.3.004 | STATE.md:150 — DEFERRED, bundle into next develop touch | `tests/auth_output_json.rs:11-15,99,141,184,298` + `src/cli/auth.rs:2126,2156,2198,2210,2222,2234` — all still cite BC-7.3.004. Expected; deferred. |
| WV2-FIX-A-FOLLOWUP-02: 2 worklog test function names embed bc_6_2_013 | STATE.md:151 — DEFERRED, bundle into next develop touch | `tests/worklog_duration_holdouts.rs:467,524` — function names `test_s_2_06_ac_005_bc_6_2_013_...` and `test_s_2_06_ac_006_bc_6_2_013_...`. Expected; deferred. |

Both entries confirmed well-formed in STATE.md. No action required.

---

## 3. DEC-012 Verification

DEC-012 is present and well-formed at `STATE.md:94`. Content:

- Description: "BC-7.3.004 mis-anchor repair: Option A (4 new sub-BCs)"
- References: `.factory/research/wave-2-gate-decisions-research.md`
- Justification: References Google AIP-162, per-shape pin rationale, develop-side deferral
- Phase: "Phase 3 Wave 2 gate"
- Date: 2026-05-08
- Author: "human (final say) + research-agent"

The research document itself (`wave-2-gate-decisions-research.md`) exists on disk. DEC-012 is structurally complete.

---

## 4. BC-7.4.013-016 Well-Formedness Verification

| Check | Result |
|-------|--------|
| BC-INDEX.md rows for BC-7.4.013-016 | Present at `BC-INDEX.md:488-491` with Confidence=HIGH |
| BC-INDEX.md frontmatter `total_bcs: 545` | PASS — correctly reflects +4 |
| bc-7-output-render.md body sections for all 4 BCs | Present at lines 272, 290, 308, 326 with Precondition/Postcondition/Invariant/Trace |
| bc-7-output-render.md frontmatter `total_bcs: 84`, `definitional_count: 38` | PASS — correctly updated |
| bc-7-output-render.md actual `#### BC-` heading count | 38 (verified by grep) |
| No duplicate BC IDs | PASS — BC-7.4.013-016 are new IDs beyond the prior max BC-7.4.012 |
| S-2.07 story frontmatter lists BC-7.4.013-016 | PASS — `S-2.07:12` lists all four |

---

## 5. WV2-SEC-01 Fix Verification

The develop-side fix (PR #310 at `6cb9994`) is confirmed present:

- `src/duration.rs:5,11` — `pub(crate) const MAX_DURATION_INPUT_LEN: usize = 64`
- `src/duration.rs:22-25` — `parse_duration_validate` now checks `input.len() > MAX_DURATION_INPUT_LEN`
- `src/duration.rs:206-231` — 2 regression-pin tests (`test_parse_duration_validate_rejects_input_longer_than_max`, `test_parse_duration_validate_accepts_input_at_max_boundary`)

The fix is correct and complete. However:

- The security review document (`wave-2-gate-security-review-pass-01.md:52,226`) still shows WV2-SEC-01 as an open finding with the approval text saying "should be addressed as a follow-up hardening task before v1.0." It carries no RESOLVED notation.
- STATE.md has zero references to WV2-SEC-01, PR #310, or `6cb9994`.

This is a minor documentation gap (the finding was fixed by a dedicated PR but neither the security review doc nor STATE.md were updated to note the resolution). It does not block gate closure — the fix exists on develop, the security review already approved the wave with WV2-SEC-01 as the only non-blocking medium finding, and the finding is therefore satisfied. But the gap is noted below as a new DRIFT finding.

---

## 6. New Findings (introduced by fix-PRs)

### P2-CV-01 (new) — BC-INDEX body Section 7 header + summary table not updated to reflect +4 BCs

- Severity: DRIFT
- Category: count-mismatch
- Tag: `[novel]` — introduced by Fix-PR A

**Description**: Fix-PR A updated `bc-7-output-render.md` frontmatter (correctly:
`total_bcs: 84`, `definitional_count: 38`) and `BC-INDEX.md` frontmatter
(`total_bcs: 545`). However, three downstream text locations in BC-INDEX.md
were not updated:

1. `BC-INDEX.md:435` — section header reads "80 BCs cumulative; 34 individually-bodied"
   (should be "84 BCs cumulative; 38 individually-bodied")
2. `BC-INDEX.md:648` — summary table row `| 7: Output Rendering | 80 | 34 |`
   (should be `| 84 | 38 |`)
3. `BC-INDEX.md:650` — summary totals row `| **Total** | **541** | **309** |`
   (should be `| **545** | **313** |`)
4. `BC-INDEX.md:652` — note reads "Canonical total is **541**"
   (should be "Canonical total is **545**")

Actual `#### BC-` heading count verified by grep: bc-7-output-render.md = 38, total
across all files = 313.

**Evidence**:
- `BC-INDEX.md:4` — frontmatter `total_bcs: 545` (correct)
- `BC-INDEX.md:14` — sections list `bc-7-output-render.md (84 BCs cumulative; 38 individually-bodied)` (correct)
- `BC-INDEX.md:435` — body section header: `80 BCs cumulative; 34 individually-bodied` (stale)
- `BC-INDEX.md:648-650` — summary table: 80/34 row + 541/309 totals (stale)
- `BC-INDEX.md:652` — "Canonical total is 541" (stale)
- `bc-7-output-render.md:4-5` — `total_bcs: 84`, `definitional_count: 38` (correct source of truth)

**Impact**: A reader inspecting the BC-INDEX body section header or summary table
will see a count (541/309) that contradicts the frontmatter (545). The CANONICAL-COUNTS.md
companion document (see P2-CV-02) has the same gap. Low operational risk — the frontmatter
is the machine-readable source of truth — but creates confusion for anyone doing manual
counts or audits.

**Remediation**: Sweep BC-INDEX.md body: (a) section 7 header → "84 BCs cumulative;
38 individually-bodied"; (b) summary table row 7 → `84 | 38`; (c) totals row →
`545 | 313`; (d) note → "Canonical total is **545**". Assign to S-3.06 (spec
numeric-claim checker scope) or bundle into next Wave 3 doc-cleanup PR.

---

### P2-CV-02 (new) — CANONICAL-COUNTS.md not updated for BC-7.4.013-016 additions

- Severity: DRIFT
- Category: count-mismatch
- Tag: `[novel]` — introduced by Fix-PR A

**Description**: `CANONICAL-COUNTS.md` (generated 2026-05-04, last_verified
"Pass 17 fixes") was not updated when Fix-PR A added BC-7.4.013-016. The file
now contains stale counts at multiple locations:

1. Line 28: `bc-7-output-render.md | 34 | 34 | YES` — actual heading count is now 38;
   the `YES` match claim is now wrong (34 ≠ 38)
2. Line 30: `Total individually-bodied | 309` — should be 313
3. Line 49: `bc-7-output-render.md | 80` (total_bcs) — should be 84
4. Line 51: `Sum | 541` — should be 545
5. Line 55: `Canonical grand total: 541` — should be 545

**Evidence**:
- `CANONICAL-COUNTS.md:28` — `bc-7-output-render.md | 34 | 34 | YES` (stale)
- `CANONICAL-COUNTS.md:30` — Total `309` (stale)
- `CANONICAL-COUNTS.md:49` — `bc-7-output-render.md | 80` (stale)
- `CANONICAL-COUNTS.md:51,55` — sum/grand total `541` (stale)
- `bc-7-output-render.md:5` — `definitional_count: 38` (correct)
- Grep verification: 38 `#### BC-` headings in bc-7-output-render.md

**Impact**: CANONICAL-COUNTS.md exists specifically to arbitrate count disputes.
If a future validation queries it to check the 545 claim, it will find 541 and
raise a false conflict. Lower operational risk than P2-CV-01 (CANONICAL-COUNTS.md
has a note that it should be regenerated periodically) but the file's purpose
requires it to be current.

**Remediation**: Update CANONICAL-COUNTS.md lines 28/30/49/51/55 for the +4
BC-7.4.013-016 additions. Assign to S-3.06 or bundle into Wave 3 doc-cleanup.

---

### P2-CV-03 (new) — security review doc and STATE.md carry no RESOLVED notation for WV2-SEC-01 (PR #310)

- Severity: DRIFT
- Category: traceability
- Tag: `[novel]`

**Description**: The security review document `wave-2-gate-security-review-pass-01.md:226`
still reads the original approval text: WV2-SEC-01 "should be addressed as a
follow-up hardening task before v1.0." The fix (PR #310, develop SHA `6cb9994`,
`src/duration.rs +55/-0`) landed on develop and is confirmed complete. Neither
the security review doc nor STATE.md carry a RESOLVED marker or cross-reference
to PR #310.

All spec claims about WV2-SEC-01 in the research doc
(`wave-2-gate-decisions-research.md:198-256`) describe it as a future to-do, not
as resolved. This is consistent with the pre-fix state. After the fix landed, no
spec-side update was made.

**Evidence**:
- `wave-2-gate-security-review-pass-01.md:226` — "should be addressed as a follow-up hardening task before v1.0" (no RESOLVED notation)
- `STATE.md` — zero occurrences of `WV2-SEC-01`, `PR #310`, or `6cb9994`
- `src/duration.rs:5,11,22-25,206-231` — fix confirmed present on develop

**Impact**: A future reviewer reading the security review will believe WV2-SEC-01
is still open. When checking develop for open security findings before v1.0, the
gap will cause unnecessary investigation. LOW operational risk (the fix is verified
on develop; the security review approved the wave with WV2-SEC-01 as non-blocking).

**Remediation**: Add a resolution postscript to `wave-2-gate-security-review-pass-01.md`
noting the fix at PR #310 / 6cb9994. Add a WV2-SEC-01 entry to STATE.md drift
table: RESOLVED — 2026-05-08 — WV2-SEC-01 — fix(security) PR #310 at 6cb9994.
Assign to S-3.06 or next Wave 3 doc-cleanup.

---

## 7. Pass-01 DRIFT Findings — Status in Pass 02

| Pass-01 ID | Severity | Description | Pass-02 Status |
|------------|----------|-------------|----------------|
| WV2-CV-02 | DRIFT | WAVE-PLAN.md Wave 2 ACTIVE/draft, S-3.10 absent | RESOLVED — `WAVE-PLAN.md:9,73,77-83,104` all corrected |
| WV2-CV-03 | DRIFT | STORY-INDEX Wave 0/1 rows show `draft` (15 stories) | STILL-OPEN — `STORY-INDEX.md:42-74` all still show `draft` |
| WV2-CV-04 | DRIFT | STORY-INDEX S-2.07 NFR/BC column omitted BC anchors | RESOLVED — `STORY-INDEX.md:113` now shows BC-7.1.001, BC-7.4.013-016, BC-7.3.005 |
| WV2-CV-05 | DRIFT | STATE.md Phase 3 counter 23/31 is 1 over arithmetic (7+8+7=22) | STILL-OPEN — `STATE.md:61,77,179` all retain `23/31 (74%)` |
| WV2-CV-06 | DRIFT | WAVE-PLAN.md S-2.06 effort `medium` vs STORY-INDEX `small` | RESOLVED — `WAVE-PLAN.md:82` now shows `small` |
| WV2-CV-07 | DRIFT | S-2.02 SHA `75289600` typo | RESOLVED — `STORY-INDEX.md:108`, `STATE.md:61` both corrected |
| WV2-CV-08 | DRIFT | nfr-catalog.md 9 of 10 NFR body rows still show open routing | RESOLVED — all 11 NFR rows show RESOLVED in Summary Table |
| WV2-CV-09 | NIT | BC-X.5.005 BC-INDEX compound vs body H1 stale | RESOLVED (part of WV2-CV-01 fix) |
| WV2-CV-10 | NIT | WAVE-PLAN.md `wave_2_status: ACTIVE` not toggled | RESOLVED — `WAVE-PLAN.md:9` now `wave_2_status: COMPLETE` |
| WV2-CV-11 | NIT | H-018 BC field has `(post-S-2.06 v2.0.0)` annotation | STILL-OPEN — `holdout-scenarios.md:195` unchanged; not in scope of fix-PRs |
| WV2-CV-12 | NIT | STATE.md S-0.05-F2 shows `TO_VERIFY` without resolution target | STILL-OPEN — `STATE.md:130` unchanged |

---

## 8. All Findings Summary

| ID | Severity | Category | Tag | Description |
|----|----------|----------|-----|-------------|
| P2-CV-01 | DRIFT | count-mismatch | [novel] [fix-pr-introduced] | BC-INDEX body section 7 header + summary table still say 80/34/541 after Fix-PR A added 4 BCs |
| P2-CV-02 | DRIFT | count-mismatch | [novel] [fix-pr-introduced] | CANONICAL-COUNTS.md still says bc-7: 80/34 and grand total 541; should be 84/38 and 545 |
| P2-CV-03 | DRIFT | traceability | [novel] [fix-pr-introduced] | security review + STATE.md carry no RESOLVED notation for WV2-SEC-01 after PR #310 merged |
| WV2-CV-03 | DRIFT | count-mismatch | [carried from pass-01] | STORY-INDEX Wave 0/1 rows (15 stories) still show `draft`; fix-PRs did not address this |
| WV2-CV-05 | DRIFT (NIT) | count-mismatch | [carried from pass-01] | STATE.md 23/31 count is 1 over (Wave 0+1+2 = 22); fix-PRs did not address this |
| WV2-CV-11 | NIT | naming-drift | [carried from pass-01] | H-018 BC field has non-standard `(post-S-2.06 v2.0.0)` annotation |
| WV2-CV-12 | NIT | traceability | [carried from pass-01] | STATE.md S-0.05-F2 drift item shows `TO_VERIFY` without resolution target |

---

## 9. Spot-Checks Performed

### BC-7.4.013-016 uniqueness

- Prior max BC-7.4 ID was BC-7.4.012 (verified by scanning `bc-7-output-render.md`)
- BC-7.4.013-016 are sequential continuation; no duplicate IDs

### NFR count verification (pass-02)

- Manually counted RESOLVED rows in nfr-catalog.md summary table: 11
- Rows: NFR-R-C, NFR-R-F, NFR-O-H, NFR-O-L, NFR-O-M, NFR-O-O, NFR-O-R, NFR-O-V, NFR-O-F, NFR-O-J, NFR-O-W
- Matches Phase 3 routing summary claim `RESOLVED: 11`
- PASS

### WV2-SEC-01 implementation completeness

- `MAX_DURATION_INPUT_LEN = 64` constant defined at `src/duration.rs:11`
- Guard at `src/duration.rs:22-25` returns `Err` if `input.len() > 64`
- Two regression-pin tests at lines 206-231 (rejects >64, accepts boundary)
- Test function names reference `WV2-SEC-01` in comment at line 203
- PASS (implementation complete; documentation gap noted as P2-CV-03)

### WAVE-PLAN.md Wave 3 parallel groups

- `WAVE-PLAN.md:106` now reads `{S-3.06, S-3.07, S-3.08, S-3.09, S-3.10} cleanup/doc (parallel)`
- S-3.10 present; dependency note at `WAVE-PLAN.md:107`
- PASS

### STATE.md deferred items integrity

- WV2-FIX-A-FOLLOWUP-01: `STATE.md:150` — well-formed with file:line evidence
- WV2-FIX-A-FOLLOWUP-02: `STATE.md:151` — well-formed with file:line evidence
- WV2-ADV-01: `STATE.md:152` — RESOLVED with spec + deferral annotation
- WV2-ADV-03: `STATE.md:153` — RESOLVED with spec + deferral annotation
- WV2-CV-01: `STATE.md:154` — RESOLVED
- WV2-CV-02: `STATE.md:155` — RESOLVED
- WV2-CV-07: `STATE.md:156` — RESOLVED
- DEC-012: `STATE.md:94` — well-formed with research doc reference
- PASS

### STORY-INDEX S-2.07 BC anchor update (WV2-CV-04)

- `STORY-INDEX.md:113` reads: `BC-7.1.001, BC-7.4.013, BC-7.4.014, BC-7.4.015, BC-7.4.016, BC-7.3.005, NFR-O-F, NFR-O-J, NFR-O-W`
- Matches S-2.07 frontmatter `bc_anchors`
- PASS

---

## 10. Verdict and Gate-Close Recommendation

### Verdict: DRIFT-FOUND

4 DRIFT + 3 NIT findings remain (7 total). 0 BLOCKING. The 4 DRIFT items are:

- **P2-CV-01/02**: BC count propagation gap in BC-INDEX body and CANONICAL-COUNTS.md
  (the source of truth frontmatter files are correct; only derived text is stale)
- **P2-CV-03**: Security review doc and STATE.md carry no RESOLVED notation for
  the WV2-SEC-01 fix; the fix itself is confirmed present on develop
- **WV2-CV-03**: STORY-INDEX Wave 0/1 rows still show `draft` (not a correctness
  gap; STATE.md and WAVE-PLAN.md correctly show these as merged)

None of the remaining drift items represent spec-correctness gaps, mis-anchors, or
broken traceability chains. All 4 BLOCKING pass-01 findings and their 3 adversary-confirmed
counterparts are resolved. The two deferred follow-up items (WV2-FIX-A-FOLLOWUP-01/02)
are properly recorded in STATE.md.

### Gate-Close Recommendation: GATE-PASSES

The Wave 2 integration gate may close. Remaining drift items are all count-propagation
or status-notation mismatches with no correctness or traceability impact. They
should be bundled into S-3.06 (spec numeric-claim checker) or a Wave 3 doc-cleanup
PR. Wave 3 may proceed.

---

_Report produced by consistency-validator (pass-02). diff_range_factory: 7fd17bf..28b0f35; diff_range_develop: ca22be0..6cb9994._
