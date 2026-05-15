---
document_type: adversarial-review-rollup
issue: 365
cycle: 3-feature-search-issue-keys-dedupe-365
date: 2026-05-15
producer: state-manager
status: ROUND-2-CONVERGED
passes_total: 17
passes_round1: 11
passes_round2: 6
counter_resets_round1: 6
counter_resets_round2: 2
consecutive_clean: 3
consecutive_clean_passes_round1: [9, 10, 11]
consecutive_clean_passes_round2: [15, 16, 17]
spec_initial_version: 0.1.0
spec_round1_final_version: 0.1.8
spec_final_version: 0.1.12
---

# F1d Adversarial Spec Convergence — Issue #365

`search_issue_keys` in-function dedupe on all exit paths; extended in round 2
to include `search_issues` symmetric dedupe.
Spec: `docs/specs/2026-05-14-search-issue-keys-dedupe.md`

## Round 1 Trajectory (passes 1–11)

| Pass | Spec Ver. | BLOCKING | CONCERN | NIT | Obs | Verdict | Counter | Notes |
|------|-----------|----------|---------|-----|-----|---------|---------|-------|
| P1 | 0.1.0 | 0 | 4 | 2 | — | NOT-CLEAN | 0/3 | Extended scope to limit-truncation path; test naming; Vec::dedup correctness pin; search_issues asymmetry comment |
| P2 | 0.1.1 | 0 | 0 | 2 | — | CLEAN | 1/3 | **Invalidated** — NIT amendments to v0.1.2 triggered counter reset |
| P3 | 0.1.2 | 0 | 1 | 3 | — | NOT-CLEAN | 0/3 | Parent-spec 4 propagation gaps not covered in Doc/Spec Fallout |
| P4 | 0.1.3 | 0 | 2 | 2 | — | NOT-CLEAN | 0/3 | Apr 2025 × dedupe triple-collision; BC-2.6.050 JRACLOUD citation stale [process-gap NIT-1] |
| P5 | 0.1.4 | 0 | 1 | 1 | — | NOT-CLEAN | 0/3 | Risk 5 arm ambiguity; pagination.rs:77-79 source citation missing |
| P6 | 0.1.5 | 0 | 2 | 5 | — | NOT-CLEAN | 0/3 | rustdoc "Retain BOTH" ambiguous; limit-truncation test `more_available` vague |
| P7 | 0.1.6 | 0 | 1 | 4 | 1 [process-gap] | NOT-CLEAN | 0/3 | Citation URL for itertools refutation missing; OBS-1 adversary-WebFetch scope |
| P8 | 0.1.7 | 0 | 1 | 2 | — | NOT-CLEAN | 0/3 | "Observation-1 (NIT-3)" orphaned label; limit-truncation normative wording |
| P9 | 0.1.8 | 0 | 0 | 0 | — | **CLEAN** | 1/3 | Senior-reviewer full lens; all axes clean |
| P10 | 0.1.8 | 0 | 0 | 5 | — | **CLEAN** | 2/3 | Security/race/resource/observability; 5 below-threshold NITs |
| P11 | 0.1.8 | 0 | 0 | 1 | — | **CLEAN** | **3/3** | Full BC/AC/test traceability matrix |

**CONVERGED at P11 — 3/3 consecutive CLEAN at spec v0.1.8.**

## Round 2 Trajectory (passes 12–17)

**Trigger:** User-approved scope expansion — extend dedupe to `search_issues` (symmetric treatment).
Spec bumped v0.1.8 → v0.1.9. Counter reset from 3/3 → 0/3.

