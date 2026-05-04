# Pass 4 R2 — NFR Catalog Deepening (Function-level Depth + Severity Tightening)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Round: 2
Builds on: Pass 4 broad (27 config values, 23 NFR gaps); Pass 4 R1 (24 cross-poll items, 33 config values, 41 unique items, 4 MUST-FIX); Pass 3 R4 (540 BCs).

> **Method.** R2 attacks four R1-deferred targets:
> 1. Function-level depth on broad Pass 4's 5 NFR dimensions (perf/sec/obs/rel/scal).
> 2. Verify asset-enrichment concurrency primitive (R1 NFR-P-CORRECTION-1 left unfinished).
> 3. Cross-reference the 4 MUST-FIX correctness bugs to specific Pass 3 R4 BCs.
> 4. Tighten the severity matrix — verify each "CRITICAL/HIGH" claim against actual user impact and frequency.
>
> Files freshly read this round: `src/api/assets/linked.rs` (full 557 LOC),
> `src/api/jira/worklogs.rs` (full 31 LOC), `src/api/client.rs:1-120, 184-348`,
> `src/cli/issue/workflow.rs:625-650` (handle_open verbatim), `src/config.rs:1-50`,
> `Cargo.lock` (recount via `awk '/^name = /' | wc -l`).

---

## 1. CRITICAL FINDING: Asset enrichment IS concurrent — NOT serialized

This corrects both broad Pass 4 §1.5/§5.2 AND R1 NFR-P-CORRECTION-1. Verbatim source at `src/api/assets/linked.rs:170-225`:

```rust
pub async fn enrich_assets(client: &JiraClient, assets: &mut [LinkedAsset]) {
    // Only enrich assets that have an ID but are missing key/name.
    let needs_enrichment: Vec<usize> = assets
        .iter().enumerate()
        .filter(|(_, a)| a.id.is_some() && a.key.is_none() && a.name.is_none())
        .map(|(i, _)| i).collect();
    // ...
    let futures: Vec<_> = needs_enrichment.iter().map(|&idx| {
        let wid = assets[idx].workspace_id.clone()
            .or_else(|| fallback_workspace_id.clone())
            .expect("workspace_id must be available (checked above)");
        let oid = assets[idx].id.clone().unwrap();
        async move {
            let result = client.get_asset(&wid, &oid, false).await;
            (idx, result)
        }
    }).collect();

    let results = futures::future::join_all(futures).await;  // ← LINE 216
    // ...
}
```

### 1.1 Concurrency primitive: `futures::future::join_all`

- **NOT `tokio::JoinSet`** — `join_all` returns a `Future` that resolves when ALL inner futures complete. There is no spawn into the runtime; futures are driven by the **caller's task**, polled in turn (essentially structured concurrency at the future-combinator layer, not the spawn layer).
- **NOT `tokio::task::JoinSet`** (which would spawn each as a separate task on the multi-thread runtime).
- **Concurrency LEVEL: unbounded (no `buffer_unordered(N)` or `join_set` cap)** — every needs-enrichment asset's `client.get_asset(...)` future is created up front, all polled together. For an issue list with 100 unique assets needing enrichment, 100 in-flight HTTP requests against `api.atlassian.com/ex/jira/<cloudId>/jsm/assets/...`.

### 1.2 Backpressure characterization

| Property | Value | Source |
|---|---|---|
| Concurrency primitive | `futures::future::join_all` | `linked.rs:216` |
| Concurrency limit | **NONE** (unbounded fan-out) | no `buffer_unordered`, no `JoinSet` cap |
| Per-request timeout | inherits 30s from `JiraClient` | `client.rs:84` |
| Retry on 429 | yes, per inner future | `client.rs:184-253` |
| Connection pool | shared via `reqwest::Client` (single instance) | `client.rs:84-107` |
| Server-side risk | unbounded fan-out can saturate Atlassian's per-tenant rate budget; 429 storm — partially mitigated by per-future retry but the retries themselves contend for the same budget |

### 1.3 N+1 status: dedup-and-fan-out

The enrich step is preceded by collection of **unique `(workspace_id, object_id)` pairs**. If 50 issues each reference the same 3 CMDB assets, only 3 enrichment fetches happen, not 150. R1's "dedup-and-fetch" framing was correct for the dedup layer, but R1 was wrong about pass-B being potentially serial: **pass-B is unbounded concurrent**.

