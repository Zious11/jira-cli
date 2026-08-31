# Changelog

All notable changes to jr will be documented here.

## [Unreleased]

### Breaking Changes

- **`--field` now parses opt-in `NAME:kind=VALUE` hint syntax** (S-578-1,
  BC-3.4.026, BC-3.4.031). `parse_field_kv` (shared by `issue create`,
  `issue edit`, and JSM `issue create`) now recognizes a trailing
  `:option`/`:id`/`:name`/`:asset` kind tag before the `=`, in addition to the
  existing bare `NAME=VALUE` form. This story ships the **parser only** —
  real dispatch on the parsed `kind` lands in S-578-2/3/4. Until then, an
  interim guard (`reject_unsupported_hint_kinds`) rejects any hinted
  `NAME:kind=VALUE` pair on `issue edit` and JSM `issue create` with an exit-64
  "field-value kind hints (:option/:id/:name/:asset) are not yet supported on
  this command" error.
  Bare `NAME=VALUE` is unaffected, **except**: a field NAME containing a colon
  immediately followed by a short token that happens to match one of the four
  kind names, or by a non-whitespace token with no space before the `=`, is
  now parsed as a (possibly invalid) hinted pair rather than treated as
  literal name text — e.g. `--field "Region:X=val"` (no space after the
  colon) now exits 64 with "unknown field-value kind 'X'". A field NAME
  containing a colon **followed by whitespace** — e.g.
  `--field "Region: EMEA=val"` — is unaffected and continues to parse exactly
  as it did before this story (name `"Region: EMEA"`, `kind: None`), because
  none of the four valid kind tags contain whitespace.

### Changed

- **`jr issue create --field NAME=VALUE` (platform, non-JSM path) no longer
  exits 64 pre-flight — it now resolves via the project's Create screen
  (`createmeta`)** (S-578-4, BC-3.3.010/BC-3.3.011, DEC-310 — registered
  2026-08-26, reverses DEC-188 from S-639-1). Previously, supplying `--field`
  without `--request-type` exited 64 with "`--field` is only valid with
  `--request-type`". That guard is removed: `--field` now resolves each pair
  against the target project/issue-type's Create screen using the SAME
  resolution machinery as `issue edit --field` (customfield_NNNNN bypass,
  cache-first field-name resolution, hint-kind dispatch), merging the result
  into the create POST body. `--on-behalf-of` is UNCHANGED — it still exits 64
  without `--request-type` (BC-3.8.013). A new ten-member dedicated-flag ×
  `--field` collision guard (D2) rejects a `--field` pair that targets the
  same wire key as a dedicated flag (e.g. `--priority X --field
  priority=Y`) before any HTTP call. This is purely permission-widening — no
  previously-working invocation is broken; an invocation that used to exit 64
  now either succeeds or fails later with a more specific resolution error.
  See CLAUDE.md's `jr issue create --field`/`--on-behalf-of` gotcha entry
  (updated by this story) and `docs/adr/0014-jsm-request-type-dispatch.md`.