| Pass | Spec Ver. | BLOCKING | CONCERN | NIT | Obs | Verdict | Counter | Notes |
|------|-----------|----------|---------|-----|-----|---------|---------|-------|
| P12 | 0.1.9 | 0 | 0 | 3 | — | **CLEAN** | 1/3 | Type accuracy NIT; out-of-scope carve-out NIT; test naming NIT |
| P13 | 0.1.9 | 0 | 6 | 0 | — | NOT-CLEAN | 0/3 | Caller list factual errors (sprint.rs wrong; queue.rs omitted); algorithmic cost-claim wrong; BC-2.6.050 mis-anchoring; BC count propagation; out-of-scope contradiction |
| P14 | 0.1.10 | 1 | 2 | 0 | — | NOT-CLEAN | 0/3 | BLOCKING: BC-2.6.051 exact edit instructions missing; search_issues test enumeration; JSM key field confirmation. Mid-pass: product-owner scope violation reverted by orchestrator. |
| P15 | 0.1.12 | 0 | 0 | 0 | — | **CLEAN** | 1/3 | Comprehensive pass; all P13/P14 resolutions verified |
| P16 | 0.1.12 | 0 | 0 | 2 | — | **CLEAN** | 2/3 | Security/resource lens; 2 below-threshold NITs (HashSet growth; order preservation) |
| P17 | 0.1.12 | 0 | 0 | 0 | — | **CLEAN** | **3/3** | Full BC/AC/test traceability matrix for expanded scope |

**ROUND-2 CONVERGED at P17 — 3/3 consecutive CLEAN at spec v0.1.12.**

## Summary Statistics — Round 1

- Total passes: 11 (P1–P11)
- Substantive (NOT-CLEAN) passes: 8 (P1–P8)
- CLEAN passes: 3 (P9, P10, P11)
- Counter resets: 6 (P1, P3, P4, P5, P6, P7, P8 each reset; note P2's
  counter-1/3 was invalidated by the NIT amendment → treated as reset at P3)
- BLOCKING findings total: 0
- CONCERN findings total: 12 (P1:4 + P3:1 + P4:2 + P5:1 + P6:2 + P7:1 + P8:1)
- NIT findings total: 24 (P1:2 + P2:2 + P3:3 + P4:2 + P5:1 + P6:5 + P7:4 + P8:2 + P10:5 + P11:1)
- Process-gap Observations: 2 (P4 NIT-1, P7 OBS-1)
- Spec version trajectory: 0.1.0 → 0.1.1 → 0.1.2 → 0.1.3 → 0.1.4 → 0.1.5 → 0.1.6 → 0.1.7 → 0.1.8

## Summary Statistics — Round 2

- Total passes: 6 (P12–P17)
- Substantive (NOT-CLEAN) passes: 2 (P13, P14)
- CLEAN passes: 4 (P12, P15, P16, P17) — note P12 was CLEAN but P13 found gaps it missed
- Counter resets: 2 (P13, P14 each reset)
- BLOCKING findings total: 1 (P14 BLOCKING-1: BC-2.6.051 exact edit instructions missing)
- CONCERN findings total: 8 (P13:6 + P14:2)
- NIT findings total: 5 (P12:3 + P16:2)
- Mid-pass event: product-owner scope violation (v0.1.11 direct BC file edits reverted by orchestrator; reframed in v0.1.12)
- Spec version trajectory: 0.1.9 → 0.1.10 → 0.1.11 → 0.1.12

## Combined Summary Statistics (Both Rounds)

- Total passes: 17
- Total NOT-CLEAN passes: 10
- Total CLEAN passes: 7
- Total counter resets: 8
- Spec version trajectory: 0.1.0 → 0.1.1 → 0.1.2 → 0.1.3 → 0.1.4 → 0.1.5 → 0.1.6 → 0.1.7 → 0.1.8 (R1 lock) → 0.1.9 → 0.1.10 → 0.1.11 → 0.1.12 (R2 lock)

## Key Themes — Round 1

### 1. Verbatim-quote discipline (recurring across P1–P8)

The single most frequent source of CONCERN-level findings was verbatim-quote
ambiguity: the spec would describe a required change in paraphrase rather than
quoting the exact replacement text. Examples:

- P1 NIT-1: `HashSet<&str>` vs `HashSet<String>` pseudocode without explanation.
- P6 CONCERN-1: "Retain BOTH disambiguation sentences" without quoting the sentences.
- P6 CONCERN-2: limit-truncation `more_available` assertion specified as "adjust based on..." rather than the exact value.
- P7 NIT-1: strike-through HTML `<s>` vs Markdown `~~` in verbatim replacement text.
- P8 NIT-2: "adjust assertions" leading sentence retained despite P6 correction.

