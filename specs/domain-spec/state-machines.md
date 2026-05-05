---
title: "State Machines"
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
traces_to: "README.md"
source_passes: "Pass 2 broad §2b.3 + Pass 8 §2.5 + Pass 1 R1 §4"
---

# State Machines

Five state machines that materially drive the `jr` system. All are verified from source in Pass 2 broad §2b.3 and Pass 8 §2.5. Diagrams use ASCII flowchart notation; prose annotations cite source locations.

---

## SM-01: OAuth Login State Machine

**Source**: `api/auth.rs:374-477`, `cli/auth.rs::login_oauth`, ADR-0006.

### States

```
                 credential-resolution
   command-parsed ──────────────────────────────────────┐
                                                          │
                    flag > env > keychain > embedded > prompt
                                                          │
                          [OAuth source resolved]         │
                                                          ▼
                              ┌─────────────────────────────────┐
                              │ choose strategy                  │
                              │ Embedded → FixedPort(53682)      │
                              │ BYO      → DynamicPort(0)        │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ bind listener (TOCTOU-closed)    │
                              │ ResolvedRedirect = bind()        │
                              │ EADDRINUSE → friendly error      │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ validate scopes                  │
                              │ persist app (presence check,     │
                              │  no decode yet)                  │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ generate state                   │
                              │ 32 bytes OsRng → 64 hex chars    │
                              │ build authorize URL              │
                              │ (NO PKCE — NFR-S-A)             │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ open browser                     │
                              │ accept ONE callback connection   │
                              │ validate state (CSRF)            │
                              │ mismatch → error (no write)      │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ exchange code for tokens         │
                              │ POST /oauth/token                │
                              │ no code_verifier                 │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ discover cloud_id                │
                              │ accessible_resources.first()     │
                              │ (silent first-wins, NEW-INV-179) │
                              │ 0 resources → error              │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────────────┐
                              │ store tokens                     │
                              │ <profile>:oauth-access-token     │
                              │ <profile>:oauth-refresh-token    │
                              │ reload config, write profile     │
                              └─────────────┬───────────────────┘
                                            │
                                            ▼
                                     OAuthResult { cloud_id, site_url, site_name }
                                     → exit 0
```

### Key Invariants

- CSRF state is 32 bytes from `OsRng` → 64 hex chars (BC-1146).
- State mismatch at callback → error; keychain NOT written.
- `accessible_resources.first()` silent first-wins for multi-site users (gap: NEW-INV-179).
- Zero accessible resources → explicit error.
- `redirect_uri` for embedded = `http://127.0.0.1:53682/callback` (literal IPv4, NOT `localhost`).
- `redirect_uri` for BYO = `http://localhost:{port}/callback` (backward-compat).

---

## SM-02: OAuth Token Refresh State Machine

**Source**: `api/auth.rs:704-770`, `cli/auth.rs::refresh_credentials`. Pass 8 §2.5.

### States

```
   [user runs jr auth refresh]
              │
              ▼
   ┌─────────────────────────────┐
   │ CURRENT implementation:     │
   │ clear-and-relogin           │
   │  • delete <profile>:oauth-* │
   │    from keychain            │
   │  • clear_profile_cache()   │
   │  • re-invoke handle_login   │
   └─────────────┬───────────────┘
                 │
                 ▼
          Full login flow (SM-01)

   [DEFERRED implementation]
              │
   api::auth::refresh_oauth_token (pub, 0 production callers)
              │
              ▼
   ┌─────────────────────────────┐
   │ resolve refresh credentials  │
   │ RefreshAppSource:            │
   │  Keychain → Embedded only    │
   │  (2-source, not 6-source)    │
   └─────────────┬───────────────┘
                 │
                 ▼
   POST /oauth/token (refresh_token grant)
                 │
                 ├── success → store new tokens, exit 0
                 └── failure → surface original 401
```

### Key Invariants

