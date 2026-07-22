# Attachment Commands Spec (S-576-1..5)

`jr issue attachment` — list, download, upload, and delete attachments on Jira issues.

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
1. Bare `-` as a file path rejected at exit 64 (stdin not supported for upload; EC-3.9.001-6).
2. File existence: every path must resolve to a regular file (exit 64 on missing/unreadable).

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

**JSM visibility (`--public`/`--internal`, S-576-5):**
When `--public` or `--internal` is set, the upload routes through the JSM two-step flow:
- **P1-004 issue key lookup (first):** `GET /rest/api/3/issue/{key}?fields=project` → extracts `fields.project.key`. 404 → exit 64 "Issue {key} not found or not accessible." Implemented by `src/api/jira/issues.rs::JiraClient::get_issue_project_key`.
- Step 1: `POST /rest/servicedeskapi/servicedesk/{sdId}/attachTemporaryFile` (multipart; returns `temporaryAttachmentId`). One call per file. `X-Atlassian-Token: no-check` mandatory.
- Step 2: `POST /rest/servicedeskapi/request/{issueKey}/attachment` with `{"temporaryAttachmentIds":[…],"public":<bool>}`.

**`--public` gates:** Non-interactive (`--no-input`/non-TTY, no `--yes`): exit 64 with "Use --yes to confirm uploading …" message. `--public + --replace-existing` → combined message. Interactive: `eprint!` + read_line single prompt (VP-576-005 — ONE prompt, not two). Cancel → exit 0; EOF → exit 130.

**`--public` on non-JSM (BC-3.9.005):** exit 64 "--public is only supported on Jira Service Management (JSM) issues."

**`--internal` on non-JSM (OQ-9):** silent no-op — falls through to the platform POST path (no error, no warning, no servicedeskapi calls).

**EC-3.9.003-7:** non-JSM guard fires AFTER `get_or_fetch_project_meta` but BEFORE the visibility gate and BEFORE any dry-run preview. Issue key lookup (P1-004) fires BEFORE the non-JSM guard.

**SEC-576-006 stale-ID self-heal:** on 404/403 from step-1, `invalidate_project_meta_cache` + re-fetch + retry ONCE only. Second failure: 404 → exit 64 "Service desk for {key} not found after refresh." (P1-001); 401 → exit 2; others propagate as-is.

**BC-3.9.006 step-2 error taxonomy:** 401 → exit 2; 403 → exit 1; other 4xx → exit 64; 5xx → exit 1. All append retry hint "Temporary attachment IDs may have expired. Try the upload again."

**`--public --dry-run` (EC-3.9.020-7):** `wouldUpload` entries include `"visibility":"public"`; human mode prints `"Would upload N file(s) [public]."`. Non-JSM guard fires before dry-run (EC-3.9.020-8).

Implemented: `src/cli/issue/attachments.rs::handle_attachment_upload` + `handle_attachment_upload_jsm`. API (platform): `src/api/jira/attachments.rs::upload_attachments`, `delete_attachment`. API (JSM): `src/api/jsm/attachments.rs::attach_temporary_file`, `post_request_attachment`. JSM meta: `src/api/jsm/servicedesks.rs::get_or_fetch_project_meta`. Issue key lookup: `src/api/jira/issues.rs::JiraClient::get_issue_project_key`.

### `jr issue attachment delete`

Deletes attachments by AID or by age filter.

**Three invocation forms:**
- `jr issue attachment delete AID [--yes]` — single targeted delete.
- `jr issue attachment delete AID1 AID2 … --yes` — multi-AID bulk (always requires `--yes`).
- `jr issue attachment delete --issue KEY --older-than DURATION --yes` — age-based bulk.

**Single-AID gate (BC-3.9.015; DEC-174):** Without `--yes`, fetches attachment metadata (GET
`/rest/api/3/attachment/{id}`) to get filename, then prompts `"Delete attachment <name> (AID)? [y/N]"`
via `eprint!` + flush + `stdin().lock().read_line()`. `"y"/"yes"` → proceed; other input → cancelled
(exit 0); EOF → `JrError::Interrupted` (exit 130). Non-interactive (`--no-input` / non-TTY stdin)
without `--yes` exits 64 `"Use --yes to confirm deletion without a prompt."`.

**DEC-168 targeted 404:** Single-AID DELETE that returns 404 exits 64 with canonical prefix
`"Attachment <AID> not found or not accessible."` followed by the raw Jira error body.
Uses `delete_attachment_targeted` (separate from the benign-skip `delete_attachment` used by S-576-3).

**Bulk 404 — benign skip (BC-3.9.010):** In multi-AID and `--older-than` paths, 404 is silently
skipped (the attachment was already deleted). Non-404 errors abort the sequence; prior deletions stand.

**`--older-than DURATION`:** Duration formats: `Nm` (minutes), `Nh` (hours), `Nd` (24 clock-hours),
`Nw` (7-day weeks). `1d` = 24h (NOT the 8h Jira workday of `src/duration.rs`).
Invalid duration → exit 64 `"invalid duration: '<VAL>'. Use formats like 30m, 2h, 1d, 7d, 2w."`.

**`--dry-run`:** On single-AID: AID validation guard fires; gate suppressed; no DELETE.
On bulk (`--issue/--older-than`): list GET fires; age filter applied; NO DELETEs; `--yes` not required.
JSON dry-run shape: `{"attachments":[{id[,filename]}],"dryRun":true,"ids":[…]}`.

**JSON success shapes:**
- Single-AID: `{"deleted":true,"id":"<AID>"}`.
- Cancel: `{"cancelled":true,"deleted":false}` (no `id`).
- Bulk: `{"count":N,"deleted":bool,"ids":[…]}`.

Implemented: `src/cli/issue/attachments.rs::handle_attachment_delete`. API:
`src/api/jira/attachments.rs::delete_attachment_targeted` (single-AID DEC-168),
`src/api/jira/attachments.rs::delete_attachment` (bulk benign-skip).

## See Also

- `docs/specs/json-output-shapes.md` — canonical JSON shapes for all four subcommands
- `CLAUDE.md` — Gotchas: `sanitize_attachment_filename`, redirect behavior, upload multipart retry, SEC-576-004, JRACLOUD-96384, `allow_hyphen_values` variadic caveat, DEC-168 targeted-vs-bulk 404 asymmetry, JSM two-step upload (SEC-576-006, BC-3.9.006)
- `.factory/specs/prd/bc-2-issue-read.md` — list/download behavioral contracts
- `.factory/specs/prd/bc-3-issue-write.md` — upload/delete behavioral contracts
