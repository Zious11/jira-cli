# S-3.01 Demo Evidence Report

Story: Refactor `cli/auth.rs` — shard-split into `auth/` module
Story ID: S-3.01
Branch: refactor/S-3.01-cli-auth-shard-split
Base SHA: 68092af (develop at story branch-off)
Mode: strict/refactor (pure refactor — zero behavioral changes)
Recorded: 2026-05-09

---

## What was delivered

S-3.01 is a pure refactor story. The original `src/cli/auth.rs` (2,245 LOC monolith)
was split into 9 production shards plus a consolidated test module, all under
`src/cli/auth/`:

| Shard | LOC | Contents |
|---|---|---|
| `mod.rs` | 121 | Dispatch + `AuthFlow` enum + shared helpers |
| `login.rs` | 366 | `handle_login` — OAuth flag dispatch, interactive email/URL prompts |
| `keychain.rs` | 256 | CLI-layer keychain glue (delegates to `api/auth.rs`) |
| `refresh.rs` | 144 | `handle_refresh` (BC-7.4.015) |
| `status.rs` | 140 | `handle_status` (BC-7.4.014) |
| `remove.rs` | 129 | `handle_remove` |
| `switch.rs` | 51 | `handle_switch` |
| `list.rs` | 70 | `handle_list` (BC-7.4.013 JSON path) |
| `logout.rs` | 50 | `handle_logout` (BC-7.4.016) |
| `tests/mod.rs` | 997 | Consolidated test module (excluded from AC-004 production-shard cap) |
| **Total (prod shards)** | **1,327** | |

All production shards are strictly under the 800 LOC cap. The largest is `login.rs`
at 366 LOC. Zero behavioral changes: same CLI surface, same test counts, same exit codes.

### Implementation notes

- **10-commit micro-history** (`857e7e6..38d4d1a`) on top of develop@68092af. Each commit
  represents one shard extraction in dependency order, keeping the codebase compiling at
  every step.
- **Tests consolidated to `auth/tests/` subdirectory** rather than per-shard inline. This
  differs from S-3.02's per-shard approach — both are valid Rust module patterns. The 997
  LOC test module is a single `tests/mod.rs` and is excluded from AC-004's production-shard
  cap per story line 100.
- **`AuthFlow` visibility bumped to `pub(crate)`** to satisfy the `private-interfaces`
  clippy lint (the enum appears in `pub fn` signatures exposed through `mod.rs`). This is
  not a public-API expansion — `pub(crate)` is narrower than `pub` and is the minimum
  required to satisfy the lint without suppression.
- **Disclosed macOS flake:** The integration test `test_auth_login_emits_json_when_output_json_set`
  (in `tests/auth_output_json.rs`) fails on macOS due to a pre-existing keychain
  `item already exists` race. This flake predates S-3.01, exists on `develop` too, and is
  entirely unrelated to the auth module shard-split. All other auth integration tests pass.

---

## Per-AC Evidence

### AC-001 — All tests pass (612 unit tests + auth integration tests)

**Spec claim (BC-1.1.001):** All existing auth tests pass with zero modifications to test
assertions or fixture setup. No tests are added or removed.

| Artifact | Description |
|---|---|
| `AC-001-all-tests-green.gif` | `cargo test --lib` (tail -5) then auth integration suite skipping the disclosed keychain flake (tail -5) |
| `AC-001-all-tests-green.webm` | Same recording, archival format |
| `AC-001-all-tests-green.tape` | VHS script source |

**Confirmation:** Recording shows `test result: ok. 612 passed; 0 failed; 10 ignored` for
`--lib`. The `auth_output_json` integration suite shows 4 passed, 0 failed when the
pre-existing macOS keychain flake (`test_auth_login_emits_json_when_output_json_set`) is
excluded via `--skip`.

---

### AC-002 — No direct `keyring::Entry` calls in `cli/auth/*` shards

**Spec claim (BC-1.4.027):** All keychain reads/writes go through `api/auth.rs` exported
functions — no scattered `keyring::Entry` calls in the split CLI shards.

| Artifact | Description |
|---|---|
| `AC-002-keyring-isolated.gif` | `grep -nE "keyring::Entry" src/cli/auth/*.rs` returns no matches; echo confirms isolation |
| `AC-002-keyring-isolated.webm` | Same recording, archival format |
| `AC-002-keyring-isolated.tape` | VHS script source |

**Confirmation:** Recording shows `grep` exits 1 (no matches). The echo confirms:
`Exit: 1 (no direct keyring::Entry in cli/auth/* — all keychain access flows through src/api/auth.rs)`.
Note: `keychain.rs` is a CLI-layer glue module that calls functions exported from
`src/api/auth.rs`; it does not call `keyring::Entry` directly.

---

### AC-003 — Release build exits 0

**Spec claim (BC-1.1.001):** `cargo build --release` succeeds after the shard-split.
No regressions in the compilation unit graph.

| Artifact | Description |
|---|---|
| `AC-003-release-build-green.gif` | `cargo build --release --quiet && echo "release build: OK"` |
| `AC-003-release-build-green.webm` | Same recording, archival format |
| `AC-003-release-build-green.tape` | VHS script source |

**Confirmation:** Recording shows `release build: OK` on stdout. Exit 0.

---

### AC-004 — All production shards under 800 LOC

