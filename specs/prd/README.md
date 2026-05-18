---
document_type: prd-index
level: L3
product: jr (jira-cli)
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
product_version: "v0.5.0-dev.7"
generated: "2026-05-04"
phase: "Phase 1 Burst 2"
status: draft
traces_to: ".factory/specs/domain-spec/README.md"
source_passes: "Pass 3 broad + R1-R4 deepening (540 BCs); Pass 4 R4 NFR convergence (43 gaps); Pass 8 synthesis"
supplements:
  - nfr-catalog.md
  - error-taxonomy.md
  - edge-case-catalog.md
  - holdout-scenarios.md
---

# L3 PRD — jr (jira-cli)

VSDD-compliant L3 Product Requirements Document for `jr` (package `jr`, binary `jr`), a
Rust CLI for automating Atlassian Jira Cloud workflows. Produced from brownfield Phase 1
Burst 2 — importing 540 behavioral contracts from Pass 3, sharded into 7 bounded contexts
plus cross-cutting utilities.

**This is a FORWARD-LOOKING spec of CORRECT behavior.** The 4 MUST-FIX bugs are specified
as their FIXED behavior, not their current buggy behavior. Phase 3 implementation closes
the gap between current code and these contracts.

---

## Document Map

| File | Bounded Context | L3 BCs | Pass 3 source |
|------|----------------|--------|--------------|
| [bc-1-auth-identity.md](bc-1-auth-identity.md) | Auth & Identity | BC-1.*.* (57) | BC-001..024 + BC-025..035 + BC-1140..1178 |
| [bc-2-issue-read.md](bc-2-issue-read.md) | Issue Read | BC-2.*.* (91) | BC-101..124 + BC-125..150 + BC-1036..1055 |
| [bc-3-issue-write.md](bc-3-issue-write.md) | Issue Write | BC-3.*.* (88) | BC-201..225 + BC-1056..1081 + BC-3.8.001..010 |
| [bc-4-assets-cmdb.md](bc-4-assets-cmdb.md) | Assets & CMDB | BC-4.*.* (32) | BC-301..315 + BC-316..324 + BC-1136..1137 |
| [bc-5-boards-sprints.md](bc-5-boards-sprints.md) | Boards & Sprints | BC-5.*.* (35) | BC-401..410 + BC-1138 |
| [bc-6-config-cache.md](bc-6-config-cache.md) | Configuration & Cache | BC-6.*.* (39) | BC-901..911 + BC-1001..1016 + BC-6.3.001 (NFR-R-D) + BC-6.2.015 (profile-fence) |
| [bc-7-output-render.md](bc-7-output-render.md) | Output Rendering | BC-7.*.* (80) | BC-1101..1118 + BC-1104..1118 (snapshots) + ADF (54) |
| [cross-cutting.md](cross-cutting.md) | Cross-cutting | BC-X.*.* (138) | BC-601..606 + BC-701..709 + BC-801..808 + BC-1082..1103 + BC-1201..1214 + BC-1401..1411 + Worklogs/Teams/Users/Projects + BC-X.12.001..008 |
| [nfr-catalog.md](nfr-catalog.md) | NFR Catalog | 41 NFRs | Pass 4 R4 + ADV-P3-007 (1C/6H/15M/19L); NFR-O-K merged into NFR-S-D per ADV-P7-002 |
| [error-taxonomy.md](error-taxonomy.md) | Error taxonomy | 11 variants | BC-1204 + exit code table |
| [edge-case-catalog.md](edge-case-catalog.md) | Edge cases | cross-cutting | Pass 3 §5 untested gaps |
| [holdout-scenarios.md](holdout-scenarios.md) | Holdout scenarios | 55 | H-001..H-047 + H-NEW-MP-001 + H-NEW-VERBOSE-001/002 + H-NEW-AUTH-002 + H-NEW-JSM-RT-001..005 |
| [BC-INDEX.md](BC-INDEX.md) | Master BC index | 566 | All BCs with traceability |

**Total BCs in PRD:** 566 (538 imported range-collapsed + 3 formalized: BC-6.3.001 from NFR-R-D draft + BC-6.2.015 profile-fence + BC-X.4.009 from ADV-P1-029; +4 BC-7.4.013-016; +2 BC-2.6.050-051; +1 BC-3.4.009; +18 BC-3.8.001..010 + BC-X.12.001..008)

---

## BC Numbering Scheme

```
BC-S.SS.NNN where:
  S   = Bounded context (1-7; X = cross-cutting)
  SS  = Subdomain within context (01-99)
  NNN = Sequence within subdomain (001-999)

Examples:
  BC-1.1.001 = Auth & Identity, OAuth subdomain, contract 001
  BC-6.3.001 = Config & Cache, Multi-profile-fields subdomain, contract 001 (NFR-R-D fix)
  BC-X.1.001 = Cross-cutting, HTTP client subdomain, contract 001
```

