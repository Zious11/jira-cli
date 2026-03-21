# ADR-0004: Per-Feature Specs, Not a Growing Master Spec

## Status
Accepted

## Context
As the CLI evolves past v1, we need a documentation strategy that:
- Keeps individual specs manageable for AI agents (token budget)
- Prevents spec drift as features are added
- Allows parallel feature development without doc conflicts

Two approaches were considered:
1. One master spec that grows with every feature
2. Per-feature specs with an architectural foundation document

## Decision
Use per-feature specs. The v1 design spec becomes the architectural foundation. Each new feature (TUI, bulk ops, Confluence, etc.) gets its own spec in `docs/specs/`.

## Rationale
- **Token economy** — AI agents have limited context windows. Loading a 2000-line master spec to work on one feature wastes tokens. A 200-line feature spec is focused.
- **Reduced merge conflicts** — two features being spec'd in parallel don't touch the same file
- **Clear ownership** — each spec has a single purpose and can be reviewed independently
- **Natural archival** — once a feature ships, its spec becomes historical reference without polluting active docs
- **Spec drift is localized** — if a feature spec drifts from implementation, only that spec needs updating, not a monolithic document

## Consequences
- Need an index (CLAUDE.md serves this purpose) pointing to active specs
- Cross-feature concerns (shared types, auth changes) must be updated across relevant specs
- The v1 design spec remains the single source for architectural decisions and system-level behavior