- Production `jr auth refresh` = clear-and-relogin (destructive but idempotent).
- `refresh_oauth_token` has ZERO production callers. It exists for future 401 auto-refresh integration (NFR-O-B — refresh_oauth_token zero-callers).
- `resolve_refresh_app_credentials` uses only 2 sources (Keychain > Embedded), not the 6-source login chain.
- Callers of `refresh_oauth_token` pass only `profile: &str`. No `client_id`/`client_secret` parameters — they are resolved internally (CLAUDE.md gotcha).

---

## SM-03: Asset Enrichment 3-Pass State Machine

**Source**: `cli/issue/list.rs:390-487`. Pass 1 R1 §4c, NEW-INV-227..231.

### States

```
   [issue page received]
              │
   ── PASS 1: Extract ────────────────────────────────────────
              │
              ▼
   ┌────────────────────────────────────────────────────────┐
   │ for each (issue, field_id) in page:                    │
   │   extract asset id + workspace_id from IssueFields.extra│
   │   dedup key = (workspace_id, object_id)                │
   │   to_enrich: HashMap<(wid, oid), ()>                   │
   │   enrich_indices: Vec<(issue_idx, field_offset)>       │
   │   (positions NOT deduplicated — issue×field pairs)     │
   └─────────────────────────────┬──────────────────────────┘
                                 │
   ── PASS 2: Resolve concurrently ──────────────────────────
                                 │
                                 ▼
   ┌────────────────────────────────────────────────────────┐
   │ workspace_id from cache or API (lazy — only if needed) │
   │ futures::future::join_all(                             │
   │   to_enrich.keys().map(|(wid, oid)| get_asset(...))   │
   │ )  ← M concurrent HTTP calls                          │
   │                                                        │
   │ [BUG NFR-R-E] resolved: HashMap<String, _>            │
   │   keyed by oid ALONE (drops workspace qualifier)       │
   │   last-write-wins on oid collision across workspaces   │
   └─────────────────────────────┬──────────────────────────┘
                                 │
   ── PASS 3: Redistribute ─────────────────────────────────
                                 │
                                 ▼
   ┌────────────────────────────────────────────────────────┐
   │ for each (i, j) in enrich_indices:                     │
   │   resolved.get(&oid) → back-inject into issue[i]       │
   │   [BUG: should be resolved.get(&(wid, oid))]           │
   └────────────────────────────────────────────────────────┘
```

### Key Invariants

- Pass 1 dedup key correctly workspace-qualifies: `(workspace_id, object_id)`.
- Pass 2 concurrent fan-out: M HTTP calls in parallel (no semaphore/buffer cap — 429-storm risk for large M, NFR-P-NEW-1).
- Pass 3 redistribution bug: `resolved` HashMap keyed by `oid` alone → multi-workspace mis-attribution (NFR-R-E HIGH).
- Single-workspace tenants unaffected (oid is unique within a workspace).
- Fix: change `resolved` to `HashMap<(String, String), _>` keyed by `(wid, oid)`.

---

## SM-04: Sprint-Aware Issue List Dispatch

**Source**: `cli/issue/list.rs` board/sprint branching. Pass 1 R1 §4d, NEW-INV-219..222.

### States

```
   [jr issue list invoked]
              │
              ├─ --jql provided? ──► use as JQL base; skip board logic
              │
              └─ no --jql
                   │
                   ├─ no board_id configured ──► project = X  ORDER BY updated DESC
                   │
                   └─ board_id configured
                        │
                        ▼
                   GET /board/<id>/config
                        │
                        ├─ 404 ──► UserError (not silent)
                        │
                        └─ board_type?
                              │
                              ├─ "scrum" ──► GET /board/<id>/sprint?state=active
                              │                  │
                              │                  ├─ active sprint found
                              │                  │      → sprint = N  ORDER BY rank ASC
                              │                  │
                              │                  └─ no active sprint
                              │                         → project = X  ORDER BY updated DESC
                              │                         [SILENT DEGRADE — no warning]
                              │
                              └─ anything else ("kanban", etc.)
                                     → project = X AND statusCategory != Done
                                       ORDER BY rank ASC
                                     [NOTE: cli/sprint.rs hard-errors on kanban;
                                      cli/issue/list.rs silently degrades — asymmetry]
```

### Key Invariants

