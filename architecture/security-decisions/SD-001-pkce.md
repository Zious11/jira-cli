# SD-001: PKCE Adoption Decision

**Status:** PENDING
**Owner:** Phase 3 SECURITY-DECIDE
**Deadline:** TBD — must resolve before Phase 3 gate
**References:** ADR-0006, NFR-S-A (nfr-catalog.md), NEW-INV-178, BC-1.5.036, R-M1 (risk-register.md)

---

## Context

`jr` uses the OAuth 2.0 authorization-code flow (3LO) with an embedded `client_secret` (ADR-0006). The current implementation has NO PKCE: `build_authorize_url` sends no `code_challenge`/`code_challenge_method`, and `exchange_code_for_token` sends no `code_verifier` (BC-1.5.036, NEW-INV-178).

RFC 8252 §8.1 recommends PKCE for native apps regardless of confidential-client status. PKCE adds defense-in-depth against authorization-code interception attacks (e.g., malicious app on the same machine intercepting the callback redirect).

**Tension with ADR-0006:** ADR-0006 states "Atlassian OAuth 2.0 (3LO) requires a `client_secret` for the token exchange step as of 2026-04 — there is no PKCE / public-client flow." If Atlassian's token endpoint now accepts PKCE + secret together, Option A (below) is straightforward. If Atlassian requires migrating to a public-client app (no secret), Option B applies.

---

## Options

### Option A: Add PKCE to the existing confidential-client flow (PKCE + secret)

- Generate `code_verifier` (32 random bytes, base64url-encoded) and `code_challenge` (SHA-256 hash of verifier, base64url-encoded) per RFC 7636.
- Include `code_challenge` + `code_challenge_method=S256` in `build_authorize_url`.
- Include `code_verifier` in `exchange_code_for_token` POST body alongside `client_secret`.
- ~30 LOC change in `src/api/auth.rs`.
- **Precondition:** Atlassian's `/oauth/token` endpoint must accept PKCE + `client_secret` simultaneously. Verify against Atlassian Developer docs before implementing.

### Option B: Migrate to public-client flow (PKCE only, no secret)

- Remove `client_secret` from the embedded app and token exchange.
- Generate PKCE verifier/challenge as in Option A.
- **Precondition:** Atlassian must support public-client 3LO without `client_secret`. As of ADR-0006's writing (2026-04), this was not confirmed. Requires Atlassian Developer Console re-registration.
- Larger scope change — embedded OAuth obfuscation in `auth_embedded.rs` and `build.rs` becomes obsolete.

### Option C: Defer with documented mitigation

- Document the threat model: code-interception attacks require a second app registered on the same localhost port AND the OS to deliver the callback to the wrong process. macOS/Linux first-listener-wins semantics make this difficult in practice.
- Record the decision in ADR-0006 addendum.
- **Condition for deferral:** Must be approved by a security review.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| TBD  | PENDING  | Awaiting Phase 3 security review and Atlassian API verification |

---

## Resolution Requirement

Before closing this SD, the Phase 3 implementer must:
1. Verify Atlassian's current `/oauth/token` behavior with respect to PKCE (`code_verifier`) + `client_secret` simultaneously.
2. Record the outcome in this document and in an ADR-0006 addendum (or new ADR-0013).
3. Update BC-1.5.036 if PKCE is implemented.
