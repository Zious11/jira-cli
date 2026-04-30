# ADR-0006: Embedded `jr` OAuth App with Compile-Time Obfuscation

## Status
Accepted (re-supersedes ADR-0002 — the original embed-secret decision is reinstated with obfuscation and a BYO escape hatch)

## Context
ADR-0002 was originally accepted (embed secret), then superseded (BYO app required). The BYO path adds a 20-minute Atlassian Developer Console registration step that most users skip — they fall back to API tokens. We want OAuth to "just work" on official binaries.

Atlassian OAuth 2.0 (3LO) requires a `client_secret` for the token exchange step as of 2026-04 — there is no PKCE / public-client flow. Atlassian's own first-party CLI (`acli`) embeds OAuth credentials and exposes only `--web` to users (https://developer.atlassian.com/cloud/acli/reference/commands/jira-auth-login/), confirming this is an accepted pattern for Atlassian CLI tooling.

## Decision
Ship official `jr` binaries with an embedded `client_id` and `client_secret` for a dedicated `jr` Atlassian OAuth app. The secret is obfuscated via a per-build random 32-byte XOR key to defeat automated secret scanners. Forks and source builds (no env vars at compile time) fall back to the existing BYO flow with zero behavior change. Power users on official binaries can still override with `--client-id` / `--client-secret` or `JR_OAUTH_CLIENT_ID` / `JR_OAUTH_CLIENT_SECRET`.

The embedded app uses a fixed callback URL `http://localhost:53682/callback` because Atlassian's authorize endpoint requires exact `redirect_uri` match (https://jira.atlassian.com/browse/JRACLOUD-92180).

## Rationale
- **UX win**: matches Atlassian's own `acli` ergonomics.
- **Honest threat model**: XOR obfuscation only defeats automated scanners. Motivated attackers extracting the secret are mitigated by Atlassian's `client_secret` rotation flow, not by concealment in the binary.
- **No infrastructure**: no jr-hosted server, no telemetry, no operational footprint beyond rotating a secret if abused.

## Consequences
- The `client_secret` will eventually leak (any binary you ship can be reverse-engineered). Rotation is the recourse — see operational runbook in `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`.
- Forks must register their own OAuth app (or supply their own `JR_BUILD_OAUTH_CLIENT_ID` / `JR_BUILD_OAUTH_CLIENT_SECRET` at compile time); they cannot reuse the official `jr` identity.
- BYO users keep their existing flow; refresh tokens stay bound to whichever app issued them. No silent app-flip mid-session.
- Port 53682 is a permanent contract — changing it is a breaking release.

## Supersedes
ADR-0002 ("OAuth 2.0 with Embedded Client Secret" → "User-provided OAuth credentials"). The new approach reverses the user-provided default while keeping it as an opt-in escape hatch.
