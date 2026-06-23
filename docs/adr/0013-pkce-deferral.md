# ADR-0013: PKCE Deferral for OAuth 2.0 Authorization Code Flow

## Status
Accepted — Reactivation trigger: Atlassian announces public PKCE support for 3LO Jira Cloud

## Context

`jr` uses Atlassian OAuth 2.0 (3LO) authorization code flow with an embedded
`client_secret` (per ADR-0006). RFC 8252 §8.1 recommends PKCE (RFC 7636) for native
applications regardless of confidential-client status, providing defense-in-depth against
authorization code interception attacks.

Three adoption options were evaluated:

- **Option A:** Add PKCE alongside the existing `client_secret` (PKCE + secret
  simultaneously)
- **Option B:** Migrate to public-client flow (PKCE only, no secret)
- **Option C:** Defer with documented mitigation

Research (2026-05-06) determined that **Atlassian Jira Cloud OAuth 2.0 (3LO) does NOT
publicly support PKCE:**

1. Atlassian Developer Console exposes no PKCE configuration controls (no
   `code_challenge_method` registration, no public-client option)
2. Official Jira Cloud OAuth 2.0 (3LO) documentation makes no mention of
   `code_challenge`, `code_challenge_method`, or `code_verifier` parameters
3. Community evidence indicates internal feature-flag PKCE capability exists but is "not
   exposed on the dev console"
4. Jira Server/Data Center OAuth provider has documented PKCE support but with known
   issue OAUTH20-2491 (rejects PKCE flows without `client_secret`, violating RFC 7636)
5. Bitbucket Cloud explicitly does not support PKCE

This makes Options A and B technically infeasible:
- Option A requires Atlassian's `/oauth/token` to accept `code_verifier` +
  `client_secret` simultaneously — undocumented and unverified; likely silently ignored
- Option B requires public-client registration in Atlassian Developer Console — not
  available as a registration option

## Decision

Defer PKCE adoption for `jr` v0.5/v0.6 with documented threat-model mitigation.

The OAuth 2.0 authorization code flow without PKCE continues to be used for both BYO
OAuth (user-provided client credentials) and embedded `jr` OAuth (per ADR-0006). The
`client_secret` is XOR-obfuscated per ADR-0006 for the embedded variant.

## Threat Model and Mitigations

The primary threat PKCE protects against is **authorization code interception**. For
`jr`, this attack requires:
1. A malicious application running on the same host as `jr`
2. Binding to the fixed callback port `127.0.0.1:53682` before `jr` does
3. Winning the OS first-listener race for the browser's callback delivery
4. Exchanging the intercepted code for a token using the embedded `client_secret`

**Mitigations already in place:**

1. **`jr` binds the listener BEFORE launching the browser.** Per macOS/Linux
   first-listener-wins semantics, a malicious app starting after `jr` cannot displace
   `jr`'s listener.
2. **Fixed IPv4 callback port `127.0.0.1:53682`** — explicitly bound to IPv4 (not
   `localhost`, which can resolve to `::1` on macOS/Chrome and miss the listener).
   Reduces attack surface versus dynamic ports.
3. **XOR-obfuscated `client_secret`** in embedded OAuth (per ADR-0006). Not a strong
   secret, but adds friction to extraction.
4. **BYO OAuth path** — users can register their own Atlassian OAuth app and provide
   `client_id`/`client_secret` via keychain, eliminating the embedded-secret extraction
   concern entirely.

**Residual risk:** R-M1 (MEDIUM). A sufficiently capable attacker with persistent
same-host code execution could reverse-engineer the XOR obfuscation, pre-position a
callback listener, and exchange an intercepted code. Acceptable for current hardening
goals; not acceptable indefinitely.

## Consequences

- The OAuth 2.0 authorization code flow without PKCE is documented and accepted for
  v0.5/v0.6. The threat model is explicit.
- `BC-1.5.036` carries the "no PKCE" body content with reference to this ADR.
- Residual risk R-M1 remains MEDIUM severity with this ADR as the documented mitigation.

**Reactivation trigger:** This ADR will be re-opened when any of the following occur:
1. Atlassian announces public PKCE support for 3LO Jira Cloud
2. Atlassian Developer Console adds PKCE configuration controls
3. Atlassian publishes guidance on PKCE for native applications using 3LO
4. OAuth 2.1 enforcement begins on Atlassian endpoints (OAuth 2.1 mandates PKCE for all
   authorization code flows)

When reactivated, re-evaluate Options A and B against the new evidence.

## See Also

- ADR-0006 — Embedded `jr` OAuth app with XOR obfuscation (defines the embedded-secret
  scheme this ADR operates alongside)
- `src/api/auth.rs` — OAuth 2.0 authorization code flow implementation
- `src/api/auth_embedded.rs` — XOR-obfuscated embedded credentials
- RFC 7636 — Proof Key for Code Exchange by OAuth Public Clients
- RFC 8252 — OAuth 2.0 for Native Apps
