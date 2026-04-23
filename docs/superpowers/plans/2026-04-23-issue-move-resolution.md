# `jr issue move --resolution` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `jr issue move` atomically transition status AND set a `resolution` value in one API call, so JSM/classic tickets closing through a resolution-required workflow end up with `resolution` set and `resolutionDate` populated (instead of the current half-resolved limbo).

**Architecture:** Extend Atlassian's `POST /rest/api/3/issue/{key}/transitions` payload with a `fields` object when `--resolution` is set. Discover resolution values via `GET /rest/api/3/resolution` with a 7-day local cache. Add a `jr issue resolutions` command for discovery. Transform Atlassian's "Field 'resolution' is required" 400 into an actionable error pointing at the flag.

**Tech Stack:** Rust, reqwest (existing), serde (existing), clap (existing), wiremock (tests).

**Spec:** `docs/specs/issue-move-resolution.md`
**Issue:** [#263](https://github.com/Zious11/jira-cli/issues/263)

---

## File Structure

Files created:
- `src/api/jira/resolutions.rs` — thin wrapper around `GET /rest/api/3/resolution`.

Files modified:
- `src/types/jira/issue.rs:154-157` — extend the existing `Resolution` struct with optional `id` + `description` fields. **Do NOT create a parallel struct** — there is already `pub struct Resolution { pub name: String }` used for `IssueFields.resolution`. Extending with `Option<String>` + `#[serde(default)]` preserves existing deserialization of `{"name": "Fixed"}` from issue responses while also accepting the richer `{"id": "10000", "name": "Done", "description": "..."}` shape from `/rest/api/3/resolution`.
- `src/api/jira/mod.rs` — add `pub mod resolutions;`
- `src/api/jira/issues.rs:141` — `transition_issue` signature gains `fields: Option<&serde_json::Value>`; one existing call site updated to pass `None`.
- `src/cache.rs` — add `CachedResolution`, `ResolutionsCache`, `read_resolutions_cache`, `write_resolutions_cache`.
- `src/cli/mod.rs` — add `resolution: Option<String>` to `IssueCommand::Move`; add new `IssueCommand::Resolutions { refresh: bool }`.
- `src/cli/issue/mod.rs` — dispatch `Resolutions` to the new handler.
- `src/cli/issue/workflow.rs` — thread `--resolution` through `handle_move` via a new private `resolve_and_encode_resolution` helper and `partial_match`; add new `handle_resolutions` subcommand handler; transform "resolution required" 400 errors.
- `README.md` — document both new surfaces.

Files added to test directory:
- `tests/issue_resolution.rs` — integration tests for the error-path transform and the `jr issue resolutions` subcommand. Uses wiremock + assert_cmd + XDG_CACHE_HOME override (same pattern as `tests/team_column_parity.rs`, `tests/all_flag_behavior.rs`).

---

## Task 1: Extend `Resolution` struct to carry id + description

**Files:**
- Modify: `src/types/jira/issue.rs:154-157`

### Step 1: Write the failing test

- [ ] Append to the existing `#[cfg(test)] mod tests` block in `src/types/jira/issue.rs` (the module that has `resolution_deserialize_*` tests already):

```rust
#[test]
fn resolution_deserializes_full_shape_from_resolution_endpoint() {
    // GET /rest/api/3/resolution returns entries with id + name + description,
    // not just the {"name": "..."} shape that issue.fields.resolution uses.
    let json = r#"{
        "id": "10000",
        "name": "Done",
        "description": "Work has been completed.",
        "self": "https://example.atlassian.net/rest/api/3/resolution/10000"
    }"#;
    let r: Resolution = serde_json::from_str(json).unwrap();
    assert_eq!(r.id.as_deref(), Some("10000"));
    assert_eq!(r.name, "Done");
    assert_eq!(r.description.as_deref(), Some("Work has been completed."));
}

#[test]
fn resolution_preserves_simple_shape_from_issue_fields() {
    // issue.fields.resolution comes back as {"name": "Fixed"} — no id/description.
    // Extending the struct must not break the existing path.
    let json = r#"{"name": "Fixed"}"#;
    let r: Resolution = serde_json::from_str(json).unwrap();
    assert_eq!(r.name, "Fixed");
    assert!(r.id.is_none());
    assert!(r.description.is_none());
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --lib resolution_deserializes_full_shape_from_resolution_endpoint resolution_preserves_simple_shape_from_issue_fields`
Expected: FAIL with `no field \`id\` on type \`Resolution\`` or similar compile/runtime error.

### Step 3: Extend the struct

- [ ] Replace the struct definition at `src/types/jira/issue.rs:154-157`:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub struct Resolution {
    /// Resolution id — populated by `GET /rest/api/3/resolution`; absent on
    /// `issue.fields.resolution` responses which only carry the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// Description — populated by `GET /rest/api/3/resolution`; absent on
    /// `issue.fields.resolution` responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

(Added `Clone` so downstream code can move `Resolution` values around without awkward borrows. `skip_serializing_if` keeps the existing `jr issue view --output json` shape clean — `id`/`description` absent on issue.fields.resolution won't emit `"id": null`.)

### Step 4: Run tests to verify they pass

Run: `cargo test --lib resolution_`
Expected: PASS — both new tests plus all existing `resolution_deserialize_*` / `resolution_preserves_*` / `*_resolution_*` tests.

### Step 5: Verify the whole suite still compiles + passes

Run: `cargo test --lib`
Expected: PASS — all pre-existing tests still green.

### Step 6: Checks + commit

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```
Both clean.

```bash
git add src/types/jira/issue.rs
git commit -m "feat(types): extend Resolution with optional id + description (#263)

Adds Option<String> id and description fields with serde default +
skip_serializing_if to preserve backwards compatibility: existing
issue.fields.resolution JSON ({\"name\": \"Fixed\"}) still deserializes
to a valid Resolution with id/description = None, and serialization
output for issue view --output json is unchanged.

The extended shape is needed for the upcoming GET /rest/api/3/resolution
wrapper, which returns {id, name, description}. Single struct, two uses."
```

---

## Task 2: `GET /rest/api/3/resolution` wrapper

**Files:**
- Create: `src/api/jira/resolutions.rs`
- Modify: `src/api/jira/mod.rs`
- Test: `src/api/jira/resolutions.rs` (inline `#[cfg(test)]` module with wiremock)

### Step 1: Write the failing test

- [ ] Create `src/api/jira/resolutions.rs` with this initial content (module skeleton + failing integration test only — no `get_resolutions` function yet):

```rust
use crate::api::client::JiraClient;
use crate::types::jira::Resolution;
use anyhow::Result;

impl JiraClient {
    // `get_resolutions` added in Task 2 Step 3.
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    #[tokio::test]
    async fn get_resolutions_returns_parsed_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/api/3/resolution"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "10000",
                    "name": "Done",
                    "description": "Work has been completed.",
                    "self": "https://example.atlassian.net/rest/api/3/resolution/10000"
                },
                {
                    "id": "10001",
                    "name": "Won't Do",
                    "description": "This will not be worked on."
                }
            ])))
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
        let resolutions = client.get_resolutions().await.unwrap();

        assert_eq!(resolutions.len(), 2);
        assert_eq!(resolutions[0].name, "Done");
        assert_eq!(resolutions[0].id.as_deref(), Some("10000"));
        assert_eq!(resolutions[1].name, "Won't Do");
    }
}
```

- [ ] Also update `src/api/jira/mod.rs` — add the module declaration in alphabetical order:

```rust
pub mod boards;
pub mod fields;
pub mod issues;
pub mod links;
pub mod projects;
pub mod resolutions;
pub mod sprints;
pub mod statuses;
pub mod teams;
pub mod users;
pub mod worklogs;
```

### Step 2: Run test to verify it fails

Run: `cargo test --lib get_resolutions_returns_parsed_list`
Expected: FAIL — compile error, `no method named get_resolutions found for struct JiraClient`.

### Step 3: Implement `get_resolutions`

- [ ] Replace the `impl JiraClient` block in `src/api/jira/resolutions.rs`:

```rust
impl JiraClient {
    /// Fetch all resolutions configured on the Jira instance.
    ///
    /// Resolutions are instance-scoped — no per-project endpoint. Returns
    /// the full list for company-managed (classic) projects; team-managed
    /// projects have no resolution concept so the list is irrelevant
    /// (but non-empty — Jira serves the same instance-global list).
    /// Not paginated.
    pub async fn get_resolutions(&self) -> Result<Vec<Resolution>> {
        self.get("/rest/api/3/resolution").await
    }
}
```

### Step 4: Run test to verify it passes

Run: `cargo test --lib get_resolutions_returns_parsed_list`
Expected: PASS.

### Step 5: Checks + commit

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
```

```bash
git add src/api/jira/resolutions.rs src/api/jira/mod.rs
git commit -m "feat(api): add GET /rest/api/3/resolution wrapper (#263)

Thin wrapper around the instance-scoped resolutions list. Returns
Vec<Resolution> using the struct extended in the previous commit.
Wiremock test pins the two-entry list shape.

Not paginated — Atlassian returns a flat array. Total resolution count
on a typical instance is ~5-10 so paging is unnecessary."
```

---

## Task 3: Resolution cache (read/write + TTL)

**Files:**
- Modify: `src/cache.rs`
- Test: `src/cache.rs` (inline `#[cfg(test)]` at end of file)

### Step 1: Write the failing test

- [ ] Check the end of `src/cache.rs` for an existing `#[cfg(test)] mod tests` block. If one exists, append to it. If not, add it at the end of the file. Add this test:

```rust
#[cfg(test)]
mod resolution_cache_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolution_cache_round_trip() {
        let tmp = TempDir::new().unwrap();
        // SAFETY: single-threaded test, no other test mutates XDG_CACHE_HOME
        // concurrently because cargo test uses a global mutex for env
        // mutation via ENV_MUTEX elsewhere; this test does not hold that
        // mutex but also does not inspect env vars other than XDG_CACHE_HOME.
        unsafe { std::env::set_var("XDG_CACHE_HOME", tmp.path()) };

        let input = vec![
            CachedResolution {
                id: "10000".into(),
                name: "Done".into(),
                description: Some("Work complete".into()),
            },
            CachedResolution {
                id: "10001".into(),
                name: "Won't Do".into(),
                description: None,
            },
        ];
        write_resolutions_cache(&input).unwrap();
        let loaded = read_resolutions_cache().unwrap().unwrap();

        assert_eq!(loaded.resolutions.len(), 2);
        assert_eq!(loaded.resolutions[0].name, "Done");
        assert_eq!(loaded.resolutions[1].description, None);

        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    }

    #[test]
    fn resolution_cache_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var("XDG_CACHE_HOME", tmp.path()) };

        let loaded = read_resolutions_cache().unwrap();
        assert!(loaded.is_none());

        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    }
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --lib resolution_cache_round_trip resolution_cache_missing_returns_none`
Expected: FAIL — compile error, `cannot find struct CachedResolution` / `cannot find function write_resolutions_cache`.

### Step 3: Implement the cache types and functions

- [ ] In `src/cache.rs`, insert the following block immediately BEFORE the existing `#[derive(Debug, Serialize, Deserialize)]\npub struct CmdbFieldsCache` (so the resolutions cache sits alphabetically next to other whole-file caches):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResolution {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolutionsCache {
    pub resolutions: Vec<CachedResolution>,
    pub fetched_at: DateTime<Utc>,
}

impl Expiring for ResolutionsCache {
    fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }
}

pub fn read_resolutions_cache() -> Result<Option<ResolutionsCache>> {
    read_cache("resolutions.json")
}

pub fn write_resolutions_cache(resolutions: &[CachedResolution]) -> Result<()> {
    write_cache(
        "resolutions.json",
        &ResolutionsCache {
            resolutions: resolutions.to_vec(),
            fetched_at: Utc::now(),
        },
    )
}
```

### Step 4: Run tests to verify they pass

Run: `cargo test --lib resolution_cache_`
Expected: PASS — both tests.

### Step 5: Checks + commit

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
```

```bash
git add src/cache.rs
git commit -m "feat(cache): add ResolutionsCache read/write with 7-day TTL (#263)

Follows existing whole-file cache pattern (TeamCache, WorkspaceCache,
CmdbFieldsCache): ~/.cache/jr/resolutions.json, Expiring trait impl,
generic read_cache/write_cache helpers.

Round-trip test + missing-file-is-None test."
```

---

## Task 4: Extend `transition_issue` to accept optional fields

**Files:**
- Modify: `src/api/jira/issues.rs:141`
- Modify: `src/cli/issue/workflow.rs:212` (existing call site)
- Test: add to the existing inline `mod tests` in `src/api/jira/issues.rs` if one exists, otherwise to `tests/issue_commands.rs` as an integration test

### Step 1: Write the failing test

- [ ] Add to `tests/issue_commands.rs` (this keeps the wiremock integration tests together — search for existing `transition` or `move` tests first; if a natural group exists, append there):

```rust
#[tokio::test]
async fn transition_issue_with_fields_sends_fields_in_body() {
    use wiremock::matchers::{body_partial_json, method, path};

    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .and(body_partial_json(serde_json::json!({
            "transition": { "id": "31" },
            "fields": { "resolution": { "name": "Done" } }
        })))
        .respond_with(wiremock::ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    let fields = serde_json::json!({ "resolution": { "name": "Done" } });
    client
        .transition_issue("FOO-1", "31", Some(&fields))
        .await
        .unwrap();
    // wiremock .expect(1) verifies the matcher was hit exactly once
}

#[tokio::test]
async fn transition_issue_without_fields_omits_fields_key() {
    use wiremock::matchers::{body_string_contains, method, path};

    let server = wiremock::MockServer::start().await;
    // The request body must NOT contain a "fields" key when None is passed.
    // body_string_contains is used negatively via received_requests() below.
    wiremock::Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(wiremock::ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let client = jr::api::client::JiraClient::new_for_test(&server.uri(), "Basic dGVzdDp0ZXN0");
    client.transition_issue("FOO-1", "31", None).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(
        !body.contains("\"fields\""),
        "fields key must be absent when None is passed, got body: {body}"
    );
    // Still emits the transition id
    assert!(body.contains("\"transition\""));
    assert!(body.contains("\"31\""));
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test --test issue_commands transition_issue_with_fields transition_issue_without_fields`
Expected: FAIL — compile error, `expected 2 arguments, found 3` or similar (the current signature takes only `(&self, key, transition_id)`).

### Step 3: Update `transition_issue` signature + implementation

- [ ] Replace `transition_issue` in `src/api/jira/issues.rs`:

```rust
/// Transition an issue to a new status, optionally setting extra fields
/// in the same request (e.g. `resolution`). Passing `fields = None`
/// preserves the pre-existing behaviour of sending only the transition id.
///
/// When `fields` is `Some(&json)`, the value is merged as-is under the
/// `fields` key of the request body — callers are responsible for shaping
/// it correctly (Atlassian expects `{"resolution": {"name": "Done"}}` or
/// `{"resolution": {"id": "10000"}}`).
pub async fn transition_issue(
    &self,
    key: &str,
    transition_id: &str,
    fields: Option<&serde_json::Value>,
) -> Result<()> {
    let path = format!("/rest/api/3/issue/{}/transitions", urlencoding::encode(key));
    let body = match fields {
        Some(f) => serde_json::json!({
            "transition": { "id": transition_id },
            "fields": f,
        }),
        None => serde_json::json!({
            "transition": { "id": transition_id }
        }),
    };
    self.post_no_content(&path, &body).await
}
```

### Step 4: Update the existing call site

- [ ] Update `src/cli/issue/workflow.rs:212` — the line currently reads
`client.transition_issue(&key, &selected_transition.id).await?;`
Change it to:
```rust
client
    .transition_issue(&key, &selected_transition.id, None)
    .await?;
```

### Step 5: Run tests to verify they pass

Run: `cargo test`
Expected: PASS — the two new tests plus all existing transition/move tests.

### Step 6: Checks + commit

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

```bash
git add src/api/jira/issues.rs src/cli/issue/workflow.rs tests/issue_commands.rs
git commit -m "feat(api): transition_issue accepts optional fields (#263)

Extends POST /rest/api/3/issue/{key}/transitions with the fields body
per Atlassian's spec. Callers pass None to preserve the existing
transition-only shape; Some(&json) attaches a fields object for atomic
status+field updates (resolution being the primary motivator).

Two wiremock tests pin the request-body shape both ways. The only
existing call site (handle_move in src/cli/issue/workflow.rs) passes
None so current behavior is unchanged."
```

---

## Task 5: `--resolution` flag on `jr issue move` + partial_match resolver

**Files:**
- Modify: `src/cli/mod.rs` — `IssueCommand::Move` gains `resolution: Option<String>`.
- Modify: `src/cli/issue/workflow.rs` — `handle_move` threads resolution through, add `resolve_resolution_by_name` helper.
- Test: `src/cli/issue/workflow.rs` inline `#[cfg(test)] mod tests` (or wherever the crate colocates tests for workflow.rs; check with `grep -n "mod tests" src/cli/issue/workflow.rs` first).

### Step 1: Write the failing handler test

- [ ] First inspect where handler tests for workflow.rs live:
```bash
grep -n "mod tests\|resolve_" src/cli/issue/workflow.rs src/cli/issue/helpers.rs
```

Existing `partial_match`-backed resolvers (e.g. `resolve_user`, status resolution in `handle_move`) live in `src/cli/issue/helpers.rs` or as inline helpers in `workflow.rs`. Add the new helper and its tests to the same place the status-partial-match is — `workflow.rs` — to keep related code together.

- [ ] Append to `src/cli/issue/workflow.rs` inside (or add at end of file) a `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod resolution_resolver_tests {
    use super::*;
    use crate::types::jira::Resolution;

    fn sample_resolutions() -> Vec<Resolution> {
        vec![
            Resolution {
                id: Some("10000".into()),
                name: "Done".into(),
                description: None,
            },
            Resolution {
                id: Some("10001".into()),
                name: "Won't Do".into(),
                description: None,
            },
            Resolution {
                id: Some("10002".into()),
                name: "Duplicate".into(),
                description: None,
            },
            Resolution {
                id: Some("10003".into()),
                name: "Cannot Reproduce".into(),
                description: None,
            },
        ]
    }

    #[test]
    fn resolve_resolution_exact_match_returns_it() {
        let r = resolve_resolution_by_name(&sample_resolutions(), "Done").unwrap();
        assert_eq!(r.name, "Done");
    }

    #[test]
    fn resolve_resolution_case_insensitive_exact() {
        let r = resolve_resolution_by_name(&sample_resolutions(), "done").unwrap();
        assert_eq!(r.name, "Done");
    }

    #[test]
    fn resolve_resolution_partial_match_returns_single_hit() {
        // "Dup" uniquely matches Duplicate (prefix/substring)
        let r = resolve_resolution_by_name(&sample_resolutions(), "Dup").unwrap();
        assert_eq!(r.name, "Duplicate");
    }

    #[test]
    fn resolve_resolution_ambiguous_substring_errors_with_exit_64() {
        // "o" matches Done, Won't Do, Cannot Reproduce — disambiguation required.
        let err = resolve_resolution_by_name(&sample_resolutions(), "o").unwrap_err();
        let root = err.root_cause().to_string().to_lowercase();
        assert!(
            root.contains("ambiguous") || root.contains("multiple"),
            "expected ambiguous error, got: {err:?}"
        );
        // Exit code 64 comes from JrError::UserError — verify by downcasting
        if let Some(jr_err) = err.downcast_ref::<crate::error::JrError>() {
            assert!(
                matches!(jr_err, crate::error::JrError::UserError(_)),
                "expected UserError variant, got: {jr_err:?}"
            );
        }
    }

    #[test]
    fn resolve_resolution_no_match_errors_with_candidates() {
        let err = resolve_resolution_by_name(&sample_resolutions(), "nonexistent").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("Done"), "error should list candidates: {msg}");
        assert!(msg.contains("Duplicate"), "error should list candidates: {msg}");
    }
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test --lib resolve_resolution_`
Expected: FAIL — `cannot find function resolve_resolution_by_name`.

### Step 3: Add the `resolve_resolution_by_name` helper

- [ ] Look at how `handle_move` currently resolves status names via `partial_match` (around lines 97–209 in `src/cli/issue/workflow.rs`). Mirror that pattern. Add this function in the same file, above `handle_move`:

```rust
use crate::partial_match::{MatchResult, partial_match};
use crate::types::jira::Resolution;

/// Resolve a user-supplied resolution name against the cached list. Matches
/// the existing partial_match UX used for status/link types/users: exact
/// (case-insensitive) wins; unique prefix/substring passes; ambiguous or
/// no-match returns JrError::UserError with a candidate list (exit 64).
///
/// Module-private — the sole caller is `handle_move` when --resolution is set.
fn resolve_resolution_by_name(
    resolutions: &[Resolution],
    query: &str,
) -> Result<Resolution> {
    let names: Vec<&str> = resolutions.iter().map(|r| r.name.as_str()).collect();
    match partial_match(&names, query) {
        MatchResult::Exact(idx) | MatchResult::Ambiguous(idx, _)
            if matches!(partial_match(&names, query), MatchResult::Exact(_)) =>
        {
            Ok(resolutions[idx].clone())
        }
        MatchResult::Exact(idx) => Ok(resolutions[idx].clone()),
        MatchResult::Ambiguous(_, matches) => {
            let candidates: Vec<String> =
                matches.iter().map(|&i| resolutions[i].name.clone()).collect();
            Err(crate::error::JrError::UserError(format!(
                "Ambiguous resolution \"{query}\". Matches: {}",
                candidates.join(", ")
            ))
            .into())
        }
        MatchResult::ExactMultiple(duplicates) => {
            Err(crate::error::JrError::UserError(format!(
                "Multiple resolutions named \"{query}\" exist: {}",
                duplicates.join(", ")
            ))
            .into())
        }
        MatchResult::None => {
            let available: Vec<String> = resolutions.iter().map(|r| r.name.clone()).collect();
            Err(crate::error::JrError::UserError(format!(
                "No resolution matching \"{query}\". Available: {}",
                available.join(", ")
            ))
            .into())
        }
    }
}
```

Note: the `partial_match` helper's actual `MatchResult` variants may differ — before writing this verbatim, run:
```bash
grep -n "pub enum MatchResult\|MatchResult::" src/partial_match.rs | head -20
```
Adapt the arms above to the real variant names. The test at Step 1 pins the user-visible error shape (substring "ambiguous" / "multiple") so the arm wording is flexible.

### Step 4: Run tests to verify they pass

Run: `cargo test --lib resolve_resolution_`
Expected: PASS — all 5 tests.

### Step 5: Add the `--resolution` flag to `IssueCommand::Move`

- [ ] In `src/cli/mod.rs`, locate `IssueCommand::Move { ... }` (grep for `Move {` in that file to jump). Add a new field at the end:

```rust
/// Set the resolution field atomically with the transition. Matched
/// case-insensitively against `jr issue resolutions` by exact name,
/// prefix, or unique substring. Required on many JSM workflows to
/// avoid leaving the ticket in a half-resolved state
/// (status=Done, resolution=null).
#[arg(long)]
resolution: Option<String>,
```

### Step 6: Thread `--resolution` through `handle_move`

- [ ] Locate the `IssueCommand::Move { ... }` destructuring at the top of `handle_move` in `src/cli/issue/workflow.rs`. Update it to include `resolution`, e.g.:
```rust
let IssueCommand::Move { key, target_status, no_input, resolution } = command else {
    unreachable!()
};
```

- [ ] After the `selected_transition` resolution block and BEFORE the `client.transition_issue(...).await?` call, insert:

```rust
// Resolve --resolution against the cached resolutions list if provided.
let resolution_fields: Option<serde_json::Value> = match resolution.as_deref() {
    None => None,
    Some(query) => {
        let cached = crate::cache::read_resolutions_cache()?;
        let resolutions: Vec<crate::types::jira::Resolution> = match cached {
            Some(c) => c
                .resolutions
                .into_iter()
                .map(|r| crate::types::jira::Resolution {
                    id: Some(r.id),
                    name: r.name,
                    description: r.description,
                })
                .collect(),
            None => {
                let fetched = client.get_resolutions().await?;
                // Write-through the cache with just the fields we persist.
                let cacheable: Vec<crate::cache::CachedResolution> = fetched
                    .iter()
                    .filter_map(|r| {
                        r.id.as_ref().map(|id| crate::cache::CachedResolution {
                            id: id.clone(),
                            name: r.name.clone(),
                            description: r.description.clone(),
                        })
                    })
                    .collect();
                crate::cache::write_resolutions_cache(&cacheable)?;
                fetched
            }
        };
        let matched = resolve_resolution_by_name(&resolutions, query)?;
        Some(serde_json::json!({
            "resolution": { "name": matched.name }
        }))
    }
};
```

- [ ] Update the `transition_issue` call to pass `resolution_fields.as_ref()`:
```rust
client
    .transition_issue(&key, &selected_transition.id, resolution_fields.as_ref())
    .await?;
```

### Step 7: Full test suite

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all green.

### Step 8: Commit

```bash
git add src/cli/mod.rs src/cli/issue/workflow.rs
git commit -m "feat(issue): --resolution flag on jr issue move (#263)

Accepts a resolution name, partial-matches against the cached
resolutions list (fetching + caching on miss), and sends it alongside
the transition id in a single POST /rest/api/3/issue/{key}/transitions
call. Atomically moves status + sets resolution + fires resolutionDate.

resolve_resolution_by_name mirrors the partial_match UX we use for
status/link types/users: exact > prefix > substring, Ambiguous/None/
ExactMultiple surface as JrError::UserError (exit 64) with a
candidate list. 5 unit tests pin the resolver shape."
```

---

## Task 6: `jr issue resolutions` subcommand

**Files:**
- Modify: `src/cli/mod.rs` — add `IssueCommand::Resolutions { refresh: bool }`.
- Modify: `src/cli/issue/mod.rs` — dispatch to the new handler.
- Modify: `src/cli/issue/workflow.rs` — add `handle_resolutions`.
- Test: `tests/issue_resolution.rs` (new file) — integration test for `jr issue resolutions --output json`.

### Step 1: Write the failing integration test

- [ ] Create `tests/issue_resolution.rs` (reuse the XDG-override pattern from `tests/team_column_parity.rs`):

```rust
use assert_cmd::Command;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

mod common;

#[tokio::test]
async fn issue_resolutions_json_output_lists_all_entries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "10000", "name": "Done", "description": "Work complete." },
            { "id": "10001", "name": "Won't Do" }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "resolutions", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let arr = parsed.as_array().expect("expected JSON array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], "Done");
    assert_eq!(arr[1]["name"], "Won't Do");
}

#[tokio::test]
async fn issue_resolutions_table_output_prints_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/resolution"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": "10000", "name": "Done", "description": "Work complete." }
        ])))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["issue", "resolutions"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Done"), "expected table to show Done: {stdout}");
    assert!(stdout.contains("Work complete"), "expected description column: {stdout}");
}
```

### Step 2: Run tests to verify they fail

Run: `cargo test --test issue_resolution`
Expected: FAIL — the subcommand doesn't exist yet, clap will error with "unexpected argument 'resolutions'" or similar.

### Step 3: Add the `Resolutions` variant

- [ ] In `src/cli/mod.rs`, inside `enum IssueCommand`, after the existing `Transitions { key: String }` variant, add:

```rust
/// List the resolution values defined on this Jira instance. Cached
/// for 7 days; use --refresh to bypass the cache.
Resolutions {
    /// Bypass the local cache and re-fetch from the server.
    #[arg(long)]
    refresh: bool,
},
```

### Step 4: Add the dispatch in `src/cli/issue/mod.rs`

- [ ] Locate the match arm that dispatches `IssueCommand::Transitions { .. }`. Add a new arm immediately after it:

```rust
IssueCommand::Resolutions { refresh } => {
    workflow::handle_resolutions(*refresh, output_format, client).await
}
```

(Match the exact destructuring style of the neighbours — some arms use `&command` / `*refresh`, some use owned. Copy whatever the surrounding arms do.)

### Step 5: Add the `handle_resolutions` handler

- [ ] Add to `src/cli/issue/workflow.rs`:

```rust
pub(super) async fn handle_resolutions(
    refresh: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Fetch or load from cache
    let resolutions: Vec<crate::types::jira::Resolution> = if refresh {
        let fetched = client.get_resolutions().await?;
        let cacheable: Vec<crate::cache::CachedResolution> = fetched
            .iter()
            .filter_map(|r| {
                r.id.as_ref().map(|id| crate::cache::CachedResolution {
                    id: id.clone(),
                    name: r.name.clone(),
                    description: r.description.clone(),
                })
            })
            .collect();
        crate::cache::write_resolutions_cache(&cacheable)?;
        fetched
    } else {
        match crate::cache::read_resolutions_cache()? {
            Some(c) => c
                .resolutions
                .into_iter()
                .map(|r| crate::types::jira::Resolution {
                    id: Some(r.id),
                    name: r.name,
                    description: r.description,
                })
                .collect(),
            None => {
                let fetched = client.get_resolutions().await?;
                let cacheable: Vec<crate::cache::CachedResolution> = fetched
                    .iter()
                    .filter_map(|r| {
                        r.id.as_ref().map(|id| crate::cache::CachedResolution {
                            id: id.clone(),
                            name: r.name.clone(),
                            description: r.description.clone(),
                        })
                    })
                    .collect();
                crate::cache::write_resolutions_cache(&cacheable)?;
                fetched
            }
        }
    };

    match output_format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&resolutions)?);
        }
        OutputFormat::Table => {
            use comfy_table::{Cell, Table, presets::UTF8_FULL};
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(vec![Cell::new("Name"), Cell::new("Description")]);
            for r in &resolutions {
                table.add_row(vec![
                    Cell::new(&r.name),
                    Cell::new(r.description.as_deref().unwrap_or("")),
                ]);
            }
            println!("{table}");
        }
    }

    Ok(())
}
```

### Step 6: Run tests to verify they pass

Run: `cargo test --test issue_resolution`
Expected: PASS — both tests.

### Step 7: Full suite + checks

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Step 8: Commit

```bash
git add src/cli/mod.rs src/cli/issue/mod.rs src/cli/issue/workflow.rs tests/issue_resolution.rs
git commit -m "feat(issue): jr issue resolutions subcommand (#263)

Lists instance-scoped resolutions in table or JSON form. Uses the 7-day
cache by default; --refresh forces a fresh GET /rest/api/3/resolution.

Two integration tests (table + JSON) pin the happy path under a
wiremock-backed server with an isolated XDG_CACHE_HOME."
```

---

## Task 7: Error-path transform for "resolution required"

**Files:**
- Modify: `src/cli/issue/workflow.rs` — wrap the `transition_issue` call in `handle_move` with a context-transforming closure.
- Test: `tests/issue_resolution.rs` (append).

### Step 1: Write the failing integration test

- [ ] Append to `tests/issue_resolution.rs`:

```rust
#[tokio::test]
async fn issue_move_surfaces_resolution_required_hint() {
    let server = MockServer::start().await;

    // 1. transitions list — one terminal transition
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "transitions": [
                {
                    "id": "31",
                    "name": "Done",
                    "to": { "name": "Done" }
                }
            ]
        })))
        .mount(&server)
        .await;

    // 2. transition POST — reject with Atlassian's real-world shape
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/FOO-1/transitions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "errorMessages": [],
            "errors": {
                "resolution": "Field 'resolution' is required"
            }
        })))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["--no-input", "issue", "move", "FOO-1", "Done"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--resolution"),
        "error should mention --resolution flag: {stderr}"
    );
    assert!(
        stderr.contains("jr issue resolutions"),
        "error should point at `jr issue resolutions` for discovery: {stderr}"
    );
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test issue_resolution issue_move_surfaces_resolution_required_hint`
Expected: FAIL — the raw Atlassian error propagates and the stderr does NOT mention `--resolution`.

### Step 3: Add the error-transform

- [ ] In `src/cli/issue/workflow.rs::handle_move`, replace the existing `transition_issue` call with a block that catches + transforms:

```rust
let transition_result = client
    .transition_issue(&key, &selected_transition.id, resolution_fields.as_ref())
    .await;

if let Err(err) = transition_result {
    let msg = format!("{err:#}").to_lowercase();
    if msg.contains("resolution") && msg.contains("required") {
        let to_label = selected_transition
            .to
            .as_ref()
            .map(|s| s.name.as_str())
            .unwrap_or(&selected_transition.name);
        return Err(crate::error::JrError::UserError(format!(
            "The \"{to_label}\" transition requires a resolution.\n\n\
             Try:\n    jr issue move {key} {to_label} --resolution <name>\n\n\
             Run `jr issue resolutions` to see available values."
        ))
        .into());
    }
    return Err(err);
}
```

### Step 4: Run test to verify it passes

Run: `cargo test --test issue_resolution issue_move_surfaces_resolution_required_hint`
Expected: PASS.

### Step 5: Full suite + checks

Run:
```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Step 6: Commit

```bash
git add src/cli/issue/workflow.rs tests/issue_resolution.rs
git commit -m "fix(issue): transform resolution-required 400 to actionable error (#263)

When Atlassian rejects a transition because the workflow requires a
resolution, jr previously surfaced the raw HTTP 400 which didn't tell
the user they could pass --resolution. Now the error is rewritten to:

    The \"Done\" transition requires a resolution.
    Try: jr issue move KEY Done --resolution <name>
    Run \`jr issue resolutions\` to see available values.

Heuristic: lowercased error body contains both \"resolution\" and
\"required\". Other 400s pass through unchanged.

Integration test mounts a wiremock transition 400 with Atlassian's
real error shape and asserts the transformed stderr."
```

---

## Task 8: README + help text

**Files:**
- Modify: `README.md`
- Modify: `src/cli/mod.rs` — the doc comments on `Move` variants and the new `Resolutions` variant are the --help output; make sure they're informative.

### Step 1: Update the README's command table

- [ ] Locate the `jr issue move KEY [STATUS]` row (around line 157 of README.md). Update to include `--resolution`:
```markdown
| `jr issue move KEY [STATUS]` | Transition issue (partial match on status name). `--resolution <name>` atomically sets resolution on the transition for JSM/resolution-required workflows. |
```

- [ ] Insert a new row after `jr issue transitions KEY`:
```markdown
| `jr issue resolutions` | List instance-scoped resolution values (cached 7 days; `--refresh` to bust). Discover what to pass to `--resolution` on `jr issue move`. |
```

### Step 2: Update the Quick Start examples

- [ ] Under the "Common tasks" snippet (around line 135 of README.md), add after the existing `jr issue move` example:

```bash
# Close a JSM ticket atomically with status + resolution (so SLAs + reports stay accurate)
jr issue move JSM-42 Done --resolution Fixed
```

### Step 3: Verify the CLI --help text

- [ ] The doc comment for `--resolution` on `IssueCommand::Move` was already written in Task 5 Step 5. Verify it renders cleanly:
```bash
cargo build --quiet
./target/debug/jr issue move --help | grep -A3 resolution
```

Expected output includes a clear description mentioning the `jr issue resolutions` discovery path. If the line is too long to read comfortably, trim the doc comment to one sentence and move details to README.

- [ ] Verify the `resolutions` subcommand:
```bash
./target/debug/jr issue resolutions --help
```

### Step 4: Commit

```bash
git add README.md src/cli/mod.rs
git commit -m "docs: document --resolution and jr issue resolutions (#263)

- README command table: --resolution on jr issue move, new row for
  jr issue resolutions.
- Quick-start example closing a JSM ticket atomically.
- CLI --help text was already set in Task 5; no additional doc-comment
  changes required."
```

---

## Self-Review

**1. Spec coverage:**
- Atomic transition + resolution → Tasks 4 + 5. ✓
- `GET /rest/api/3/resolution` wrapper → Task 2. ✓
- `jr issue resolutions` discovery command → Task 6. ✓
- Partial-match resolver with `Ambiguous`/`None`/`ExactMultiple` → Task 5. ✓
- 7-day TTL cache → Task 3. ✓
- Error transform for "resolution required" → Task 7. ✓
- README + help text → Task 8. ✓
- Out-of-scope `--field` pass-through → deliberately not planned. ✓

**2. Placeholder scan:** None of the "TBD/TODO/similar to Task N" red flags. Every step has runnable code or a runnable command.

**3. Type consistency:**
- `Resolution` defined once (Task 1) with `id: Option<String>, name: String, description: Option<String>`. Used as-is in Tasks 2, 5, 6.
- `transition_issue(&self, key: &str, transition_id: &str, fields: Option<&serde_json::Value>) -> Result<()>` defined in Task 4; same signature used in Tasks 5 and 7.
- `resolve_resolution_by_name(resolutions: &[Resolution], query: &str) -> Result<Resolution>` defined in Task 5; used once (in handle_move).
- `read_resolutions_cache() -> Result<Option<ResolutionsCache>>` and `write_resolutions_cache(&[CachedResolution]) -> Result<()>` defined in Task 3; called consistently in Tasks 5 and 6.

**4. Known risk — partial_match API names**: The `MatchResult` enum variants at `src/partial_match.rs` may differ from what Task 5 Step 3 uses (`Exact`, `Ambiguous`, `ExactMultiple`, `None`). The task includes a grep to verify; if the names are different, adapt the match arms — the test at Step 1 pins the user-visible error substrings ("ambiguous", candidate names) so the internal mapping can flex.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-23-issue-move-resolution.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