- **CI: mutation-test scope gap closed for `field.rs` + `field_resolve.rs`
  (FIX-F6-MUTANTS-SCOPE):** `src/cli/field.rs` (~91 mutants — `jr field options
  <field>`'s M1/M2/M3 context-mechanism resolution) and
  `src/cli/issue/field_resolve.rs` (~45 mutants — the shared `--field`
  resolution/dispatch hub for `issue edit --field` and `issue create --field`)
  are now in `examine_globs` (18 → 20 entries). Both files were omitted since
  creation, meaning the required `mutants` CI gate generated zero mutants for
  either file across every field-dx PR to date (S-580-1, #578 parts 1-5). See
  `docs/specs/cargo-mutants-policy.md`.

## [0.7.0-dev.2] - 2026-08-25

### Added

- **`jr issue list`/`jr issue view --fields <CSV>`** (S-575-1, #724): opt-in
  field selection lets you request a specific comma-separated set of fields
  in `--output json` responses instead of the full default payload.
- **`jr issue list --updated-recent <duration>`** (S-579-1, #725): filters
  issues updated within a rolling duration window (e.g. `1h`, `2d`), mirroring
  the existing `--recent` (created-within) filter but scoped to `updated`.
- **`jr issue list --sort <field>:asc|desc` shorthand** (S-588-1,
  BC-2.1.024/025, #726): a compact `field:direction` form for specifying sort
  order, alongside the existing sort flags.

### Changed

- **`--fields comment` now preserves raw ADF structure** on `issue list`/`issue
  view` instead of flattening it (S-584-1, #732).
- **`jr issue list --updated-recent` supplied alone now proceeds like
  `--recent`** rather than requiring a companion filter (F5 reconciliation,
  DEC-306, #733).

### Fixed

- **`validate_duration` returns `Err` instead of panicking on multibyte
  input** (FIX-F6-LRE-1, #734): malformed duration strings containing
  multibyte UTF-8 characters (e.g. `--updated-recent`) are now rejected with
  a proper error instead of crashing on a byte-index panic.

### Internal

- **Live E2E coverage for the component command family** (S-COMP-E2E-1,
  #719), with two follow-up poll-budget widenings to fix index-lag flakiness
  in `poll_component_filter`/`poll_jql` (#720, #721), and an orphaned
  component-fixture sweeper added to `e2e-sweeper.yml` (S-COMP-E2E-SWEEP-1,
  #722).
- **Dependency bumps:** `step-security/harden-runner` 2.20.1→2.21.0 (#723),
  `clap_complete` 4.6.7→4.6.9 (#687), `Swatinem/rust-cache` (#711),
  `taiki-e/install-action` 2.85.8→2.85.13 (#717),
  `github/codeql-action/upload-sarif` (#718).

## [0.7.0-dev.1] - 2026-08-19

### Breaking Changes

- **`jr auth switch --profile <X> <NAME>` now exits 64** (S-663-1,
  BC-1.2.047). `--profile` was previously *accepted* and had no effect on
  which profile was activated — the positional `<NAME>` always won — though
  it did force an active-profile existence check via `Config::load_with`,
  producing the confusing "both values must be real profiles, only one
  matters" incantation the issue reports. Migration: drop `--profile` and
  run `jr auth switch <NAME>`. All other `auth` subcommands (`list`, `remove`,
  `login`, `status`, `refresh`, `logout`) continue to honor `--profile`
  unchanged. (#663)
- **`jr issue edit --dry-run` now reads stdin and renders an ADF preview
  (S-692-1, DEC-274).** Previously `--dry-run --description-stdin` never read
  stdin and emitted a fixed placeholder string
  (`"<from stdin — not yet read in dry-run>"`) for
  `plannedChanges.description`, and bare `--description` had no ADF preview at
  all. Both `--description` and `--description-stdin` now render the actual
  ADF document via the same `markdown_to_adf`/`text_to_adf` conversion the
  live (non-dry-run) path uses, exposed as a new additive
  `plannedChanges.descriptionAdf` field (`--output json`) / a
  `"  description (ADF): rendered OK"` line (table mode).
  `plannedChanges.description` still carries the raw input string verbatim
  (BC-3.4.013/#398 unaffected). A `markdown_to_adf` `MAX_ADF_DEPTH` recursion
  failure now exits 64 from `--dry-run` too, closing a false-OK regression
  where a pathologically nested description previously returned exit 0 under
  `--dry-run` while the corresponding live edit would exit 64. Any automation
  asserting on the old literal placeholder string will observe a different
  value. Note: `--dry-run --description-stdin` now performs a blocking read
  of stdin (matching the live path). Invocations that previously returned
  immediately without stdin attached will now wait for EOF — pipe input, or
  redirect from /dev/null. (#692)

### Added

- **`jr queue view` surfaces queue-configured custom fields in `--output json`**
  (S-693-1, BC-X.8.009 AMENDED): the resolved queue's declared `fields[]`
  (filtered to `customfield_<digits>` tokens only) now flow into the batch
  issue fetch, so `customfield_*` values configured as queue columns appear
  in JSON output via the existing `IssueFields` flatten mechanism. Table
  output is unchanged (no new column; render-side work tracked separately as
  #575). The `--id` path incurs one additional `list_queues` call to obtain
  this field config that the `<name>` path already has in hand; on failure
  it degrades to base fields only with a stderr warning rather than failing
  the command. (#693)
- **Due date visibility (S-668-1):** `jr issue view` and `jr issue list --output json`
  now include the `duedate` field. `jr issue view` shows a Due Date row; `jr issue list
  --duedate` adds an opt-in Due Date column. (#668)
- **`jr component list`** (S-604-1, BC-8.1.001/002/003/004): lists a project's
  components in table or `--output json` form; `--counts` enriches each row
  with `relatedIssueCounts` via fail-soft N+1 GETs (per-component failure
  degrades to `-`/`null` with a stderr warning rather than failing the
  command). Foundation piece for the new component-management command family
  (types, API, cache, resolver). (#604)
- **`jr component create` and `jr component edit`** (S-604-2, BC-8.1.005/006/007):
  two new subcommands for managing Jira project components.
  `jr component create --project KEY NAME [--description …] [--lead …] [--assignee-type …]`
  creates a component; `jr component edit NAME_OR_ID [--project KEY] [--name …]
  [--description …] [--lead …]` renames or updates an existing one. Numeric component IDs
  bypass the project-component list lookup. Leading-dash component names (e.g. `-legacy`)
  are accepted on both subcommands. (#604)
- **`jr component delete`** (S-604-3, BC-8.2.001-008): deletes a project
  component; refuses (exit 64) unless either `--move-to <NAME_OR_ID>`
  (reassigns affected issues to another component before the DELETE) or
  `--orphan` (interactive confirmation, or `--yes` non-interactively, naming
  the affected-issue count) is supplied. Affected issue keys are snapshotted
  via JQL *before* the DELETE fires. `--output json` reports `deleted`,
  `movedIssuesTo`, `affectedIssueCount`, and `affectedIssues`. (#604)
- **`jr component rename OLD NEW`** (S-608-1, BC-8.3.001-007): renames a
  component in place (its `id` is unchanged by the rename). The
  single-project form requires `--project KEY`; `--all-projects` fans the
  rename out across every project with a component named `OLD`, per-project
  fail-soft; `--dry-run` previews the change set (including the
  `--all-projects` fan-out) without issuing any mutating HTTP call. (#608)
- **`jr issue list --component`** (S-606-1, BC-2.1.018-022): filters issues
  by component name. Bare `--component NAME` (repeatable) OR-combines;
  `--component not:NAME` excludes (EMPTY-inclusive, since JQL `NOT IN`
  excludes issues with no component); `--component none` matches issues with
  no component (zero resolver HTTP calls); `--component all:NAME1,NAME2`
  AND-combines. Names are resolved to ids up front; an unresolvable or
  ambiguous name exits 64 before any JQL search fires. (#606)
- **`jr issue create --component` and `jr issue edit --component`, single-key
  path** (S-605-1, BC-3.4.022/024/025): `issue create --component NAME`
  (repeatable) sets the issue's initial `components` on creation.
  `issue edit KEY --component add:NAME --component remove:NAME` (single key)
  sends native Jira `update`-verb PUT operations (`{"add":{"name":…}}` /
  `{"remove":{"name":…}}`), with an editmeta-gated read-modify-write
  fallback. Component name resolution is a single project-scoped
  component-list GET; unknown/ambiguous names exit 64 pre-flight. (#605)
- **`jr issue edit --component`, multi-key/`--jql` bulk path** (S-605-2,
  BC-3.4.023): bulk `--component add:`/`remove:` across multiple keys or a
  `--jql`-resolved set uses `POST /bulk/issues/fields` with a
  `multiselectComponents` object and integer `componentId`s, issuing up to
  two sequential POSTs when both `add:` and `remove:` are present in the
  same invocation. (#605)

### Fixed

- **Component command family — F5 adversarial-hardening fixes** (#709,
  #715): consolidated numeric component-ID resolution onto a single
  codepath; `--project` is now accepted as a global flag (not just a
  subcommand-local one) on `component create`; component names are
  URL-encoded in outgoing API calls; `jr issue list --component`'s read path
  now unions matches for case-only duplicate component names (e.g.
  `Backend`/`backend`) instead of silently keeping only one; `jr component
  rename --all-projects` now returns the correct exit code when no project
  contains a matching component.

### Internal

- chore(git): reconnect the v0.6.0 release commit (93d422fd) into develop's ancestry — graph-only, no code change (#699).

## [0.6.0] - 2026-08-13

First stable release of the 0.6.0 line, consolidating the `0.6.0-dev.1`
through `0.6.0-dev.12` pre-releases. Highlights below; see the per-dev
sections for full detail.

### Breaking Changes

- **`jr issue create --field`/`--on-behalf-of` without `--request-type` now exit 64
  pre-flight instead of warning and proceeding** (S-639-1, DEC-188, BC-3.8.012/013
  [AMENDED]). These flags are JSM-only; supplying them on the platform create path
  now fails fast, before any HTTP call, project-key resolution, interactive prompt,
  or `--description-stdin` read. Migration: add `--request-type <NAME>` or drop the
  flag. (#639)
- **`jr issue comment` is now a subcommand group.** The flat form
  `jr issue comment KEY "message"` is no longer valid — migrate to
  `jr issue comment add KEY "message"`. New subcommands `delete`, `edit`, and
  `view` ship in this release alongside `add`. (S-577-1, #577)
- **`jr issue move <key> <done-status>` requires an explicit resolution on
  done-category transitions** (carried over from 0.5.0, BC-3.2.013, ADR-0015).
  Supply `--resolution <name>` or `--no-resolution`; interactive sessions prompt.
- **`--verbose` no longer prints HTTP request/response bodies by default** — use
  `--verbose-bodies` for full body inspection (SD-003, PII-leakage hardening).

### Added

- **Windows build support (ADR-0016):** pre-built `x86_64-pc-windows-msvc.zip`
  release binaries, Windows Credential Manager–backed keychain storage, idiomatic
  `%APPDATA%\jr` / `%LOCALAPPDATA%\jr` config/cache paths, an 8 MB main-thread
  stack fix, and Windows CI coverage across the `clippy`/`test` matrices.
- **Attachment commands (S-576 series):** `jr issue attachment list/upload/
  download/delete`, including streaming download with CWE-22 disk-path
  sanitization, multipart upload with `--replace-existing` and `--dry-run`,
  single/bulk/age-based delete, and JSM `--public`/`--internal` visibility
  support via the servicedeskapi two-step flow with stale-ID self-heal. (#576)
- **Comment CRUD with visibility (S-577 series):** `jr issue comment add/edit/
  delete/view`, with `--internal`/`--public` visibility flags on `edit` using
  merge (not replace) semantics on Jira's comment-properties endpoint, and an
  interactive confirmation gate before making a comment public. (#577)
- **Expanded Markdown → ADF coverage:** GFM task lists (`taskList`/`taskItem`),
  GFM alerts (`> [!NOTE]` → `panel`), superscript/subscript, bare-URL
  autolinking, footnotes, and block-level HTML preservation with `hardBreak`
  interior newlines. A recursion-depth guard (CWE-674, SEC-001) protects both
  the forward and reverse conversion paths. (#470, #471, #472, #473, #474,
  #481, #483, #489, #492, #522, #553)
- **JSM enhancements:** request-type discovery caching, queue list/view,
  attachment visibility flows, and an expanded live-E2E suite (self-closing
  teardown, resolution discovery).
- **CI/spec-guard hardening (S-CIGATE/S-626 series):** a fail-closed `ci-gate`
  aggregator as the single required branch-protection check; a genuine MSRV
  1.85.0 validation job (previously silently ran under `stable`); the `ci.yml`
  workflow file itself is now parsed with a real YAML parser
  (`saphyr-parser`) for structural CI-gate assertions instead of line-based
  heuristics; `cargo-mutants` wired in as a hard-required merge gate; BC-body
  and cargo-mutants-policy citation guards; CLAUDE.md dead-citation CI guard.
  (#519–#553, #626, S-CIGATE-1..3)

### Fixed

- Attachment download/upload/delete edge cases: integer-vs-string `id` in
  live Jira responses, RFC 3339 fractional-second parsing, Content-Disposition
  CRLF/quote/backslash injection guards (CWE-93), disk-write error
  classification with remediation hints, and 404 body-surfacing asymmetries
  between targeted and bulk paths (DEC-168). (#576, #644, #646, #647, #649)
- ADF code-mark exclusivity (inline code inside bold/superscript no longer
  emits HTTP-400-rejected ADF), listItem/footnote/panel content-model
  conformance, and multi-line inline/block HTML no longer emitting raw `\n`
  into text nodes. (#470–#492, #522, #571)
- `jr issue edit --field` no longer crashes on GDPR-era Jira instances with
  `accountId`-only picker `allowedValues`; `jr api -X`/`--method` now accepts
  case-insensitive HTTP methods. (#589, #590)
- CI correctness: MSRV job false-green (`RUSTUP_TOOLCHAIN` precedence fix),
  `verify-signatures` step no-op on signing-configured forks, fork-ops signing
  workflows hardened against CWE-77 env injection and TOCTOU races. (#535, #626)

### Changed

- Internal: split the oversized `src/cli/issue/create.rs` (2,880 LOC) into
  `create.rs`/`edit.rs`/`jsm_create.rs` per the ADR-0012 shard rule; extracted
  `interactions.rs` from `workflow.rs`. No behavior change. (#556, #558)
- `jr issue create --request-type` and `jr project fields` now emit
  pretty-printed (not compact) JSON, consistent with every other `jr --output
  json` path. (#526)
- `comfy-table` pinned to `=7.2.1` (MSRV protection); dependency bumps across
  the CI/security toolchain (`anyhow`, `gitleaks-action` v3, `codeql-action`,
  `checkout`, and others).
- Opt-in fork release-ops workflows (Apple binary signing, release backfill,
  gap-fill, upstream sync) added, inert by default in the canonical repo.

## [0.6.0-dev.12] - 2026-08-12

### Breaking Changes

- **`jr issue create --field`/`--on-behalf-of` without `--request-type` now exit 64
  pre-flight instead of warning and proceeding (S-639-1, closes #639, DEC-188,
  BC-3.8.012/013 [AMENDED]):** Previously (S-383), supplying `--field NAME=VALUE` or
  `--on-behalf-of <accountId>` on the platform create path (i.e. without
  `--request-type`) emitted a `warning: … is ignored on the platform create path`
  line to stderr and the platform issue was still created (exit 0). These flags are
  self-declared JSM-only flags, and DEC-188 promotes this to a categorical user
  error: `jr issue create` now exits 64 BEFORE any HTTP call, project-key resolution,
  interactive prompt, or `--description-stdin` read. If both flags are supplied
  together, ONE combined error fires (not two). **Migration:** add `--request-type
  <NAME>` to route the request through the JSM API, or drop `--field`/`--on-behalf-of`
  to create a standard platform issue. `--output json` invocations get the same
  structured `{"error": "…", "code": 64}` envelope on stderr as any other pre-flight
  guard. See `docs/specs/issue-create-preflight-guards.md`.

### Fixed

- **MSRV CI job genuinely validates 1.85.0 (S-626-1, #626, CI correctness):** The `msrv`
  job in `ci.yml` previously pointed at the tip of dtolnay's `1.85.0` version branch,
  which hard-codes the toolchain and has no `toolchain` input. The action correctly
  installed 1.85.0 and set it as `rustup default`, but `cargo check` ran in the repo
  root where `rust-toolchain.toml` (`channel = "stable"`) outranks `rustup default`
  in rustup's precedence chain — so the check silently ran under stable (a
  false-green). The fix replaces the SHA to `fa04a1451ff1842e2626ccb99004d0195b455a88`
  (a version of the action that declares `toolchain` as a required input), adds
  `with: {toolchain: "1.85.0"}`, and adds `RUSTUP_TOOLCHAIN: "1.85.0"` to the
  `cargo check` step — which outranks `rust-toolchain.toml` at process level —
  making the MSRV check genuine. The `msrv` job's `cargo check` also gained
  `--locked`, so the gate validates the committed `Cargo.lock` instead of
  silently re-resolving other and transitive dependencies at check time —
  a real MSRV risk given dependencies in the checked lib+bins graph (e.g. `dirs`)
  that ship no `rust-version` manifest field.
  (The exact `=7.2.1` `comfy-table` pin described below is unaffected either
  way: an exact-`=` pin cannot re-resolve to a different version regardless
  of `--locked`.) No user-visible behaviour change.

### Changed

- **`comfy-table` pinned to 7.2.1 (S-626-1, #626):** `comfy-table 7.2.2` uses let-chains
  (`edition = "2024"`, Rust ≥1.88 required) and deleted its `rust-version` manifest field,
  so a caret range `"7"` would silently resolve to an incompatible version without cargo
  enforcing any MSRV constraint. Pinned to `=7.2.1` until the codebase is ready for an
  MSRV raise to 1.88 (tracked as a dedicated follow-up story). User impact: None for
  binary users or source-builders on Rust ≥1.85.0.

- **Three in-tree let-chains rewritten to nested `if` blocks (S-626-1, #626, internal):**
  `src/cli/auth/keychain.rs`, `src/cli/board.rs`, and `src/cli/issue/list.rs` each
  contained one let-chain that is valid only under Rust ≥1.88 edition 2024. Rewritten as
  semantically-equivalent nested `if` blocks to restore MSRV 1.85.0 compliance. No
  user-visible behaviour change.

## [0.6.0-dev.11] - 2026-07-25

### Fixed

- **`jr issue attachment download` — integer id in metadata response (FIX-576-DL, #576):**
  `GET /rest/api/3/attachment/{id}` returns `"id"` as a JSON integer on live Jira Cloud
  (e.g. `10008`), while the issue-fields list endpoint returns it as a string. The S-576-2
  mocks used string IDs throughout, so the type mismatch was invisible until the first live
  validation run (S-576-6, run 30031724733), which produced `invalid type: integer \`10008\`,
  expected a string`. `AttachmentMetadata.id` now uses `deserialize_string_or_int_as_string`
  to accept both forms; `AttachmentObject.id` (list path) is unaffected.

- **`jr issue attachment download` — `--newest` RFC 3339 parser accepts any fractional-second precision (FIX-F5-006, #644):**
  The `%.3f` strptime specifier in the `--newest` sort path accepted only exactly 0 or 3
  fractional-second digits; timestamps with 1, 2, or 4+ digits (all valid RFC 3339) failed to
  parse and sorted last, causing `--newest N` to select older attachments over genuinely-newer
  ones. The sort path now uses the same relaxed `chrono::DateTime` RFC 3339 parser already
  used by the `--older-than` path.

- **`jr issue attachment delete` — interactive single-AID 404 surfaces Jira error body (FIX-F5-006, #644):**
  The interactive confirmation gate (`handle_attachment_delete`) now appends the raw Jira error
  body to the canonical `"Attachment <AID> not found or not accessible."` prefix (DEC-168
  body-surfacing contract). The download path (`handle_single_download`) retains
  canonical-only output per BC-2.7.012.

- **`jr issue attachment upload` — SEC-576-004 Content-Disposition guard extended to `"` (FIX-F5-006, #644):**
  A double-quote (`"`) in a server-supplied filename was not mapped to `_` before passing to
  `Part::file_name()`, allowing it to prematurely terminate the `filename=` value and expose
  subsequent Content-Disposition data to parser misreading. The guard now maps `\r`, `\n`, `\0`,
  and `"` to `_` in both the platform and JSM upload paths (CWE-93).

- **`jr issue attachment upload` — JSM step-2 transport errors report connectivity hint, not expired-ID hint (FIX-F5-006, #644):**
  Network errors from `post_request_attachment` (step 2 of the JSM upload flow) previously emitted
  `ApiError { status: 0 }` and the "Temporary attachment IDs may have expired" retry hint, which
  misled users on connectivity failures. Transport errors now map to `JrError::NetworkError` with
  a "Could not reach {host} — check your connection" message; the retry hint is scoped to HTTP
  error branches only.

- **`jr issue attachment upload` — SEC-576-004 Content-Disposition guard extended to `\` (FIX-F5-007, #646):**
  A backslash (`\`) in a server-supplied filename was not mapped to `_`. As the RFC 2616
  quoted-string escape character, a stray `\` in a `filename=` value causes parsers to misread
  the next character as an escaped sequence. The guard now maps `\r`, `\n`, `\0`, `"`, and `\`
  to `_` in both the platform and JSM upload paths (CWE-93 symmetry).

- **`jr issue attachment download` — `--id` 404 message is canonical-only; Jira body not leaked (FIX-F5-008, #647):**
  A prior fix inadvertently moved Jira error body appending into `get_attachment_metadata`
  itself, causing the download path to include the raw Jira body in its 404 message. The
  BC-2.7.012 asymmetry is restored: `handle_single_download` emits the canonical-only prefix
  `"Attachment <id> not found or not accessible."`; the delete interactive gate continues to
  append `\n{body}` per DEC-168. Also fixed: `batch_path_is_within_dir` now canonicalizes
  the resolved directory before the containment check, preventing false rejections on paths
  containing `..` components.

- **`jr issue attachment download` — disk-write errors classified with remediation hints (FIX-F5-010, #649):**
  All I/O failure sites in the streaming download path (file create, write, flush, rename) now
  produce user-friendly error messages with remediation hints instead of raw OS error strings:
  `StorageFull`/`QuotaExceeded` → `"Disk full: not enough space to write <dest>: <os_err>. Free up disk space and try again."`;
  `PermissionDenied`/`ReadOnlyFilesystem` → `"Permission denied: cannot write to <dir>: <os_err>. Check directory permissions and try again."`;
  all other errors → `"Failed to write <dest>: <os_err>."`.
  All three sites now display the final destination path rather than the internal
  `tmp_<hex>` staging path that was previously leaked in error messages.

### Added

- **`jr issue attachment upload --public/--internal` — JSM visibility + servicedeskapi two-step (S-576-5, #576):**
  `--public` and `--internal` now route through the JSM two-step upload flow instead of exiting 64.
  Step 1: `POST /rest/servicedeskapi/servicedesk/{sdId}/attachTemporaryFile` (multipart; one call per file;
  `X-Atlassian-Token: no-check` mandatory). Step 2: `POST /rest/servicedeskapi/request/{issueKey}/attachment`
  with `{"temporaryAttachmentIds":[…],"public":<bool>}`. Customer-visible vs internal distinction is controlled
  by the `public` boolean in the step-2 body. JSM project determination: `get_or_fetch_project_meta` resolves
  `service_desk_id` via `ServiceDesk.project_id` string equality (BC-X.8.010). Non-JSM guard: `--public` on a
  non-JSM issue exits 64 (BC-3.9.005); `--internal` on a non-JSM issue is a silent no-op that falls through to
  the platform path (OQ-9). Guard fires AFTER project meta fetch but BEFORE the visibility gate and dry-run
  preview (EC-3.9.003-7). SEC-576-006 stale-ID self-heal: on step-1 404/403, invalidate the cache entry,
  re-resolve sdId, and retry once. Visibility gate: non-interactive (`--no-input`/non-TTY, no `--yes`) exits
  64; `--public + --replace-existing` uses the combined message. Interactive: single `eprint!`+read_line prompt
  (VP-576-005). Cancel → exit 0 + `{"cancelled":true,"uploaded":false}` JSON; EOF → exit 130.
  Step-2 error taxonomy (BC-3.9.006): 401 → exit 2; 403 → exit 1; other 4xx → exit 64; 5xx → exit 1;
  all append retry hint. `--dry-run --public` adds `"visibility":"public"` to `wouldUpload` entries (EC-3.9.020-7).

## [0.6.0-dev.10] - 2026-07-15

### Breaking Changes

- **`jr issue comment` is now a subcommand group (S-577-1, issue #577):**
  The flat form `jr issue comment KEY "message"` is no longer valid. Migrate to
  `jr issue comment add KEY "message"`. Invoking the flat form now exits 2 with a
  migration hint: `error: use \`jr issue comment add\` instead`.
  The `add` subcommand accepts the same flags (`--stdin`, `--file`, `--markdown`,
  `--internal`). New subcommands `delete` (S-577-3), `edit` (body-only, S-577-4), and `view`
  (S-577-6) are all fully implemented in this release.

### Added

- **`jr issue attachment download` — streaming download + CWE-22 path sanitization (S-576-2, #576):**
  `jr issue attachment download KEY --id AID` downloads a single attachment by numeric ID.
  `--all` downloads every attachment to the output directory; `--newest N` downloads the N
  most-recent by created descending. Output path: bare sanitized filename for single-id; SHA-1
  prefix (`<40hex>_<name>`) for batch to guarantee NAME_MAX safety (ADV-010, BC-2.7.010).
  Streaming write uses an atomic temp-file + rename pattern — no partial files on error.
  CWE-22 mitigation (BC-2.7.011): 5-step disk-path sanitization strips directory components,
  rejects NUL bytes, scrubs `/\:`, and caps at 214 bytes. Windows device-name escape
  (`CON`→`_CON`, `NUL`→`_NUL`, etc.) applied at the single-id call site only.
  Two-step download: Step 1 = `GET /rest/api/3/attachment/{id}` (metadata); Step 2 =
  `GET /rest/api/3/attachment/content/{id}` — NEVER uses the `content` URL from metadata
  (JSDCLOUD-10841), NEVER appends `?redirect=false` (JRACLOUD-97046).
  JSON manifest: `{"downloaded":[{"filename","id","path","size"}]}` — `filename` is the raw
  Jira-supplied name (P27-001); `size` is bytes-actually-written, not metadata size (P31-002).
  Batch uses fail-soft semantics: per-file failures emit `warning:` to stderr and continue;
  partial failure exits 1 after printing the manifest. `--filter` and `--force` flags supported.
  feat(issue): attachment download single/batch/newest + streaming + CWE-22 sanitization (#576)

- **`jr issue attachment upload` — multipart upload + replace-existing + dry-run (S-576-3, #576):**
  `jr issue attachment upload KEY FILE [FILE…]` uploads one or more files to an issue via
  multipart/form-data POST with `X-Atlassian-Token: no-check` (required by Jira's XSRF
  check on all upload endpoints). Multiple same-name attachments can coexist in Jira
  (JRACLOUD-96384); `--replace-existing` deletes ALL filename matches BEFORE the new upload
  (`wouldDelete` set in dry-run). `--yes` skips the interactive confirmation prompt;
  `--dry-run` previews the operation without mutating state (prints JSON or table).
  Confirmation prompt reads from stdin via `stdin().lock().read_line()` (BC-3.9.014 gate);
  non-interactive mode (`--no-input` / non-TTY stdin) requires `--yes` or exits 64.
  SEC-576-004: `\r`/`\n`/`\0` are stripped from filenames in `Content-Disposition` headers
  to prevent CRLF/quote injection (CWE-93). ADR-0017 retry constraint: 429 rebuilds the
  entire multipart form from fresh `tokio::fs::File::open` (multipart bodies are not
  cloneable via `Request::try_clone()`). 413 → exit 1 with verbatim "Attachment too large"
  message. `--public`/`--internal` interim-rejected at exit 64 (AC-017; removed at S-576-5).
  JSON success shape: array of curated attachment objects (identical to `attachment list`
  shape per VP-576-004). Table: 4-column (Filename / Size / ID / Created).
  feat(issue): attachment upload platform POST + --replace-existing + --dry-run path-c (#576)

- **`jr issue attachment delete` — single-AID + bulk + --older-than + --dry-run (S-576-4, #576):**
  `jr issue attachment delete AID [--yes]` deletes a single attachment by numeric ID.
  Without `--yes`, an interactive gate prompts `"Delete attachment <name> (AID)? [y/N]"`
  (metadata GET fetches the filename; CWE-116 `display_sanitize_filename` applied to prompt).
  DEC-168: targeted single-AID 404 → exit 64 + canonical prefix
  `"Attachment <AID> not found or not accessible."` + Jira error body (surfaced, not silent).
  Non-interactive mode (`--no-input` or non-TTY stdin) without `--yes` → exit 64
  `"Use --yes to confirm deletion without a prompt."`. EOF on gate stdin → exit 130.
  Multi-AID bulk: `jr issue attachment delete AID1 AID2 … --yes` — `--yes` always required
  for multi-AID. Bulk 404 is a BENIGN SKIP (asymmetry from targeted single-AID 404).
  Non-404 error on any AID ABORTS the sequence; first deletions stand.
  Age-based bulk: `jr issue attachment delete --issue KEY --older-than DURATION --yes` fetches
  the issue's attachment list and deletes those older than the parsed duration. Duration formats:
  `Nm` (minutes), `Nh` (hours), `Nd` (24 clock-hours), `Nw` (7-day weeks); `1d` = 24h (NOT
  Jira's 8h workday). Invalid duration → exit 64 canonical error message.
  `--dry-run` on single-AID: emits hint (human) or `{"attachments":[{"id":"…"}],"dryRun":true,"ids":[…]}`
  (JSON), no DELETE. `--dry-run` on bulk: read-only (list GET, age filter, NO DELETEs), emits
  manifest; `--yes` NOT required in dry-run. JSON shapes: single-AID success → `{"deleted":true,"id":"…"}`;
  bulk success → `{"count":N,"deleted":true/false,"ids":[…]}`; cancel → `{"cancelled":true,"deleted":false}`.
  feat(issue): attachment delete single/bulk/older-than + dry-run paths a/b (#576)

- **`jr issue attachment list` — table + JSON output + client-side filters (S-576-1, #576):**
  `jr issue attachment list KEY` lists all attachments on an issue in a six-column table
  (ID, Filename, Type, Size, Created, Author). `--output json` returns a curated array
  with keys `{author,contentUrl,created,filename,id,mimeType,size}` (alphabetical BTreeMap
  order; `"self"` omitted, `"content"` renamed to `"contentUrl"`, `size` is a raw u64).
  Three client-side filters: `--filter mime=<glob>` (case-insensitive, `*` crosses `/`),
  `--filter name=<glob>`, `--filter size-max=<bytes>`; multiple `--filter` flags AND-compose.
  Zero-attachment hint emitted to stderr in human mode only (suppressed in JSON mode per
  BC-2.7.001 EC-2.7.001-1). Filter-count hint "Showing N of M attachments." fires on stderr
  in BOTH modes when a filter reduces the count. CWE-116 display-sanitization: bidi/control
  chars in filenames are replaced with `?` in the Filename column.
  feat(issue): attachment list subcommand + JSON output + filters (#576)

- **`jr issue comment edit --internal/--public` — visibility flags + public confirmation gate (S-577-5, closes #577):**
  `--internal` sets `sd.public.comment={internal:true}` on the comment (agent-only
  on JSM projects); `--public` sets `{internal:false}` (visible to customers).
  Both use **MERGE semantics**: the PUT `properties` array is merged with existing
  properties — an unrelated property (e.g. `jr.test.marker`) is not clobbered.
  A body-only edit (no flag) sends no `"properties"` key; existing visibility is
  PRESERVED unchanged. `--public` requires confirmation: interactive mode prompts
  `"Confirm? [y/N]"`; non-interactive exits 64 with a `--yes` hint unless `--yes`
  is supplied. `--stdin` implies `--no-input` (flag-based, TTY-agnostic). On cancel
  the JSON path returns `{"cancelled":true,"updated":false}`. JSDCLOUD-6050 hint
  fires to stderr on either flag (best-effort on JSM; no-op on non-JSM). JSON
  response includes `changed_fields.jsm_internal: true/false` only when a visibility
  flag was passed; absent in the default body-only path.
  `--yes` without `--public` is accepted as a silent no-op (DEC-169 leniency convention — no clap `requires` pairing).
  This is the last story of bundle SOH-COMMENT-CRUD-1 (wave D).

- **`jr issue comment edit` — body sources + body-only PUT (S-577-4, issue #577):**
  `jr issue comment edit KEY --id ID [BODY | --file F | --stdin]` updates a comment's
  body via a body-only PUT request (`{"body": <adf>}` — no `"properties"` key in the
  default path). Four body sources are supported: positional text, `--file`, `--stdin`,
  and `--markdown` (modifier). Guards: `--id` charset validation (exit 64),
  file-not-found → exit 64 (explicit remap, not exit 1), empty/whitespace body → exit 64.
  `--output json` returns `{changed_fields:{body:<raw-pre-trim>},id,key,updated:true}`.
  Human mode prints `"Updated comment ID on KEY"` to stderr.
  404/403 → exit 64 with dual-line preamble + Jira error body surface.

- **`jr issue comment add/delete/edit/view` subcommand group (S-577-1):**
  `jr issue comment` is now a subcommand group. `add` is fully implemented
  (replaces the old flat form). `edit` (body-only, S-577-4) and `view` (S-577-6)
  are also fully implemented in this release. `delete` (S-577-3) is likewise fully
  implemented (y/N confirmation gate, `--yes` bypass, 404/403 exit 64).
  Interaction handlers extracted from `workflow.rs` to a new
  `src/cli/issue/interactions.rs` shard per ADR-0012 / PF-017.

- **`jr issue comment view KEY --id ID` — read a single comment (S-577-6, #577):**
  `jr issue comment view FOO-1 --id 10001` fetches the comment with
  `GET /rest/api/3/issue/{key}/comment/{id}?expand=properties` and renders six
  labeled fields (ID, Author, Created, Updated, JSM internal, Restricted) plus
  an unlabeled body block rendered via ADF-to-text. The `JSM internal:` field
  shows `Yes`/`No`/`N/A` from the `sd.public.comment` entity property. The
  `Restricted:` field uses a 4-rung ladder (role/group value, `id=<identifier>`,
  `<type>:<value>`, or `None`). `--output json` passes the raw API response
  through losslessly (`serde_json::Value` passthrough — no typed round-trip that
  would silently drop extra fields). Invalid `--id` charset exits 64; 404/403
  exits 64 with Jira's error body surfaced. (Over-deep comment bodies are
  rejected at the JSON parse layer, exit 1.)

- **CI: BC-body Trace/Source citation guard (Guard 1) (DEC-148):** adds
  `scripts/check-bc-citation-symbols.sh` (BC-CITE-001; validates `src/` file and symbol
  citations in `**Trace**:`/`**Source**:` fields of all `bc-*.md` bodies; definition-anchored
  symbol grep; self-test fixtures; coverage-floor guard) as a step in the `spec-guard` CI job.
  Prevents the Seam-extraction citation-drift class (DEC-147/148/149).
  Calibration: measured N=309 citations (304 `.rs` + 5 `.snap`) on factory-artifacts @ 2b09313; FLOOR=231 = floor(0.75 × 309); non-.rs `src/` citations receive file-existence-only validation (tier ii).
- **CI: mutants-policy citation guard (Guard 2) + examine_globs existence guard (Guard 3) (DEC-150):** adds `scripts/check-cargo-mutants-policy-citations.sh` (validates §Scope function-location bulleted list; CI-MUTANTS-CITE-001; self-test fixtures; SCOPE-EMPTY guard) and `tests/mutants_glob_existence.rs` (validates examine_globs entries resolve to real files; coverage floor; MUTANTS-GLOBS-KEY-MISSING guard).

### Security

- **Bump `anyhow` 1.0.102 → 1.0.103 (RUSTSEC-2026-0190):** Resolves an unsoundness
  in `Error::downcast_mut` present in anyhow < 1.0.103. No behaviour change; `Cargo.lock`-only update.

### Fixed

- **`jr issue edit --field` no longer crashes on GDPR-era Jira instances where
  user/group picker fields use `accountId`-only `allowedValues` entries:** `AllowedValue.id`
  changed from `String` to `Option<String>` so id-absent entries are tolerated silently.
  If the user targets such an option entry directly, exit 64 with an actionable message
  is emitted instead of a serde crash. All pre-existing `--field` tests remain green
  (#589).

- **`jr api -X` / `--method` now accepts case-insensitive HTTP method values:** `DELETE`,
  `delete`, and `Delete` are all accepted, matching `curl -X` / `gh api -X` convention.
  Previously clap rejected uppercase inputs with `invalid value 'DELETE'` (#590, #582).

- **ADF code-mark exclusivity — inline-code spans inside bold/superscript no longer emit
  HTTP-400-rejected ADF (BC-7.2.015, #571):** `push_code` now strips typographic marks at
  emission via an allowlist filter: `link` and `annotation` co-marks are retained; `strong`,
  `em`, `strike`, `subsup`, and defensive `underline`/`textColor`/`backgroundColor` are
  removed. The ADF `code_inline_node` schema forbids typographic marks alongside `code` —
  without this fix, patterns such as `` **`x`** `` and `` ^`x`^ `` produced ADF that Jira
  rejected with HTTP 400. The filter operates on a clone of `active_marks` so surrounding
  non-code text retains its marks unchanged. `adf_to_text` stays read-lenient by design. (#593)

### Changed

- **CI: mutation-test scope restored for `edit.rs` + `jsm_create.rs` after ADR-0012
  Seam A/B split (DEC-149):** `src/cli/issue/edit.rs` (~99 mutants) and
  `src/cli/issue/jsm_create.rs` (~9 mutants) are now in `examine_globs`. These
  behavior-dense surfaces — bulk routing forks, C-1 guard, label endpoint fork, JSM
  dispatch — were outside mutation coverage since the ADR-0012 Seam A (PR #556) and
  Seam B (PR #558) splits that relocated `handle_edit*` and `handle_jsm_create` out of
  `create.rs`. Total scope: 594 → ~702 mutants. See `docs/specs/cargo-mutants-policy.md`.

- **CI: `mutants` job is now a hard-required merge gate (MUTATION-CI-TIMEOUT):** The
  `cargo-mutants` mutation-testing job is wired into `ci-gate.needs`, so PRs touching
  scoped files block merge when the kill rate falls below 90%. The absolute per-mutant
  ceiling is `--timeout 240` (seconds); job `timeout-minutes` was raised 60→90. A PR
  that generates 200+ mutants and causes the 90-minute job to be cancelled is also
  blocked — the correct response is to split the PR. The `cargo-mutants` tool version
  is pinned to major v27 to protect schema/exit-code assumptions. See
  `docs/specs/cargo-mutants-policy.md`.

## [0.6.0-dev.7] - 2026-06-26

### Security

- **SEC-001 (CWE-674):** Pathologically nested markdown/ADF input (≥256 levels) now
  exits 64 with "nesting too deep" instead of stack-overflowing. Guard applies to
  `markdown_to_adf` (forward path) and `adf_to_text` (reverse path).

- **`JR_SERVICE_NAME` is now debug-gated (SEC-JR-SERVICE-NAME-GATE, #551):** the
  keychain-service-name override env var is honored only in debug builds; release
  binaries ignore it and always use the compiled-in `jr-jira-cli` service name. Closes
  a test-seam env var that release binaries should not respect.

### Fixed

- **Invalid profile name via `--profile` or `JR_PROFILE` now exits 64 instead of 78 (H-019):** Supplying an invalid profile name (bad charset, empty, or too long) via the `--profile` flag or `JR_PROFILE` environment variable now exits 64 (EX_USAGE — user input error) instead of 78 (EX_CONFIG — config file error). This matches the exit code produced by the unknown-profile check on the same code path. The config-file boundary (`[profiles."foo:bar"]` in `config.toml`) already exited 64 and is unchanged.

- **`verify-signatures` CI step now exercises correctly in signing-configured forks:** The step was a no-op on forks that set `SIGNING_ENABLED=true` because it did not propagate the expected environment. Fixed so signature verification runs as intended when the opt-in workflow is active. No behavior change in the canonical repo (signing disabled).

### Changed

- Dependency bumps:
  - `codecov/codecov-action` 6.0.1 → 7.0.0 (#519)
  - `insta` 1.47.2 → 1.48.0 (#541)
  - `quinn-proto` 0.11.14 → 0.11.15 (RUSTSEC-2026-0185, non-reachable from jr — http3 feature off)
  - `actions/checkout` 6.0.3 → 7.0.0 (#550)

- Internal (no behavior change): split the oversized `src/cli/issue/create.rs` (2,880 LOC)
  into three cohesive modules — `create.rs` (394), `edit.rs` (2,067), `jsm_create.rs` (444) —
  to satisfy the ADR-0012 shard rule (#556, #558). Test parity preserved (1957/93).

## [0.6.0-dev.6] - 2026-06-19

### Added

- **CLAUDE.md dead-citation CI guard (`tests/claude_md_citations.rs`, #544, #545;
  full VSDD F1–F7):** A new always-run test (`test_all_backtick_citations_in_claude_md_resolve`)
  fails the build when an in-scope backtick file-path citation in `CLAUDE.md` points to a
  missing file. Reports the real line number for each broken citation. Coverage scope:
  `src/`, `tests/`, `docs/`, `.github/`, `scripts/`, and a curated set of root-level files
  (`CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `build.rs`, `.cargo/config.toml`,
  `.cargo/mutants.toml`). Excluded: `.factory/` directory (artifact branch, not
  workspace files), glob patterns (`*`, `**`), and human-readable shorthands (e.g.
  `<file>::<fn>`). CWE-22 path-traversal-safe (rejects any token containing `..`).
  (#544: base guard; #545: F6 hardening — `SEC-001` dotdot guard, shared constants,
  mutation-resistance pins, SEC-002 path-canonicalization bypass proof.)

## [0.6.0-dev.5] - 2026-06-19

### Fixed

- **`backfill-release.yml` safe release upsert — check-then-upsert replaces destructive
  delete+recreate (#539, S-FORK-OPS-BACKFILL-1):** `backfill-release.yml` now checks
  whether a GitHub Release for the tag already exists before touching it. If a release
  exists, it upserts assets individually (skipping already-present files); if it does not
  exist, it creates a new release. This eliminates the previous destructive
  `gh release delete` + `gh release create` pattern, which permanently discarded any
  curated release notes that had been written for an existing release.
- **`backfill-release.yml` Windows target parity (#539, S-FORK-OPS-BACKFILL-1):**
  Backfilled releases now include the `x86_64-pc-windows-msvc` `.zip` asset alongside
  the macOS and Linux tarballs, matching the asset matrix produced by `release.yml`.

### Changed

- **`GITLEAKS_DISABLED` fork-ops opt-out documented (#538):** Added `GITLEAKS_DISABLED`
  to the repository-variable reference in `docs/specs/fork-friendly-release-ops.md`.
  Fork maintainers who hit false-positive gitleaks alerts on the signing workflows can
  set this variable to skip the gitleaks scan step. No runtime behavior changed in the
  canonical repo.
- **Backfill matrix-parity test guard anchored to distinct upsert branches (#540,
  FIX-F5-001):** The CI test that verifies zip assets are produced and upserted by
  `backfill-release.yml` now creates isolated branches per test scenario so each upsert
  operation targets a distinct branch, eliminating false-pass conditions that could arise
  from shared branch state across test cases.

## [0.6.0-dev.4] - 2026-06-18

### Fixed

- **Fork-ops signing workflows hardened against CWE-77 env injection, TOCTOU alpha-tag
  race, and missing `set -eo pipefail` (AC-001/002/003/005, #535):**
  `sign-and-publish.yml` and `backfill-release.yml` now env-bind all workflow-context
  expansions (`github.event.workflow_run.head_branch`, `inputs.tag`) before they reach
  `run:` script bodies, eliminating command-injection vectors. Alpha-tag reservation is
  replaced with an atomic 5-step `gh api POST git/refs` retry loop (bounded to 10
  attempts) that removes the TOCTOU-unsafe delete→count→construct pattern. Signature-
  verify steps use `mktemp + trap cleanup` instead of predictable `/tmp` paths (CWE-377)
  and switch from `set -e` to `set -eo pipefail` (CWE-390). A new
  `check-signing-workflow-injection` CI job (YAML-structure-aware Python3+PyYAML scanner)
  is wired into `ci-gate.needs` (AC-005/S-CIGATE-1) to prevent regressions. (#535)
- **`rustup target add` defensive step ported to `sign-and-publish.yml` and
  `backfill-release.yml`:** Native macOS builds in both workflows were vulnerable to
  `error[E0463]: can't find crate for 'core'` because `rust-toolchain.toml`'s stable
  pin re-routes the build through a toolchain that lacks the matrix target.
  `release.yml` already carried the defensive `rustup target add ${{ matrix.target }}`
  step; this ports the same step to the two workflows that were missing it. Inert in
  the canonical repo (signing workflows gated on `SIGNING_ENABLED`); prevents build
  failures in downstream forks that opt in.

### Changed

- **Gatekeeper acceptance and hardened-runtime verification added after notarization
  (closes #210 literal gap):** After every `Notarize` step in `sign-and-publish.yml`
  (alpha-sign, stable-sign) and `backfill-release.yml` (sign), the workflow now asserts
  load-bearing post-signing properties. Stapled containers (`.pkg`, `.dmg`) are checked
  via `spctl --assess --type install|open`; bare Mach-O binaries are checked via
  `codesign -dvv` for Authority, TeamIdentifier, and `runtime` CodeDirectory flag
  (hardened runtime — the property #210 identifies as the root cause of unstable
  Keychain partition entries). Inert in canonical repo; active for forks that set
  `SIGNING_ENABLED=true`. (#210)
- **Shared-docs guidance added for CLAUDE.md/README/ADR sync hygiene across fork repos
  (`docs/specs/fork-friendly-release-ops.md`):** Documents the expectation that fork
  maintainers keep their local copies of `CLAUDE.md`, `README.md`, and relevant ADRs
  in sync with upstream when merging release-ops workflow changes, to prevent drift
  between CI behavior and documentation. No runtime behavior changed.

## [0.6.0-dev.3] - 2026-06-18

### Fixed

- **`jr issue comments` no longer stalls on repeated `nextPageToken` (S-525, BC-2.4.043):**
  `list_comments` in `src/api/jira/issues.rs` now carries the same non-advancing-offset
  guard that `get_changelog` uses — if `next <= start_at` after a page with `has_more=true`,
  the paginator aborts with an error instead of looping forever. Mirrors the JRACLOUD-95368
  pattern already present on the JQL search path. (#531)
- **Multi-line inline HTML in `--description`/comments no longer causes HTTP 400 (BC-7.2.011,
  #522):** `push_text`, `push_code`, and `text_to_adf` now enforce the ADF text-node invariant
  (no raw `\r` or `\n`) at the chokepoint level. In non-codeBlock context `\r\n`, lone `\r`,
  and bare `\n` are all mapped to a single space — matching `SoftBreak` semantics. In codeBlock
  context `\r\n`→`\n` and lone `\r`→`\n` (newlines preserved). This closes a reachable HIGH
  bug where multi-line inline HTML (e.g. `foo <span\nx>bar` in a `--description` or `issue
  comment`) emitted a raw `\n` into an ADF text node, which Jira rejected with HTTP 400. (#523)
- **Block-level HTML interior newlines rendered as `hardBreak` nodes instead of raw `\n`
  (BC-7.2.011, #492):** `markdown_to_adf` now applies Algorithm B to `Tag::HtmlBlock` content:
  the block is split on line boundaries and emitted as alternating `text`/`hardBreak` nodes
  (leading/trailing `hardBreak`s trimmed, empty result suppressed). Previously, multi-line
  block HTML emitted raw `\n` characters inside text nodes — an ADF schema violation that Jira
  rejected with HTTP 400. (#492)

### Changed

- **`jr issue create --request-type` and `jr project fields` now emit pretty-printed JSON
  (#526):** All `--output json` paths in `src/cli/` are now routed through
  `output::render_json`. The two commands that previously used compact `serde_json::json!`
  Display output — `handle_jsm_create` and `handle_fields` — now emit pretty-printed JSON
  consistent with every other `jr` command. `jq` and programmatic parsers that accept
  whitespace-insensitive JSON are unaffected; scripts that did byte-exact comparison against
  compact output will need updating. (#527, S-526)
- **`write_cmdb_fields_cache` and `write_object_type_attr_cache` now use model-b
  error handling (S-525/CR-007):** Both cache writers in `src/cache.rs` swallow disk-write
  errors with an `eprintln!("warning: …")` and return `Ok(())` rather than propagating via
  `?`. A failed cache write no longer prevents a successful API call from completing. Call
  sites updated to use `.ok()` (idiomatic for discarding an always-`Ok` result). (#531)
- **CI Gate aggregator job added as the single required branch-protection status check
  (S-CIGATE-1):** `ci.yml` now contains a `ci-gate` job (`needs: [fmt, clippy, test, msrv,
  deny, spec-guard]`, `if: always()`) that is the only job wired into branch protection on
  `develop`/`main`. New required CI jobs must be added to `ci-gate.needs`, never directly to
  branch protection, to prevent the matrix-rename fragility class. (#533 note: `cargo-mutants`
  examine_globs extended to cover `src/api/jira/issues.rs` and `src/cache.rs`; a keyring-
  touching test gated behind `JR_RUN_KEYRING_TESTS`.)
- **Opt-in release operations workflows added (inert by default):** Four new GitHub Actions
  workflows — Apple binary signing (`sign-and-publish.yml`), release backfill
  (`backfill-release.yml`), gap-fill (`release-gap-fill.yml`), and fork sync
  (`sync-upstream.yml`) — are activated only when the corresponding repository variables
  (`SIGNING_ENABLED`, `HOMEBREW_TAP_REPO`, `RELEASE_GAP_FILL_ENABLED`, `SYNC_UPSTREAM_REPO`)
  are set. The canonical repo has none of these set; existing CI is unaffected.
  (`docs/specs/fork-friendly-release-ops.md`, #503)
- **Documentation accuracy sweep (DRIFT-D1..D12, CR-003/CR-004):** internal doc-only
  corrections to `CLAUDE.md` and architecture notes; no runtime behavior changed. (#524)

## [0.6.0-dev.2] - 2026-06-14

### Added

- **Windows pre-built binary:** `jr-<version>-x86_64-pc-windows-msvc.zip` (containing
  `jr.exe`) is now published to GitHub Releases alongside the existing Unix `.tar.gz`
  artifacts. Packaged via PowerShell `Compress-Archive`; SHA-256 checksum file included.
  (ADR-0016)
- **Windows credential storage:** The `keyring` crate's `windows-native` feature is
  enabled, storing OAuth tokens and API tokens in Windows Credential Manager
  (`CRED_TYPE_GENERIC`). Prior to this change the keyring crate silently used a null
  backend on Windows, losing credentials across invocations. (ADR-0016, Decision 5b)
- **Idiomatic Windows config/cache paths:** On Windows, `jr` now resolves config to
  `%APPDATA%\jr` (`dirs::config_dir()`) and cache to `%LOCALAPPDATA%\jr`
  (`dirs::cache_dir()`). Unix paths (`~/.config/jr`, `~/.cache/jr/v1/<profile>/`)
  are unchanged. (BC-6.1.014, BC-6.2.016, ADR-0016 Decision 4)
- **Windows CI coverage:** `windows-latest` is added to both the `clippy` and `test`
  job matrices in `ci.yml`, providing per-PR regression protection for the
  `#[cfg(windows)]` code paths in `src/config.rs` and `src/cache.rs`. (ADR-0016,
  Decision 3)

## [0.5.0] - 2026-06-11

First stable release of the 0.5.0 line, consolidating the `0.5.0-dev.1`
through `0.5.0-dev.14` pre-releases. Highlights below; see the per-dev
sections for full detail.

### Breaking Changes

- **`jr issue move <key> <done-status>` now requires an explicit resolution on
  done-category transitions** (BC-3.2.013, ADR-0015). Supply `--resolution <name>`
  or `--no-resolution`; interactive sessions prompt. Bulk (multi-key) move is
  unaffected. (#465)

### Added

- **Jira Service Management support:** `jr requesttype list/fields`, `jr queue
  list/view`, `jr issue create --request-type` (servicedeskapi dispatch),
  `--internal` comments, JSM-aware 401 scope hints, and input validation.
  (#288 series, #379, #385, #394, #395)
- **Markdown → ADF coverage:** GFM task lists, GFM alerts (`> [!NOTE]`) → panel,
  super/subscript, bare-URL autolinking, footnotes, and block-HTML preservation.
  (#470, #471, #473, #474, #481, #487, #489)
- **Bulk operations:** multi-key `issue edit`/`move` via the Atlassian Bulk API,
  `--jql` selection, `--dry-run`, `--max` cap, and multi-field edits. (#110, #331, #345)
- **`issue edit` enhancements:** `--field NAME=VALUE` arbitrary custom-field edits,
  `--no-parent`, and changed-field echo on create/edit. (#324, #399, #401)
- **Auth:** OAuth auto-refresh on 401 with per-profile single-flight, multi-cloudId
  disambiguation (`--cloud-id`), and `auth --output json`. (#309, #320, #321)
- **Search:** keys-only JQL API and in-function pagination dedupe. (#362, #367)

### Fixed

- Bulk wire-schema corrections: label objects, `issueType` camelCase, `priorityId`,
  nested bulk-transition body, and numeric issue/task IDs in poll responses.
  (#447–#453, #449, #450, #479)
- ADF `listItem` and footnote content-model conformance. (#470, #481)
- Security hardening: `JR_BASE_URL` release-gate, `errorMessages` stderr
  sanitization, and `task_id` validation. (#355, #356, #357)
- Field-resolution numeric boundary parsing. (#418, #427)

### Changed

- Extensive live-Jira E2E suite with fork-safe CI gating, `cargo-mutants` CI,
  regression holdout suites, gitleaks secret scanning, and StepSecurity runner
  hardening. (#300–#306, #346, #373, #433–#499)

## [0.5.0-dev.14] - 2026-06-11

### Breaking Changes

- **`jr issue move <key> <done-status>` now requires an explicit resolution on
  done-category transitions** (BC-3.2.013, ADR-0015, S-JSM-RESOLUTION-REQUIRED).
  When the target transition is done-category AND offers a resolution field (or has
  `isConditional=true`), the command now enforces resolution upfront:
  - Non-interactive (`--no-input` or no TTY): exits 64 unless `--resolution <name>`
    or `--no-resolution` is supplied.
  - Interactive (TTY): prompts for a resolution via `dialoguer::Select`.
  - `--no-resolution`: explicit opt-out for intentional null-resolution closes (e.g.,
    "Won't Do" automation paths). Mutually exclusive with `--resolution` (exit 2).
  - Scripts relying on the silent bypass must add `--resolution <name>` (recommended)
    or `--no-resolution` (explicit opt-out). Bulk `jr issue move` (multi-key) is NOT
    affected — only single-key move is subject to proactive enforcement.
  - `jr issue transitions --output json` output is byte-identical to pre-feature
    (`skip_serializing` on `Transition.fields` and `Transition.is_conditional`).

### Added

- **`--no-resolution` flag on `jr issue move`:** explicit opt-out from proactive
  resolution enforcement (BC-3.2.013). Use when closing on done-category transitions
  where a null resolution is genuinely intentional. Mutually exclusive with
  `--resolution`. No effect on non-done-category transitions. (S-JSM-RESOLUTION-REQUIRED)
- **`jr issue move` proactive resolution detection:** single-key path now calls
  `GET .../transitions?expand=transitions.fields` to detect whether the target
  transition offers a resolution field — no additional round-trip (replaces the plain
  `GET .../transitions` call in `handle_move`). `jr issue transitions` read command
  unchanged. (BC-3.2.013, ADR-0015, S-JSM-RESOLUTION-REQUIRED)
- **GFM task lists → ADF `taskList`/`taskItem` (#471, BC-7.2.010):** `markdown_to_adf`
  now maps `- [ ] task` / `- [x] done` (tight and loose lists, nested sublists) to
  native ADF `taskList` and `taskItem` nodes with `state: "TODO"/"DONE"`. Jira renders
  these as interactive checkboxes. Each item receives a document-unique `localId` via a
  DFS post-processing pass. Loose multi-paragraph items are merged using `hardBreak`
  separators. Nested `taskList` children and hoisted sibling lists are handled correctly.
  (`docs/specs/adf-task-list.md`)
- **GFM alerts → ADF `panel` (#483, BC-7.2.009):** `> [!NOTE]`, `> [!TIP]`,
  `> [!IMPORTANT]`, `> [!WARNING]`, `> [!CAUTION]` blockquotes are mapped to ADF
  `panel` nodes (panelType: `info`/`success`/`note`/`warning`/`error`). Nested panels
  and blockquotes are unwrapped recursively; nested tables are flattened to paragraphs.
  ADF `listItem` gains a `panel` unwrap arm. Round-trips back to GFM markers via
  `adf_to_text`. (`docs/specs/adf-panel-content-model.md`)
- **Markdown superscript/subscript → ADF `subsup` (#474):** `^x^` maps to ADF
  `subsup sup`; `~x~` maps to `subsup sub`. Double-tilde `~~x~~` continues to map
  to `strike`. Heading attributes (`## Title {#id}`) are parsed and silently dropped
  (ADF headings have no id attribute) rather than leaking `{#id}` into the title text.
- **Bare `http(s)://` URLs → ADF `link` mark (#473):** a post-build pass applies a
  `link` mark to bare URL runs in text nodes, so URLs are clickable in Jira without
  the author needing explicit Markdown link syntax. Scope is explicit-scheme only
  (`http(s)://`); `www.`-prefixed and bare emails are out of scope. Existing inline
  links and code spans are never double-linked. (`src/adf.rs`)
- **JSM live-E2E coverage expansion (S-JSM-E2E-1):** replaces 2 shallow JSM smoke tests
  with 7 shape-asserting / round-trip live tests — queue list/view (by-name + `--id`),
  requesttype list/fields (numeric-bypass pin), internal vs external comment visibility
  round-trip, `issue create --request-type` write round-trip (ADR-0014 dispatch fork), and
  the non-JSM `require_service_desk` guard (exit-64 + message assertions). Also adds a
  SURFACE flag-subset guard (`test_e2e_live_flags_are_subset_of_surface_table`) that closed
  a pre-existing gap (`--priority` missing from the `issue edit` SURFACE row). Zero `src/`
  change. Gated on `JR_E2E_JSM_PROJECT`; set to `EJ` in the `jira-e2e` GitHub Environment
  to activate. (S-JSM-E2E-1)
- **Fork-safe E2E CI gate (#459):** `e2e.yml` and `e2e-sweeper.yml` are now gated by a
  repository variable `JR_E2E_ENABLED`. Both workflow jobs skip cleanly on forks and any
  repo where the variable is not set (empty string `!= 'true'`). A preflight step in
  `e2e.yml` asserts all required secrets/variables are present before consuming runner
  minutes building Rust. **Maintainers:** after merging, create a repository variable
  `JR_E2E_ENABLED=true` at Settings → Secrets and variables → Actions → Variables
  (repository scope, NOT environment scope) to re-enable nightly E2E on the canonical
  repo. Without this step both workflows skip on every trigger. See
  `docs/specs/e2e-fork-safe-ci-enablement.md §5.1`.
- **README E2E status badge:** `[![E2E](...e2e.yml/badge.svg?branch=develop)]` added as
  the second badge in the badge row. Shows green for passing or skipped runs; shows red
  when tests fail. Badge is pinned to the canonical repo.
- **High-value live E2E coverage (#467, #468, E2E-HV-1/2):** expanded live test suite
  covers project list, user list, sprint add/remove, bulk move, write-flag paths
  (description/stdin/markdown, comment channels, story points, parent, `--field`).
- **Assign-by-query live E2E coverage (#458, E2E-PG-4).**

### Fixed

- **Leading-dash values now accepted for all free-text write-command args (#496, #471):**
  `issue create`/`edit` `--summary` and `--description`, `worklog add --message`, the
  `issue comment` positional message, and `issue remote-link --title` all carry
  `allow_hyphen_values = true`. This fixes the `"unexpected argument"` clap error that
  occurred when passing GFM markdown task lists (e.g. `--description "- [ ] todo"`),
  bullet-list content, or titled links as free-text write inputs. Surfaced by the nightly
  E2E test `test_e2e_markdown_task_list_produces_task_items`. Use `--description="…"` or
  `--description-stdin` for programmatic usage where the value may start with a dash.
  (`src/cli/mod.rs`)
- **`markdown_to_adf` listItem content-model conformance (#470, #477):** markdown like
  `- > quote`, `- # heading`, `- ---`, and indented tables inside list items no longer
  emit `blockquote`/`heading`/`table`/`rule` nodes as direct `listItem` children (ADF
  schema violation). Blockquotes are unwrapped recursively, headings are downconverted
  to paragraphs with inline marks preserved, tables are flattened to one paragraph per
  row, and rules are dropped. (BC-7.2.006, `docs/specs/adf-listitem-content-model.md`)
- **Markdown footnotes no longer emit malformed ADF (#481, #472):** `[^1]` reference
  markers and footnote definitions are now preserved as plain text markers and an
  appended definition section (one `rule` divider + one paragraph per definition) instead
  of being dropped or emitting ADF structures that Jira rejects with HTTP 400. Duplicate
  definition labels are deduped; empty container shells left by pulldown-cmark after
  hoisting are pruned. (`src/adf.rs`)
- **Block-level HTML preserved as literal text instead of being dropped (#489, #490):**
  `markdown_to_adf` now routes `Tag::HtmlBlock` through a `NodeKind::HtmlBlock` path
  rather than the silent `Sink` catch-all, so `<div>…</div>` and similar block HTML
  appears as a literal paragraph in ADF rather than vanishing. Interior newlines are kept
  verbatim; a single trailing newline is trimmed. Symmetric with the pre-existing inline
  HTML preservation. (`src/adf.rs`)
- **Bulk transition body now nested in `bulkTransitionInputs` wrapper (#479):**
  `POST /rest/api/3/bulk/issues/transition` was sending a flat top-level schema
  (`selectedIssueIdsOrKeys` + `transitionId` at root) that live Jira rejects with
  HTTP 400 "bulkTransitionInputs must not be empty". Fixed to the nested form required
  by the live API: `{"bulkTransitionInputs":[{"selectedIssueIdsOrKeys":[…],"transitionId":"…"}],"sendBulkNotification":false}`.
  (FIX-BULK-TRANSITION-001)
- **JSM E2E self-close teardown (#464, S-JSM-E2E-2):** the comment-visibility and
  create-request live tests now self-close their EJ tickets by dynamically discovering a
  closing transition (`statusCategory.key == "done"`) instead of the hardcoded `"Done"`
  status name, which the EJ JSM workflow rejects — created EJ tickets were being left open
  on every nightly run. Best-effort teardown preserved (warn-and-return on failure).
  Zero `src/` change. (S-JSM-E2E-2)
- **ADF read-path and gate-guard hardening (#499, #475):** `adf_to_text` human-mode
  coverage for `taskList`/`taskItem`/`panel`/`subsup`/block-HTML round-trips; E2E
  gate-guard `test_every_ignored_test_has_gate_guard` and
  `test_e2e_gate_disabled_when_env_unset` tightened to catch newly-added tests missing
  their `if !e2e_enabled() { return; }` guard.

### Changed

- Dependency bumps:
  - **`gitleaks/gitleaks-action` 2.3.9 → 3.0.0 (#469) — MAJOR version bump.** The
    v3 release drops the legacy `GITHUB_TOKEN`-based secret scanning in favour of a
    purpose-built token; existing workflows may need to update the action inputs if
    they customise the gitleaks configuration. Review the upstream v3 migration guide
    before merging if you maintain a fork.
  - `reqwest` 0.13.3 → 0.13.4 (#461)
  - `chrono` 0.4.44 → 0.4.45 (#497)
  - `EmbarkStudios/cargo-deny-action` 2.0.18 → 2.0.20 (#466)
  - `step-security/harden-runner` 2.19.3 → 2.19.4 (#463)
  - `github/codeql-action` 4.35.5 → 4.36.2 (#462, #498)
  - `actions/checkout` 6.0.2 → 6.0.3 (#484)

## [0.5.0-dev.13] - 2026-06-01

### Fixed

- **`jr issue edit --priority` (bulk / multi-key) now sends the correct `{"priorityId":"<id>"}` schema**,
  resolving the priority name to its id via `GET /rest/api/3/priority` and validating against real
  Jira Cloud. Adds live E2E coverage for priority (single + multi-key bulk), `worklog add`, and
  `issue` unassign. (#452, E2E-PG-4)
- **`jr issue edit --type` (bulk / multi-key) now uses the verified Jira Bulk Ops schema** — camelCase
  `issueType` in `editedFieldsInput`, project-scoped name→`issueTypeId` resolution via createmeta, and a
  cross-project exit-64 guard before any API call. (#331, #453)
- **`createmeta` issue-types response is parsed correctly** — the deserializer now reads the `issueTypes`
  field (not `values`) with offset-based pagination, fixing live issueType bulk-edit resolution that the
  mock-only tests had masked. (#331, #455)

### Changed

- Wired `JR_E2E_ISSUE_TYPE_ALT` into the live E2E workflow so the issueType bulk round-trip test runs in
  CI (`jira-e2e` environment). (#331, #454)
- Compacted `CLAUDE.md` gotchas / AI-agent-notes (~36% smaller) with no loss of load-bearing guidance. (#456)
- Dependency bumps (each cleared a 7-day soak measured from the dependency's version publish date):
  - `serde_json` 1.0.149 → 1.0.150 (#404)
  - `ossf/scorecard-action` 2.4.0 → 2.4.3 (#424)
  - `actions/dependency-review-action` 4.9.0 → 5.0.0 (#422)
  - `github/codeql-action` 3.35.5 → 4.35.5 (#423)
  - `actions/upload-artifact` 4.6.2 → 7.0.1 (#426)
  - `actions/checkout` 4.3.1 → 6.0.2 (#425)
  These four GitHub Actions majors move all workflows onto the Node.js 24 runtime; GitHub-hosted runners
  satisfy the new minimum runner requirement.

## [0.5.0-dev.12] - 2026-06-01

### Added

- Live-Jira E2E test suite (`tests/e2e_live.rs`) plus a non-blocking CI workflow
  (`.github/workflows/e2e.yml`) that exercises `jr` against a real Jira Cloud site.
  Gated behind `JR_RUN_E2E=1` (a complete no-op in normal `cargo test`); runs on push
  to `develop`/`main`, nightly, and on demand, inside a branch-restricted `jira-e2e`
  GitHub Environment. Covers read paths (issue/board/sprint/worklog/user/project, JSM
  optional) and a create→verify→edit→comment→worklog→transition write flow on a dedicated
  `E2E` project, with run-scoped labels and guaranteed close-only teardown. No `src/`
  changes; auth via the existing debug-only `JR_AUTH_HEADER`/`JR_BASE_URL` test seams.
  Includes enhancements from follow-up rounds: deeper assertions, new coverage (label
  add/remove, typed issue link/unlink, remote-link), error-path and robustness/ops
  hardening, an orphan-cleanup sweeper, and first-live-run fixes (empty-status default,
  sprint non-scrum skip). (S-E2E-1..5, #433, #434, #440, #441, #442)
- Offline CLI-surface guard (`tests/e2e_cli_surface_guard.rs`) that validates every `jr`
  subcommand path and flag referenced in `tests/e2e_live.rs` against `jr --help` at CI
  time, without requiring `JR_RUN_E2E` or any network access. Catches assumed-surface
  defects before live runs. (E2E-PG-1, #443)
- Live E2E coverage for label add/remove, `issue link/unlink --type`, and
  `issue remote-link`. (E2E-PG-4, #445)

### Fixed

- **`jr issue edit --label add:X / remove:Y` now works against real Jira Cloud.**
  Both single-key and multi-key label editing were previously broken end-to-end
  (returning HTTP 400 / failing to parse responses) and had only mock-test coverage.
  Single-key now uses `PUT /rest/api/3/issue/{key}` with the `update.labels` payload
  (bare string values; synchronous 204); multi-key now uses the correct `labelsFields`
  schema for the bulk endpoint, with `{"name":"<label>"}` objects per action element.
  Bulk poll responses also now tolerate Jira returning `taskId` and issue IDs as JSON
  integers rather than strings. (#447, #448, #449, #450; closes #446, BUG-LABEL-400)

## [0.5.0-dev.10] - 2026-05-26

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
- `jr issue create --request-type` and `jr issue create` (JSM path) now emit
  auth-aware 401 error hints. When a 401 occurs against `/rest/servicedeskapi/*`,
  the error message distinguishes between OAuth scope gaps (`write:servicedesk-request`
  missing) and API-token expiry, with actionable next-step guidance. (Issue #384)
- JSM input validation and UX polish for `jr issue create --request-type`: empty
  `--request-type` value is rejected at parse time (exit 64); combining
  `--markdown` with `--field description=` is rejected with a conflict error;
  using platform-only flags (`--type`, `--team`, `--sprint`, etc.) on the JSM
  path now emits a per-flag warning to stderr listing the ignored flags. (Issue #385)
- `jr issue edit --type` now emits an enriched error message when the transition
  is rejected with HTTP 400, including the target type name, the current hierarchy
  level, and a hint that cross-hierarchy type changes are not supported by Jira
  Cloud. `--no-parent` with a non-existent parent ID now surfaces a clear
  fake-endpoint hint instead of a raw 404. (Issue #388)

### Fixed

- `jr issue edit --label ... --field ...` combination on a single key now exits 64 with a
  clear conflict error instead of silently dropping the `--field` write and exiting 0. The
  `--label` routing fork calls a labels-only handler that does not accept custom-field pairs;
  the `--label` mutual-exclusion block now rejects this combination before any HTTP call.
  (FIX-F5-001, follow-up to issue #396)

### Dependencies

- `rand` bumped from 0.9.4 to 0.10.1. No user-visible behavior change; `jr` uses only
  the OS CSPRNG path (unaffected by the soundness fix in GHSA-cq8v-f236-94qc /
  RUSTSEC-2026-0097, which applies to `ThreadRng` with the `log` feature — neither of
  which `jr` enables). Dependency hygiene update. (Issue #413)

### BREAKING CHANGE (v0.6)

- `--verbose` no longer prints HTTP request/response bodies by default. Use `--verbose-bodies` for full body inspection. The new flag emits a PII warning.
- Rationale: prevents accidental PII leakage in shared terminals, debug log files, and AI-agent context windows. See [SD-003](.factory/architecture/security-decisions/SD-003-verbose-pii-redaction.md) for details.
- Migration: replace `jr ... --verbose` with `jr ... --verbose --verbose-bodies` if you relied on body inspection.
