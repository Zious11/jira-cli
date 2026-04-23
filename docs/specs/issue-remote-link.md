# `jr issue remote-link` — link Confluence/web URLs to issues

**Issue:** [#199](https://github.com/Zious11/jira-cli/issues/199)

## Problem

`jr issue link` only accepts two Jira issue keys and only creates issue↔issue links. Users who want to attach a Confluence page or arbitrary web URL to an issue (to render under the "Confluence pages" / "Web links" panel on the Jira UI) have no option — the current workaround is pasting the URL into the description as plaintext, which does not surface in the linked-items panel.

## Validation

Validated against Atlassian REST API v3 docs (`developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-remote-links/`):

- **Endpoint:** `POST /rest/api/3/issue/{issueIdOrKey}/remotelink`
- **Required body field:** `object` (the remote-link target).
- **Minimum viable body:** `{"object": {"url": "...", "title": "..."}}`. Both `url` and `title` are needed for the link to render usefully; the API doesn't reject a missing title outright, but the UI needs it.
- **Response:** `{"id": <number>, "self": "<url>"}` with status `201 Created` (new) or `200 OK` (update via matching `globalId`).
- **Scope:** classic `write:jira-work` — already held by `jr`.
- **`application.type` is informational, NOT a UI-panel selector.** The Jira UI groups remote links into "Confluence pages" vs "Web links" panels based on its own app-integration metadata, not user-supplied `type` values. This spec does not attempt to auto-route to the Confluence panel.
- **`globalId`** acts as an upsert key. Omitted by V1 (always create a new link).

## Design

### Command

```
jr issue remote-link <KEY> --url <URL> [--title <TITLE>]
```

- `<KEY>`: positional, required. The Jira issue to attach the remote link to (e.g. `PROJ-123`).
- `--url`: required. The remote URL.
- `--title`: optional. Label shown in the Jira UI. Defaults to the URL when omitted (so scripts can create links with a single flag).

`--no-input` is a no-op (no prompts — this is a pure-flag command).

### Output

| Mode | Shape |
|---|---|
| `--output json` | `{"key": "<issue>", "id": <linkId>, "url": "<url>", "title": "<title>", "self": "<atlassian-rest-url>"}` |
| Default (table) | `Linked PROJ-123 → https://... (id: 10000)` on stderr (via `output::print_success`) |

Exit 0 on success. On Atlassian error: surface the error body, exit 1 (or 2 for auth errors, per existing conventions).

### Scope — V1

- Create a remote link. That's the only operation the issue asks for.
- **Not in V1:** list, delete, update, globalId support, auto-Confluence-icon inference. Those can be follow-up issues if wanted.

### Why not extend `jr issue link`?

`jr issue link KEY1 KEY2 --type X` takes two positional issue keys. Adding `--url` would make `KEY2` conditionally required and mix issue-link and remote-link semantics on one command. A separate `remote-link` subcommand is cleaner:

- Different resource (`/issue/{key}/link` vs `/issue/{key}/remotelink`)
- Different args (two keys vs one key + URL)
- Different response shape

Mirrors the `jr issue link` vs future `jr issue remote-unlink` symmetry (not implemented in V1 but leaves the namespace clean).

## Files touched

| Path | Change |
|---|---|
| `src/cli/mod.rs` | Add `IssueCommand::RemoteLink { key, url, title }` variant. |
| `src/cli/issue/mod.rs` | Dispatch `RemoteLink` to the new handler. |
| `src/cli/issue/links.rs` | New `handle_remote_link` function. Minimal: builds body, calls API, prints output. |
| `src/api/jira/links.rs` | New `JiraClient::create_remote_link(key, url, title) -> Result<CreateRemoteLinkResponse>`. |
| `src/types/jira/issue.rs` (or a new `links.rs`) | New `CreateRemoteLinkResponse { id: u64, self_url: String }` (with `#[serde(rename = "self")]`). |
| `src/cli/issue/json_output.rs` | New `remote_link_response(key, id, url, title, self_url)` helper. |
| `tests/issue_remote_link.rs` (new) | Wiremock integration: happy path + missing-title default + server error. |
| `README.md` | One-line mention under the commands table. |

## Testing

All tests use `wiremock::MockServer` + `assert_cmd::Command::cargo_bin("jr")` per the established pattern.

1. **Happy path — with title.** Mount `POST /rest/api/3/issue/PROJ-123/remotelink` returning 201 + `{"id": 10000, "self": "..."}`. Invoke `jr issue remote-link PROJ-123 --url https://example.com --title "Example"`. Assert stdout JSON has `.key`, `.id`, `.url`, `.title`, `.self`. Assert the POST body contained `{"object": {"url": "https://example.com", "title": "Example"}}`.
2. **Happy path — title defaults to URL.** Same mock, invoke without `--title`. Assert the POST body contained `{"object": {"url": "https://example.com", "title": "https://example.com"}}` and stdout shows the URL as the title.
3. **Server error.** Mount POST returning 400 + `{"errorMessages": ["Issue does not exist"]}`. Assert non-zero exit, stderr surfaces the error body.

## Out of scope

- Listing remote links (`GET /issue/{key}/remotelink`).
- Deleting remote links.
- `--relationship` flag (Atlassian supports it; no current user demand).
- `--global-id` flag / upsert support.
- Auto-detection of Confluence URLs to attach an `application.type` (confirmed no UI effect).
- `jr issue view` rendering remote links in its output.