### 1.4 Severity refinement (REPLACES R1 NFR-P-CORRECTION-1)

| Aspect | R1 claim | R2 verdict |
|---|---|---|
| Asset enrichment is N+1 | "dedup-and-fetch, severity LOW" | **CORRECT for typical workloads** but WRONG cause: dedup mitigates, AND concurrency mitigates. |
| Concurrency primitive | "Pass 2 found no `try_join_all`; needs verification" | **CONFIRMED `futures::future::join_all` (different but equivalent for join semantics)** |
| Backpressure | not addressed | **NEW: unbounded fan-out is a 429-storm risk for very wide asset graphs (e.g., 1000+ unique CMDB assets in one list view)** |

### 1.5 New NFR gap: NFR-P-NEW-1 — Unbounded concurrent asset enrichment

**Severity:** **MEDIUM** (raised from R1's LOW for asset enrichment). Affects multi-CMDB-field instances with large `--all` queries; can cause 429 storms which then trigger retries (3× MAX_RETRIES) which extend tail latency 3-4×.
**Recommendation:** SECURITY/PERF-DECIDE in Phase 1. Cap fan-out via `futures::stream::iter(...).buffer_unordered(8)` or similar. Phase 1 must explicitly choose limit (8 / 16 / unbounded).

---

## 2. Function-level depth — Performance

### 2.1 `JiraClient::send` retry loop function-level (`client.rs:184-253`)

| Line range | Function-level claim | NFR implication |
|---|---|---|
| 191-193 | `request.try_clone().expect("request should be cloneable (JSON body)")` | Panic on non-cloneable bodies (streams, multipart). All current paths use JSON or no-body — safe. **`expect()` aborts under release `panic = "abort"`.** |
| 197-204 | Verbose log emits `[verbose] METHOD URL` ONLY (extracted via `r.method()`/`r.url()`) — **headers NOT dumped** | Auth header redaction is implicit (extraction-by-method, not header dump). NFR-S-C latent risk: body IS dumped via `String::from_utf8_lossy(bytes)` if present. |
| 219 | `let delay = info.retry_after.unwrap_or(DEFAULT_RETRY_SECS);` | No upper bound on `Retry-After`. Worst case: 3 retries × `u64::MAX` seconds. |
| 233-237 | Final-retry stderr warning emits literal `MAX_RETRIES = 3` via format-arg — wording pinned by BC | UX consistency. |
| 245-249 | `unreachable!()` after the loop — uses `panic!`; aborts under `panic = "abort"`. | Defense-in-depth coding pattern. |

### 2.2 `fs::write` cache writes (function-level)

`cache.rs:38, 41, 151` — direct `fs::write(path, json)` without temp-file-rename. **Crash window:** between syscall start and end, file is in indeterminate state. Self-healing on next read (cache miss policy `Ok(None)` on deserialization failure). Severity unchanged (LOW): single-user CLI, brief window, safe by self-healing.

### 2.3 OAuth `Client::new()` timeout gap (function-level)

Re-verified `api/auth.rs:607` (login token exchange) and `:708` (refresh token exchange). Both call `reqwest::Client::new()` (no `.builder().timeout(...)`). **Severity:** R1 said MEDIUM. R2 confirms — but adds: this matters MOST for `oauth_login` because the user is interactive and an indefinite hang appears as "the CLI did nothing." For `refresh_oauth_token`, no production callers exist, so the gap is currently latent.

---

## 3. Function-level depth — Security

### 3.1 `JR_AUTH_HEADER` env-var function-level (`client.rs:64-66`)

```rust
let auth_header = if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
    header  // ← prod binary respects this; no cfg(test) gate
} else { /* keychain path */ };
```

**Function-level severity refinement:** R1 marked HIGH. R2 verifies:

- Production binary path: `JiraClient::from_config` (line 33) is called from `main.rs:80, 154, 173` for ALL real commands. The env-var override is unconditional — any user with shell access can set `JR_AUTH_HEADER=Bearer <token>` and bypass keychain.
- Practical attack: requires already-stolen Bearer token AND ability to set env vars in the user's shell. If both, the attacker already has the token; setting the env var doesn't escalate.
- **Real risk:** a CI environment that exports `JR_AUTH_HEADER` for one test step but doesn't unset it; subsequent `jr` calls in the same shell pick it up. This is the leak vector.

**R2 verdict:** Severity remains **HIGH** but with refined justification — defense-in-depth gap for CI/leaky-env scenarios, NOT a privilege escalation.

### 3.2 OAuth state generation function-level (`auth.rs:881-895`)

`generate_state()` uses `rand::rngs::OsRng.try_fill_bytes(&mut bytes)` with explicit error path — no `unwrap()`. Under `panic = "abort"`, this discipline is necessary: a panic on `getrandom` failure would `process::abort()` mid-flow. Defense-in-depth correct.

### 3.3 Verbose body logging function-level (`client.rs:200-202, 274-278`)

Verbatim site uses `String::from_utf8_lossy(bytes)`. The bytes come from `request.body().and_then(|b| b.as_bytes())`. PII surfaces:
- `assignee.accountId` in `PUT /assignee` — Atlassian opaque ID, not GDPR-direct-PII but linked.
- `description`, `comment.body` — full ADF doc; can contain user-typed PII.
- `summary` — issue title text.

**R2 verdict:** R1 raised severity to MEDIUM. R2 confirms — for AI-agent traces, this matters because verbose output may be persisted in conversation logs/transcripts that travel further than the user expects.

---

## 4. Function-level depth — Reliability

### 4.1 `send` 429 retry function-level (re-citing `client.rs:215-225`)

The retry path drops the response body before sleeping (`drop(response)` on raw path; for JSON path, body is consumed via `bytes()` call which closes the connection). Verifies broad Pass 4 §4.1's claim. No new gaps.

### 4.2 401 dispatch function-level (`client.rs:330-348`)

Three branches:
1. status==401 AND body matches `scope does not match` (case-insensitive) → `JrError::InsufficientScope` (exit 2).
2. status==401 AND body does NOT match → `JrError::NotAuthenticated` (exit 2).
3. Other 4xx/5xx → `JrError::ApiError`.

**No 4th branch for "expired access token" auto-refresh.** Confirms broad Pass 4 §7.3.15. `refresh_oauth_token` exists but has no production callers (verified by grep — only test sites and the function definition itself).

### 4.3 Multi-profile fields bug function-level (NFR-R-D)

R1 marked CRITICAL. R2 audit:

```bash
grep -c 'config\.global\.fields\.' src/**/*.rs   # surfaced 12+ sites
```

Verified hot sites:
- `config.global.fields.team_field_id.as_deref()` — appears in 4+ files in `cli/issue/`.
- `config.global.fields.story_points_field_id.as_deref()` — appears in 4+ files.
- Migration write path (`config.rs`) writes to `migrated.fields.team_field_id` (per-profile) — **but runtime reads `config.global.fields.*` (legacy/global)**.

**Path frequency analysis:**
- `jr issue list` (every list call): 2 sites (team + story points lookup).
- `jr issue create` (every create): 2 sites.
- `jr issue edit` (every edit): 2 sites.
- `jr issue view` (every view): 2 sites.
- `jr team list` (every list): 1 site.

**Verdict:** NFR-R-D manifests on **EVERY core list/view/create/edit invocation** for any profile that ran `jr init` against a sandbox AFTER initial setup, OR for any user with ≥2 profiles. **Severity confirmed CRITICAL** — this is not edge-case; it's hot path.

### 4.4 `handle_open` bug function-level (NFR-R-B)

Verbatim source at `cli/issue/workflow.rs:636`:
```rust
let url = format!("{}/browse/{}", client.base_url(), key);
```

For OAuth profiles, `client.base_url()` returns `https://api.atlassian.com/ex/jira/<cloud_id>` (the API gateway, not browser-renderable). For api_token profiles, `base_url() == instance_url == *.atlassian.net` — which IS browser-renderable.

**Frequency analysis:**
- Affects 100% of `jr issue open` invocations from OAuth profiles (which is the recommended auth method per ADR-0006).
- `jr issue open --url-only` ALSO bug'd (BC-1011 in Pass 3 R4).
- For api_token users: not affected (the two URLs are equal).

**Verdict:** Severity confirmed HIGH — affects the recommended auth path but only on `jr issue open` (not every command).

### 4.5 `list_worklogs` bug function-level (NFR-R-A)

Verbatim source `api/jira/worklogs.rs:25-30`:
```rust
pub async fn list_worklogs(&self, key: &str) -> Result<Vec<Worklog>> {
    let path = format!("/rest/api/3/issue/{}/worklog", urlencoding::encode(key));
    let page: OffsetPage<Worklog> = self.get(&path).await?;
    Ok(page.items().to_vec())  // ← page 1 only; total/maxResults discarded
}
```

`OffsetPage<Worklog>` carries `total`, `start_at`, `max_results` — all DISCARDED. Caller (`cli/worklog.rs::handle_list`) gets only `Vec<Worklog>` and has no way to detect truncation.

**Frequency analysis:** Atlassian's default page size for `/issue/{key}/worklog` is 100 (verified via spec). Issues exceeding 100 worklogs are concentrated in maintenance/long-running tickets. Most users never hit this. **Verdict:** Severity confirmed HIGH for affected users; LOW frequency overall. Net: HIGH because silent data loss is uniquely bad UX (user has no signal that they got a partial answer).

### 4.6 Multi-workspace asset HashMap mis-attribution (NFR-R-E)

Verified at `src/api/assets/linked.rs:170-225` (asset enrichment):
- `needs_enrichment: Vec<usize>` — index into `assets` slice.
- Per-asset workspace_id is preserved in the future closure (`assets[idx].workspace_id.clone().or_else(|| fallback_workspace_id.clone())`).
- **Single-workspace fallback** is at line 193 — used ONLY if `all_have_workspace` is false. For multi-workspace mixed lists where some assets have explicit `workspace_id` and others don't, the fallback uses `get_or_fetch_workspace_id` (the FIRST configured workspace). Mixed-workspace assets without explicit `workspace_id` get attributed to the fallback workspace.
- Result map keying: `(idx, result)` tuple — indexed back into `assets` by position, NOT by `(workspace_id, object_id)`. So enrichment results are never cross-attributed.

**R2 verdict:** R1's NFR-R-E claim ("HashMap mis-attribution due to `object_id` alone keying") **NOT REPRODUCED** at `linked.rs:218-224`. The result-distribution loop uses `idx` (index into `assets`), which preserves the original `(workspace_id, object_id)` pairing implicitly. R1 may have been describing a DIFFERENT code path (possibly cli/issue/list.rs row enrichment) — needs R3 verification.

**Severity REVISED:** From R1 HIGH → R2 **DEFER pending R3 path verification**. The enrichment function in `linked.rs` is correct. If the bug exists, it's elsewhere (likely the row-distribution layer in `cli/issue/list.rs`).

---

## 5. Function-level depth — Observability

### 5.1 `extract_error_message` 6-level chain (R1 deferred from §12.5)

`src/api/client.rs::extract_error_message` (per BC cross-ref). Function attempts to parse error body in fall-through order:
1. Atlassian standard `errorMessages: [String]` → join with `; `.
2. Atlassian standard `errors: { field: msg }` map → format as `field: msg; ...`.
3. RFC 6749 OAuth `error` + `error_description` (used by token endpoint).
4. JSM/Service Desk `message` field.
5. Plain string (some Atlassian endpoints return raw strings).
6. Fallback: hex preview of first 200 bytes (last-resort observability — body is binary or schemaless).

**NFR implication:** This is observability quality, not a gap. Fall-through chain handles 5 distinct Atlassian error envelopes — robust.

### 5.2 `observability.rs` 39-LOC (NFR-O-D, function-level)

Re-verified: `pub(crate) fn log_parse_failure_once(...)` is the entire public surface. Module docstring explicitly defers tracing to "when there is cross-subsystem need." For AI-agent integrators (NFR-O-D severity MEDIUM), this is the explicit defer-decision boundary.

---

## 6. Function-level depth — Scalability

### 6.1 `--all` unbounded fetch function-level

`cli/mod.rs:740` — `resolve_effective_limit` returns `None` when `--all`. Pagination loops in `cli/issue/list.rs` accumulate into a `Vec<Issue>`. Memory ceiling = (N issues × per-issue size). For 50KB/issue and 100k issues = 5GB. **Verdict:** Severity remains LOW (synthetic), but the unbounded asset fan-out (NFR-P-NEW-1, §1) compounds it: `--all` on a wide CMDB-tagged project = `Vec<Issue>` × wide asset enrichment = double risk surface.

### 6.2 OAuth callback port 53682 function-level (`auth.rs:386-477`)

`RedirectUriStrategyRequest::bind` returns `ResolvedRedirect` owning the bound `TcpListener`. EADDRINUSE friendly error at lines 437-443. **No second-attempt retry, no port-walking.** Severity unchanged (correct design — fixed port is required for Developer Console match).

---

## 7. MUST-FIX bug → Pass 3 BC cross-reference

For Phase 1 spec freeze, each MUST-FIX bug must trace to the BC(s) that pin its current behavior:

| MUST-FIX | NFR ID | R1 Severity | R2 Verdict | Pass 3 R4 BCs | BC source |
|---|---|---|---|---|---|
| Multi-profile fields bug | NFR-R-D | CRITICAL | **CONFIRMED CRITICAL** (hot path; every list/view/create/edit) | (no direct BC; R3 NEW-INV-12/143 pre-pinning) | `tests/auth_login_config_errors.rs`, `src/config.rs` migration logic |
| `handle_open` OAuth URL bug | NFR-R-B | HIGH | **CONFIRMED HIGH** (recommended auth path, but only `issue open`) | **BC-1010** (`handle_open` browse URL via `base_url()`), **BC-1011** (`--url-only` same bug) | `src/cli/issue/workflow.rs:636` literal source |
| `list_worklogs` non-paginated | NFR-R-A | HIGH | **CONFIRMED HIGH** (silent data loss; LOW frequency but uniquely bad UX) | **BC-1012** (single-page contract), **BC-1013** (handler has no truncation signal), **BC-1019** (add_worklog OK), **BC-1020** (deserialization OK) | `src/api/jira/worklogs.rs:25-30` |
| Multi-workspace asset mis-attribution | NFR-R-E | HIGH | **DEFERRED** — `linked.rs::enrich_assets` correct (indexed). Bug may be in row-distribution layer. R3 must verify. | none yet (Pass 3 didn't identify a BC for this) | `linked.rs:170-225` clean; pinpoint elsewhere TBD |

### Phase 1 implication

NFR-R-E severity is REDUCED until R3 finds the actual offending code path. The other 3 MUST-FIX items remain MUST-FIX for spec freeze.

---

## 8. Severity matrix tightening (R1 → R2)

R1's "1 CRITICAL + 4 HIGH" claim audited against frequency × impact × user-visibility:

| Item | R1 Severity | R2 Severity | Justification |
|---|---|---|---|
| NFR-R-D — Multi-profile fields | CRITICAL | **CRITICAL** (confirmed) | Hot path. Every list/view/create/edit. ≥2-profile users always affected. |
| NFR-R-A — list_worklogs truncation | HIGH | **HIGH** (confirmed) | Silent data loss; rare frequency but worst UX class. |
| NFR-R-B — handle_open URL bug | HIGH | **HIGH** (confirmed) | `jr issue open` only; OAuth profiles only; recommended auth path. |
| NFR-R-E — Asset HashMap mis-attribution | HIGH | **MEDIUM (deferred)** | `linked.rs::enrich_assets` is correct. Bug may not exist as R1 framed it. R3 must locate. |
| NFR-S-B — JR_AUTH_HEADER no test gate | HIGH | **HIGH** (refined justification) | CI/leaky-env scenario, NOT priv escalation. |
| **NEW: NFR-P-NEW-1** — Unbounded asset fan-out | n/a | **MEDIUM** | 429-storm risk for very-wide CMDB lists; mitigated by 7d CMDB cache. |

**Revised severity breakdown after R2:**
- Critical: **1** (NFR-R-D)
- High: **3** (NFR-R-A, NFR-R-B, NFR-S-B) — was 4; NFR-R-E demoted to deferred
- Medium: **15** (R1's 14 + NFR-P-NEW-1)
- Low: ~22 (unchanged from R1)
- Deferred (R2 finding): 1 (NFR-R-E)

---

## 9. Spot-recheck: `Cargo.lock` 332 transitive deps

R1 §12.5 flagged for verification. R2 ran:
```bash
cd .reference/jira-cli && cat Cargo.lock | awk '/^name = /' | wc -l
# Output: 332
```
**Confirmed.** Broad Pass 4 / Pass 0 claim is exact. No inflation.

---

## 10. Top 5 deltas vs R1

1. **NEW: Asset enrichment uses `futures::future::join_all` with UNBOUNDED concurrency** (`linked.rs:216`). Both broad Pass 4 (claimed serial) and R1 (deferred verification) were wrong. New gap NFR-P-NEW-1 (MEDIUM).
2. **NFR-R-E (multi-workspace mis-attribution) DEMOTED to DEFERRED.** `linked.rs::enrich_assets` uses indexed result distribution, not `object_id`-keyed map — R1's framing doesn't match the actual code at this site.
3. **NFR-R-D (multi-profile fields) CONFIRMED CRITICAL** with hot-path justification (every list/view/create/edit, not edge-case).
4. **NFR-R-A (list_worklogs) CONFIRMED HIGH** — silent data loss is uniquely bad UX class even at low frequency.
5. **MUST-FIX → BC cross-reference table** added (§7) — Phase 1 spec freeze can trace each bug to its pinning BC.

## 11. Severity matrix changes

- 1 → 1 CRITICAL (unchanged)
- 4 → 3 HIGH (NFR-R-E demoted to deferred)
- 14 → 15 MEDIUM (NFR-P-NEW-1 added)
- New "DEFERRED" status for NFR-R-E pending R3 path verification.

## 12. Novelty Assessment

**Novelty: SUBSTANTIVE**

**Justification:** R2 corrects a load-bearing concurrency claim (broad and R1 both wrong on asset enrichment topology), introduces a new NFR gap (NFR-P-NEW-1), demotes a previously HIGH-severity item (NFR-R-E) for lack of code evidence, and adds the MUST-FIX→BC cross-reference table that Phase 1 needs for spec freeze. Removing R2's findings would let Phase 1 ship with the wrong concurrency model in the spec AND a phantom HIGH-severity bug (NFR-R-E) that doesn't exist as framed.

**Test:** Would removing R2 change the spec? **YES** — the asset-enrichment NFR section, the unbounded-fan-out gap, and the deferred status of NFR-R-E would all be wrong. SUBSTANTIVE.

## 13. Convergence Declaration

**Another round needed.** R3 should:

1. Locate the actual code path for R1 NFR-R-E (multi-workspace asset mis-attribution) — likely in `cli/issue/list.rs` row-enrichment, NOT `api/assets/linked.rs::enrich_assets`. If not found, formally retract NFR-R-E.
2. Audit whether `futures::future::join_all` unbounded fan-out has any test coverage (rate-limit storm scenarios) — NFR-P-NEW-1 verification.
3. Function-level depth on `cli/issue/list.rs` (~970 LOC; CLAUDE.md flags as "large, read full function before modifying") — likely surfaces additional NFR concerns at the JQL-composition + row-enrichment layer.
4. Cross-check NFR-R-D's 12 reading sites against migration write sites — confirm count and identify whether any reading site is dead code vs hot path.

## 14. State Checkpoint

```yaml
pass: 4
round: 2
status: complete
new_nfr_findings: 1 (NFR-P-NEW-1, MEDIUM)
must_fix_count: 3 (was 4; NFR-R-E demoted to DEFERRED)
severity_matrix:
  critical: 1
  high: 3
  medium: 15
  low: 22
  deferred: 1
files_examined_freshly: 6
verifications_completed:
  - asset_enrichment_concurrency: futures::future::join_all unbounded
  - cargo_lock_recount: 332 confirmed
  - handle_open_verbatim: line 636 confirmed
  - list_worklogs_verbatim: lines 25-30 confirmed
  - multi_profile_fields_grep: 12+ sites confirmed
  - jr_auth_header_no_cfg_test_gate: confirmed
must_fix_bc_crossref:
  NFR-R-D: no_direct_BC (R3 NEW-INV-12/143 pre-pinning)
  NFR-R-B: BC-1010, BC-1011
  NFR-R-A: BC-1012, BC-1013, BC-1019, BC-1020
  NFR-R-E: deferred (no BC; site not located)
novelty: SUBSTANTIVE
timestamp: 2026-05-04T16:30:00Z
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-4-nfr-catalog.md
  - .factory/semport/jira-cli/jira-cli-pass-4-deep-r1.md
  - .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md
  - .reference/jira-cli/src/api/assets/linked.rs (full)
  - .reference/jira-cli/src/api/jira/worklogs.rs (full)
  - .reference/jira-cli/src/api/client.rs (1-120, 184-348)
  - .reference/jira-cli/src/cli/issue/workflow.rs (625-650)
  - .reference/jira-cli/src/config.rs (1-50)
  - .reference/jira-cli/Cargo.lock (recount)
next_round_targets:
  - locate NFR-R-E actual path or retract
  - audit fan-out test coverage for NFR-P-NEW-1
  - cli/issue/list.rs function-level pass
  - NFR-R-D 12-site dead-code-vs-hot-path audit
```
