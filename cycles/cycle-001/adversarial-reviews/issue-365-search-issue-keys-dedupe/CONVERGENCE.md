---
document_type: adversarial-review-rollup
issue: 365
cycle: 3-feature-search-issue-keys-dedupe-365
date: 2026-05-14
producer: state-manager
status: CONVERGED
passes_total: 11
counter_resets: 6
consecutive_clean: 3
consecutive_clean_passes: [9, 10, 11]
spec_final_version: 0.1.8
spec_initial_version: 0.1.0
---

# F1d Adversarial Spec Convergence — Issue #365

`search_issue_keys` in-function dedupe on all exit paths.  
Spec: `docs/specs/2026-05-14-search-issue-keys-dedupe.md`

## Trajectory

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

## Summary Statistics

- Total passes: 11
- Substantive (NOT-CLEAN) passes: 8 (P1–P8)
- CLEAN passes: 3 (P9, P10, P11)
- Counter resets: 6 (P1, P3, P4, P5, P6, P7, P8 each reset; note P2's
  counter-1/3 was invalidated by the NIT amendment → treated as reset at P3)
- BLOCKING findings total: 0
- CONCERN findings total: 12 (P1:4 + P3:1 + P4:2 + P5:1 + P6:2 + P7:1 + P8:1)
- NIT findings total: 24 (P1:2 + P2:2 + P3:3 + P4:2 + P5:1 + P6:5 + P7:4 + P8:2 + P10:5 + P11:1)
- Process-gap Observations: 2 (P4 NIT-1, P7 OBS-1)
- Spec version trajectory: 0.1.0 → 0.1.1 → 0.1.2 → 0.1.3 → 0.1.4 → 0.1.5 → 0.1.6 → 0.1.7 → 0.1.8

## Key Themes

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

## Process-Gap Flags (for state-manager)

| ID | Source | Description | Recommendation |
|----|--------|-------------|----------------|
| PG-365-1 | P4 NIT-1 | BC Trace field stale-test-count pattern is a recurring gap (parallel to DRIFT-001/DRIFT-004). BC body bodies cite test counts that drift as tests are added. Fixing requires a dedicated maintenance sweep. | Deferral entry in STATE.md Drift Items; no new GitHub issue warranted unless a sweep story is scheduled. |
| PG-365-2 | P7 OBS-1 | F1d adversary asked to validate research-agent citations without WebFetch access. Scope boundary unclear. | Codify: F1d adversary accepts research-agent output as verified ground truth; adversary's citation-completeness check is limited to "does the spec provide a citable reference?" not "is the reference correct?". No new GitHub issue; recommend adding a sentence to the F1d process description in FACTORY.md or the adversarial-review skill. |

## Convergence Declaration

3/3 consecutive CLEAN passes (P9 senior review, P10 security/resource, P11 full
traceability) at spec version v0.1.8 (unchanged since P8). No spec amendments
since P8.

**F1d FULLY CONVERGED. Spec v0.1.8 locked. Ready for F1-gate → F2/F3.**
