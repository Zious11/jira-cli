# Changelog

All notable changes to jr will be documented here.

## [Unreleased]

### BREAKING CHANGE (v0.6)

- `--verbose` no longer prints HTTP request/response bodies by default. Use `--verbose-bodies` for full body inspection. The new flag emits a PII warning.
- Rationale: prevents accidental PII leakage in shared terminals, debug log files, and AI-agent context windows. See [SD-003](.factory/architecture/security-decisions/SD-003-verbose-pii-redaction.md) for details.
- Migration: replace `jr ... --verbose` with `jr ... --verbose --verbose-bodies` if you relied on body inspection.
