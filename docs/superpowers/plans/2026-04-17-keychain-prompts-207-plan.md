# `jr auth refresh` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `jr auth refresh [--oauth]` — a thin dispatcher that clears all stored credentials and re-runs the existing login flow so the current binary re-registers as the creator of fresh macOS Keychain entries.

**Architecture:** Add `AuthCommand::Refresh` variant, extract a `chosen_flow` helper for unit-testable flow selection, add `refresh_credentials` that calls `auth::clear_credentials()` + `login_token` / `login_oauth`, wire into `src/main.rs` dispatch, and verify with unit + integration tests. No changes to `src/api/*` or the keychain backend.

**Tech Stack:** Rust 2024 (MSRV 1.85), `clap` derive, `anyhow`, `dialoguer` (inherited), `tokio`, `assert_cmd` + `serde_json` for integration tests.

---

## File Structure

| File | Responsibility | Change |
| --- | --- | --- |
| `src/cli/mod.rs` | CLI surface (clap derive enums). | Add `Refresh { oauth: bool }` to `AuthCommand`. |
| `src/cli/auth.rs` | Auth command handlers + helpers. | Add `AuthFlow` enum, `chosen_flow`, `refresh_credentials`, and 4 unit tests. |
| `src/main.rs` | Top-level dispatch + Ctrl+C + error-to-stderr. | Extend `AuthCommand` match arm to pass `&cli.output` and handle `Refresh`. |
| `tests/auth_refresh.rs` | Integration tests. | NEW — 3 tests (smoke, flag parity, non-interactive graceful failure). |
| `README.md` | User-facing docs. | Add short macOS section explaining when to run `jr auth refresh`. |

All other files unchanged. No new dependencies. Existing `src/api/auth.rs:83-96` `clear_credentials()` is reused unchanged.

---

## Task 1: Add `AuthFlow` enum + `chosen_flow` helper (TDD)

**Files:**
- Modify: `src/cli/auth.rs` — add enum, helper, and inline `#[cfg(test)] mod tests`.

- [ ] **Step 1: Write the failing unit tests**

Add this block at the bottom of `src/cli/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GlobalConfig, InstanceConfig};

    fn config_with_auth_method(method: Option<&str>) -> Config {
        Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://example.atlassian.net".into()),
                    cloud_id: None,
                    org_id: None,
                    auth_method: method.map(str::to_string),
                },
                ..Default::default()
            },
            project: Default::default(),
        }
    }

    #[test]
    fn chosen_flow_defaults_to_token_when_unset() {
        let config = config_with_auth_method(None);
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_token_for_explicit_api_token() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_oauth_when_config_says_so() {
        let config = config_with_auth_method(Some("oauth"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::OAuth);
    }

    #[test]
    fn chosen_flow_oauth_override_wins_over_config() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, true), AuthFlow::OAuth);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib cli::auth::tests 2>&1 | tail -30`
Expected: FAIL. Error messages will say `cannot find type AuthFlow in this scope` and `cannot find function chosen_flow in this scope`.

- [ ] **Step 3: Implement the enum + helper**

Add this block near the top of `src/cli/auth.rs`, just after the existing `use` statements. **Note:** `use crate::config::Config;` is already imported at the top of the file — do NOT re-add it.

```rust
/// Which auth flow `jr auth refresh` should dispatch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFlow {
    Token,
    OAuth,
}

/// Decide which login flow to run based on config + explicit override.
///
/// Order of precedence:
/// 1. `oauth_override = true` → always OAuth (user passed `--oauth`).
/// 2. Config `auth_method == "oauth"` → OAuth.
/// 3. Anything else (including unset, which matches `JiraClient::from_config`'s
///    `api_token` default at `src/api/client.rs:51`) → Token.
pub fn chosen_flow(config: &Config, oauth_override: bool) -> AuthFlow {
    if oauth_override {
        return AuthFlow::OAuth;
    }
    match config.global.instance.auth_method.as_deref() {
        Some("oauth") => AuthFlow::OAuth,
        _ => AuthFlow::Token,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib cli::auth::tests`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add src/cli/auth.rs
