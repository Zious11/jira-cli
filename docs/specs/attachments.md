# Attachment Commands Spec (S-576-1..3)

`jr issue attachment` — list, download, and upload attachments on Jira issues.

## Subcommands

### `jr issue attachment list KEY`

Lists all attachments on an issue. Table columns: ID | Filename | Type | Size | Created | Author.
`--output json` returns a curated array `[{author, contentUrl, created, filename, id, mimeType, size}]`
(alphabetical BTreeMap order; `"self"` omitted; `"content"` renamed to `"contentUrl"`; `size` is raw u64).

Client-side filters (AND-composed): `--filter mime=<glob>`, `--filter name=<glob>`, `--filter size-max=<bytes>`.
Zero-attachment hint to stderr (human mode only). Filter-count hint "Showing N of M attachments." on stderr in both modes.
CWE-116: bidi/control chars in filenames replaced with `?` in Filename column via `display_sanitize_filename`.

Implemented: `src/cli/issue/attachments.rs::handle_attachment_list`. API: `src/api/jira/attachments.rs::list_attachments`.

### `jr issue attachment download KEY`

Two-step download: (1) `GET /rest/api/3/attachment/{id}` (metadata); (2) `GET /rest/api/3/attachment/content/{id}`.
Step 2 NEVER appends `?redirect=false` (JRACLOUD-97046). reqwest follows 302/303 redirects; strips `Authorization`
on cross-host redirect (GHSA-9857-6MW7-FQ2M — correct behavior for CDN downloads).

Modes: `--id AID` (single), `--all` (all to output dir), `--newest N` (N most-recent by created desc).
CWE-22 (`sanitize_attachment_filename`): basename-only, NUL-reject, char-scrub, 214-byte cap.
Windows device-name escape at single-id call site only. Batch uses SHA-1 prefix.
Atomic write: temp-file + rename. Fail-soft batch semantics.
JSON: `{"downloaded":[{filename, id, path, size}]}`.

Implemented: `src/cli/issue/attachments.rs::handle_attachment_download`. API (single-id): `src/api/jira/attachments.rs::get_attachment_metadata` + `src/api/jira/attachments.rs::get_attachment_content`. API (batch): `src/api/jira/attachments.rs::list_attachments` (used by `--all` / `--newest`).

### `jr issue attachment upload KEY FILE [FILE…]`

Uploads one or more files via `POST /rest/api/3/issue/{key}/attachments` (multipart/form-data).
`X-Atlassian-Token: no-check` is MANDATORY on every request (SEC-576-003).

**Pre-checks (before any HTTP):**
1. `--public`/`--internal` rejected at exit 64 with interim message (AC-017; removed at S-576-5).
2. Bare `-` as a file path rejected at exit 64 (stdin not supported for upload; EC-3.9.001-6).
3. File existence: every path must resolve to a regular file (exit 64 on missing/unreadable).

**`--replace-existing`:** fetches existing attachment list, finds all same-filename matches
(case-sensitive exact match; JRACLOUD-96384 allows multiple same-name attachments to coexist).
VP-576-003 ordering: ALL DELETEs complete before the POST. Confirmation gate (BC-3.9.014):
non-interactive (`--no-input` / non-TTY stdin) requires `--yes` or exits 64. Interactive:
`eprint!` + flush + `stdin().lock().read_line()` — NOT `dialoguer::Confirm`.

**`--dry-run`:** previews without mutating. Category 1 gates (confirmation) suppressed;
category 2 eligibility guards (BC-3.9.005 non-JSM, flag combos) NOT suppressed;
category 3 pre-flight file checks (file-not-found, issue-404) NOT suppressed (EC-3.9.020-9).
JSON dry-run shape: `{"dryRun": true, "wouldDelete": [{filename, id}], "wouldUpload": [{filename}]}`.

**Retry (ADR-0017):** `Request::try_clone()` returns `None` for multipart bodies.
On 429 the entire request is rebuilt: reopen `tokio::fs::File::open` for every file,
construct a new `reqwest::multipart::Form`, and POST. `src/api/jira/attachments.rs::upload_attachments`.

**SEC-576-004 CRLF guard:** `\r`, `\n`, `\0` in filename → `_` before `Part::file_name()` (CWE-93).

**Error taxonomy:** 413 → exit 1 "Attachment too large: the file exceeds the server-configured limit."
404 (issue) → exit 64. 401 → exit 2. 403/400/5xx → exit 1.

**Cancel JSON:** `{"cancelled": true, "uploaded": false}` on stdout + "Upload cancelled." on stderr; exit 0.
**Success JSON:** curated array identical to `attachment list` shape (VP-576-004).
**Table:** 4-column (Filename / Size / ID / Created).

Implemented: `src/cli/issue/attachments.rs::handle_attachment_upload`. API: `src/api/jira/attachments.rs::upload_attachments`, `delete_attachment`.

## See Also

- `docs/specs/json-output-shapes.md` — canonical JSON shapes for all three subcommands
- `CLAUDE.md` — Gotchas: `sanitize_attachment_filename`, redirect behavior, upload multipart retry, SEC-576-004, JRACLOUD-96384, `allow_hyphen_values` variadic caveat
- `.factory/specs/prd/bc-2-issue-read.md` — list/download behavioral contracts
- `.factory/specs/prd/bc-3-issue-write.md` — upload behavioral contracts
