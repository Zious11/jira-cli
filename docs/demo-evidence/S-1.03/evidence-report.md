---
document_type: demo-evidence-report
product: "jr (Jira CLI)"
story_id: S-1.03
pipeline_run: "2026-05-07"
demo_type: cli
recording_tool: vhs
status: complete
---

# Demo Evidence Report — S-1.03

## Product: jr (Jira CLI)
## Story: S-1.03 — Tracing observability wire-up (NFR-O-A)
## Pipeline Run: 2026-05-07
## Branch: feat/S-1.03-tracing-observability

---

## Per-AC Demo Recordings

| AC | Tests | Description | GIF | WebM | Tape | Size (webm) | Status |
|----|-------|-------------|-----|------|------|-------------|--------|
| AC-001 | test_s_1_03_cargo_toml_has_tracing_dep, test_s_1_03_cargo_toml_has_tracing_subscriber_dep | Cargo.toml has pinned tracing + tracing-subscriber (env-filter) deps | [gif](AC-001-cargo-toml-deps.gif) | [webm](AC-001-cargo-toml-deps.webm) | [tape](AC-001-cargo-toml-deps.tape) | 550K | recorded |
| AC-002 | test_s_1_03_main_initializes_tracing_subscriber, test_s_1_03_main_uses_env_filter | main.rs initializes tracing-subscriber with EnvFilter | [gif](AC-002-main-subscriber-init.gif) | [webm](AC-002-main-subscriber-init.webm) | [tape](AC-002-main-subscriber-init.tape) | 520K | recorded |
| AC-003 | test_s_1_03_client_uses_tracing_debug, test_s_1_03_client_no_verbose_request_eprintln | client.rs uses tracing::debug! (method+URL/rate-limit); [verbose] eprintln! replaced | [gif](AC-003-client-tracing-debug.gif) | [webm](AC-003-client-tracing-debug.webm) | [tape](AC-003-client-tracing-debug.tape) | 551K | recorded |
| AC-004 | All 6 verbose_bodies tests | SD-003 contract preserved after tracing wire-up (regression check) | [gif](AC-004-sd-003-regression-preserved.gif) | [webm](AC-004-sd-003-regression-preserved.webm) | [tape](AC-004-sd-003-regression-preserved.tape) | 855K | recorded |
| AC-005 | test_s_1_03_auth_has_tracing_entry_points, test_s_1_03_auth_no_client_secret_in_tracing_fields | auth.rs has tracing entry points; no secrets in tracing fields | [gif](AC-005-auth-tracing-no-secrets.gif) | [webm](AC-005-auth-tracing-no-secrets.webm) | [tape](AC-005-auth-tracing-no-secrets.tape) | 549K | recorded |
| AC-006 | test_s_1_03_lib_rs_does_not_init_subscriber, test_s_1_03_observability_rs_does_not_init_subscriber | Subscriber init guarded in main.rs only; no double-init panics | [gif](AC-006-no-double-init.gif) | [webm](AC-006-no-double-init.webm) | [tape](AC-006-no-double-init.tape) | 611K | recorded |

---

## Combined Demo

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| 10/10 observability green | All S-1.03 observability tests pass together | [gif](AC-combined-all-s-1-03-pass.gif) | [webm](AC-combined-all-s-1-03-pass.webm) | [tape](AC-combined-all-s-1-03-pass.tape) | 946K | recorded |

---

## Bonus Demo — Live Tracing in Action

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| BONUS | Default WARN level (silent) vs RUST_LOG=debug (tracing active) | [gif](BONUS-live-tracing.gif) | [webm](BONUS-live-tracing.webm) | [tape](BONUS-live-tracing.tape) | 968K | recorded |

The bonus demo shows two things in sequence:
1. `cargo run --quiet -- --help 2>&1 | head -5` — normal CLI output at default WARN level; no tracing noise
2. `RUST_LOG=debug cargo test --test observability test_s_1_03_main_initializes_tracing_subscriber -- --nocapture ...` — confirms the tracing-subscriber infrastructure is live and RUST_LOG controls log emission at runtime

---

## AC Coverage Summary

