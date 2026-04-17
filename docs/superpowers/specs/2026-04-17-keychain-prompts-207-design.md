# `jr auth refresh` — Design Spec

**Issue:** #207 — macOS Keychain prompts 4 times on every command after binary replacement (unsigned-binary ACL + partition-list invalidation).

**Scope:** Ship `jr auth refresh` as the immediate user-recovery path. File a separate follow-up issue to track Developer ID signing + notarization as the true root-cause fix.

**Non-goals:**
- Migrate the macOS backend to the modern Data Protection keychain (`SecItem*` API). Validated as not helpful for unsigned CLIs: macOS still prompts on keychain reads from unsigned binaries regardless of which API is used. Apple's guidance (Quinn, DevForum thread 649081) for the Data Protection keychain assumes a provisioning profile.
- Ad-hoc codesign in release CI (`codesign -s -`). Validated against Apple TN2206: cdhash is a byte-hash of the binary, so every rebuild produces a new cdhash, and legacy Keychain ACL/partition checks still fail on upgrade. Only a Developer ID signature (stable `teamid`) survives rebuilds, and that is tracked separately.
- Add `Logout` to `AuthCommand`. `clear_credentials()` stays internal (used by `Refresh`). `Logout` can be added when there is a concrete use case.
- Back-populate `--email` / `--token` / `--client-id` / `--client-secret` non-interactive flags on `auth login`. This is a pre-existing gap (`auth login` has only `--oauth`). `auth refresh` inherits the gap. See "Known limitations".

---

## Motivation

When the `jr` binary is replaced at its installed path — via `brew upgrade`, manual `cp`, `curl | tar`, or any other mechanism — macOS shows **4 Keychain prompts on every command**, indefinitely. Neither "Allow" nor "Always Allow" persists across invocations, because the binary's identity keeps changing.

The root cause, verified against the `keyring` v3.6.3 source (`hwchen/keyring-rs@v3.6.3/src/macos.rs`) and Apple TN2206:

1. `keyring` v3.6.3 with `apple-native` uses `security_framework::os::macos::passwords::find_generic_password`, which wraps the **legacy** `SecKeychainFindGenericPassword` API (file-based keychain).
2. The legacy API enforces two independent access-control layers per read — trusted-application ACL **and** partition ID list — each of which fails with `errSecCSUnsigned` (-67050) for an unsigned replacement binary and triggers its own prompt.
3. `src/api/auth.rs:31-39` `load_api_token()` reads **two** entries (`email` + `api-token`). `2 entries × 2 checks = 4 prompts.`
4. `jr auth login` does not fix it — `set_password` updates the value but does not transfer ACL ownership. The original creator's identity stays in the item's ACL.