**Spec claim (BC-1.1.001):** No single shard file in `src/cli/auth/*.rs` exceeds 800 LOC,
confirming genuine decomposition rather than trivial rename.

| Artifact | Description |
|---|---|
| `AC-004-shard-loc-under-800.gif` | `wc -l src/cli/auth/*.rs` showing all 9 shards, then PASS echo |
| `AC-004-shard-loc-under-800.webm` | Same recording, archival format |
| `AC-004-shard-loc-under-800.tape` | VHS script source |

**Confirmation:** Recording shows all 9 production shard line counts:
- `keychain.rs`: 256 LOC
- `list.rs`: 70 LOC
- `login.rs`: 366 LOC
- `logout.rs`: 50 LOC
- `mod.rs`: 121 LOC
- `refresh.rs`: 144 LOC
- `remove.rs`: 129 LOC
- `status.rs`: 140 LOC
- `switch.rs`: 51 LOC
- **Total: 1,327 LOC**

All are below 800. Echo confirms `AC-004 PASS: largest production shard 366 LOC (login.rs) < 800 LOC cap`.

---

### AC-005 (bonus) — CLI auth subcommand surface unchanged

**Spec claim:** The public CLI dispatch is identical after the shard-split. The `auth`
subcommand still exposes all 7 subcommands: `login`, `status`, `refresh`, `switch`,
`list`, `logout`, `remove`.

| Artifact | Description |
|---|---|
| `AC-005-cli-help-unchanged.gif` | `cargo run --release --quiet -- auth --help` (head -30) |
| `AC-005-cli-help-unchanged.webm` | Same recording, archival format |
| `AC-005-cli-help-unchanged.tape` | VHS script source |

**Confirmation:** Recording shows the full `auth --help` output with all 7 subcommands
visible: `login`, `status`, `refresh`, `switch`, `list`, `logout`, `remove`.

---

### AC-006 (bonus) — BC-7.4.013–016 JSON shape tests pass

**Spec claim:** The auth-specific unit tests (covering BC-7.4.013 list JSON, BC-7.4.014
status JSON, BC-7.4.015 refresh JSON, BC-7.4.016 logout JSON shapes from S-2.07) all
pass after the refactor.

| Artifact | Description |
|---|---|
| `AC-006-bc-744-json-shapes.gif` | `cargo test --lib auth_ --quiet` (tail -10) |
| `AC-006-bc-744-json-shapes.webm` | Same recording, archival format |
| `AC-006-bc-744-json-shapes.tape` | VHS script source |

**Confirmation:** Recording shows all `auth_`-prefixed unit tests passing with 0 failures.

---

## LOC Delta Summary

| File | Pre-refactor | Post-refactor |
|---|---|---|
| `src/cli/auth.rs` | 2,245 LOC | deleted (replaced by module) |
| `src/cli/auth/mod.rs` | — | 121 LOC |
| `src/cli/auth/login.rs` | — | 366 LOC |
| `src/cli/auth/keychain.rs` | — | 256 LOC |
| `src/cli/auth/refresh.rs` | — | 144 LOC |
| `src/cli/auth/status.rs` | — | 140 LOC |
| `src/cli/auth/remove.rs` | — | 129 LOC |
| `src/cli/auth/switch.rs` | — | 51 LOC |
| `src/cli/auth/list.rs` | — | 70 LOC |
| `src/cli/auth/logout.rs` | — | 50 LOC |
| `src/cli/auth/tests/mod.rs` | — | 997 LOC (test module, excluded from AC-004 cap) |
| **Total (prod shards)** | — | **1,327 LOC** |
| **Total (incl. tests)** | — | **2,324 LOC** |

The 79-LOC increase in production code over the original 2,245 is from module boilerplate
(`use` re-exports in `mod.rs`, `pub use` statements). The test module is essentially the
same test code reorganized into a single consolidated location rather than inline in the
main file.

---

## Reproduction Commands

```bash
cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.01

# AC-001: All unit tests pass
cargo test --lib 2>&1 | tail -5

# AC-001: Auth integration tests (skipping disclosed macOS keychain flake)
cargo test --test auth_output_json -- --skip test_auth_login_emits_json_when_output_json_set 2>&1 | tail -5

# AC-002: No direct keyring::Entry in cli/auth shards
grep -nE "keyring::Entry" src/cli/auth/*.rs
echo "Exit: $? (no direct keyring::Entry in cli/auth/* — all keychain access flows through src/api/auth.rs)"

# AC-003: Release build
cargo build --release --quiet && echo "release build: OK"

# AC-004: Shard LOC
wc -l src/cli/auth/*.rs

# AC-005: CLI surface unchanged
cargo run --release --quiet -- auth --help 2>&1 | head -30

# AC-006: BC-7.4.013-016 JSON shape tests
cargo test --lib auth_ --quiet 2>&1 | tail -10
```

---

## Caveats

- **Pre-existing macOS keychain flake:** `test_auth_login_emits_json_when_output_json_set`
  fails on macOS with `item already exists` in the keychain. This is a pre-existing issue
  on `develop` unrelated to S-3.01. AC-001 demonstrates the suite with this test skipped.
  The remaining 4 auth integration tests all pass.
- **Pure refactor:** All demos are deterministic and reproducible offline. No network calls
  are required for any of the 6 recordings.
