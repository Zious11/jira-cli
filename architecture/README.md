# Architecture Index — jr (jira-cli)

**Snapshot SHA:** `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
**Generated:** 2026-05-04
**Phase:** 1 Burst 3 (architecture refresh)
**Sources:** Pass 1 R1+R2, Pass 8 synthesis, L3 PRD, 6 existing ADRs

---

## Deployment Topology

`jr` is a **single-service, single static binary**. No daemon, no helper processes, no IPC, no managed service. State lives in three local files (config.toml, per-profile cache, OS keychain). The only network egress is outbound HTTPS to Atlassian APIs.

---

## Document Map

| File | Contents |
|------|----------|
| [system-overview.md](system-overview.md) | 5-layer architecture, deployment topology, binary model, network egress map |
| [component-graph.md](component-graph.md) | Mermaid module dependency graph (Pass 1 R1+R2 verified, acyclic DAG) |
| [state-machines.md](state-machines.md) | 5 definitive state machines with Mermaid diagrams, transition tables, source pins |
| [cross-cutting.md](cross-cutting.md) | Error handling, output discipline, rate limit, pagination, ADF, JQL, partial_match |
| [risk-register.md](risk-register.md) | 26 architectural risks (11 R1-NEW + 14 broad-pass + 1 R1-NEW reclassified to CRITICAL + 1 Pass-2 ADV-P2-004 addition; R-M3 merged into R-L11 at Pass 8) by severity |
| [adr-index.md](adr-index.md) | ADR-0001..0012 table with harmonization notes |
| [adr/](adr/) | New ADRs: 0007..0012 |
| [dtu-assessment.md](dtu-assessment.md) | DTU assessment (DTU_REQUIRED: false — pure local/OS execution) |
| [security-decisions/](security-decisions/) | SD-001 (PKCE), SD-002 (JR_AUTH_HEADER prod gating), SD-003 (--verbose PII redaction) |

**Related (in `.factory/`):**
| File | Contents |
|------|----------|
| [`../cicd-setup.md`](../cicd-setup.md) | CI/CD gap analysis — GAP-1 SHA pinning (cross-referenced by risk-register.md R-H6), GAP-2 deny.toml, GAP-3..5 |

---

## ADR Registry

| ADR | Title | Status |
|-----|-------|--------|
| ADR-0001 | Thin Client vs Generated API Client | Accepted |
| ADR-0002 | OAuth 2.0 with Embedded Client Secret | Superseded by ADR-0006 |
| ADR-0003 | reqwest with rustls-tls | Accepted |
| ADR-0004 | Per-Feature Specs, Not a Growing Master Spec | Accepted |
| ADR-0005 | GraphQL hostNames for Org Discovery | Accepted |
| ADR-0006 | Embedded `jr` OAuth App with Compile-Time XOR Obfuscation | Accepted |
| ADR-0007 | Multi-Profile Fields Bug Fix Strategy | Accepted |
| ADR-0008 | Asset Enrichment HashMap Key Correctness | Accepted |
| ADR-0009 | handle_open Instance URL Fix | Accepted |
| ADR-0010 | list_worklogs Pagination Fix | Accepted |
| ADR-0011 | Type-Level Profile Fence (Newtype) | Deferred |
| ADR-0012 | Module Shard Rule Codification | Accepted |

Source ADRs (0001–0006) live in `.reference/jira-cli/docs/adr/`. New ADRs (0007–0012) live in `.factory/architecture/adr/`.

---

## State Machine Index

| # | Name | File | Source pins |
|---|------|------|-------------|
| SM-1 | OAuth login | state-machines.md §1 | `api/auth.rs:382-690` |
| SM-2 | OAuth refresh (dual-path) | state-machines.md §2 | `cli/auth.rs::refresh_credentials`; `api/auth.rs:704` |
| SM-3 | Asset enrichment 3-pass | state-machines.md §3 | `cli/issue/list.rs:395-463` |
| SM-4 | Sprint-aware list dispatch | state-machines.md §4 | `cli/issue/list.rs`, `cli/sprint.rs` |
| SM-5 | Cache lifecycle | state-machines.md §5 | `cache.rs:16-150` |

---

## MUST-FIX Bug Register (L3 Forward-Looking)

| Bug ID | BC Anchor | NFR | Severity | ADR | Fix Scope |
|--------|-----------|-----|----------|-----|-----------|
| Multi-profile fields | BC-6.3.001 | NFR-R-D | CRITICAL | ADR-0007 | Add `Config::field_id()` accessor; update 14 sites |
| list_worklogs pagination | BC-X.5.002 | NFR-R-A | HIGH | ADR-0010 | Refactor to `paginate_offset` loop (~10 LOC) |
| handle_open URL | BC-3.4.001 | NFR-R-B | HIGH | ADR-0009 | One-line: `base_url()` → `instance_url()` |
| Asset HashMap key | BC-4.3.001 | NFR-R-E | HIGH | ADR-0008 | Change key type at 3 sites (~5 LOC) |

---

## Bounded Context to Module Map (L3 PRD § traceability)

| L3 BC | Module path(s) |
|-------|----------------|
| BC-1.* Auth & Identity | `cli/auth.rs`, `api/auth.rs`, `api/auth_embedded.rs` |
| BC-2.* Issue Read | `cli/issue/list.rs`, `cli/issue/view.rs`, `cli/issue/comments.rs`, `cli/issue/changelog.rs`, `cli/issue/format.rs` |
| BC-3.* Issue Write | `cli/issue/create.rs`, `cli/issue/workflow.rs`, `cli/issue/links.rs`, `cli/issue/helpers.rs`, `cli/issue/json_output.rs` |
| BC-4.* Assets & CMDB | `cli/issue/assets.rs`, `api/assets/linked.rs`, `api/assets/objects.rs`, `api/assets/workspace.rs`, `api/assets/schemas.rs`, `api/assets/tickets.rs` |
| BC-5.* Boards & Sprints | `cli/board.rs`, `cli/sprint.rs`, `api/jira/boards.rs`, `api/jira/sprints.rs` |
| BC-6.* Config & Cache | `config.rs`, `cache.rs` |
| BC-7.* Output Rendering | `output.rs`, `adf.rs`, `cli/issue/format.rs`, `cli/issue/json_output.rs` |
| BC-X.* Cross-cutting | `api/client.rs`, `api/pagination.rs`, `api/rate_limit.rs`, `error.rs`, `jql.rs`, `partial_match.rs`, `duration.rs`, `observability.rs`, `main.rs` |
