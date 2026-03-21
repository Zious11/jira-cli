# ADR-0002: OAuth 2.0 with Embedded Client Secret

## Status
Accepted

## Context
Jira Cloud supports two authentication methods: API tokens (email + token) and OAuth 2.0 (3LO). For OAuth, Atlassian requires a `client_secret` for the token exchange — there is no public client / PKCE flow.

CLI tools cannot truly protect an embedded secret (it can be extracted from the binary). We needed to decide how to handle this.

## Decision
Ship with an embedded `client_id` and `client_secret` in the binary. Use API token as the fallback authentication method.

## Rationale
- **Industry standard** — GitHub CLI, Slack CLI, and other production CLI tools embed OAuth secrets. The secret controls which app is making requests, not user authorization.
- **User security boundary is browser consent** — the real security comes from the user explicitly approving access in their browser, not from the secret.
- **No user registration required** — users don't need to create their own OAuth app on Atlassian Developer Console.
- **Token rotation** — Atlassian rotates refresh tokens on each refresh. The embedded secret + token rotation + scoped access provides defense in depth.

## Consequences
- The `client_secret` is not truly confidential — anyone can extract it from the binary
- If the secret is abused, we can rotate it and release a new version
- API token remains as a fallback for environments where OAuth is impractical
- OAuth scopes are explicit: `read:jira-work`, `write:jira-work`, `read:jira-user`, `offline_access`
