# ADR Index — jr (jira-cli)

**traces_to:** README.md
**Source ADRs (0001–0006):** `.reference/jira-cli/docs/adr/`
**New ADRs (0007–0012):** `.factory/architecture/adr/`

---

## ADR Summary Table

| ADR | Title | Status | Architecture Section |
|-----|-------|--------|---------------------|
| [ADR-0001](#adr-0001-thin-client) | Thin Client vs Generated API Client | **Accepted** | system-overview.md §L3+L4 |
| [ADR-0002](#adr-0002-oauth-superseded) | OAuth 2.0 with Embedded Client Secret | **Superseded** by ADR-0006 | — (historical only) |
| [ADR-0003](#adr-0003-reqwest-rustls) | reqwest with rustls-tls | **Accepted** | system-overview.md §deployment |
| [ADR-0004](#adr-0004-per-feature-specs) | Per-Feature Specs, Not a Growing Master Spec | **Accepted** | README.md §specs |
| [ADR-0005](#adr-0005-graphql-org-discovery) | GraphQL hostNames for Org Discovery | **Accepted** | system-overview.md §network-egress |
| [ADR-0006](#adr-0006-embedded-oauth) | Embedded `jr` OAuth App with XOR Obfuscation | **Accepted** | state-machines.md §SM-1 |
| [ADR-0007](adr/0007-multi-profile-fields-fix.md) | Multi-Profile Fields Bug Fix Strategy | **Accepted** | risk-register.md §R-C1 |
| [ADR-0008](adr/0008-asset-enrichment-key-correctness.md) | Asset Enrichment HashMap Key Correctness | **Accepted** | state-machines.md §SM-3 |
| [ADR-0009](adr/0009-handle-open-instance-url.md) | handle_open Instance URL Fix | **Accepted** | risk-register.md §R-H3 |
| [ADR-0010](adr/0010-list-worklogs-pagination.md) | list_worklogs Pagination Fix | **Accepted** | cross-cutting.md §4 |
| [ADR-0011](adr/0011-type-level-profile-fence.md) | Type-Level Profile Fence (Newtype) | **Deferred** | risk-register.md §R-L1 |
| [ADR-0012](adr/0012-shard-rule.md) | Module Shard Rule Codification | **Accepted** | risk-register.md §R-M5 |

---

## ADR-0001: Thin Client {#adr-0001-thin-client}

**Decision:** Wrap reqwest directly, hand-write API call functions. Reject OpenAPI-generated client and separate SDK crate.

**Harmonization with current architecture:**
- Reaffirmed. All 17 `impl JiraClient` resource files (`api/jira/*`, `api/jsm/*`, `api/assets/*`) follow this pattern.
- The product-namespaced directory structure (`api/jira/`, `api/jsm/`, `api/assets/`) is the architectural expression of this decision.
- Applies to both the validated and raw-passthrough HTTP paths.
- **No tension** with L3 PRD BCs.

---

## ADR-0002: OAuth Embedded Secret (Superseded) {#adr-0002-oauth-superseded}

**Status:** Superseded by ADR-0006.

**Chain:** ADR-0002 (accepted: embed secret) → intermediate reversal (BYO required) → ADR-0006 (re-introduce embedded with obfuscation + BYO escape hatch).

**Historical note:** The intermediate "BYO required" phase added the 20-minute Atlassian Developer Console registration barrier that ADR-0006 removes for official binaries.

---

## ADR-0003: reqwest with rustls-tls {#adr-0003-reqwest-rustls}

**Decision:** `reqwest` with `default-features = false`, `rustls` TLS backend.

**Harmonization with current architecture:**
- Reaffirmed. `reqwest 0.13` (rustls-tls, no native-tls) confirmed in Cargo.toml.
- Feature name changed from `rustls-tls` (reqwest 0.12) to `rustls` (0.13) — doc corrected.
- **NFR-S cross-reference:** `rustls` consistently enforces TLS 1.2+ and uses `webpki-roots` CA bundle. Corporate environments with custom CA certificates need `RUSTLS_NATIVE_CERTS=1`.
- No tension with L3 NFR catalog.

---

## ADR-0004: Per-Feature Specs {#adr-0004-per-feature-specs}

**Decision:** Per-feature specs in `docs/specs/`, not a growing master spec. v1 design spec is the architectural foundation.

**Harmonization with VSDD:**
- Reaffirmed. VSDD Phase 2 maps `docs/specs/` per-feature specs to individual stories.
- The L3 PRD (`.factory/specs/prd/`) is the VSDD equivalent of the "architectural foundation" role — it supersedes the v1 design spec for formal BCs.
- Tension (low): VSDD adds BC-level traceability that ADR-0004's token-economy rationale didn't anticipate. The resolution: `docs/specs/` files remain developer-facing; VSDD PRD files are the formal spec authority.

---

## ADR-0005: GraphQL hostNames for Org Discovery {#adr-0005-graphql-org-discovery}

**Decision:** Single `POST /gateway/api/graphql` call using the instance hostname returns both `cloudId` and `orgId` via `tenantContexts` query with `hostNames` parameter.

**Harmonization with current architecture:**
- Reaffirmed. `api/jira/teams.rs` implements this.
- Results cached in `config.toml` (`cloud_id`, `org_id`) after first discovery (during `jr init` or lazily on first `--team` usage).
- **No tension** with L3 PRD BCs.
- **Observed limitation:** `accessible_resources` (OAuth cloud site discovery at auth time) uses a different mechanism — `GET /oauth/token/accessible-resources`. These two discovery paths coexist without conflict (ADR-0005 is for team API access; OAuth discovery is for auth setup).

---

## ADR-0006: Embedded OAuth App with XOR Obfuscation {#adr-0006-embedded-oauth}

**Decision:** Ship official binaries with embedded `client_id` and `client_secret` for a dedicated `jr` Atlassian OAuth app. Secret is XOR-obfuscated with per-build random 32-byte key. Forks/source builds fall back to BYO flow. Fixed callback port 53682 (literal `127.0.0.1`, not `localhost`).

**Harmonization with current architecture:**
- Reaffirmed. All implementation details verified in `api/auth_embedded.rs` and `build.rs`.
- **Tension — PKCE (NFR-S-A):** ADR-0006 states "Atlassian OAuth 2.0 (3LO) requires a `client_secret` for the token exchange step as of 2026-04 — there is no PKCE / public-client flow." Pass 1 R1 found no PKCE in the current code (NEW-INV-178). RFC 8252 recommends PKCE for native apps regardless. This is an **open decision**: should ADR-0006 receive an addendum to clarify PKCE stance? Phase 3 SECURITY-DECIDE must resolve.
- **Tension — first-result-wins (NEW-INV-179):** `accessible_resources.first()` is silent first-wins. ADR-0006 does not address multi-site OAuth users. NFR-O-S tracks the `--cloud-id` enhancement.

---

## Harmonization Decision Log

| Decision | ADR impacted | Action |
|----------|-------------|--------|
| PKCE not implemented (NEW-INV-178) | ADR-0006 | Phase 3 SECURITY-DECIDE: see SD-001-pkce.md; PKCE addendum to ADR-0006 OR new ADR-0013 |
| `accessible_resources` first-result-wins (NEW-INV-179) | ADR-0006 | Phase 3 DEFER: `--cloud-id` flag (NFR-O-S); does not require ADR amendment |
| `JR_AUTH_HEADER` production leak (NFR-S-B) | None (new finding) | Phase 3 SECURITY-DECIDE: see SD-002-jr-auth-header-prod-gating.md |
| `--verbose` PII body exposure (NFR-S-C) | None (new finding) | Phase 3 SECURITY-DECIDE: see SD-003-verbose-pii-redaction.md |
| Shard rule applied once then violated 3× (P5R1-AP-04) | ADR-0004 (tangentially) | ADR-0012 codifies the rule explicitly |
| Multi-profile fields bug (NFR-R-D) | None (new finding) | ADR-0007 documents fix strategy |
| DEFAULT_OAUTH_SCOPES drift risk (BC-1.3.023, ADV-P1-025) | ADR-0006 | Phase 3 add-on: add a post-build smoke-test in `release.yml` asserting `jr auth status --output json` reports a scope string matching the BC-1.3.023 constant. Prevents silent scope drift between code and the embedded app's Atlassian Developer Console registration. |
