# ADR-0001: Thin Client vs Generated API Client

## Status
Accepted

## Context
We needed to decide how to interact with the Jira REST API v3. Three approaches were considered:

1. **Thin client** — wrap reqwest directly, hand-write API call functions
2. **OpenAPI-generated client** — auto-generate Rust bindings from Jira's OpenAPI spec
3. **Separate client crate** — build a reusable Jira SDK library, then build CLI on top

## Decision
Use a thin client (option 1).

## Rationale
- **Full control** over request/response handling, easy to debug
- **Minimal dependencies** and fast compile times
- **Add endpoints as needed** — we only call ~15 endpoints, Jira's OpenAPI spec defines hundreds
- **Generated clients are fragile** — jirust-cli took the generated approach and still has broken features. Jira's OpenAPI spec is massive and sometimes inaccurate.
- **No premature abstraction** — a reusable SDK is overhead we don't need when we're the only consumer

## Consequences
- Must manually add each new API endpoint (acceptable — we add them one at a time as features need them)
- Must handle pagination, rate limiting, and error parsing ourselves (done once in `api/client.rs`)
- No automatic coverage of new Jira API features until we add them