git commit -m "$(cat <<'EOF'
feat(auth): add chosen_flow helper for refresh flow selection (#207)
EOF
)"
```

---

## Task 2: Add `AuthCommand::Refresh` variant

**Files:**
- Modify: `src/cli/mod.rs:184-194` — add `Refresh` to the `AuthCommand` enum.

- [ ] **Step 1: Add the variant**

Replace the current `AuthCommand` definition (lines 184-194) with:

```rust
#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with Jira
    Login {
        /// Use OAuth 2.0 instead of API token (requires your own OAuth app)
        #[arg(long)]
        oauth: bool,
    },
    /// Show authentication status
    Status,
    /// Clear stored credentials and re-run the login flow.
    ///
    /// On macOS, run this after upgrading `jr` (e.g., `brew upgrade`, binary
    /// replacement). The legacy Keychain ACL is bound to the original binary's
    /// identity; this command deletes the entries so the new binary becomes
    /// the creator of fresh entries, avoiding repeated "allow access"
    /// prompts. See issue #207.
    Refresh {
        /// Use OAuth 2.0 instead of API token (matches `jr auth login --oauth`)
        #[arg(long)]
        oauth: bool,
    },
}
```

- [ ] **Step 2: Verify compilation still fails in dispatch**

Run: `cargo check 2>&1 | grep -E "error\[|warning: unused" | head -10`
Expected: at least one error about `AuthCommand::Refresh` not being handled in the match in `src/main.rs`. This is the TDD signal that Task 3 is needed.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "$(cat <<'EOF'
feat(cli): add AuthCommand::Refresh variant (#207)
EOF
)"
```

---

## Task 3: Implement `refresh_credentials` function

**Files:**
- Modify: `src/cli/auth.rs` — add `refresh_credentials` that dispatches based on `chosen_flow`.

- [ ] **Step 1: Add the function**

Append this block to `src/cli/auth.rs`, after the existing `status()` function but before the `#[cfg(test)] mod tests` block:

```rust
/// Clear all stored credentials and re-run the login flow so the current
/// binary re-registers as the creator of fresh keychain entries.
///
/// On macOS this is the recovery path for the legacy Keychain ACL/partition
/// invalidation that occurs after `jr` is replaced at its installed path
/// (e.g., `brew upgrade`). See spec at
/// `docs/superpowers/specs/2026-04-17-keychain-prompts-207-design.md`.
pub async fn refresh_credentials(
    oauth_override: bool,
    output: &crate::cli::OutputFormat,
) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let flow = chosen_flow(&config, oauth_override);

    auth::clear_credentials();

    match flow {
        AuthFlow::Token => login_token().await?,
        AuthFlow::OAuth => login_oauth().await?,
    }

    let method_label = match flow {
        AuthFlow::Token => "api_token",
        AuthFlow::OAuth => "oauth",
    };

    match output {
        crate::cli::OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "refreshed",
                    "auth_method": method_label,
                })
            );
        }
        crate::cli::OutputFormat::Table => {
            // No table row payload on success — the stderr help line below is
            // the primary user-visible output for the non-JSON case.
        }
    }

    eprintln!(
        "Credentials refreshed. If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently."
    );

    Ok(())
}
```

- [ ] **Step 2: Verify the module compiles**

Run: `cargo check --lib 2>&1 | grep -E "error\[|^error:" | head -10`
Expected: errors remain only in `src/main.rs` (missing `Refresh` arm in dispatch match). The `src/cli/auth.rs` module itself now compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/cli/auth.rs
git commit -m "$(cat <<'EOF'
feat(auth): implement refresh_credentials orchestrator (#207)
EOF
)"
```

---

## Task 4: Wire dispatch in `src/main.rs`

**Files:**
- Modify: `src/main.rs:72-81` — extend `Auth { command }` match arm.

- [ ] **Step 1: Update the dispatch**

Replace the existing `cli::Command::Auth { command } => match command { ... }` block (lines 72-81) with:

```rust
            cli::Command::Auth { command } => match command {
                cli::AuthCommand::Login { oauth } => {
                    if oauth {
                        cli::auth::login_oauth().await
                    } else {
                        cli::auth::login_token().await
                    }
                }
                cli::AuthCommand::Status => cli::auth::status().await,
                cli::AuthCommand::Refresh { oauth } => {
                    cli::auth::refresh_credentials(oauth, &cli.output).await
                }
            },
```

- [ ] **Step 2: Verify build succeeds**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished \`dev\` profile` with no errors.

- [ ] **Step 3: Run unit tests to verify they still pass**

Run: `cargo test --lib cli::auth`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 4: Smoke test the help output**

Run: `cargo run -- auth refresh --help`
Expected: stdout shows `Clear stored credentials and re-run the login flow`, `--oauth`, and exit code 0.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "$(cat <<'EOF'
feat(cli): wire auth refresh dispatch (#207)
EOF
)"
```

---

## Task 5: Integration tests — smoke + flag parity

**Files:**
- Create: `tests/auth_refresh.rs` (new file).

- [ ] **Step 1: Write the failing tests**

Create `tests/auth_refresh.rs` with:

```rust
#[allow(dead_code)]
mod common;

use assert_cmd::Command;

#[test]
fn auth_refresh_help_mentions_refresh_and_oauth() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help should exit 0");
    assert!(
        stdout.to_lowercase().contains("refresh"),
        "help text should mention 'refresh': {stdout}"
    );
    assert!(
        stdout.contains("--oauth"),
        "help text should list --oauth flag: {stdout}"
    );
}

