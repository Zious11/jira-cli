# Changelog

All notable changes to jr will be documented here.

## [Unreleased]

### Added

- `issue edit`: new `--field NAME=VALUE` flag (repeatable) for setting arbitrary custom
  fields on an existing issue. Supports string, number, single-select (option), date,
  datetime, and user field types. Single-select options are resolved from `editmeta`
  `allowedValues` by human label (case-insensitive). Unsupported types (array, CMDB/any)
  exit 64 with an actionable hint. Field-name resolution uses case-insensitive substring
  match against `list_fields()`; supply `customfield_NNNNN` directly to bypass name
  resolution. Multi-key bulk path rejects `--field` (exit 64). (Issue #396)
- `jr issue edit` and `jr issue create` now echo changed/set fields on success.
  Table mode prints one `  field → value` line per field to stderr (alphabetical
  order; resolved team display name; `(updated)` marker for description; `(cleared)`
  for `--no-parent` / `--no-points`). `jr issue edit --output json` gains a
  `changed_fields` object in the response body with raw field values (description
  carries the raw user-supplied string, not the `(updated)` marker). (Issue #398)
- JSM request type support in `jr issue create` via `--request-type <NAME|ID>`,
  `--field NAME=VALUE`, `--on-behalf-of <accountId>` flags. When `--request-type`
  is set, the command dispatches to `POST /rest/servicedeskapi/request` instead of
  the platform `POST /rest/api/3/issue` endpoint; platform path is byte-for-byte
  unchanged otherwise. (Issue #288)
- `write:servicedesk-request` added to `DEFAULT_OAUTH_SCOPES`. Existing OAuth users
  **MUST re-consent** (`jr auth refresh` or `jr auth login`) to gain the new scope
  before JSM request creation will work. Existing access tokens continue working with
  old scopes until expiry; re-consent is triggered on the next token mint. (Issue #288)

### Fixed

- `jr issue edit --label ... --field ...` combination on a single key now exits 64 with a
  clear conflict error instead of silently dropping the `--field` write and exiting 0. The
  `--label` routing fork calls a labels-only handler that does not accept custom-field pairs;
  the `--label` mutual-exclusion block now rejects this combination before any HTTP call.
  (FIX-F5-001, follow-up to issue #396)

### BREAKING CHANGE (v0.6)

- `--verbose` no longer prints HTTP request/response bodies by default. Use `--verbose-bodies` for full body inspection. The new flag emits a PII warning.
- Rationale: prevents accidental PII leakage in shared terminals, debug log files, and AI-agent context windows. See [SD-003](.factory/architecture/security-decisions/SD-003-verbose-pii-redaction.md) for details.
- Migration: replace `jr ... --verbose` with `jr ... --verbose --verbose-bodies` if you relied on body inspection.
