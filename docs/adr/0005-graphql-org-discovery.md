# ADR-0005: GraphQL hostNames for Org Discovery

## Status
Accepted

## Context
The Team field in Jira Cloud stores teams as UUIDs. Listing available teams requires the Atlassian Teams REST API, which needs an organization ID (`orgId`). We needed to decide how to programmatically discover the `orgId` (and `cloudId`) for a Jira Cloud instance.

Three approaches were considered:

1. **`/_edge/tenant_info`** — `GET {instance}/_edge/tenant_info` returns `cloudId`, then a separate GraphQL call with `cloudIds` parameter returns `orgId`. Two API calls.
2. **GraphQL `tenantContexts` with `hostNames`** — A single `POST {instance}/gateway/api/graphql` call using the instance hostname returns both `cloudId` and `orgId`.
3. **OAuth accessible-resources** — `GET https://api.atlassian.com/oauth/token/accessible-resources` returns `cloudId` per site, but only works with OAuth tokens and does not return `orgId`.

## Decision
Use the GraphQL `tenantContexts` query with the `hostNames` parameter (option 2).

## Rationale
- **Single call** — one request returns both `cloudId` and `orgId`, vs. two sequential calls with option 1.
- **`/_edge/tenant_info` is undocumented** — it appears in Atlassian support articles but has no stability guarantee, no published API contract, and has known issues with query parameters returning 403 errors. Production CLI tools should not depend on undocumented endpoints.
- **`hostNames` is simpler** — we already know the hostname from the user's configured instance URL. No need to first discover `cloudId` to then discover `orgId`.
- **Both parameters work** — live testing confirmed that both `cloudIds` and `hostNames` are accepted by the GraphQL gateway. We chose `hostNames` because it eliminates the `/_edge/tenant_info` dependency entirely.
- **Option 3 is OAuth-only** — API token users (our default auth method) cannot use the accessible-resources endpoint.

## Consequences
- Depends on the Atlassian GraphQL gateway (`/gateway/api/graphql`), which is not marked as a stable public API. If Atlassian changes it, we need to adapt.
- The hostname is extracted from the configured instance URL by stripping the scheme — this is simple string manipulation, not URL parsing.
- Results are cached in `config.toml` (`cloud_id`, `org_id`) after first discovery, so the GraphQL call only happens once per instance setup (during `jr init` or lazily on first `--team` usage).
- If the GraphQL endpoint becomes unavailable, the fallback is for users to run `jr init` again or manually set `org_id` in their config.