- No active sprint → silent degradation to project scope (NO `eprintln!`). NEW-INV-219.
- Kanban: JQL `statusCategory != Done ORDER BY rank ASC`. Hardcoded heuristic for WIP region. NEW-INV-220.
- ALL 3 project-clause emit sites apply `jql::escape_value(pk)`. NEW-INV-221.
- Any `board_type` other than `"scrum"` falls to kanban arm (future board types auto-fall through). NEW-INV-222.
- Cross-module asymmetry: `cli/sprint.rs` hard-errors on kanban; `cli/issue/list.rs` silently degrades. Pass 8 §2.5.

---

## SM-05: Cache Entry Lifecycle

**Source**: `cache.rs:14-34`. Pass 1 R1 §4e, Pass 8 §2.5.

### States

```
   ┌─────────┐
   │  MISS   │◄────────────────────────────────────────────────────────┐
   │(Ok(None))│                                                          │
   └────┬────┘                                                          │
        │ network fetch                                                 │
        ▼                                                               │
   ┌──────────┐  write_cache (fs::write — non-atomic)                  │
   │ WRITING  │                                                         │
   └────┬─────┘                                                         │
        ▼                                                               │
   ┌──────────────┐                                                     │
   │  HIT-FRESH   │  read_cache → Ok(Some(T))                          │
   │  (age < 7d)  │                                                     │
   └────┬─────────┘                                                     │
        │ time elapses                                                  │
        ▼                                                               │
   ┌──────────────┐  read_cache → Ok(None)                             │
   │    STALE     │─────────────────────────────────────────────────────┤
   │  (age ≥ 7d)  │                                                     │
   └──────────────┘                                                     │
                                                                        │
   ┌──────────────┐  read_cache → Ok(None) + stderr warning            │
   │   CORRUPT    │─────────────────────────────────────────────────────┘
   │ (deser-fail) │  [file remains on disk until next write]
   └──────────────┘
```

### Key Invariants

- All three non-fresh states (`MISS`, `STALE`, `CORRUPT`) return `Ok(None)`. NEVER `Err`.
- `STALE` and `CORRUPT` are indistinguishable to callers. Only stderr differentiates them.
- Non-atomic write (`fs::write`): crash-window exists. Self-heals via `CORRUPT → MISS` on next read.
- Map-cache writes (project_meta, object_type_attrs) on CORRUPT: silently replace entire map with just the new entry. All other keys lost. NEW-INV-07.
- `clear_profile_cache(name)`: no-op when `~/.cache/jr/v1/<name>/` absent.
- 6 distinct cache categories: 4 use `Expiring` only (teams, workspace, cmdb_fields, resolutions); 1 uses keyed-map only (project_meta); 1 uses both `Expiring` AND keyed-map (object_type_attrs, hybrid).
- `project_meta` uses per-entry TTL; `object_type_attrs` uses file-level TTL. Asymmetric design.

---

## SM-06: Profile Lifecycle (bonus — from Pass 2 §2b.3)

**Source**: `cli/auth.rs:540-1180`.

```
   [nonexistent]
        │ jr auth login --profile NEW  (or jr init for "default")
        │  • creates [profiles.NEW] in config.toml
        │  • writes <NEW>:oauth-* to keychain (or shared api-token)
        ▼
   [exists, inactive]
        │ jr auth switch NEW
        │  • sets default_profile = NEW
        ▼
   [active]
        ├── jr auth switch OTHER     ──► [inactive again]
        │    • default_profile = OTHER
        │
        ├── jr auth logout           ──► [inactive, no tokens]
        │    • deletes <NEW>:oauth-*       │ jr auth login → [active again]
        │    • config entry stays          │
        │
        └── jr auth remove NEW      ──► [nonexistent]
             • config entry removed
             • <NEW>:oauth-* deleted
             • cache/v1/<NEW>/ deleted
             • ERROR if NEW == active
```

### Key Invariant

- `auth remove` fails if the profile is active. You must switch away first.
- `auth logout` keeps the config entry; only tokens are removed. The profile remains "exists, inactive".
- `auth refresh` = clear-tokens + re-login (see SM-02). The profile stays in the config.