**Lesson:** In Doc and Spec Fallout sections, all implementer instructions must
be in normative ("MUST") voice with exact verbatim replacement text. Paraphrase
or advisory voice ("adjust", "update") is insufficient when the instruction is
the authoritative source for implementation.

### 2. Parent-spec propagation gaps (P3)

The scope extension in P1 (limit-truncation path added) correctly extended the
implementation but the Doc and Spec Fallout section only addressed the
implementation files, not the four required updates to the PARENT spec
(`docs/specs/2026-05-13-search-issue-keys.md`). These updates are:

1. Close-out note on the deferred follow-up bullet.
2. Test #13 inventory entry rename + assertion-flip description.
3. Risks bullet resolution annotation.
4. Backwards Compatibility paragraph update.

This is a recurring pattern in this project: when a feature closes a deferred
follow-up from a prior spec, the F1d phase must explicitly audit the parent spec
for items that become stale. Codified in this spec as "F3 implementer instruction."

### 3. Apr 2025 detector × dedupe interaction (P4–P5)

The most complex correctness concern in this cycle: the `all_keys.len() > max`
check added for the Apr 2025 server-side regression (server returns >maxResults
rows + `isLast:true`) interacts with per-iteration dedupe in a narrow triple-
collision corner. Per-iteration dedupe can collapse the overshoot (when the extra
row is a drift-duplicate), silencing the Apr 2025 overshoot signal in the specific
`nextPageToken-absent + drift-duplicate-overshoot` corner.

Resolution: Risk 5 added with full analysis. Accepted trade-off (the triple-
collision is rarer than the bug Option A closes). The client relies on
`next_page_token.is_some()` at `src/api/pagination.rs:77-79`, not `isLast`
(which is not deserialized) — this source-code anchor was required to make the
risk analysis precise.

### 4. Citation accuracy and research-agent handoff (P7)

Pass 7 surfaced an `[process-gap]` Observation about the adversary being asked
to validate the refutation of a Perplexity claim (`itertools::unique()` is
consecutive-only) without WebFetch access. This is at the boundary of what the
F1d adversary can verify: the research-agent (F1) already ran the verification
(documented in `.factory/research/issue-365-design-validation.md` §Q1 §C.2),
but the spec's research-caveat block did not provide a citable URL for the
adversary to confirm.

Resolution: docs.rs URLs for `Vec::retain` and `HashSet::insert` added to
the References section. The process-gap observation stands: F1d adversary scope
is spec-text completeness, not re-verification of research-agent findings.

### 5. BC stale citation propagation (P4, tagged as process-gap)

The BC-2.6.050 body at `bc-2-issue-read.md:496` retained `JRACLOUD-94632` in two
places after PR #364 rebind. This was not caught in PR #364's review because BC
bodies are not in the primary review surface for a spec-citation PR. The F1d
adversary caught it at P4.

Resolution: Observation-1 added to the BC-2.6.050 fallout section, specifying
both occurrences must be updated and warning that a single sed-style substitution
would silently leave the second occurrence stale. The stale BC Trace field
(test-count drift) was separately noted as pre-existing tech debt (NIT-4, do not
fix in this PR).

## Key Themes — Round 2

### 1. Caller-list factual accuracy (P13, high-impact)

The most impactful finding in round 2: the caller list for `search_issues` was
wrong in two independent ways simultaneously. `sprint.rs` was listed as a
`search_issues` caller but actually uses a completely different function
(`get_sprint_issues`, a separate Agile REST endpoint). `queue.rs` was a real
caller that was completely omitted. These two errors would have sent an implementer
to the wrong file and missed the JSM path entirely.

Root cause: the scope expansion (adding `search_issues`) was written without
re-verifying the caller graph from source. The F1d adversary is the right check
for this because it reads specs as an implementer would — including verifying
that named source files actually contain the referenced functions.

### 2. BC count propagation is a CI-gate requirement, not a nice-to-have (P13–P14)

