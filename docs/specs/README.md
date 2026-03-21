# Feature Specs

Each major feature gets its own spec before implementation begins.

## Active Specs

- **v1 (foundation):** `../superpowers/specs/2026-03-21-jr-jira-cli-design.md` — Core architecture, all v1 commands, auth, config, CI/CD

## Planned (not yet spec'd)

- TUI mode
- Bulk operations
- Git integration
- Confluence support
- JSM support
- Assets support

## Spec Lifecycle

1. **Draft** — Feature is being designed, spec is in progress
2. **Approved** — Spec reviewed and validated, ready for implementation
3. **Implemented** — Feature shipped, spec is now reference documentation
4. **Superseded** — Replaced by a newer spec (link to replacement)

## Spec Drift Prevention

After each feature ships:
- Review spec against actual implementation
- Update spec to match reality (code is source of truth)
- Note deviations in a "Implementation Notes" section at the bottom of the spec

Before starting work on a feature area:
- Re-read the relevant spec
- If it contradicts the codebase, update the spec first
- If unsure, check `git log` for recent changes in that area

Specs are living documents. Outdated specs are worse than no specs — they mislead AI agents into making wrong decisions.