Running manually `security delete-generic-password -s jr-jira-cli -a <key>` followed by `jr auth login` does fix it (empirically verified by the issue's reporter). This spec exposes that fix as a built-in command.

## Scope decision

Four realistic options were considered (see Rejected alternatives). Chosen: **`jr auth refresh` subcommand** (Option G). It:

- Fixes existing broken installations without requiring users to learn `security(1)`.
- Keeps the PR small and reversible.
- Uses only existing primitives (`clear_credentials` + `login_token` / `login_oauth`).
- Does not foreclose the eventual Developer ID fix.

## Design

### New subcommand

```
jr auth refresh [--oauth]
```

**Semantics:** Delete all stored credentials from the keychain, then re-run the same login flow `jr auth login` would use — so the current binary becomes the creator of fresh keychain entries.

**Flag:**
- `--oauth`: force the OAuth login flow even if the stored `auth_method` is `api_token`. Matches the existing `jr auth login --oauth` shape so users don't need to remember a different flag.

**Behavior:**

1. Load the global config (`Config::load()`). Extract `config.global.instance.auth_method`; default `"api_token"` if unset (matches `src/api/client.rs:51` behavior).
2. Call `auth::clear_credentials()` — best-effort deletion of all 6 keychain entries (`email`, `api-token`, `oauth-access-token`, `oauth-refresh-token`, `oauth_client_id`, `oauth_client_secret`). Already wraps each delete in `if let Ok`, so missing entries do not fail the refresh.
3. Dispatch:
   - `--oauth` flag set, OR `auth_method == "oauth"` → `login_oauth()`.
   - Otherwise → `login_token()`.
4. After the login flow succeeds, print a help line to stderr:
   > `Credentials refreshed. If prompted to allow keychain access, choose "Always Allow" so future commands run silently.`
5. Return `Ok(())`. Exit code 0.

**Idempotency:** Running `jr auth refresh` on a clean install (no keychain entries) is equivalent to `jr auth login`. Safe to run more than once.

**Post-refresh read silence:** Empirically, after the two `set_password` writes (email, api-token), macOS shows two write prompts of the form "allow this application to add to your keychain", each with Deny / Allow / Always Allow. If the user clicks "Always Allow", the new binary is added to the item's ACL and subsequent reads are silent until the next binary replacement. If the user clicks "Allow" (single-use), the next command will prompt again — this is why the spec wires in the stderr help line.

### Files touched

| File | Change |
| --- | --- |
| `src/cli/mod.rs` | Add `Refresh { #[arg(long)] oauth: bool }` variant to `AuthCommand`. |
| `src/cli/auth.rs` | Add `pub async fn refresh_credentials(oauth_override: bool) -> Result<()>`. Thin dispatcher over `clear_credentials` + `login_token` / `login_oauth`. |
| CLI dispatch (`main.rs` or wherever `AuthCommand` is matched) | Add `AuthCommand::Refresh { oauth } => auth::refresh_credentials(oauth).await`. Implementor verifies exact match-site during task 1. |
| `README.md` | Add a short "macOS users" section: when and why to run `jr auth refresh`, with the "Always Allow" instruction. |
| `src/cli/auth.rs` inline tests | Extract a `chosen_flow(config, oauth_override) -> AuthFlow` helper; unit-test the four combinations. |
| `tests/auth_refresh.rs` (NEW) | 3 integration tests — smoke (`--help`), flag parity (`--oauth` accepted), non-interactive failure mode (stdin closed → non-zero exit, not a panic). |

No changes to `Cargo.toml`, no new dependencies, no changes to `src/api/*` or the keychain backend. Pure surface-area addition.

### JSON output

When `--output json` is set globally, success emits `{"status":"refreshed","auth_method":"api_token"|"oauth"}`. Errors go through the existing `JrError` → JSON path in `src/error.rs` and `src/main.rs` — no new code needed on the error side.

### Error handling

- `clear_credentials()` swallows per-entry errors; a missing entry is normal during refresh.
- The re-login step propagates `anyhow::Error` via `?`. If `dialoguer::Input::interact_text()` returns an EOF error (stdin closed / non-TTY), it propagates naturally — the process exits non-zero. No new handling required; this matches `jr auth login` today.
- Ctrl+C during the prompt produces SIGINT → exit 130 via the existing Ctrl+C handler in `main.rs`.

### Testing

The command is a thin orchestrator, so tests target the orchestration seam, not the underlying keychain work.

**Unit tests (`src/cli/auth.rs`, inline `#[cfg(test)]`):**

- `chosen_flow` with `auth_method = None, oauth_override = false` → `AuthFlow::Token`.
- `chosen_flow` with `auth_method = Some("api_token"), oauth_override = false` → `AuthFlow::Token`.
- `chosen_flow` with `auth_method = Some("oauth"), oauth_override = false` → `AuthFlow::OAuth`.
- `chosen_flow` with `auth_method = Some("api_token"), oauth_override = true` → `AuthFlow::OAuth`.

**Integration tests (`tests/auth_refresh.rs`, NEW):**

- `jr auth refresh --help` exits 0 and stdout contains both `refresh` and `--oauth`.
- `jr auth refresh --oauth --help` exits 0 (clap accepts the flag).
- `jr auth refresh` with stdin closed and `JR_AUTH_HEADER` / `JR_BASE_URL` unset exits non-zero, stderr does not contain `panic`. Mirrors the `#187` stderr-assertion pattern.

**Deliberately skipped:**

- No test that actually deletes or recreates real keychain entries. `keyring` already tests its platform backends; CI runners have no keychain; mocking the OS keychain buys no real coverage.
- No macOS-specific CI gate. The fix compiles and runs identically on Linux (where it has no effect, because there is no ACL problem). The command works the same way on all platforms.

### Known limitations

**Non-interactive mode is not supported today.** `jr auth login` does not yet accept `--email` / `--token` / `--client-id` / `--client-secret` flags, so it fails in non-TTY contexts. `jr auth refresh` inherits this gap. For CI / agent workflows this means:
- The user needs to run `jr auth refresh` interactively on first upgrade, then agent commands work silently thereafter (until the next upgrade).
- Adding non-interactive flag equivalents is out of scope here; will be filed as a follow-up issue ("auth login / refresh: non-interactive flag equivalents").

**Still requires user action on every upgrade.** This spec is the mitigation, not the cure. The cure is Developer ID signing, tracked as a separate issue below.

## Caveats

**Ad-hoc codesign does not survive binary upgrades.** Validated against Apple TN2206: `cdhash` is a hash of the binary's exact bytes, so every rebuild has a different cdhash. The legacy Keychain's partition ID list can only trust the cdhash directly (ad-hoc has no `teamid`), so the trust does not transfer across rebuilds. This is why the spec does **not** add an ad-hoc codesign step to the release CI.

**The modern Data Protection keychain does not help unsigned CLIs.** Apple's `kSecUseDataProtectionKeychain` flag requires a provisioning profile (Quinn, DevForum thread 649081). An unsigned CLI calling `SecItemAdd` / `SecItemCopyMatching` still prompts — you've traded one prompt shape for another without fixing the ownership problem. This is why the spec does **not** migrate to keyring v4's `use_apple_protected_store` or depend directly on `security-framework::passwords`.

**Only Developer ID + notarization is a real cure.** The partition ID list has a `teamid:` entry type; a Developer-ID-signed binary with stable `teamid` will match that entry across all rebuilds. This is tracked as a follow-up issue, not implemented in this spec.

**Post-refresh silence depends on user clicking "Always Allow".** Clicking "Allow" (one-time) leaves the next read still unprompted via the creator bit, but the behavior starts to diverge between macOS versions and is not guaranteed. The stderr help line makes the correct choice explicit; users who click "Allow" will need to re-refresh after the next command triggers a prompt.

## Architecture decisions

**Thin dispatcher, not a new keychain primitive.** The refresh flow is `clear_credentials` + existing login flow. Introducing a new primitive would duplicate logic and create a test surface that needs mocking. A dispatcher that selects between `login_token` and `login_oauth` based on config is the minimum viable addition.

**Naming: `refresh` matches `gh auth refresh` prior art.** GitHub CLI's `gh auth refresh` also re-stores credentials to fix credential-store ownership after upgrades (per its man page). Users coming from `gh` will have the right mental model. `kubectl`, `gcloud`, and `aws-cli` do not have a single-command equivalent — they require manual clear + re-login, which is exactly the friction this command removes.

**Unit-testable seam is `chosen_flow`, not `refresh_credentials`.** `refresh_credentials` itself would require mocking `clear_credentials`, `login_token`, and `login_oauth` — three mocks for a three-step function, which is low-value. Testing the flow-selection helper gives the same coverage for the one piece of real logic without the mocking overhead.

## Rejected alternatives

- **Ad-hoc codesign in release CI only (Option B).** Invalidated by cdhash-instability (Apple TN2206). No effect on upgrades.
- **Modern `SecItem` migration (Option A / D).** Invalidated for unsigned CLIs (Quinn, DevForum 649081). Does not help our distribution model.
- **Move credentials to a mode-600 config file on macOS (Option H).** Weaker security posture for a credentials-handling CLI. Rejected.
- **Detect and self-heal on `errSecCSUnsigned` during `get_password` (Option E).** Logically broken: during `get_password` the credentials are inside the keychain behind the prompts. The CLI does not have them in memory to re-store.
- **Add `Logout` subcommand alongside `Refresh`.** YAGNI — the issue does not require it. `clear_credentials` is not dead code if `Refresh` uses it. `Logout` can be added when there is a concrete use case.
- **Add non-interactive flag equivalents to `auth login` as part of this PR.** Pre-existing gap; adding them would expand the scope significantly (4 new flags, password-arg security considerations). Filed as a separate follow-up issue.

## Follow-up issues to be filed

Filed as part of the PR, linked from the spec + PR description:

1. **`macOS: sign + notarize release binaries with Developer ID to eliminate post-upgrade Keychain prompts (follows #207)`**
   - Labels: `enhancement`, `macOS`, `blocked-on-budget`.
   - Body: Apple Dev Program ($99/yr) + CI signing + notarization. Gives a stable `teamid:` in the Keychain partition list, which survives all rebuilds. Replaces `jr auth refresh` as the user-facing workflow.

2. **`auth login / refresh: add non-interactive flag equivalents (--email, --token, --client-id, --client-secret)`**
   - Labels: `enhancement`, `automation`.
   - Body: Needed for agent / CI workflows post-upgrade. Reference CLAUDE.md's agentic-CLI principles.

## Success criteria

- `jr auth refresh` is wired up as an `AuthCommand` variant with `--oauth` flag parity.
- Running it on a broken post-upgrade install (manual test, macOS only) results in at most two keychain prompts (both writes, with "Always Allow" available), then silent reads thereafter.
- Running it on Linux is a no-op semantically (no ACLs to repair), but still clears + re-stores — does not regress existing flows.
- `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test` all pass.
- Two follow-up issues filed.