| AC | Story | Requirement | NFR | Demos |
|----|-------|-------------|-----|-------|
| AC-001 | S-1.03 | Cargo.toml has `tracing` and `tracing-subscriber` with explicit version pins | NFR-O-A | AC-001 + combined |
| AC-002 | S-1.03 | main.rs initializes tracing-subscriber with EnvFilter (WARN default, escalates on --verbose) | NFR-O-A | AC-002 + combined + bonus |
| AC-003 | S-1.03 | client.rs uses tracing::debug! for method+URL and rate-limit events; [verbose] eprintln! replaced | NFR-O-A | AC-003 + combined |
| AC-004 | S-1.03 | SD-003 contract preserved: verbose_bodies 6/6 still pass after tracing wire-up | SD-003 regression | AC-004 + combined |
| AC-005 | S-1.03 | auth.rs has tracing entry points at OAuth flow boundaries; no secret values in field lists | NFR-O-A (security) | AC-005 + combined |
| AC-006 | S-1.03 | Subscriber init guarded in main.rs only; lib.rs and observability.rs do not call .init() | NFR-O-A (architecture) | AC-006 + combined |

---

## Visual Review Summary

| Demo | AC | Outcome | Notes |
|------|----|---------|-------|
| AC-001-cargo-toml-deps | AC-001 | 2 passed; 0 failed | tracing + tracing-subscriber with env-filter confirmed |
| AC-002-main-subscriber-init | AC-002 | 2 passed; 0 failed | tracing_subscriber:: + EnvFilter both in main.rs |
| AC-003-client-tracing-debug | AC-003 | 2 passed; 0 failed | tracing::debug! present; [verbose] eprintln! request lines removed |
| AC-004-sd-003-regression-preserved | AC-004 | 6 passed; 0 failed | Full verbose_bodies suite green; SD-003 regression prevented |
| AC-005-auth-tracing-no-secrets | AC-005 | 2 passed; 0 failed | Auth tracing entries present; client_secret/token values not logged |
| AC-006-no-double-init | AC-006 | 2 passed; 0 failed | lib.rs and observability.rs do not call subscriber .init() |
| AC-combined-all-s-1-03-pass | ALL | 10 passed; 0 failed | Full observability suite green |
| BONUS-live-tracing | AC-002 | 1 passed; 0 failed | RUST_LOG=debug confirms tracing subscriber is live at runtime |

---

## Toolchain

| Tool | Version | Status |
|------|---------|--------|
| VHS | 0.11.0 | installed |
| cargo | 1.94.0 (85eff7c80 2026-01-15) | installed |
| Playwright | N/A | not applicable (CLI product) |
| Font | Menlo (system) | installed |

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | clean |
| `cargo build --release` | clean |
| `cargo test --test observability` | 10/10 passed |
| `cargo test --test verbose_bodies` | 6/6 passed |
| `cargo clippy --all --all-features --tests -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## PR Embedding Snippet

```markdown
## Demo Evidence — S-1.03: Tracing observability wire-up

### Combined: 10/10 observability tests green
![AC-combined-all-s-1-03-pass](docs/demo-evidence/S-1.03/AC-combined-all-s-1-03-pass.gif)

### BONUS: Live tracing — default WARN level vs RUST_LOG=debug
![BONUS-live-tracing](docs/demo-evidence/S-1.03/BONUS-live-tracing.gif)

Full per-AC recordings: [docs/demo-evidence/S-1.03/](docs/demo-evidence/S-1.03/)
```

---

## Implementation Notes

- **NFR-O-A satisfied:** `tracing` and `tracing-subscriber` (with `env-filter`) added to Cargo.toml with explicit version pins.
- **main.rs subscriber init:** `tracing_subscriber::fmt()` initialized with `EnvFilter` — default level WARN, escalates when `--verbose` or `--verbose-bodies` flags are set.
- **client.rs migration:** `[verbose]` method+URL and rate-limit `eprintln!` calls replaced with `tracing::debug!` structured events. PII warning banner (`[jr] WARNING:`) intentionally kept as `eprintln!` (user-visible, not a log event).
- **auth.rs tracing:** `tracing::debug!` / `tracing::info!` events added at `oauth_login`, `exchange_code_for_token`, and `refresh_oauth_token` entry points. `client_secret`, `access_token`, and `refresh_token` values are never passed as tracing field arguments.
- **Double-init guard:** `tracing_subscriber::...init()` lives only in `src/main.rs`. `lib.rs` and `observability.rs` do not call `.init()`, preventing double-init panics in test mode.
- **SD-003 regression preserved:** All 6 `verbose_bodies` tests still pass — adding tracing did not disturb the PII warning, body suppression, or `--verbose` stacking behavior.
- **Commits covered:** `18a63a3` (Red Gate tests), `59d48e3` (implementation)
- **All 10 observability tests + 6 SD-003 tests pass; 600+ lib + integration tests preserved; clippy and fmt clean**