Traceability: every BC carries `**Trace**: Pass 3 BC-NNN` to preserve audit linkage.

---

## Ubiquitous Language Overlay

Terms redefined or clarified in L3 context vs L2 Domain Spec:

| Term | L3 Behavioral Meaning |
|------|----------------------|
| **Idempotent** | Command exits 0 with `changed: false` or `transitioned: false` JSON key when target state already reached. ZERO state-mutating HTTP calls fired. |
| **MUST-FIX** | A behavioral contract that describes the FIXED target behavior, not current buggy behavior. Implementation is gated on this contract turning green. |
| **Fail-closed** | On ambiguous input (single-substring match, multi-workspace asset, etc.), the system REFUSES the operation with exit 64 (UserError) and never guesses. Distinct from "fail-open" (accept and proceed). |
| **Actionable error** | Every error message MUST contain a next-step suggestion (CLAUDE.md convention). Pin: at least one of: `jr auth login`, `--jql`, `--resolution`, `jr issue resolutions`, `jr team list --refresh`, `check your connection`, `jr init` appears in stderr. |
| **Per-profile isolation** | Cache reads/writes and keychain lookups are namespaced to the active profile. Cross-profile leakage is a correctness bug in L3 terms. |
| **Canonical JQL** | The composed JQL after `strip_order_by`, paren-wrapping, project-scope, filter-clauses, and `ORDER BY` re-appending. |

---

## MUST-FIX Bug Register (L3 forward-looking contracts)

These four BCs describe the CORRECT (post-fix) behavior. Phase 3 must turn them GREEN.

| L3 BC ID | NFR ID | Severity | Short description | L3 File |
|----------|--------|----------|-------------------|---------|
| BC-6.3.001 | NFR-R-D | **CRITICAL** | Per-profile `story_points_field_id`/`team_field_id` survive `Config::save_global()` round-trip; all 14 hot-path read sites use `active_profile()` not `global.fields.*` | [bc-6-config-cache.md](bc-6-config-cache.md) |
| BC-X.5.002 | NFR-R-A | **HIGH** | `list_worklogs` paginates until all worklogs fetched; no silent truncation at first page | [cross-cutting.md](cross-cutting.md) |
| BC-3.4.001 | NFR-R-B | **HIGH** | `handle_open` composes `<instance_url>/browse/<key>` using `client.instance_url()` not `client.base_url()` | [bc-3-issue-write.md](bc-3-issue-write.md) |
| BC-4.3.001 | NFR-R-E | **HIGH** | Asset enrichment `resolved` HashMap is keyed by `(workspace_id, oid)` not `oid` alone; no cross-workspace overwrite | [bc-4-assets-cmdb.md](bc-4-assets-cmdb.md) |

---

## Supplement Index

| File | Consumer | Contents |
|------|----------|----------|
| [nfr-catalog.md](nfr-catalog.md) | Architect, Phase 3 | 41 NFR gaps (1C/6H/15M/19L); severity, recommendation, Phase 3 routing |
| [error-taxonomy.md](error-taxonomy.md) | Implementer, Test-writer | 11 JrError variants × exit codes; 7-level `extract_error_message` chain |
| [edge-case-catalog.md](edge-case-catalog.md) | Test-writer, Holdout-evaluator | Cross-cutting edge cases; untested behavior gaps from Pass 3 §5 |
| [holdout-scenarios.md](holdout-scenarios.md) | Holdout-evaluator | 55 holdout scenarios (H-001..H-047 + H-NEW-MP-001 + H-NEW-VERBOSE-001/002 + H-NEW-AUTH-002 + H-NEW-JSM-RT-001..005) |

---

## Competitive Differentiators (L3 Traceability)

| Differentiator | BC anchors |
|----------------|-----------|
| Multi-profile auth with per-profile credential isolation | BC-1.1.007, BC-1.2.013, BC-1.2.023-R, BC-6.1.*, BC-6.2.009, BC-6.2.010, BC-1.4.027 |
| Embedded OAuth app (no user setup required) | BC-1.3.019, BC-1.3.020, BC-1.3.021, BC-1.3.022, BC-1.5.031, BC-1.5.034 |
| Fully non-interactive mode (`--no-input`) for CI/AI agents | BC-7.1.003, BC-3.2.008, BC-X.7.004, BC-X.3.007 |
| Idempotent state mutations | BC-3.1.007 (move), BC-3.1.004 (assign), BC-3.5.001 (link) |
| Assets/CMDB inline in issue list | BC-4.1.001, BC-2.1.016 (--asset filter) |
| Actionable error messages with remediation hints | BC-X.3.007 (next-step convention, universal) |
| Multi-workspace asset enrichment (post-fix) | BC-4.3.001 (NFR-R-E fix) |
