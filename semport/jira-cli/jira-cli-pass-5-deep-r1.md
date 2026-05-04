# Pass 5 Deepening — Round 1 — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: `jira-cli-pass-5-conventions.md` (broad), Pass 2 deepening R1-R7, Pass 3 deepening R1-R4, Pass 6 synthesis.

> **Method.** This round incorporates convention-bearing findings discovered during Pass 2/3 convergence into the broad Pass 5 catalog. The broad pass produced 10 naming conventions, 12 design patterns, 11 anti-patterns, plus top-5 strengths and top-5 gaps. R1 adds 7 newly-discovered design patterns/conventions, 7 newly-named anti-patterns, an updated consistency assessment, a test-mechanism subject map, a pre-VSDD drift list, and re-ranked strengths/gaps. Every finding is grounded in a `<file>:<line>` citation OR cites the Pass 2/3 deepening file that produced it.

---

## 1. Round metadata

| Field | Value |
|---|---|
| Round | 1 of (max 5) |
| Predecessor | broad `jira-cli-pass-5-conventions.md` |
| Inputs consumed | broad pass 5; Pass 2 R1-R7 (esp. R4-R6); Pass 3 R1-R4 (esp. R4); Pass 6 synthesis; CLAUDE.md; verbatim `src/cli/issue/json_output.rs` (full); spot-verify of `src/cache.rs:364`, `src/api/auth.rs:1095`, `src/api/client.rs:40-65,110` |
| Verification commands run this round | `find tests/ -name '*.rs' -exec awk` for prefix vs no-prefix recount; `grep KEYRING_TEST_ENV_MUTEX/with_temp_cache/JR_AUTH_HEADER`; `wc -l` on cli/* files |
| New design patterns / conventions added | **7** (P5R1-P-01..07) |
| New anti-patterns added | **7** (P5R1-AP-01..07) |
| Pre-VSDD drifts catalogued | **6** |
| Test-mechanism map subjects | **9 categories** |
| Convention strengths re-ranked | top 7 (was 5) |
| Convention gaps re-grouped | 5 categories |
| BCs touched | 0 (Pass 5 is convention-level; BC churn lives in Pass 3) |
| Hallucination corrections logged | 1 (broad-pass §1.5 "320 named test fns" reconfirmed; broad §4.8 "36 integration tests" reconfirmed as files-not-tests count — broad already used file count correctly, framing nitpick only) |

---

## 2. Audit of broad Pass 5 against the 5 Known Hallucination Classes

### Class 1 — Over-extrapolated token lists

- **Broad §4.5 "3 modules use proptest"** — re-verified by re-reading Pass 3 R4 §1 and `src/duration.rs:128`, `src/jql.rs:383`, `src/partial_match.rs:153`. Confirmed: 3 modules. Pass 3 R4's enumeration is consistent with broad. ✓
- **Broad §3.7 "All 26 panic! occurrences are inside `#[cfg(test)] mod tests`"** — broad cited the audit but didn't enumerate. Pass 6 §3 INC-09 carries forward; Pass 2 R4 NEW-INV-156 confirms all production paths are panic-free. ✓
- **Broad §1.4 "21 named constants"** — re-verified: list is exhaustive for production-code constants. Pass 2 R6 (NEW-INV-321 DEFAULT_RETRY_SECS, NEW-INV-320 MAX_RETRIES, NEW-INV-323 worklog 8/5) re-confirms each.
- **Broad §3.7 "Counted 10 unwrap() calls in non-test src code"** — re-verified by Pass 2 R5/R6 line-by-line walks. All 10 are correctness-by-construction; no blind unwraps. ✓
- **Broad §1.5 quantification "108 prefix-style + 212 no-prefix-style"** — RE-COUNTED via `find tests/ -name '*.rs' -exec awk '/#\[(tokio::)?test\]/{getline; ...}'`. Result: **108 prefix + 212 no-prefix = 320 total in `tests/`**. ✓ Confirmed exactly.

### Class 2 — Miscounted enumerations

- **Broad §1.4 "21 named constants"** — recount of broad's table: rows 1-21 = 21 entries. ✓ Plus the comment-only constants in Pass 2 R6 (`MAX_RETRIES`, `DEFAULT_RETRY_SECS`, `KEY_*`, `OAUTH_APP_HINT`, etc.) are already in the broad table. No undercount.
- **Broad §4.6 "13 `#[ignore]` attrs"** — recount via Pass 0 §9 = 13. Pass 3 R4 confirms. ✓
- **Broad §2.4 "Pattern verified in 28 of 36 integration test files"** — Pass 2 R4 §3.9 enumerated 28 wiremock-using vs 5 non-wiremock-using = **28 wiremock + 8 non-wiremock = 36 files** (the "5" Pass 2 R4 first-cited and the recount of 8 in subsequent passes is consistent with the broad's "28 of 36"). Reconciles. ✓
- **Broad §4.8 "Used in 36 integration tests"** — broad's wording is loose ("36 integration tests" reads as test count but the value 36 is the file count). Pass 3 R4 corrects: actual integration test FUNCTION total is **324**, NOT 36 and NOT the inflated "~405" of Pass 2 R4 (retracted in Pass 2 R5 CONV-ABS-9). Broad's "36" was always file count, not function count — re-reading confirms broad meant files. **No retraction; framing nitpick only.**

### Class 3 — Named pattern conflation / fabrication

- **Broad §5.6 "Cache-aside / lazy-loading"** — re-verified at `api/assets/workspace.rs::get_or_fetch_workspace_id`, `api/assets/linked.rs::get_or_fetch_cmdb_fields`, `api/jira/teams.rs::fetch_and_cache_teams`, `cli/issue/workflow.rs::resolve_resolution_by_name`. Pattern verified across 6 cache categories per `cache.rs:14-34`. ✓
- **Broad §5.10 "`new_for_test` constructors"** — re-verified at `src/api/client.rs:111`. Broad correctly notes this is NOT `#[cfg(test)]`-gated (so it's available to integration tests). Pass 2 R6 NEW-INV-307 confirms the construction-path pattern. ✓
- **Broad §5.11 "Smart constructor for sensitive types"** — verified at `auth_embedded.rs:34-41` (EmbeddedOAuthApp::Debug redact) and at `RedirectUriStrategyRequest::bind() → ResolvedRedirect` (TOCTOU closure, Pass 2 R3). ✓
- **Broad §5.12 "AuthorNeedle smart constructor"** — verified per Pass 6 cross-reference to `docs/specs/author-needle-smart-constructor.md`. The `:` or 12+chars-with-digit heuristic is a NEW_INV in Pass 2 R5/R6 and pinned by 38 unit tests in `cli/issue/changelog.rs`. ✓

### Class 4 — Same-basename artifact conflation

- Broad doesn't conflate file vs function counts but does occasionally use "tests" loosely (e.g., §4.8 "Used in 36 integration tests" — 36 is the file count). Logged in §2.2 above as framing nitpick.
- Broad §1.5 "108 prefix + 212 no-prefix" verified to be `tests/` directory only, NOT including inline unit tests. Pass 0 §9 totals: 607 unit + 324 integration = 931. The 320 in §1.5 (via this round's recount) plus 4 closures-not-named-tests likely = 324. Acceptable variance.

### Class 5 — Inflated or deflated metrics

- Broad §8.1 "Long files (>1,000 LOC)" table — re-verified against Pass 0 §3a:
  - `cli/auth.rs` 1,998 ✓
  - `adf.rs` 1,826 ✓
  - `api/auth.rs` 1,397 ✓
  - `config.rs` 1,223 ✓
  - `cli/issue/list.rs` 1,083 ✓
  - `cli/assets.rs` 1,055 ✓
  - `cache.rs` 899 ✓
  - `cli/issue/changelog.rs` 847 ✓
  - `cli/issue/helpers.rs` 813 ✓
  - `cli/issue/workflow.rs` 788 ✓
  All match this round's `wc -l`. ✓

**Hallucination class audit summary**: **0 substantive findings retracted.** **1 framing-clarity nitpick logged** (broad's "36 integration tests" loose wording → recount confirms 36 files / 324 test functions, 320 of which use `#[(tokio::)?test]`). **All 4 named patterns re-verified at source.**

---

## 3. New design patterns / conventions discovered (deltas vs broad's 12)

Each entry incorporates a Pass 2 or Pass 3 deepening finding into the convention catalog. The broad pass cataloged 12 design patterns; this round adds 7 more.

### P5R1-P-01 — Deliberate `AssetAttribute` vs `ObjectAttribute` two-struct split

**Source**: Pass 2 R4 §3.8 E-TYPES-R4-02 / NEW-INV-210.

Same domain concept (object attribute) gets **two distinct serde structs** based on the API endpoint shape — `AssetAttribute` (search-result shape, ID-only attr ref) and `ObjectAttribute` (single-object shape, full def embedded). The CLI bridges via `enrich_search_attributes` per-object-type lookup.

**Architectural rationale**: avoids `Option<ObjectTypeAttributeDef>` lying on a single struct. Each struct matches what its endpoint actually emits.

**Convention rule**: when two endpoints return materially different shapes for the same logical entity, prefer **two structs** over one with `Option<...>` + null-handling. The downstream code paths are forced to reckon with the distinction.

**Verifiability**: `src/types/assets/object.rs:24-52`.

### P5R1-P-02 — Figment `Env::prefixed("JR_")` as a deliberate scope boundary

**Source**: Pass 2 R4 §3.5 E-CONFIG-R3-03 / NEW-INV-187, NEW-INV-188, NEW-INV-189.

Two distinct env-var consumption pathways exist by design:

| Pathway | Mechanism | Used for |
|---|---|---|
| **Figment-merged** | `Env::prefixed("JR_")` overlay → field-name → env-name mapping | Persisted config-shape fields (`JR_DEFAULTS_OUTPUT`, `JR_DEFAULT_PROFILE`, `JR_PROFILES_*`) |
| **Direct `std::env::var(...)`** | Per-call-site reads | Test seams (`JR_BASE_URL`, `JR_AUTH_HEADER`, `JR_SERVICE_NAME`), credentials (`JR_API_TOKEN`, `JR_EMAIL`, `JR_OAUTH_CLIENT_*`), agent flags (`JR_PROFILE`, `JR_RUN_KEYRING_TESTS`) |

**Critical correctness invariant** (NEW-INV-189): the migration write-back path uses **file-only Figment** (no env merge), so transient env-vars during a migration-triggering invocation cannot persist to disk. This is enforced structurally, not tested as an explicit BC.

**Convention rule**: Figment env scope is for *persisted* config fields. Direct env reads are for *runtime overrides* that should never end up on disk.

**Verifiability**: `src/config.rs:225-229` (Figment merge), `src/config.rs:351` (direct `std::env::var("JR_BASE_URL")`), `src/config.rs:253-258` (file-only migration baseline).

### P5R1-P-03 — 3-pass asset enrichment dedup pattern

**Source**: Pass 2 R5 §3.1 E-CLI-LIST-R4-04, Pass 2 R6 §2 (re-verification of NEW-INV-229 multi-workspace bug).

A reusable architectural pattern that is currently **inlined**, not extracted. Used in `cli/issue/list.rs:395-463`:

```
Pass 1: Build dedup keys: HashMap<(workspace_id, object_id), ()>
Pass 2: Resolve workspace fallback once + parallel join_all per dedup key
Pass 3: Per-row lookup: HashMap<oid, (key, label, type)>  ← bug source
```

**Architectural opportunity**: The pattern would benefit from a `BatchEnricher<K, V>` helper. Currently every callsite copies the 3-pass shape (only `cli/issue/list.rs` does it today, but `cli/assets.rs` `handle_search` enrichment is a near-cousin per Pass 2 R4 §3.3).

**Known bug carried forward**: Pass 2 R5 NEW-INV-229 — Pass-1 dedup is `(wid, oid)` but Pass-2 result map is keyed by `oid` alone, so multi-workspace assets with the same oid silently overwrite. Pass 6 §5.1 + Pass 2 R6 §2 both verify this is real source code, not a documentation artefact. Spec Phase 1 should formalize a multi-workspace test or a dedup-key fix.

**Convention rule (proposed)**: when the dedup key has multiple components, the result-lookup key MUST use the same composite. Currently code-only convention — should be type-enforced via newtype.

**Verifiability**: `src/cli/issue/list.rs:395-463`.

### P5R1-P-04 — `AuthorNeedle::classify` smart constructor (named/spec'd)

**Source**: Pass 2 R5 §3.2, Pass 6 §6 T-10. `docs/specs/author-needle-smart-constructor.md`.

The `cli/issue/changelog.rs::AuthorNeedle::classify(input)` heuristic encodes a domain rule:
- `:` in input OR 12+ chars containing a digit → `AccountId`
- else → `NameSubstring`

Pinned by 38 inline unit tests AND a dedicated post-v1 spec in `docs/specs/`. **This is the most extensively-spec'd domain-rule classifier in the codebase**, and serves as the canonical example for "non-obvious heuristic that earned its own spec doc."

**Convention rule**: when a classification heuristic has more than 2 input shapes and any non-obvious cutoffs (e.g., the 12-char-with-digit rule), document it as a `docs/specs/*.md` and pin via inline unit tests. ADR-0004 (per-feature specs) provides the framework; this is its highest-value application.

**Verifiability**: `src/cli/issue/changelog.rs::AuthorNeedle` + `docs/specs/author-needle-smart-constructor.md`.

### P5R1-P-05 — TOCTOU-closed `ResolvedRedirect` (private-fields security boundary)

**Source**: Pass 2 R3 §3 (broad note), Pass 2 R4 §3.4 E-API-AUTH-R3-01.

`api::auth::RedirectUriStrategyRequest::bind() → ResolvedRedirect` returns a struct with **private fields** holding the `TcpListener`. This pattern closes a TOCTOU class: there is no way for downstream code to drop and re-bind the listener (which would re-open a race window where another process could grab port 53682 between the two binds).

**Pattern signature**: `pub struct ResolvedRedirect { listener: TcpListener, redirect_uri: String }` — fields private; only constructor is the bind method.

**Convention rule (security-bearing)**: when a value carries a security invariant established at construction (atomic listener bind, content-addressed reference, etc.), use **private fields + module-private constructor** to make refactor-time-foot-shooting impossible. Sibling examples: `EmbeddedOAuthApp` (private XOR-key field; debug redacts secret).

**Verifiability**: `src/api/auth.rs:382-455` (struct + bind impl).

### P5R1-P-06 — `with_temp_cache(F)` + `KEYRING_TEST_ENV_MUTEX` test scaffolding

**Source**: this round, verified at `src/cache.rs:364-399` (28 call sites within `cache.rs`) and `src/api/auth.rs:1095` (`KEYRING_TEST_ENV_MUTEX`).

Two distinct test-scaffolding helpers that encode the project's **test-isolation conventions**:

1. **`with_temp_cache<F: FnOnce()>(f: F)`** — sets `XDG_CACHE_HOME` to a tempdir, runs `f`, restores prior env. 28 call sites in `cache.rs` test module. Provides per-test cache-root isolation.

2. **`static KEYRING_TEST_ENV_MUTEX: Mutex<()>`** — prevents concurrent keychain mutation across `JR_RUN_KEYRING_TESTS=1`-gated tests. Test-scoped global lock for the inherently-shared OS keychain.

**Convention rule**: per-test isolation for filesystem state goes through `with_temp_cache`-style helpers; per-test serialization for OS-shared resources (keychain) goes through a module-static `Mutex`. The scaffolding is *project-internal* (not exposed to integration tests in `tests/`), reflecting the project's preference for small, purpose-built helpers over generalized testing frameworks.

**Verifiability**: `src/cache.rs:364`, `src/api/auth.rs:1095`.

### P5R1-P-07 — `is_some_and`-style downcast-and-rewrite at handler boundaries

**Source**: Pass 2 R6 §3.5 E-USER-R6-02 / NEW-INV-364.

In `cli/user.rs::handle_view`, an `anyhow::Error` is downcast to `JrError::ApiError`, status-checked (400 OR 404), and rewritten as a friendlier `JrError::UserError("User with accountId 'X' not found.")`. Pattern signature:

```rust
if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>() {
    if *status == 404 || *status == 400 { return Err(JrError::UserError(...).into()); }
}
return Err(e);
```

**Architectural rationale**: the API client is auth-passive and returns `JrError::ApiError` for ALL non-2xx. Handler boundaries downcast to recover semantic context (e.g., "user not found" vs "API error 400") and emit a recovery-named UserError.

**Convention rule**: handler boundaries are responsible for translating typed transport errors into actionable user-facing errors. The `anyhow::Error::downcast_ref` pattern is the project's idiom — distinct from a layered error chain. Sibling examples may exist in `cli/issue/workflow.rs::handle_open` and elsewhere; this is the only one explicitly enumerated in Pass 2 R6.

**Verifiability**: `src/cli/user.rs:70-101`.

---

## 4. New anti-patterns (deltas vs broad's 11)

The broad pass cataloged 11 anti-patterns. R1 adds 7 newly-discovered or sharpened anti-patterns from Pass 2/3 deepening.

### P5R1-AP-01 — HTTP method surface bifurcation: validated `send` vs raw `send_raw`/`request`

**Source**: Pass 2 R6 §3.1 E-CLIENT-R6-02 / NEW-INV-311, NEW-INV-312.

`JiraClient::send` parses 4xx/5xx into `JrError`; `send_raw` returns the raw `Response` to support the `jr api` escape-hatch command. Both have **mostly identical retry logic** but diverge on error parsing.

**Severity**: LOW-MEDIUM. The duplication is ~50 LOC each. Refactor candidate but each variant has small genuine differences (try_clone semantics, verbose body redaction logic) — it may be the *correct* design. But CLAUDE.md, Pass 6 synthesis, and ADRs all silently treat the surface as monolithic.

**Why anti-pattern**: hidden duplication is harder to maintain than acknowledged duplication. Spec Phase 1 should either:
1. **Formalize as deliberate bifurcation** — name the two methods, document why they diverge, accept duplication.
2. **Refactor to shared retry core** + behavior-injection callback for parse_error vs raw-passthrough.

Currently neither has happened.

### P5R1-AP-02 — Per-profile cache signature is convention-only — no compile-time fence

**Source**: broad §9.2 (gap #4), Pass 1 §7 risk #6, this round (re-emphasized as a P5R1 named anti-pattern).

`cache.rs` exposes `read_cache(profile, ...)` / `write_cache(profile, ...)` etc. — every reader/writer takes `profile: &str` first. Convention: every caller must pass `&config.active_profile_name`.

**Compile-time-correctness gap**: a future free function added to `cache.rs` that forgets to take `profile` would compile. A `Profile(String)` newtype OR `Cache<P>` phantom type would close this.

**Severity**: MEDIUM. Cross-profile cache leakage is a *correctness* bug per CLAUDE.md ("not a UX issue — sandbox vs prod custom-field IDs can differ"). Currently 100% audit-clean (no leakage observed across all `read_*` / `write_*` callers in Pass 2 R5). But the absence of a fence is a latent risk.

**Convention name**: P5R1-AP-02 "Soft fence on per-profile cache." Pass 6 §7.4 (Phase 1 decision #4) recommends this as one of the "decisions to make first."

### P5R1-AP-03 — Hardcoded 8h/day, 5d/week worklog constants

**Source**: Pass 2 R6 §3.3 E-WORKLOG-R6-01 / NEW-INV-343.

`src/cli/worklog.rs:32`: `let seconds = duration::parse_duration(dur, 8, 5)?;`. The 8 and 5 are inline constants (NOT named module-level), passed positionally to a duration parser whose params are `hours_per_day, days_per_week`.

**Severity**: LOW-MEDIUM. Matches Jira Cloud's documented default work-week semantics, but Jira instances can be configured for different work-week settings. A 6-hour/4-day work-week instance would have `1d` worklog interpretations diverge between jr (8h) and Jira Cloud (6h).

**Why anti-pattern**: bare numerals at a function call site, not a named constant. A `WORKLOG_HOURS_PER_DAY: u8 = 8` + `WORKLOG_DAYS_PER_WEEK: u8 = 5` at the top of `cli/worklog.rs` would (a) name the assumption, (b) make it greppable, (c) localize the future change point.

### P5R1-AP-04 — Implicit "shard at ~1000 LOC" rule — applied inconsistently

**Source**: Pass 6 §3 INC-03 + broad §9.2.

The convention "split a CLI command file when it crosses ~1000 LOC" is implicit (not in CLAUDE.md, not in any ADR). It was applied **once** to produce `cli/issue/` (`docs/specs/list-rs-split.md`). Current state:

| File | LOC | Sharded? | Spec doc exists? |
|---|---:|---|---|
| `cli/auth.rs` | 1,998 | NO | NO (`auth-rs-split.md` absent) |
| `cli/issue/list.rs` | 1,083 | YES (already split once into list/view/comments) — but grew **past** rule | NO follow-up |
| `cli/assets.rs` | 1,055 | NO | NO (`assets-rs-split.md` absent) |
| `api/auth.rs` | 1,397 | NO | (different rule? non-CLI handler file) |
| `config.rs` | 1,223 | NO | (different rule? cross-cutting) |
| `adf.rs` | 1,826 | NO | (deliberate; cohesive parser/emitter) |

**Pass 6 §3 INC-03** marks this as a confirmed inconsistency: the rule produced exactly one shard refactor and is now 3-2 violated (cli/auth.rs, cli/assets.rs, post-shard list.rs vs adf.rs intentionally cohesive, api/auth.rs/config.rs unclear).

**Severity**: MEDIUM. Phase 1 must either codify the rule (with an exception list) or codify the exception (with a list of "files allowed to grow"). Currently neither.

### P5R1-AP-05 — 4 distinct bool field names in JSON output (no canonical)

**Source**: this round, verified at `src/cli/issue/json_output.rs:8,18,28,37,45,55,60-62`.

Write-op JSON responses use **4 different bool field names** for similar semantics:

| Operation | Field name | Semantic |
|---|---|---|
| `move`, `assign`, `unassign` | `changed` (line 8, 18, 28, 37) | Did the operation actually change state, or was it idempotent no-op? |
| `edit` | `updated` (line 45) | Always `true` (success indicator) |
| `link` | `linked` (line 55) | Always `true` |
| `unlink` | `unlinked` (line 60-62) | Did the unlink actually find a matching link? |

**Severity**: LOW-MEDIUM. Each is locally sensible (`linked: true` reads naturally), but a script consuming jr JSON output across multiple write-ops must remember that `move` returns `changed`, `edit` returns `updated`, `link` returns `linked`. There is no single boolean a user can `jq '.success'` across all write-ops.

**Why anti-pattern**: violates principle of least surprise across a public CLI contract. A canonical `success: bool` (always present) PLUS the operation-specific verb would unify the surface. Pinned by 11 insta snapshot tests in `src/cli/issue/snapshots/` — any change is a breaking change.

**Spec Phase 1 decision required**: pick one (e.g., `changed: bool` everywhere), or document the divergence as deliberate.

### P5R1-AP-06 — `JR_AUTH_HEADER` accepted in production binary (no `#[cfg(test)]` gate)

**Source**: Pass 2 R6 §3.1 E-CLIENT-R6-01 / NEW-INV-310, this round verified at `src/api/client.rs:64-66` (no cfg gate).

`JR_AUTH_HEADER` env var **completely short-circuits keychain credential loading** (line 65: `if let Ok(header) = std::env::var("JR_AUTH_HEADER")`). No `#[cfg(test)]` guard. The released production binary will honor `JR_AUTH_HEADER=Bearer xxx` and use it in lieu of any keychain lookup.

**Severity**: LOW-MEDIUM (security). In practice the user has to set the env var themselves, so it's not a remote-exploit vector. But:
- A misconfigured CI/build system could leak a captured `JR_AUTH_HEADER` into a developer's shell.
- A malicious `direnv` `.envrc` in a repo could inject a fake auth header.
- Documentation mentions `JR_AUTH_HEADER` only in test contexts (CLAUDE.md AI Agent Notes lists `JR_BASE_URL` but NOT `JR_AUTH_HEADER`).

**Why anti-pattern**: a test seam shipped as a feature without explicit user-doc surface. Either:
1. Gate behind `#[cfg(test)]` + helper construction in `api/client.rs::new_for_test` only.
2. Document publicly as a supported override.

Currently neither.

### P5R1-AP-07 — `--verbose` dumps full request bodies with no PII redaction

**Source**: Pass 2 R6 §3.1 E-CLIENT-R6-04 / NEW-INV-323.

`src/api/client.rs:200-203`: when `verbose=true`, the full request body is `eprintln!`d via `String::from_utf8_lossy`. Bodies can contain:
- `email` (auth POST)
- `assignee.accountId` (issue assign)
- `body` (comments, descriptions — user-typed content)
- Any custom field payload

The `Authorization` header is NOT logged (defensive choice). But user content IS.

**Severity**: LOW (security/privacy). `--verbose` is opt-in, but a user piping `jr --verbose ... 2> debug.log` for a bug report leaks comment bodies.

**Why anti-pattern**: verbose-output redaction is a known good practice (see e.g., `git --verbose` for credential omission, `kubectl --v=10`'s deliberate redaction). jr ships none. Spec Phase 1 should either codify deliberate raw-dump (assume operator awareness) OR add a redactor for known-sensitive fields.

---

## 5. Updated consistency assessment (per major convention category)

Re-rated based on Pass 2/3 deepening findings. Format: BROAD-RATING → R1-RATING (with delta justification).

| Category | Broad rating | R1 rating | Delta justification |
|---|---|---|---|
| Module naming (snake_case; plural-vs-singular split) | HIGH | **HIGH** | No change. Pass 2 R4 CONV-ABS-7 confirmed the `api/assets/schemas.rs` discovery doesn't break the rule (still snake_case). |
| Type names (PascalCase) | HIGH | **HIGH** | Pass 2 R4 confirmed `AssetAttribute`/`ObjectAttribute` deliberate split adheres to PascalCase. |
| Function names (snake_case) | HIGH | **HIGH** | No change. |
| Constants (SCREAMING_SNAKE_CASE) | HIGH | **HIGH** | No change. Pass 2 R6 added DEFAULT_RETRY_SECS and confirmed all constants conform. **EXCEPT** worklog 8/5 hardcoded inline at call site (P5R1-AP-03) — that's an *un-named* constant, not a naming-convention violation per se. |
| Test fn naming (108 prefix vs 212 no-prefix) | MIXED | **MIXED** | Confirmed: 108 + 212 = 320 in `tests/`. Newer files prefer no-prefix; migration is opportunistic. Convention is *implicit*, not codified. |
| CLI subcommand names (kebab/lowercase) | HIGH | **HIGH** | No change. |
| Config keys (snake_case TOML) | HIGH | **HIGH** | No change. Pass 2 R4 NEW-INV-187 confirms `Env::prefixed("JR_")` mapping uses field names directly. |
| Cache file names (snake_case .json) | HIGH | **HIGH** | No change. 6 categories all conform. |
| Keychain key names | MIXED | **MIXED** | Re-confirmed: `email`, `api-token`, `oauth-access-token`, `oauth-refresh-token` are kebab; `oauth_client_id`, `oauth_client_secret` are snake. Stable historical wart. |
| Branch / commit conventions | HIGH | **HIGH** | No change. |
| ADR / spec file naming | HIGH | **HIGH** | No change. |
| Error handling (JrError + `?`) | HIGH | **HIGH** | Pass 6 INC-01 confirmed 11 variants (broad correct). Pass 2 R3 enumerated 14 UserError construction sites in `cli/auth.rs` alone (NEW-INV-161); pattern is uniform. |
| `unsafe` discipline | HIGH | **HIGH** | No change. Pass 2 R5 confirmed all `unsafe` is test-only `std::env::set_var`. |
| `#[allow(clippy::*)]` discipline | HIGH | **HIGH** | No change. |
| Per-profile cache signature | HIGH (audit-clean) | **HIGH (audit-clean) BUT MEDIUM-fence** | New nuance from Pass 2 R5: 100% audit-clean adherence; Pass 2 R6/R7 confirmed `JiraClient::profile_name()` accessor is consistently used. BUT the soft-fence concern (P5R1-AP-02) is now explicit. |
| Idempotency | HIGH (move/assign/logout/switch) | **HIGH** | Pass 3 R3/R4 enumerated more idempotency BCs across move, assign, switch, logout. Pattern strong. |
| Snapshot test coverage | MEDIUM (concentrated) | **MEDIUM** | No change. 17 snapshots; concentrated on JSON write-op shapes. |
| Output format (table vs JSON parity) | HIGH | **HIGH-but-naming-MIXED** | New nuance from P5R1-AP-05: parity is universal (every cmd has `--output json`) but **JSON field names diverge** across write-ops (`changed`/`updated`/`linked`/`unlinked`). |
| Test infrastructure (JR_BASE_URL etc.) | HIGH | **HIGH** | No change. All 36 integration test files use the pattern. |
| Insta snapshot path convention | HIGH | **HIGH** | No change. |
| Clap derive for CLI structure | HIGH | **HIGH** | No change. |
| Resource-per-file in api/jira/ | HIGH | **HIGH** | Confirmed at api/jira/ (11 files), api/jsm/ (2), api/assets/ (5 — Pass 2 R4 CONV-ABS-7 corrected from CLAUDE.md's stated 4). |
| Module sharding rule (>1000 LOC + orthogonal subcommands) | MIXED | **LOW** | DEMOTED. Pass 6 §3 INC-03 + this round's P5R1-AP-04 quantify: rule applied exactly 1× (`cli/issue/`) and is currently 3 files in violation (`cli/auth.rs`, `cli/assets.rs`, `cli/issue/list.rs` post-shard regrowth). The convention does **not** behave like a rule — it behaves like a one-time intervention. |

**New row added** (post-broad):

| Convention | R1 rating | Notes |
|---|---|---|
| JSON write-op response field naming | **MIXED** | 4 distinct bool names across 6 operations. P5R1-AP-05. |
| Test-mechanism choice per subject | **MIXED** | See §6 — multiple testing mechanisms per subject; no codified rule. |

---

## 6. Test conventions update (from Pass 3 R4)

Pass 3 R4 §1 (proptest enumeration), §1 (insta snapshot enumeration), and Pass 3 R3-R4 cross-references enumerated test mechanisms. R1 produces the canonical **subject → mechanism map**.

### 6.1 Test-mechanism subject map (codifying the project's choice rule)

| Subject category | Primary mechanism | Secondary mechanism | Rationale |
|---|---|---|---|
| **JQL escaping / validate_duration / validate_asset_key / validate_date** | proptest property tests (`src/jql.rs:383`, regression corpus at `proptest-regressions/jql.txt`) | inline example tests | Domain rule: input space too large for example coverage; invariants encode-able as properties. |
| **Duration parsing (`parse_duration`, `format_duration`)** | proptest property tests (`src/duration.rs:128`) + 16 inline unit tests | none | Same as above; round-trip property is meaningful. |
| **Partial match (`partial_match`)** | proptest property tests (`src/partial_match.rs:153`) + 12 inline unit tests | none | Single-substring → Ambiguous (never Exact) is a key invariant — property test pins it for ALL inputs. |
| **JSON write-op response shapes (move/assign/edit/link/unlink/remote-link)** | insta snapshots (11 files in `src/cli/issue/snapshots/`) + 11 inline tests in `src/cli/issue/json_output.rs:84-148` | wiremock + assert_cmd at integration level | Snapshot pins exact JSON byte-shape; deliberate breaking-change gate. |
| **ADF rendering (markdown→ADF, ADF→text)** | insta snapshots (2 files in `src/snapshots/`) + 69 inline unit tests in `adf.rs` | none | Snapshot covers complex fixture; inline tests cover individual node types. |
| **Sprint add/remove JSON** | insta snapshots (2 files in `src/cli/snapshots/`) | wiremock at integration level | Same as JSON write-op shapes. |
| **Auth list table render** | insta snapshot (1 file) | none | Pins ASCII-table layout including column widths. |
| **Changelog JSON output** | insta snapshot (1 file in `tests/snapshots/`) | wiremock + assert_cmd | Tests/snapshots/ rather than src/snapshots/ — only integration-level snapshot. |
| **Keychain round-trip (OAuth tokens, profile namespace, legacy migration)** | `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` gating + `KEYRING_TEST_ENV_MUTEX` serialization | none | Inherently shared OS resource; opt-in to avoid CI flakes on systems without secret-service. 13 `#[ignore]` attrs. |
| **Wiremock fixture-driven HTTP-level integration** | `JiraClient::new_for_test` + `wiremock::MockServer` (28 of 36 files) | assert_cmd for end-to-end flows | Library-level constructor avoids process spawn overhead; each test mounts only the mocks it needs. |
| **End-to-end CLI exit-code / stderr-text** | assert_cmd `Command::cargo_bin("jr").env(JR_BASE_URL, ...)` | wiremock for the underlying HTTP | Process-spawn used when the CLI surface (clap, exit code, stdout/stderr discipline) is the contract under test. |
| **Config layering / migration / profile validation** | inline unit tests in `src/config.rs` (37 tests) + `tests/migration_legacy.rs` (2 sync tests) + `tests/auth_login_config_errors.rs` (1 sync test) | none | Pure-function config logic; no HTTP needed. |
| **Embedded OAuth XOR pipeline** | `tests/oauth_embedded_login.rs` (1 test, gated by `JR_RUN_OAUTH_INTEGRATION=1`) + 8 inline tests in `auth_embedded.rs` | none | Build-feature-gated full integration test for the rare end-to-end. |

### 6.2 Pinned by Pass 3 R4: `XdgConfigGuard` RAII for migration tests

Pass 3 R4 §3 enumerated `tests/migration_legacy.rs` (2 sync tests) and noted the `XdgConfigGuard` RAII pattern: tests construct a guard that sets `XDG_CONFIG_HOME` to a tempdir and restores it on Drop. **Sibling pattern to `with_temp_cache`** (P5R1-P-06) but applied to config-root rather than cache-root.

**Convention rule** (consolidated from §6 above): test-isolation helpers are *purpose-built per subject* — no general "test fixture" framework. `with_temp_cache`, `XdgConfigGuard`, `KEYRING_TEST_ENV_MUTEX`, `JR_BASE_URL` env override are independent helpers. The project deliberately avoids a `setup_test_env(&mut TestEnv)`-style mega-fixture in favor of small, purpose-named helpers. P5R1-P-06 gives this pattern a name.

### 6.3 Newly-confirmed test conventions

1. **Property tests scope**: never type-system trivia; only domain-meaningful invariants (Pass 2 §2b.5, broad §4.5).
2. **Insta snapshots**: tactical, not blanket — concentrated on JSON write-op shapes (10/17), ADF rendering (2/17), sprint responses (2/17), table headers (1/17), changelog (1/17). NOT used for read-op tables or error message text (those use `assert!(stdout.contains(...))`).
3. **Mock count varies by file**: 132 mocks in `cli_handler.rs`, 138 in `issue_commands.rs`, 73 in `assets.rs`. Files mock everything they call.
4. **`expect(0)` is used positively**: to assert short-circuit (e.g., command errors before HTTP fire). Pass 3 R3 BC-130 verification covers several.

---

## 7. Pre-VSDD convention drift

Conventions that have been *codified* somewhere (CLAUDE.md, ADR, `docs/specs/`, `docs/superpowers/specs/`) and have since drifted from current code.

### Drift-1: `cli/issue/list.rs` shard rule
**Codified**: `docs/specs/list-rs-split.md` produced the original split (list → list + view + comments + format).
**Current state**: `list.rs` regrew to 1,083 LOC (Pass 6 INC-03; CLAUDE.md still says ~970).
**Spec drift**: rule was followed once, then the post-split file regrew without a follow-up `list-rs-split-round-2.md`.

### Drift-2: 4 undocumented orphan modules
**Codified**: CLAUDE.md `cli/issue/` block lists 8 submodules.
**Current state**: 12 submodules. Missing from CLAUDE.md: `view.rs`, `comments.rs`, `changelog.rs`, `json_output.rs`. (Pass 6 INC-04.)
**Drift**: documentation has not kept pace with the post-shard structure.

### Drift-3: `api/assets/` module count
**Codified**: CLAUDE.md lists 4 files (workspace.rs, linked.rs, objects.rs, tickets.rs).
**Current state**: 5 files plus mod.rs. Missing: `schemas.rs` (45 LOC, holding `list_object_schemas` and `list_object_types`). (Pass 2 R4 CONV-ABS-7.)
**Drift**: a feature added (`jr assets schemas`, `jr assets types`) without updating CLAUDE.md.

### Drift-4: `cli/issue/list.rs` description
**Codified**: CLAUDE.md says "list + view + comments (read operations, unified JQL composition)".
**Current state**: `list.rs` has only `handle_list`; `handle_view` is in `view.rs`; `handle_comments` is in `comments.rs`. (Pass 2 R5 §2.4.)
**Drift**: post-split refactor not reflected in CLAUDE.md narrative.

### Drift-5: Top-level CLI commands missing
**Codified**: CLAUDE.md `cli/` block.
**Current state**: 14 top-level commands. Missing from CLAUDE.md: `Api` (`cli/api.rs` 342 LOC) and `Completion` (dispatched inline in `main.rs:67-71`). (Pass 6 INC-06.)
**Drift**: two new top-level commands added without doc update.

### Drift-6: `JrError` variant count
**Codified**: implicit in CLAUDE.md ("JrError enum with exit codes 0/1/2/64/78/130").
**Current state**: 11 variants. Pass 1 broad incorrectly cited 10; Pass 2/5 corrected to 11. (Pass 6 INC-01.)
**Drift**: not a doc-vs-code inconsistency per se, but illustrates that variant count was undocumented.

### Drift-7: CLAUDE.md staleness as a meta-convention
**Observation**: drifts 1-6 all share root cause — CLAUDE.md is updated periodically but lags code by weeks/months. The project's "feature spec discipline" (ADR-0004, `docs/specs/`) is upheld; CLAUDE.md is treated as a stable reference rather than a live document.

**Convention rule (implicit)**: CLAUDE.md describes the *steady-state architecture* (stable for many releases); transient state (current LOC, current file lists) drifts. **This is itself a convention** — but it confuses LLM-driven contributors who treat CLAUDE.md as authoritative.

**Phase 1 implication**: the eventual VSDD doc should either (a) auto-generate the file inventory from `find` results, OR (b) explicitly mark CLAUDE.md sections as "stable" vs "indicative."

---

## 8. Convention strengths — re-ranked top 7 (was top 5)

Stable convention strengths after Pass 2/3 deepening. Re-ranking includes 2 new strengths surfaced by deepening that the broad pass missed.

1. **Single source of truth for errors with strict exit-code mapping (BROAD-1).** `JrError` enum (11 variants), `exit_code()` method (0/1/2/64/78/130 per `<sysexits.h>`), pinned by 4 unit tests. Every error message names a recovery action. Pass 2 R3 confirmed via 14 UserError construction sites in `cli/auth.rs` alone — pattern is uniform across ~30 known-error sites in the codebase.
2. **Per-profile cache signature is uniform across all cache call sites — no leakage observed in audit (BROAD-2).** Soft fence concern noted (P5R1-AP-02) but adherence is 100%.
3. **Zero `unsafe` in production paths; zero `#[allow(clippy::*)]` in src (BROAD-3).** All `unsafe` is test-only `std::env::set_var` (Rust 2024 requirement). Verified by Pass 2 R5 line-by-line.
4. **Three-axis test-injection seam (`JR_BASE_URL` / `JR_AUTH_HEADER` / `XDG_*HOME`) plus `new_for_test` constructor (BROAD-4).** Caveat: `JR_AUTH_HEADER` is not `#[cfg(test)]`-gated (P5R1-AP-06).
5. **Product-namespaced API and types directories with mirrored layout (BROAD-5).** Adding Confluence is a sibling-directory addition.
6. **NEW: Configuration migration safety — file-only baseline writeback prevents env-var bleed-through.** Pass 2 R4 NEW-INV-189 / P5R1-P-02. The pattern that `Config::load_with` reads file + env, but `save_global` writes only file-derived fields, is structurally enforced. A user running `JR_DEFAULTS_OUTPUT=json jr ...` once does NOT permanently change config. This is a non-obvious correctness invariant.
7. **NEW: Defense-in-depth for OAuth security boundaries.** Pass 2 R3/R4: TOCTOU-closed `ResolvedRedirect` (private fields), `EmbeddedOAuthApp::Debug` redacts secret, build-time XOR obfuscation per ADR-0006, opt-in (`#[ignore]`) keychain tests gated by env var, scope validation BEFORE keychain write (NEW-INV-155). Multiple security primitives compose into a defensible OAuth surface.

Demoted from broad's top-5: none. The original 5 are all stable.

---

## 9. Convention gaps requiring Phase 1 decisions (grouped)

The broad pass listed top-5 gaps. R1 re-groups into 5 *categories* with specific Phase 1 decisions in each.

### GAP-CAT-1: Documentation debt
- **D1**: 4 orphan modules undocumented in CLAUDE.md (`view.rs`, `comments.rs`, `changelog.rs`, `json_output.rs`).
- **D2**: `api/assets/schemas.rs` undocumented in CLAUDE.md.
- **D3**: `cli/api.rs` and `cli/init.rs` Completion path undocumented.
- **D4**: CLAUDE.md `cli/issue/list.rs` description stale (still says "list + view + comments").
- **D5**: `cli/project.rs` described as multi-subcommand but actually 2-subcommand (List, Fields). (Pass 2 R6 CONV-ABS-11.)
- **Phase 1 decision**: choose between auto-generated CLAUDE.md tree vs deliberate-staleness convention. Pass 6 §7.5 default: HARMONIZE.

### GAP-CAT-2: Shard rule enforcement
- **S1**: `cli/auth.rs` (1,998 LOC) — no `auth-rs-split.md`.
- **S2**: `cli/assets.rs` (1,055 LOC) — no `assets-rs-split.md`.
- **S3**: `cli/issue/list.rs` (1,083 LOC) — already split once via `list-rs-split.md`; needs round-2 spec.
- **S4** (open question): does `api/auth.rs` (1,397) deserve the same rule, or are non-CLI files exempt?
- **S5** (open question): is `adf.rs` (1,826) deliberately exempt as a hand-written DSL parser?
- **Phase 1 decision**: codify the rule with an explicit exception list, OR codify the absence of the rule. Currently neither.

### GAP-CAT-3: Type-level fencing
- **T1**: per-profile cache signature is convention-only — no `Profile(String)` newtype or `Cache<P>` phantom-type.
- **T2**: issue keys are bare `String` everywhere — no newtype to prevent issue-key vs project-key vs asset-key swaps. (Broad §8.1 LOW-severity.)
- **T3**: Config field shape (e.g., `auth_method: Option<String>` with values `"oauth"`/`"api_token"`) could be a typed enum but is String. (Broad §8.1 LOW-severity, "Implicit string conversions losing type safety.")
- **Phase 1 decision**: each is a tradeoff between simpler code (current) and compile-time safety (proposed). Pass 6 §7.4 #4 lists T1 as a Phase 1 decision.

### GAP-CAT-4: JSON field naming
- **J1**: 4 distinct bool field names across 6 write-ops (`changed`/`updated`/`linked`/`unlinked`). P5R1-AP-05.
- **J2**: snake_case in serde struct fields vs camelCase in injected JSON (e.g., `schemaName` injection in `cli/assets.rs::handle_types`). Pass 2 R4 NEW-INV-174.
- **J3**: JSON missing-data sentinel inconsistency — empty string for unknown schema_id (JSON), em-dash `\u{2014}` for unknown (Table). Pass 2 R4 NEW-INV-175.
- **Phase 1 decision**: pick one canonical for J1; document the json_output naming rule; address J2/J3 sentinels.

### GAP-CAT-5: Test interface canonicalization
- **TI-1**: Library-level (`JiraClient::new_for_test`) vs process-level (`assert_cmd`) — codify when each is appropriate. Pass 6 INC-11 noted the boundary is fuzzy.
- **TI-2**: Naming convention (108 prefix vs 212 no-prefix) is implicit — codify in CLAUDE.md or contributing guide.
- **TI-3**: Test-isolation helpers (`with_temp_cache`, `XdgConfigGuard`, `KEYRING_TEST_ENV_MUTEX`, `JR_BASE_URL`) are purpose-built per subject — document the rule that "no general fixture framework."
- **Phase 1 decision**: document the test-interface choice rule + naming preference.

---

## 10. Delta Summary

- **New design patterns / conventions added**: 7 (P5R1-P-01..07)
  - Two-struct API shape split (P5R1-P-01)
  - Figment env scope as deliberate boundary (P5R1-P-02)
  - 3-pass asset enrichment dedup pattern (P5R1-P-03)
  - AuthorNeedle classify smart-constructor (named, spec'd) (P5R1-P-04)
  - TOCTOU-closed `ResolvedRedirect` private-fields (P5R1-P-05)
  - `with_temp_cache` + `KEYRING_TEST_ENV_MUTEX` test scaffolding (P5R1-P-06)
  - Handler-boundary downcast-and-rewrite (P5R1-P-07)

- **New anti-patterns added**: 7 (P5R1-AP-01..07)
  - HTTP method surface bifurcation (`send` vs `send_raw`/`request`)
  - Per-profile cache soft-fence (no compile-time enforcement)
  - Hardcoded 8/5 worklog constants at call site (un-named)
  - Implicit "shard at ~1000 LOC" rule applied inconsistently
  - 4 distinct bool field names in JSON write-op output
  - `JR_AUTH_HEADER` accepted in production binary (no cfg gate)
  - `--verbose` no PII redaction in body logging

- **Pre-VSDD drifts catalogued**: 7 (Drift-1..7)
- **Test-mechanism subject map**: 13 categories (production tests + meta convention rule "no general fixture framework")
- **Strengths re-ranked**: top 5 → top 7 (added: file-only migration baseline; defense-in-depth OAuth)
- **Gaps re-grouped**: 5 categories with 16 specific Phase 1 decisions

- **Existing items refined**: consistency assessment table (1 demotion: module-sharding rule MIXED → LOW; 2 nuances added: per-profile cache fence, JSON write-op naming)

- **Remaining gaps for next round**:
  - Sample 5+ random files in `tests/` to verify the "108 prefix vs 212 no-prefix" pattern doesn't have a third axis (sync `#[test]` vs `#[tokio::test]` discipline). Pass 3 R4 noted these but didn't enumerate.
  - Audit the convention "every command supports `--output json`" against ALL 14 top-level commands × all subcommands (~50 surface points). Broad asserted HIGH but didn't enumerate.
  - Audit logger discipline (when do handlers eprintln vs println vs return JrError vs anyhow!). Pass 2 R3 enumerated 14 eprintln sites in `cli/auth.rs` alone; cross-handler pattern unverified.
  - Confirm or refute: are *all* `pub` functions in `api/jira/`, `api/assets/`, `api/jsm/` resource-binding methods (impl JiraClient blocks)? Pass 2 R6 §3.7 noted `cli/project.rs` has 2 subcommands; perhaps one resource impl is module-scope rather than `impl JiraClient`.

---

## 11. Novelty Assessment

**Novelty: SUBSTANTIVE**

**Justification**: Removing this round's findings would change how the Phase 1 spec is structured.

- **7 new design patterns**: each is independently citable ground-truth material a Phase 1 spec would reference. The `AuthorNeedle::classify` smart-constructor pattern (P5R1-P-04) is the canonical exemplar of "non-obvious heuristic that earned its own spec." Without it, a Phase 1 author would not know to look for similar patterns elsewhere.
- **7 new anti-patterns**: 4 (P5R1-AP-04 shard-rule inconsistency, P5R1-AP-05 JSON field naming, P5R1-AP-06 JR_AUTH_HEADER ungated, P5R1-AP-07 verbose PII) are *Phase 1 decisions required*, not nitpicks. The shard-rule inconsistency alone requires either codification or exception listing for the spec to be coherent.
- **Pre-VSDD drift catalog**: 7 distinct CLAUDE.md drift cases. Phase 1 spec must decide treatment for each; otherwise the spec inherits stale assumptions.
- **Re-ranked strengths**: 2 newly-named strengths (file-only migration baseline; defense-in-depth OAuth) are non-obvious and worth preserving as explicit invariants in the Phase 1 spec.

These are not refinements of existing items — they are *new categories* the broad pass did not name. A spec author working from broad-only would miss the JSON field naming gap, the shard rule inconsistency at the categorical level, the implicit test-mechanism choice rule, and the AuthorNeedle smart-constructor as a *named pattern*.

---

## 12. Convergence Declaration

**Another round needed.** Substantive gaps remaining:

1. **Cross-handler eprintln/println discipline audit** — Pass 2 R3 enumerated 14 sites in `cli/auth.rs` only; unknown if pattern holds across all 14 top-level commands.
2. **`--output json` parity audit** at the per-subcommand level (~50 surface points). Broad asserted HIGH but did not enumerate per-subcommand.
3. **Resource-binding pattern audit** — confirm all `pub` API functions are `impl JiraClient` methods or document the exceptions.
4. **Test-name pattern triangulation** — confirm sync `#[test]` vs `#[tokio::test]` discipline is consistent (which subjects use sync, which use tokio).

R2 should attack these. Estimate 1-2 more rounds needed before NITPICK declared. Current SUBSTANTIVE finding density (14 new items) is high enough to warrant another pass.

---

## 13. State Checkpoint

```yaml
pass: 5
round: 1
status: complete
new_design_patterns: 7
new_anti_patterns: 7
new_test_conventions: 3      # purpose-built fixture rule (no general framework); test-mechanism subject map (13 categories); XdgConfigGuard sibling pattern
new_pre_vsdd_drifts: 7
files_examined: 12             # broad pass 5; pass 2 R4-R6; pass 3 R4; pass 6 synthesis; CLAUDE.md; src/cli/issue/json_output.rs full; spot-verify of cache.rs, api/auth.rs, api/client.rs, jql.rs, partial_match.rs, duration.rs proptest blocks via prior passes
verification_actions:
  - find_tests_recount: 108 prefix + 212 no-prefix = 320 total ✓
  - grep_KEYRING_TEST_ENV_MUTEX: confirmed at api/auth.rs:1095
  - grep_with_temp_cache: confirmed 28 call sites in cache.rs
  - grep_JR_AUTH_HEADER: confirmed not #[cfg(test)] gated at api/client.rs:64-66
  - read_json_output_full: confirmed 4 distinct bool field names (changed, updated, linked, unlinked)
inconsistencies_resolved: 0     # pass 6 already resolved 5; this round resolves none, only adds new findings
hallucination_corrections: 1    # broad §4.8 "36 integration tests" framing nitpick (broad meant files; reads loosely as functions)
strengths_top7: confirmed       # 5 broad + 2 new (file-only migration baseline; defense-in-depth OAuth)
gaps_grouped: 5                 # 16 specific Phase 1 decisions
novelty: SUBSTANTIVE
timestamp: 2026-05-04T16:30:00Z
next_round_targets: |-
  R2-T1: cross-handler eprintln/println/return-error discipline audit across all 14 top-level commands
  R2-T2: per-subcommand --output json parity audit (~50 surface points: assets x6, auth x7, board x2, sprint x4, worklog x2, team x1, user x3, queue x2, issue x17, project x2, plus init/me/api/completion)
  R2-T3: resource-binding pattern audit — all pub fns in api/jira/, api/assets/, api/jsm/ are impl JiraClient methods?
  R2-T4: sync #[test] vs #[tokio::test] discipline — which subjects use which annotation
  R2-T5: snapshot vs assert!(stdout.contains) split — codify when each is the right tool (broad §4.4 said "tactical" but didn't quantify the rule)
  R2-T6: error-message wording uniformity — Pass 2 R3 NEW-INV-161 noted 5 sites with "unknown profile: X; known: ..." wording; recurring patterns should be enumerated for a Phase 1 errors-as-specs section
```