Adding BC-2.6.051 requires count propagation in two files (BC-INDEX.md and
bc-2-issue-read.md frontmatter). This is enforced by `scripts/check-spec-counts.sh`
which is a mandatory CI check per CLAUDE.md DRIFT-001. P13 identified the
propagation requirement in principle; P14 escalated it to BLOCKING because
the exact edit instructions were still missing. The lesson: whenever a new BC is
introduced, the F3 implementer instructions MUST include the exact count-field
edits, not just a note to "run the check script."

### 3. Semantic anchoring: new BCs for new functions (P13 CONCERN-4)

Appending `search_issues` dedupe behavior to BC-2.6.050 (which is titled and
scoped to `search_issue_keys`) violates the one-contract-one-function principle.
The adversary caught this as a BC-anchoring error. Resolution: BC-2.6.051 as a
separate new BC, framed as an F3 deliverable. This enforces the rule that BC IDs
should be navigable by function name without cross-function contamination.

### 4. Factory role boundary: product-owner must not write BC files during F1d (P14 mid-pass)

During the P14 amendment cycle, the product-owner agent directly edited BC
catalog files (BC-INDEX.md, bc-2-issue-read.md) to pre-write BC-2.6.051. This
violates the F1d phase scope — F1d belongs to the adversary and state-manager;
BC file writes during F1d are unauthorized. The orchestrator reverted via
`git restore` and reframed the product-owner's contribution as forward-looking
F3 instructions in v0.1.12. This pattern is now documented as an anti-pattern:
product-owners should frame their BC additions as implementer instructions, not
direct writes, when the factory is in F1d phase.

## Process-Gap Flags (for state-manager)

| ID | Source | Description | Recommendation |
|----|--------|-------------|----------------|
| PG-365-1 | P4 NIT-1 | BC Trace field stale-test-count pattern is a recurring gap (parallel to DRIFT-001/DRIFT-004). BC body bodies cite test counts that drift as tests are added. Fixing requires a dedicated maintenance sweep. | Deferral entry in STATE.md Drift Items; no new GitHub issue warranted unless a sweep story is scheduled. |
| PG-365-2 | P7 OBS-1 | F1d adversary asked to validate research-agent citations without WebFetch access. Scope boundary unclear. | Codify: F1d adversary accepts research-agent output as verified ground truth; adversary's citation-completeness check is limited to "does the spec provide a citable reference?" not "is the reference correct?". No new GitHub issue; recommend adding a sentence to the F1d process description in FACTORY.md or the adversarial-review skill. |

## Round 1 Convergence Declaration

3/3 consecutive CLEAN passes (P9 senior review, P10 security/resource, P11 full
traceability) at spec version v0.1.8 (unchanged since P8). No spec amendments
since P8.

**F1d ROUND-1 FULLY CONVERGED. Spec v0.1.8 locked.**

## Round 2 Convergence Declaration

Round 2 was triggered by user-approved scope expansion: extend dedupe to
`search_issues` (symmetric treatment). Spec bumped v0.1.8 → v0.1.9. 6 passes
(P12–P17). 3/3 consecutive CLEAN passes (P15 comprehensive, P16 security/resource,
P17 full traceability) at spec version v0.1.12 (unchanged since P14). No spec
amendments since v0.1.12.

Key round-2 facts:
- Caller-list factual errors caught at P13 (sprint.rs wrong; queue.rs omitted)
- Algorithmic cost-claim corrected from O(p) to O(K×N) at P13
- BC-2.6.050 mis-anchoring caught at P13; BC-2.6.051 introduced as F3 deliverable
- BC count propagation exact edits added at P14 (BLOCKING escalation)
- Product-owner scope violation (direct BC file edits during F1d) reverted by orchestrator at P14
- Combined trajectory: P12 CLEAN (1/3) → P13 NOT-CLEAN (0/3) → P14 NOT-CLEAN (0/3) → P15 CLEAN (1/3) → P16 CLEAN (2/3) → P17 CLEAN (3/3)

**F1d ROUND-2 FULLY CONVERGED. Spec v0.1.12 locked. Ready for F1-gate (round 2,
human approval) → F2/F3.**