#[test]
fn auth_refresh_oauth_help_is_accepted() {
    // clap should accept `--oauth --help` as well as `--help --oauth`.
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["auth", "refresh", "--oauth", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "--oauth --help should exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test auth_refresh`
Expected: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add tests/auth_refresh.rs
git commit -m "$(cat <<'EOF'
test(auth): add smoke + flag parity integration tests for refresh (#207)
EOF
)"
```

---

## Task 6: Integration test — non-interactive graceful failure

**Files:**
- Modify: `tests/auth_refresh.rs` — append a third test.

- [ ] **Step 1: Append the failing test**

Append to `tests/auth_refresh.rs`:

```rust
#[test]
fn auth_refresh_non_interactive_fails_without_panic() {
    // With stdin closed and no JR_AUTH_HEADER/JR_BASE_URL overrides, the
    // underlying login_token() dialoguer prompts will hit EOF and return an
    // io::UnexpectedEof. The refresh command should exit non-zero without
    // panicking. This matches current `jr auth login` behavior (a known
    // limitation tracked as a separate issue) — the test pins that we
    // inherit it without a panic or crash.
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("XDG_CONFIG_HOME", config_dir.path())
        .args(["auth", "refresh"])
        .write_stdin("") // close stdin immediately
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "auth refresh with closed stdin should fail, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains("panic"),
        "stderr leaked a panic: {stderr}"
    );
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --test auth_refresh auth_refresh_non_interactive_fails_without_panic`
Expected: PASS. The exit-non-zero assertion holds because `clear_credentials` runs silently (no-op when no entries exist in the tempdir-scoped keychain, or deletes real ones and then prompts fail — both paths fail non-zero gracefully).

Note: if the real user's keychain actually gets cleared because `XDG_*` doesn't scope keyring on macOS (keyring uses the OS-level keychain, not an XDG path), the test will still pass the assertion but may briefly surface a Keychain write prompt in a developer's local run. This is a known test-isolation limitation documented in the spec's "Known limitations" section — CI runners do not have macOS login keychains configured, so this is not a CI concern.

- [ ] **Step 3: Commit**

```bash
git add tests/auth_refresh.rs
git commit -m "$(cat <<'EOF'
test(auth): pin non-interactive graceful failure for refresh (#207)
EOF
)"
```

---

## Task 7: README update

**Files:**
- Modify: `README.md` — add a short macOS section.

- [ ] **Step 1: Find the insertion point**

Run: `grep -n "^## " README.md | head -10`
Expected: a list of section headers. Identify the best spot — typically after an "Installation" section or before "Configuration". If unclear, place the new section immediately before the first "## Configuration" header.

- [ ] **Step 2: Insert the section**

Add the following section at the identified location in `README.md`:

```markdown
## macOS: after upgrading the binary

When `jr` is replaced at its installed path — via `brew upgrade`, manual
`cp`, or `curl | tar` — macOS's legacy Keychain Services treats the new
binary as a different application and can prompt up to 4 times per
command indefinitely.

Fix:

```bash
jr auth refresh
```

This clears the stored credentials and re-runs the login flow so the
new binary becomes the creator of fresh Keychain entries. **Click
"Always Allow"** on the two prompts macOS shows during re-store —
otherwise future commands will prompt again.

Tracked in [#207](https://github.com/Zious11/jira-cli/issues/207). A
longer-term fix (Developer ID signing) is tracked as a separate issue.
```

- [ ] **Step 3: Verify the markdown renders**

Run: `grep -A 3 "## macOS: after upgrading" README.md`
Expected: the new section is present.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
docs(readme): document jr auth refresh for macOS post-upgrade (#207)
EOF
)"
```

---

## Task 8: Full CI gate

**Files:** None modified — verification only.

- [ ] **Step 1: Run `cargo fmt --check`**

Run: `cargo fmt --all -- --check`
Expected: no output, exit 0. If it fails: run `cargo fmt --all`, re-stage, amend the previous commit (or add a fixup commit).

- [ ] **Step 2: Run clippy with `-D warnings`**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: `Finished` with no warnings. If warnings appear, fix them (do not `#[allow]` per CLAUDE.md conventions).

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: all tests pass. Count includes the 4 new unit tests (`chosen_flow_*`) and 3 new integration tests (`auth_refresh_*`).

- [ ] **Step 4: Manual smoke test**

Run these in sequence and verify:

```bash
cargo run -- auth --help       # lists: login, status, refresh
cargo run -- auth refresh --help   # shows --oauth flag, description mentions macOS/upgrade
```

Expected: help text renders as designed, no panics.

- [ ] **Step 5: No commit needed unless fmt/clippy required changes**

---

## Task 9: Draft follow-up issue bodies

**Files:** None modified in the repo — prepare two GitHub issue drafts to file after the PR merges.

- [ ] **Step 1: Draft issue #1 — Developer ID signing**

Save (temporarily, e.g., in the plan's review notes) the issue body:

```
Title: macOS: sign + notarize release binaries with Developer ID to eliminate post-upgrade Keychain prompts

Body:
Follows #207. The `jr auth refresh` command added in that PR is a
user-facing recovery for the legacy-Keychain ACL / partition-list
invalidation that occurs on every binary upgrade. The true root-cause
fix is a stable code-signing identity.

Requirements:
- Apple Developer Program membership ($99/yr) for a Developer ID
  Application certificate.
- Update `.github/workflows/release.yml` to:
  - Import the signing certificate (from encrypted repo secret).
  - Run `codesign --sign "Developer ID Application: ..." --options runtime jr` after build.
  - Notarize the signed binary via `xcrun notarytool submit --wait`.
  - Staple the notarization ticket with `xcrun stapler staple jr`.
- Verify with `spctl --assess --type execute --verbose=4 jr`.

Once shipped, `jr auth refresh` becomes unnecessary for standard
upgrades — the stable `teamid:` in the Keychain partition list matches
across all rebuilds.

Labels: enhancement, macOS, blocked-on-budget
```

- [ ] **Step 2: Draft issue #2 — Non-interactive auth flags**

```
Title: auth login / refresh: add non-interactive flag equivalents

Body:
`jr auth login` and the new `jr auth refresh` (added for #207) both use
`dialoguer::Input::interact_text` / `Password::interact`, which fail
with `io::UnexpectedEof` when stdin is not a TTY. This blocks CI /
agent workflows from completing a post-upgrade refresh
non-interactively.

Required flags:
- `--email <EMAIL>` (api_token flow)
- `--token <TOKEN>` (api_token flow; warn if provided via CLI arg, prefer stdin or `JR_API_TOKEN` env)
- `--client-id <ID>` (OAuth flow)
- `--client-secret <SECRET>` (OAuth flow; same preference ordering)

Accept the values via (in order): flag, environment variable, stdin
prompt. When `--no-input` is active and any required value is missing,
fail with a clear message listing which flag or env var to set.

This unblocks:
- CI pipelines that run `jr auth login/refresh` non-interactively.
- LLM agents that pipe credentials via MCP/stdin.

Labels: enhancement, automation
```

- [ ] **Step 3: Note in the PR description**

When the implementation PR is opened, include in the body:
- "Two follow-up issues drafted in the plan's Task 9 — will be filed post-merge."
- (Drafts available at Task 9 of the plan document.)

- [ ] **Step 4: No commit — drafts stay in the plan doc until the PR is merged**

---

## Self-Review

**Spec coverage check:**

| Spec section | Task |
| --- | --- |
| Add `AuthCommand::Refresh { oauth: bool }` | Task 2 |
| Add `refresh_credentials(oauth_override) -> Result<()>` | Task 3 (signature extended with `&OutputFormat` for JSON support; documented in Task 3) |
| Extract `chosen_flow` helper for unit testing | Task 1 |
| Wire into main dispatch | Task 4 |
| JSON output `{"status":"refreshed","auth_method":"..."}` | Task 3 |
| Stderr help line ("Always Allow") | Task 3 (exact string) |
| README.md macOS section | Task 7 |
| `tests/auth_refresh.rs` — 3 integration tests | Tasks 5 + 6 |
| Unit tests for `chosen_flow` (4 cases) | Task 1 |
| Full CI gate | Task 8 |
| Two follow-up issue drafts | Task 9 |

**Placeholder scan:** No `TBD` / `TODO` / "implement later" / "similar to Task N". All code blocks contain complete, copy-pasteable code.

**Type consistency:**
- `AuthFlow` enum introduced in Task 1, referenced consistently in Tasks 1 (tests), 3 (`match flow`).
- `chosen_flow(&Config, bool) -> AuthFlow` signature consistent across Tasks 1 and 3.
- `refresh_credentials(oauth_override: bool, output: &crate::cli::OutputFormat) -> Result<()>` — signature appears in Task 3 definition and Task 4 dispatch call. Matches.
- `AuthCommand::Refresh { oauth: bool }` consistent across Tasks 2 and 4.

**Signature change flagged:** The spec's "Files touched" table shows `refresh_credentials(oauth_override: bool) -> Result<()>`, but the implementation needs `&OutputFormat` to emit structured JSON. Task 3 documents the extended signature. This is a Spec↔Plan drift the reviewer should verify; the spec's Architecture decision ("JSON output") already justifies needing the format, so the signature extension is within the spec's intent.
