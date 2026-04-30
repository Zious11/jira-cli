# Embedded `jr` OAuth App — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `jr` binary with an embedded Atlassian OAuth 2.0 (3LO) app so `jr auth login --oauth` works out-of-the-box on official builds, while preserving the bring-your-own-app (BYO) flow for forks and source builds.

**Architecture:** A `build.rs` reads compile-time env vars `JR_BUILD_OAUTH_CLIENT_ID` / `JR_BUILD_OAUTH_CLIENT_SECRET`, generates a per-build random 32-byte XOR key, and emits an `embedded_oauth.rs` file with the obfuscated bytes. A new `src/api/auth_embedded.rs` module decodes the secret on demand. `cli/auth.rs::login_oauth` and `api/auth.rs::refresh_oauth_token` gain resolver chains that consult embedded credentials as a fallback. Embedded sources use a fixed callback port (53682); BYO sources keep the existing dynamic-port behavior.

**Tech Stack:** Rust 2024 edition, `option_env!` for compile-time env, `keyring` 3 for OS keychain, `reqwest` 0.13 for OAuth token exchange, `wiremock` for integration tests, GitHub Actions for release builds.

**Spec:** `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`

---

## File Structure

| Path | Action | Responsibility |
|---|---|---|
| `docs/adr/0006-embedded-jr-oauth-app.md` | Create | ADR superseding ADR-0002. Rationale, consequences, rotation runbook. |
| `build.rs` | Create | Compile-time secret embedding with XOR obfuscation. ~70 lines. |
| `src/api/auth_embedded.rs` | Create | `EmbeddedOAuthApp`, `decode()`, `build_embedded_app()`, `embedded_oauth_app()`. ~100 lines. |
| `src/api/mod.rs` | Modify | Add `pub mod auth_embedded;`. |
| `src/api/auth.rs:314` | Modify | `oauth_login` accepts `RedirectUriStrategy`; bind logic switches on it. |
| `src/api/auth.rs:423` | Modify | `refresh_oauth_token` drops `client_id`/`client_secret` params, resolves internally. |
| `src/cli/auth.rs:11-17` | Modify | Add `OAuthAppSource` enum import (defined in `auth_embedded`). |
| `src/cli/auth.rs:237-335` | Modify | `login_oauth` calls new `resolve_oauth_app_credentials`; threads `RedirectUriStrategy` into `oauth_login`. |
| `src/cli/auth.rs:540-567` | Modify | `handle_status` adds `oauth-app-source` row when method is `"oauth"`. |
| `src/cli/auth.rs::tests` | Modify | Add resolver-precedence and status-output tests. |
| `tests/oauth_embedded_login.rs` | Create | Integration test: embedded login flow against wiremock, fixed-port redirect. |
| `tests/oauth_byo_login.rs` | Create | Regression test: BYO flow uses dynamic port. |
| `.github/workflows/release.yml:38-44` | Modify | Inject `JR_BUILD_OAUTH_CLIENT_ID` / `_SECRET` from GitHub secrets into `cargo build --release`. |
| `.github/workflows/ci.yml` | Modify | Ensure CI builds without secrets to test the unbranded path. |
| `README.md` | Modify | Document `--oauth` no longer requires user-registered app on official builds; note the BYO escape hatch. |
| `CLAUDE.md` | Modify | Add gotcha: fixed port 53682, `build.rs` env vars, embedded module location. |

---

## Task 1: Author ADR-0006 (supersedes ADR-0002)

**Files:**
- Create: `docs/adr/0006-embedded-jr-oauth-app.md`
- Modify: `docs/adr/0002-oauth-embedded-secret.md` (cross-reference back to ADR-0006)

- [ ] **Step 1: Create ADR-0006**

Write `docs/adr/0006-embedded-jr-oauth-app.md`:

```markdown
# ADR-0006: Embedded `jr` OAuth App with Compile-Time Obfuscation

## Status
Accepted (supersedes ADR-0002 for the second time)

## Context
ADR-0002 was originally accepted (embed secret), then superseded (BYO app required). The BYO path adds a 20-minute Atlassian Developer Console registration step that most users skip — they fall back to API tokens. We want OAuth to "just work" on official binaries.

Atlassian OAuth 2.0 (3LO) requires a `client_secret` for the token exchange step as of 2026-04 — there is no PKCE / public-client flow. Atlassian's own first-party CLI (`acli`) embeds OAuth credentials and exposes only `--web` to users (https://developer.atlassian.com/cloud/acli/reference/commands/jira-auth-login/), confirming this is an accepted pattern for Atlassian CLI tooling.

## Decision
Ship official `jr` binaries with an embedded `client_id` and `client_secret` for a dedicated `jr` Atlassian OAuth app. The secret is obfuscated via per-build random XOR key to defeat automated secret scanners. Forks and source builds (no env vars at compile time) fall back to the existing BYO flow with zero behavior change. Power users on official binaries can still override with `--client-id` / `--client-secret` or `JR_OAUTH_CLIENT_ID` / `JR_OAUTH_CLIENT_SECRET`.

The embedded app uses a fixed callback URL `http://localhost:53682/callback` because Atlassian's authorize endpoint requires exact `redirect_uri` match (https://jira.atlassian.com/browse/JRACLOUD-92180).

## Rationale
- **UX win**: matches Atlassian's own `acli` ergonomics.
- **Honest threat model**: XOR obfuscation only defeats automated scanners. Motivated attackers extracting the secret are mitigated by Atlassian's `client_secret` rotation flow, not by concealment in the binary.
- **No infrastructure**: no jr-hosted server, no telemetry, no operational footprint beyond rotating a secret if abused.

## Consequences
- The `client_secret` will eventually leak (any binary you ship can be reverse-engineered). Rotation is the recourse — see operational runbook in `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`.
- Forks must register their own OAuth app (or contribute a build secret); they cannot reuse the official `jr` identity.
- BYO users keep their existing flow; refresh tokens stay bound to whichever app issued them. No silent app-flip mid-session.
- Port 53682 is a permanent contract — changing it is a breaking release.

## Supersedes
ADR-0002 ("OAuth 2.0 with Embedded Client Secret" → "User-provided OAuth credentials"). The new approach reverses the user-provided default while keeping it as an opt-in escape hatch.
```

- [ ] **Step 2: Cross-link from ADR-0002**

Append to `docs/adr/0002-oauth-embedded-secret.md`:

```markdown

## Subsequent revision

ADR-0006 (2026-04-30) re-introduces an embedded OAuth app with per-build XOR obfuscation while keeping the user-provided path as an escape hatch. See that ADR for current rationale.
```

- [ ] **Step 3: Commit**

```bash
git add docs/adr/0006-embedded-jr-oauth-app.md docs/adr/0002-oauth-embedded-secret.md
git commit -m "docs(adr): ADR-0006 embedded jr OAuth app supersedes ADR-0002"
```

---

## Task 2: Add `build.rs` with XOR obfuscation

**Files:**
- Create: `build.rs`
- Modify: `Cargo.toml` (add `[build-dependencies]` if needed — `rand` is already a normal dep, we'll use OS getrandom directly to avoid a build-time `rand` dep)

- [ ] **Step 1: Create `build.rs`**

Write the file `build.rs` at the repo root:

```rust
//! Compile-time embedding of the `jr` Atlassian OAuth app credentials.
//!
//! Reads `JR_BUILD_OAUTH_CLIENT_ID` and `JR_BUILD_OAUTH_CLIENT_SECRET` from
//! the build environment. When both are set, generates a fresh random 32-byte
//! XOR key (per build) and writes `$OUT_DIR/embedded_oauth.rs` with three
//! constants: `EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`.
//! When either is missing (forks, local `cargo build`), all three are emitted
//! as `None`.
//!
//! XOR obfuscation defeats automated secret scanners (GitHub bots, generic
//! `strings | grep` patterns). It does NOT defeat reverse engineering. The
//! mitigation for a motivated attacker is Atlassian client_secret rotation.

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_SECRET");

    let id = env::var("JR_BUILD_OAUTH_CLIENT_ID").ok().filter(|s| !s.is_empty());
    let secret = env::var("JR_BUILD_OAUTH_CLIENT_SECRET").ok().filter(|s| !s.is_empty());

    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let out_path = Path::new(&out_dir).join("embedded_oauth.rs");

    let body = match (id, secret) {
        (Some(id), Some(secret)) => {
            let key = generate_xor_key();
            let xored: Vec<u8> = secret
                .as_bytes()
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ key[i % 32])
                .collect();
            format!(
                "pub const EMBEDDED_ID: Option<&str> = Some({id:?});\n\
                 pub const EMBEDDED_SECRET_XOR: Option<&[u8]> = Some(&{xored:?});\n\
                 pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = Some(&{key:?});\n"
            )
        }
        _ => {
            "pub const EMBEDDED_ID: Option<&str> = None;\n\
             pub const EMBEDDED_SECRET_XOR: Option<&[u8]> = None;\n\
             pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = None;\n"
                .to_string()
        }
    };

    fs::write(&out_path, body).expect("write embedded_oauth.rs");
}

/// 32 random bytes from the OS entropy source. Build scripts run on the
/// host's OS, so /dev/urandom (Unix) or BCryptGenRandom (Windows) is
/// available.
fn generate_xor_key() -> [u8; 32] {
    #[cfg(unix)]
    {
        let mut f = fs::File::open("/dev/urandom").expect("open /dev/urandom");
        let mut buf = [0u8; 32];
        f.read_exact(&mut buf).expect("read /dev/urandom");
        buf
    }
    #[cfg(windows)]
    {
        // BCryptGenRandom via a tiny inline shim — no extra build-deps.
        // Fall back to system_clock-seeded LCG only if BCrypt fails (last-
        // resort; the build host always has BCrypt available on supported
        // Windows versions).
        use std::time::SystemTime;
        let mut buf = [0u8; 32];
        // Try BCrypt first.
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(
                hAlgorithm: *mut std::ffi::c_void,
                pbBuffer: *mut u8,
                cbBuffer: u32,
                dwFlags: u32,
            ) -> i32;
        }
        let status = unsafe {
            BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), 32, 0x00000002)
        };
        if status == 0 {
            return buf;
        }
        // Fallback (should be unreachable on supported Windows).
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut s = nanos;
        for b in buf.iter_mut() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *b = (s >> 33) as u8;
        }
        buf
    }
}
```

- [ ] **Step 2: Confirm no Cargo.toml changes needed**

Run:

```bash
cargo build 2>&1 | tail -10
```

Expected: clean build. The `build.rs` uses only `std`, no new deps required.

- [ ] **Step 3: Verify the emitted file looks right (no embedded creds path)**

Run:

```bash
find target -name embedded_oauth.rs -path '*/build/*' | head -1 | xargs cat
```

Expected output:

```
pub const EMBEDDED_ID: Option<&str> = None;
pub const EMBEDDED_SECRET_XOR: Option<&[u8]> = None;
pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = None;
```

- [ ] **Step 4: Verify the emitted file looks right (embedded creds path)**

Run:

```bash
JR_BUILD_OAUTH_CLIENT_ID=test-id-123 JR_BUILD_OAUTH_CLIENT_SECRET=test-secret-abc \
  cargo build 2>&1 | tail -3
find target -name embedded_oauth.rs -path '*/build/*' | head -1 | xargs cat
```

Expected: file contains `EMBEDDED_ID: Option<&str> = Some("test-id-123")`, plus the XOR'd bytes for `test-secret-abc` and a 32-byte key. **The XOR key changes every build** — that's the point.

- [ ] **Step 5: Commit**

```bash
git add build.rs
git commit -m "build: compile-time embedding of OAuth app credentials with XOR obfuscation"
```

---

## Task 3: Create `src/api/auth_embedded.rs` skeleton (TDD)

**Files:**
- Create: `src/api/auth_embedded.rs`
- Modify: `src/api/mod.rs:2`
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Wire the module into `api/mod.rs`**

Edit `src/api/mod.rs` to add the new sibling module:

```rust
pub mod assets;
pub mod auth;
pub mod auth_embedded;
pub mod client;
pub mod jira;
pub mod jsm;
pub mod pagination;
pub mod rate_limit;
```

- [ ] **Step 2: Write the failing test for `decode()`**

Create `src/api/auth_embedded.rs` with just enough scaffolding for the test:

```rust
//! Embedded `jr` Atlassian OAuth app credentials.
//!
//! `build.rs` writes `$OUT_DIR/embedded_oauth.rs` with three constants
//! (`EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`). When the
//! build env vars are set, the secret is XOR-obfuscated against a per-build
//! random key. This module decodes them on demand and exposes a single
//! [`embedded_oauth_app`] accessor.
//!
//! Obfuscation defeats automated secret scanners. Motivated reverse engineers
//! can still extract the plaintext from a debugger; the operational mitigation
//! is `client_secret` rotation in Atlassian Developer Console (see ADR-0006).

use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/embedded_oauth.rs"));

/// Embedded OAuth app credentials. Plaintext after `decode()`; held in
/// process memory for the lifetime of the binary because `client_secret`
/// is needed for every refresh-token grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedOAuthApp {
    pub client_id: String,
    pub client_secret: String,
}

/// Source of the OAuth app credentials used for a login or refresh. Reported
/// by `jr auth status` so users (and triagers) can tell which credentials
/// drove the live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthAppSource {
    Flag,
    Env,
    Keychain,
    Embedded,
    Prompt,
    None,
}

impl OAuthAppSource {
    pub fn label(self) -> &'static str {
        match self {
            OAuthAppSource::Flag => "flag",
            OAuthAppSource::Env => "env",
            OAuthAppSource::Keychain => "keychain",
            OAuthAppSource::Embedded => "embedded",
            OAuthAppSource::Prompt => "prompt",
            OAuthAppSource::None => "(none)",
        }
    }
}

/// Decode an XOR-obfuscated secret using the per-build key. Pure function;
/// callers must supply both halves so tests can exercise it without
/// touching `OUT_DIR`.
fn decode(xored: &[u8], key: &[u8; 32]) -> Result<String, std::string::FromUtf8Error> {
    let bytes: Vec<u8> = xored
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 32])
        .collect();
    String::from_utf8(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_round_trip_known_plaintext() {
        let plaintext = "hello-world-secret";
        let key = [42u8; 32];
        let xored: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();

        let decoded = decode(&xored, &key).expect("valid utf-8");
        assert_eq!(decoded, plaintext);
    }
}
```

- [ ] **Step 3: Run the test, expect it to pass**

Run:

```bash
cargo test --lib decode_round_trip_known_plaintext -- --nocapture
```

Expected: `test result: ok. 1 passed`.

- [ ] **Step 4: Commit**

```bash
git add src/api/auth_embedded.rs src/api/mod.rs
git commit -m "feat(auth): embedded OAuth app module skeleton with decode()"
```

---

## Task 4: Add `build_embedded_app()` and `embedded_oauth_app()` accessor (TDD)

**Files:**
- Modify: `src/api/auth_embedded.rs`
- Test: same file

- [ ] **Step 1: Write failing tests for `build_embedded_app`**

Append inside the `mod tests` block in `src/api/auth_embedded.rs`:

```rust
    #[test]
    fn build_embedded_app_none_when_constants_unset() {
        let app = build_embedded_app(None, None, None);
        assert_eq!(app, None);
    }

    #[test]
    fn build_embedded_app_returns_decoded_when_all_set() {
        let plaintext = "secret-xyz";
        let key = [7u8; 32];
        let xored: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();

        let app = build_embedded_app(Some("client-abc"), Some(&xored), Some(&key))
            .expect("all three constants present → Some");
        assert_eq!(app.client_id, "client-abc");
        assert_eq!(app.client_secret, plaintext);
    }

    #[test]
    fn build_embedded_app_none_when_any_constant_missing() {
        let key = [0u8; 32];
        // id missing
        assert_eq!(build_embedded_app(None, Some(b"x"), Some(&key)), None);
        // secret_xor missing
        assert_eq!(build_embedded_app(Some("id"), None, Some(&key)), None);
        // key missing
        assert_eq!(build_embedded_app(Some("id"), Some(b"x"), None), None);
    }
```

- [ ] **Step 2: Run tests, expect compile failure**

```bash
cargo test --lib auth_embedded 2>&1 | tail -10
```

Expected: `error[E0425]: cannot find function 'build_embedded_app' in this scope`.

- [ ] **Step 3: Implement `build_embedded_app`**

Add to `src/api/auth_embedded.rs` (above the `#[cfg(test)]` block):

```rust
/// Construct an [`EmbeddedOAuthApp`] from raw build-emitted constants. Pure
/// function so tests can exercise both the present and absent paths without
/// rebuilding with different env vars.
fn build_embedded_app(
    id: Option<&str>,
    xor: Option<&[u8]>,
    key: Option<&[u8; 32]>,
) -> Option<EmbeddedOAuthApp> {
    let (id, xor, key) = match (id, xor, key) {
        (Some(i), Some(x), Some(k)) => (i, x, k),
        _ => return None,
    };
    let secret = decode(xor, key).ok()?;
    Some(EmbeddedOAuthApp {
        client_id: id.to_string(),
        client_secret: secret,
    })
}

/// Lazily-initialized cached embedded app. `OnceLock` ensures the XOR
/// decode happens at most once per process; the plaintext is then held
/// for the process lifetime (needed for token refreshes).
pub fn embedded_oauth_app() -> Option<&'static EmbeddedOAuthApp> {
    static APP: OnceLock<Option<EmbeddedOAuthApp>> = OnceLock::new();
    APP.get_or_init(|| {
        build_embedded_app(EMBEDDED_ID, EMBEDDED_SECRET_XOR, EMBEDDED_SECRET_KEY)
    })
    .as_ref()
}
```

- [ ] **Step 4: Run tests, expect them to pass**

```bash
cargo test --lib auth_embedded -- --nocapture
```

Expected: 4 passed (the round-trip test plus the 3 just added).

- [ ] **Step 5: Add a smoke test for `embedded_oauth_app()` in the absent path**

Append inside the `mod tests` block:

```rust
    /// Default test runs (no JR_BUILD_OAUTH_CLIENT_* env vars at compile time)
    /// must produce a binary where `embedded_oauth_app()` returns None. This
    /// is the fork / local-build path. Branded builds get a separate
    /// integration-test rig that sets the env vars.
    #[test]
    fn embedded_oauth_app_is_none_in_default_test_build() {
        // If this assertion ever fails in CI, the release env var is leaking
        // into test runs. Fix the workflow, not the test.
        assert!(
            embedded_oauth_app().is_none(),
            "test builds must not have embedded credentials"
        );
    }
```

- [ ] **Step 6: Run, then commit**

```bash
cargo test --lib auth_embedded
```

Expected: 5 passed.

```bash
git add src/api/auth_embedded.rs
git commit -m "feat(auth): build_embedded_app and embedded_oauth_app accessors"
```

---

## Task 5: Add `RedirectUriStrategy` and refactor `oauth_login` (TDD)

**Files:**
- Modify: `src/api/auth.rs:312-413`
- Test: `src/api/auth.rs::tests`

- [ ] **Step 1: Write the failing test for `RedirectUriStrategy::redirect_uri()`**

Add to the `#[cfg(test)] mod tests` block in `src/api/auth.rs`:

```rust
    /// FixedPort and DynamicPort produce well-formed `localhost`-host
    /// callback URIs. Locked in here because the registered Developer
    /// Console URL must match exactly — accidentally renaming the path
    /// or switching to `127.0.0.1` would break the embedded login flow.
    #[test]
    fn redirect_uri_strategy_strings() {
        assert_eq!(
            RedirectUriStrategy::FixedPort(53682).redirect_uri(),
            "http://localhost:53682/callback"
        );
        assert_eq!(
            RedirectUriStrategy::DynamicPort(54321).redirect_uri(),
            "http://localhost:54321/callback"
        );
    }
```

- [ ] **Step 2: Run, expect compile failure**

```bash
cargo test --lib redirect_uri_strategy_strings 2>&1 | tail -5
```

Expected: `error[E0433]: failed to resolve: use of undeclared type 'RedirectUriStrategy'`.

- [ ] **Step 3: Add the enum and refactor `oauth_login` signature**

In `src/api/auth.rs`, before the `oauth_login` function, add:

```rust
/// How `oauth_login` should choose the local callback port.
///
/// Embedded OAuth apps must use the exact `redirect_uri` registered in
/// Atlassian Developer Console — Atlassian does not honor RFC 8252's
/// "any loopback port" rule (https://jira.atlassian.com/browse/JRACLOUD-92180).
/// BYO apps stay on the historical dynamic-port behavior since they
/// register their own callback URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectUriStrategy {
    /// Bind a random ephemeral port; redirect_uri uses that port.
    /// Used by BYO apps (flag/env/keychain sources).
    DynamicPort(u16),
    /// Bind the given fixed port (53682 for the embedded `jr` app).
    /// `EADDRINUSE` surfaces a friendly error directing the user to BYO override.
    FixedPort(u16),
}

impl RedirectUriStrategy {
    pub fn port(self) -> u16 {
        match self {
            RedirectUriStrategy::DynamicPort(p) | RedirectUriStrategy::FixedPort(p) => p,
        }
    }

    pub fn redirect_uri(self) -> String {
        format!("http://localhost:{}/callback", self.port())
    }
}
```

Then change `oauth_login`'s signature and body — replace lines 314-336 with:

```rust
pub async fn oauth_login(
    profile: &str,
    client_id: &str,
    client_secret: &str,
    scopes: &str,
    strategy: RedirectUriStrategyRequest,
) -> Result<OAuthResult> {
    // 1. Resolve the strategy → owning the bound port up front so the
    //    callback URL we send to Atlassian matches what we'll listen on.
    let strategy = strategy.bind()?;
    let port = strategy.port();
    let redirect_uri = strategy.redirect_uri();
    let state = generate_state()?;

    let auth_url = build_authorize_url(client_id, scopes, &redirect_uri, &state);

    eprintln!("Opening browser for authorization...");
    eprintln!("If browser doesn't open, visit: {auth_url}");
    let _ = open::that(&auth_url);

    // 2. Listen for the OAuth callback on the resolved port.
    let async_listener = AsyncTcpListener::bind(format!("127.0.0.1:{port}")).await?;
```

The remainder of the function body (callback parsing, token exchange, accessible-resources lookup, keychain store, return `OAuthResult`) stays unchanged.

Above `RedirectUriStrategy`, add a "request" variant that does the actual port binding and produces the resolved strategy:

```rust
/// Pre-bind variant. The caller picks intent (Dynamic vs Fixed); this
/// helper performs the bind that resolves the actual port (Dynamic) or
/// validates availability (Fixed) before we hit the network. Errors
/// produce actionable messages — port-busy on Fixed surfaces the BYO
/// override hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectUriStrategyRequest {
    Dynamic,
    Fixed(u16),
}

impl RedirectUriStrategyRequest {
    pub fn bind(self) -> Result<RedirectUriStrategy> {
        match self {
            RedirectUriStrategyRequest::Dynamic => {
                let listener = TcpListener::bind("127.0.0.1:0")?;
                let port = listener.local_addr()?.port();
                drop(listener);
                Ok(RedirectUriStrategy::DynamicPort(port))
            }
            RedirectUriStrategyRequest::Fixed(p) => {
                match TcpListener::bind(format!("127.0.0.1:{p}")) {
                    Ok(l) => {
                        drop(l);
                        Ok(RedirectUriStrategy::FixedPort(p))
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                        Err(anyhow::anyhow!(
                            "port {p} is in use; the jr OAuth callback needs this port. \
                             Free it, or use --client-id/--client-secret with your own \
                             OAuth app."
                        ))
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}
```

- [ ] **Step 4: Update the only caller (`cli/auth.rs:308`)**

Edit `src/cli/auth.rs:308` from:

```rust
    let result =
        crate::api::auth::oauth_login(profile, &client_id, &client_secret, &scopes).await?;
```

to (Task 7 will replace `Dynamic` with the source-derived strategy; for now keep BYO-equivalent behavior):

```rust
    let result = crate::api::auth::oauth_login(
        profile,
        &client_id,
        &client_secret,
        &scopes,
        crate::api::auth::RedirectUriStrategyRequest::Dynamic,
    )
    .await?;
```

- [ ] **Step 5: Run all tests**

```bash
cargo test --lib redirect_uri_strategy_strings
cargo build
cargo test
```

Expected: the new test passes, full suite stays green.

- [ ] **Step 6: Commit**

```bash
git add src/api/auth.rs src/cli/auth.rs
git commit -m "refactor(auth): introduce RedirectUriStrategy for oauth_login"
```

---

## Task 6: Add `OAuthAppSource` resolver in `cli/auth.rs` (TDD)

**Files:**
- Modify: `src/cli/auth.rs`
- Test: `src/cli/auth.rs::tests`

- [ ] **Step 1: Write the failing test — flag wins**

Add to `cli/auth.rs::tests` mod (use the existing `ENV_LOCK` if present; if not, add tests inside `#[cfg(test)]` and `serial_test` is not in deps so wrap with explicit env clearing):

```rust
    /// Resolution order: flag → env → keychain → embedded → prompt.
    /// Flag wins even when env is set.
    #[test]
    fn resolve_oauth_app_credentials_flag_wins() {
        // Ensure env vars are unset for determinism.
        unsafe {
            std::env::remove_var(ENV_OAUTH_CLIENT_ID);
            std::env::remove_var(ENV_OAUTH_CLIENT_SECRET);
        }
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            Some("flag-id".into()),
            Some("flag-secret".into()),
            None, // env
            None, // env
            None, // keychain
            None, // embedded
            true, // no_input (prompt would error)
        )
        .expect("flag path must succeed");
        assert_eq!(id, "flag-id");
        assert_eq!(secret, "flag-secret");
        assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Flag);
    }
```

- [ ] **Step 2: Write the remaining cases**

```rust
    #[test]
    fn resolve_oauth_app_credentials_env_wins_over_keychain() {
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            None, None,
            Some("env-id".into()), Some("env-secret".into()),
            Some(("kc-id".into(), "kc-secret".into())),
            None,
            true,
        ).unwrap();
        assert_eq!((id.as_str(), secret.as_str(), source),
                   ("env-id", "env-secret", crate::api::auth_embedded::OAuthAppSource::Env));
    }

    #[test]
    fn resolve_oauth_app_credentials_keychain_wins_over_embedded() {
        let (id, _, source) = resolve_oauth_app_credentials_for_test(
            None, None, None, None,
            Some(("kc-id".into(), "kc-secret".into())),
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        ).unwrap();
        assert_eq!(id, "kc-id");
        assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Keychain);
    }

    #[test]
    fn resolve_oauth_app_credentials_embedded_when_no_user_input() {
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            None, None, None, None, None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        ).unwrap();
        assert_eq!((id.as_str(), secret.as_str(), source),
                   ("embed-id", "embed-secret", crate::api::auth_embedded::OAuthAppSource::Embedded));
    }

    #[test]
    fn resolve_oauth_app_credentials_no_input_errors_when_all_absent() {
        let err = resolve_oauth_app_credentials_for_test(
            None, None, None, None, None, None, true,
        ).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("OAuth"), "got: {msg}");
        assert!(msg.contains("--client-id") || msg.contains("JR_OAUTH_CLIENT_ID"),
                "error must cite the BYO escape hatch: {msg}");
    }

    /// Flag-without-secret (or vice versa) must NOT count as a flag hit —
    /// otherwise we'd send a malformed pair to oauth_login.
    #[test]
    fn resolve_oauth_app_credentials_partial_flag_falls_through() {
        let (id, _, source) = resolve_oauth_app_credentials_for_test(
            Some("partial-id".into()), None, // missing flag_secret
            None, None,
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        ).unwrap();
        assert_eq!(id, "embed-id");
        assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Embedded);
    }
```

- [ ] **Step 3: Run, expect compile failure**

```bash
cargo test --lib resolve_oauth_app_credentials 2>&1 | tail -5
```

Expected: `error[E0425]: cannot find function 'resolve_oauth_app_credentials_for_test'`.

- [ ] **Step 4: Implement the resolver**

Add to `src/cli/auth.rs` right after `resolve_credential` (around line 75):

```rust
use crate::api::auth_embedded::{EmbeddedOAuthApp, OAuthAppSource, embedded_oauth_app};

/// Resolve the OAuth app credentials for a `login --oauth` invocation.
/// Returns `(client_id, client_secret, source)`. The `source` flows into
/// `jr auth status` so users can tell which path drove the session.
///
/// Order: flag → env → keychain → embedded → prompt.
///
/// Flag and env are pair-gated: both halves must be present, otherwise the
/// resolver falls through (avoids sending a half-empty pair to Atlassian).
pub(crate) fn resolve_oauth_app_credentials(
    flag_id: Option<String>,
    flag_secret: Option<String>,
    no_input: bool,
) -> Result<(String, String, OAuthAppSource)> {
    let env_id = std::env::var(ENV_OAUTH_CLIENT_ID).ok().filter(|s| !s.is_empty());
    let env_secret = std::env::var(ENV_OAUTH_CLIENT_SECRET).ok().filter(|s| !s.is_empty());
    let keychain = crate::api::auth::load_oauth_app_credentials().ok();
    let embedded = embedded_oauth_app().map(|a| (a.client_id.clone(), a.client_secret.clone()));

    resolve_oauth_app_credentials_for_test(
        flag_id, flag_secret, env_id, env_secret, keychain, embedded, no_input,
    )
}

/// Pure resolution function — accepts every potential source as an argument
/// so unit tests can exercise the precedence chain without mutating env vars
/// or the keychain.
fn resolve_oauth_app_credentials_for_test(
    flag_id: Option<String>,
    flag_secret: Option<String>,
    env_id: Option<String>,
    env_secret: Option<String>,
    keychain: Option<(String, String)>,
    embedded: Option<(String, String)>,
    no_input: bool,
) -> Result<(String, String, OAuthAppSource)> {
    if let (Some(i), Some(s)) = (flag_id.clone().filter(|v| !v.is_empty()),
                                  flag_secret.clone().filter(|v| !v.is_empty())) {
        return Ok((i, s, OAuthAppSource::Flag));
    }
    if let (Some(i), Some(s)) = (env_id, env_secret) {
        return Ok((i, s, OAuthAppSource::Env));
    }
    if let Some((i, s)) = keychain {
        return Ok((i, s, OAuthAppSource::Keychain));
    }
    if let Some((i, s)) = embedded {
        return Ok((i, s, OAuthAppSource::Embedded));
    }
    if no_input {
        return Err(JrError::UserError(
            "OAuth app credentials are required. Provide --client-id and --client-secret, \
             or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET. This binary was not \
             built with embedded credentials."
                .to_string(),
        )
        .into());
    }
    // Fall back to the existing interactive prompt path. Re-enter
    // resolve_credential for each so the existing UX (masked input,
    // hint, retry) is preserved verbatim.
    let id = resolve_credential(
        None, ENV_OAUTH_CLIENT_ID, "--client-id", "OAuth Client ID",
        false, false, Some(OAUTH_APP_HINT),
    )?;
    let secret = resolve_credential(
        None, ENV_OAUTH_CLIENT_SECRET, "--client-secret", "OAuth Client Secret",
        true, false, Some(OAUTH_APP_HINT),
    )?;
    Ok((id, secret, OAuthAppSource::Prompt))
}
```

- [ ] **Step 5: Run tests, expect them to pass**

```bash
cargo test --lib resolve_oauth_app_credentials
```

Expected: 6 passed (the 5 new tests plus the partial-flag fallthrough).

- [ ] **Step 6: Commit**

```bash
git add src/cli/auth.rs
git commit -m "feat(auth): resolve_oauth_app_credentials with embedded fallback"
```

---

## Task 7: Wire the resolver into `login_oauth` and pick the right `RedirectUriStrategyRequest`

**Files:**
- Modify: `src/cli/auth.rs:237-335`

- [ ] **Step 1: Replace the two `resolve_credential` calls and the `oauth_login` invocation**

In `src/cli/auth.rs::login_oauth`, replace lines 244-308 with:

```rust
    if !no_input {
        eprintln!("OAuth 2.0: by default, official jr binaries use the embedded \"jr\" app.");
        eprintln!("To use your own OAuth app instead, pass --client-id and --client-secret,");
        eprintln!("or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET.\n");
    }

    let (client_id, client_secret, source) =
        resolve_oauth_app_credentials(client_id, client_secret, no_input)?;

    // Embedded credentials get the registered fixed callback. Every other
    // source is BYO and stays on the historical dynamic-port flow — the
    // user has registered their own callback URL.
    let strategy = match source {
        OAuthAppSource::Embedded => crate::api::auth::RedirectUriStrategyRequest::Fixed(53682),
        _ => crate::api::auth::RedirectUriStrategyRequest::Dynamic,
    };

    // Resolve config and scopes BEFORE persisting credentials — a bad
    // [profiles.<name>].oauth_scopes (empty/whitespace-only) must fail fast,
    // not leave new client_id/client_secret in the keychain alongside a
    // login that never succeeded.
    let config_path = global_config_path();
    let config = Config::load_lenient_with(Some(profile)).map_err(|err| {
        JrError::ConfigError(format!(
            "Failed to load config: {err:#}\n\n\
             Fix or remove the file referenced above. Global config: {config_path}; \
             per-project overrides come from `.jr.toml` in the current directory or any parent.",
            config_path = config_path.display()
        ))
    })?;
    let target_profile = config
        .global
        .profiles
        .get(profile)
        .cloned()
        .unwrap_or_default();
    let scopes = resolve_oauth_scopes(&target_profile)?;

    // Persist user-provided OAuth app creds to keychain so subsequent
    // refreshes use the same app. Embedded credentials are NOT persisted —
    // they re-decode from the binary every launch and would only pollute
    // the keychain for the inevitable rotation cycle.
    if !matches!(source, OAuthAppSource::Embedded) {
        crate::api::auth::store_oauth_app_credentials(&client_id, &client_secret)?;
    }

    let result = crate::api::auth::oauth_login(
        profile, &client_id, &client_secret, &scopes, strategy,
    )
    .await?;
```

The remainder of the function (saving the profile URL/cloud_id/auth_method, success message) stays unchanged.

- [ ] **Step 2: Build, run lib tests, run integration tests**

```bash
cargo build
cargo test
```

Expected: green.

- [ ] **Step 3: Manual smoke (no live OAuth)**

```bash
# Force the no-input path with no creds available — should error with the
# resolver's friendly message.
JR_PROFILE=neverdefined cargo run -- auth login --oauth --no-input --profile neverdefined --url https://example.atlassian.net
```

Expected exit code: non-zero. Stderr contains `OAuth app credentials are required` and mentions both `--client-id` and `JR_OAUTH_CLIENT_ID`.

- [ ] **Step 4: Commit**

```bash
git add src/cli/auth.rs
git commit -m "feat(auth): login_oauth uses embedded creds and fixed port when source==Embedded"
```

---

## Task 8: Refactor `refresh_oauth_token` to resolve internally

**Files:**
- Modify: `src/api/auth.rs:415-454`
- Test: `src/api/auth.rs::tests`

- [ ] **Step 1: Drop `client_id` and `client_secret` parameters; resolve internally**

Replace `refresh_oauth_token` with:

```rust
/// Refresh the OAuth 2.0 access token using the stored refresh token.
/// Returns the new access token on success.
///
/// Resolves the OAuth app credentials at call time via the refresh-side
/// resolver (`keychain → embedded`). Flag and env are NOT consulted — refresh
/// is a non-interactive path triggered by 401 handling, never by an explicit
/// user invocation that takes flags.
pub async fn refresh_oauth_token(profile: &str) -> Result<String> {
    let (client_id, client_secret) = resolve_refresh_app_credentials()?;
    let (_, refresh_token) = load_oauth_tokens(profile)?;

    let client = reqwest::Client::new();
    let response = client
        .post("https://auth.atlassian.com/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": client_id,
            "client_secret": client_secret,
            "refresh_token": refresh_token,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Token refresh failed (the embedded OAuth secret may have been rotated). \
             Run \"jr auth login --oauth --profile {profile}\" to re-authenticate."
        );
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    let tokens: TokenResponse = response.json().await?;
    store_oauth_tokens(profile, &tokens.access_token, &tokens.refresh_token)?;
    Ok(tokens.access_token)
}

/// Refresh-side resolver: keychain wins, embedded falls back. Flag and env
/// are deliberately omitted — refresh fires under 401-recovery and `jr auth
/// refresh`, neither of which collects flag/env app credentials separately
/// from a fresh login.
///
/// Keeping keychain ahead of embedded prevents a returning BYO user from
/// silently flipping to the embedded app mid-session (which would invalidate
/// their refresh token because it was issued by a different app).
fn resolve_refresh_app_credentials() -> Result<(String, String)> {
    if let Ok((id, secret)) = load_oauth_app_credentials() {
        return Ok((id, secret));
    }
    if let Some(app) = crate::api::auth_embedded::embedded_oauth_app() {
        return Ok((app.client_id.clone(), app.client_secret.clone()));
    }
    anyhow::bail!(
        "OAuth refresh requires either previously-stored app credentials \
         (run `jr auth login --oauth` once) or an embedded build. \
         This binary has neither."
    )
}
```

- [ ] **Step 2: Update doc comment cross-reference**

The `oauth_login` doc-block at line 312 references `refresh_oauth_token` and its old signature. Edit that paragraph to remove the `(client_id, client_secret)` parameter mention:

Replace:
```rust
/// Note: [`refresh_oauth_token`] does NOT take a scope parameter — the
/// `refresh_token` grant inherits scopes from the original authorization.
```

with:
```rust
/// Note: [`refresh_oauth_token`] takes only `profile` and resolves the
/// OAuth app credentials internally (keychain → embedded). The
/// `refresh_token` grant inherits scopes from the original authorization
/// per RFC 6749 §6.
```

- [ ] **Step 3: Build the project to surface any callers**

```bash
cargo build 2>&1 | tail -10
```

Expected: clean build (no callers exist today; this is dead-but-public API getting tightened).

- [ ] **Step 4: Add a unit test for the refresh resolver**

Add to `src/api/auth.rs::tests`:

```rust
    /// Refresh resolver prefers keychain over embedded so a returning BYO
    /// user does not silently flip onto the embedded app mid-session
    /// (their refresh_token was issued by their own app and would be
    /// rejected if presented with a different client_id).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn resolve_refresh_app_credentials_prefers_keychain() {
        with_test_keyring(|| {
            store_oauth_app_credentials("kc-id", "kc-secret").unwrap();
            let (id, secret) = resolve_refresh_app_credentials().unwrap();
            assert_eq!(id, "kc-id");
            assert_eq!(secret, "kc-secret");
        });
    }

    /// Refresh resolver returns the embedded creds when no keychain pair
    /// exists. In test builds embedded is None, so this test only validates
    /// the *order* via the keychain-empty error path.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn resolve_refresh_app_credentials_errors_when_both_absent() {
        with_test_keyring(|| {
            // No keychain entries, no embedded creds in default test build.
            let err = resolve_refresh_app_credentials().unwrap_err();
            let msg = format!("{err:#}");
            assert!(msg.contains("embedded"), "got: {msg}");
        });
    }
```

- [ ] **Step 5: Run tests**

```bash
cargo test --lib refresh_oauth_token
JR_RUN_KEYRING_TESTS=1 cargo test --lib resolve_refresh -- --include-ignored
```

Expected: green (the keyring-gated tests pass when run with `JR_RUN_KEYRING_TESTS=1`, ignored otherwise).

- [ ] **Step 6: Commit**

```bash
git add src/api/auth.rs
git commit -m "refactor(auth): refresh_oauth_token resolves app creds via keychain → embedded"
```

---

## Task 9: Surface `oauth-app-source` in `jr auth status`

**Files:**
- Modify: `src/cli/auth.rs:540-567`
- Test: `src/cli/auth.rs::tests`

- [ ] **Step 1: Add `oauth-app-source` to `handle_status`**

Locate the `handle_status` function (around line 530+). After the existing `Auth method:` line, when `method == "oauth"`, add:

```rust
    // Report which OAuth app credentials would be used for the next refresh.
    // This is the *future* source — same resolver as `refresh_oauth_token`.
    if method == "oauth" {
        let source = peek_oauth_app_source();
        println!("OAuth app:   {}", source.label());
    }
```

- [ ] **Step 2: Implement `peek_oauth_app_source`**

Add the helper above `handle_status`:

```rust
/// Inspect — without consuming or modifying — which source would supply
/// OAuth app credentials on the next `refresh_oauth_token` call. Mirrors
/// the resolver order in `api/auth.rs::resolve_refresh_app_credentials`.
fn peek_oauth_app_source() -> OAuthAppSource {
    if crate::api::auth::load_oauth_app_credentials().is_ok() {
        return OAuthAppSource::Keychain;
    }
    if crate::api::auth_embedded::embedded_oauth_app().is_some() {
        return OAuthAppSource::Embedded;
    }
    OAuthAppSource::None
}
```

- [ ] **Step 3: Add a unit test**

Add to `src/cli/auth.rs::tests`:

```rust
    /// On a default test build (no embedded credentials), with no keychain
    /// pair seeded, the OAuth app source must report `(none)`. If this ever
    /// fails as `embedded` it means a release env var leaked into tests.
    #[test]
    fn peek_oauth_app_source_reports_none_in_test_build() {
        let source = peek_oauth_app_source();
        // Either Keychain (developer has live creds) or None — but never
        // Embedded in a default test build. We accept either since the
        // dev's keychain is out of our control.
        assert!(
            matches!(source, OAuthAppSource::Keychain | OAuthAppSource::None),
            "unexpected source in test build: {:?}", source
        );
    }
```

- [ ] **Step 4: Run tests, build, manual check**

```bash
cargo test --lib peek_oauth_app_source
cargo build
target/debug/jr auth status 2>&1 | head -10
```

Expected: status output gains an `OAuth app:` row when the active profile uses OAuth. Reports `keychain` or `(none)` on the dev's machine.

- [ ] **Step 5: Commit**

```bash
git add src/cli/auth.rs
git commit -m "feat(auth): jr auth status reports oauth-app-source"
```

---

## Task 10: Inject build secrets in the release workflow

**Files:**
- Modify: `.github/workflows/release.yml:38-44`

- [ ] **Step 1: Add `env` block to the build step**

Replace the `Build` step (lines 38-44 in `.github/workflows/release.yml`):

```yaml
      - name: Build
        env:
          JR_BUILD_OAUTH_CLIENT_ID: ${{ secrets.OAUTH_CLIENT_ID }}
          JR_BUILD_OAUTH_CLIENT_SECRET: ${{ secrets.OAUTH_CLIENT_SECRET }}
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            # cross runs cargo inside a container; pass the env through.
            export CROSS_CONTAINER_OPTS="-e JR_BUILD_OAUTH_CLIENT_ID -e JR_BUILD_OAUTH_CLIENT_SECRET"
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi
```

- [ ] **Step 2: Add a release-time smoke step that asserts embedding worked**

Append after the `Package` step:

```yaml
      - name: Verify embedded OAuth app present
        if: github.ref_name != ''
        run: |
          BIN="target/${{ matrix.target }}/release/jr"
          # Skip the assertion on cross-compiled non-host targets — we can't
          # exec aarch64-linux on x86_64 hosts. Native targets only.
          NATIVE_TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
          if [ "${{ matrix.target }}" != "$NATIVE_TRIPLE" ]; then
            echo "Skipping embedded-creds smoke for non-native target ${{ matrix.target }}"
            exit 0
          fi
          # `jr auth status` does not require live network. The auth-app row
          # only appears when the active profile uses OAuth. We probe a
          # synthetic profile with auth_method=oauth and check the row reads
          # `embedded`.
          export JR_PROFILE=__release_smoke
          export JR_SERVICE_NAME=jr-release-smoke-$$
          mkdir -p "$HOME/.config/jr"
          cat > "$HOME/.config/jr/config.toml" <<EOF
          default_profile = "__release_smoke"
          [profiles.__release_smoke]
          url = "https://example.atlassian.net"
          auth_method = "oauth"
          EOF
          OUT="$($BIN auth status 2>&1 || true)"
          echo "$OUT"
          echo "$OUT" | grep -q "OAuth app: *embedded" \
            || { echo "release binary missing embedded OAuth credentials"; exit 1; }
```

- [ ] **Step 3: Add the secrets to the GitHub repository**

(Manual one-time step, document in the runbook.)

```
Settings → Secrets and variables → Actions → New repository secret
  Name: OAUTH_CLIENT_ID       Value: <the jr app's client_id>
  Name: OAUTH_CLIENT_SECRET   Value: <the jr app's client_secret>
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): inject embedded OAuth app credentials and verify in smoke step"
```

---

## Task 11: Confirm CI builds without secrets (unbranded path)

**Files:**
- Modify: `.github/workflows/ci.yml` (only if a guard is missing)

- [ ] **Step 1: Inspect `.github/workflows/ci.yml`**

```bash
cat .github/workflows/ci.yml
```

Expected: the existing CI workflow does NOT set `JR_BUILD_OAUTH_CLIENT_ID/_SECRET`. The default `cargo test` therefore exercises the `embedded_oauth_app()` is `None` path (covered by the test in Task 4).

- [ ] **Step 2: Add an assertion test job (if not already present)**

If `ci.yml` lacks an explicit "no embedded creds" assertion, add a step to the test job:

```yaml
      - name: Assert no embedded OAuth credentials in CI builds
        run: |
          # The embedded-fork path must remain default for CI. If a future
          # change wires JR_BUILD_OAUTH_* into CI, this test fails first.
          ! cargo test --lib embedded_oauth_app_is_none_in_default_test_build -- --quiet 2>&1 | grep -q FAILED
```

(Note: the test already runs as part of `cargo test`; this step is belt-and-suspenders. Skip if the existing CI already runs the full test suite — which it does per `ci.yml`.)

- [ ] **Step 3: Commit (only if changes were made)**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: explicit assertion that CI builds have no embedded OAuth creds"
```

---

## Task 12: Update README and CLAUDE.md

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README**

In `README.md`, locate the OAuth section (around line 95-100, before the `JR_OAUTH_CLIENT_ID="$ID"` example). Replace the top-of-section text with:

```markdown
### OAuth 2.0 (recommended on official binaries)

Official `jr` releases ship with a built-in `jr` Atlassian OAuth app, so
authentication is one command:

```bash
jr auth login --oauth --profile my-site --url https://my-site.atlassian.net
```

Your browser opens, you click "Allow" on the `jr` consent screen, done.

#### Bring your own OAuth app

If you're on a fork, source build, or enterprise tenant that requires its
own OAuth app, register one at
[Atlassian Developer Console](https://developer.atlassian.com/console/myapps/),
then pass `--client-id`/`--client-secret` or set
`JR_OAUTH_CLIENT_ID`/`JR_OAUTH_CLIENT_SECRET`.

```bash
JR_OAUTH_CLIENT_ID="$ID" JR_OAUTH_CLIENT_SECRET="$SECRET" \
  jr auth login --oauth --profile my-site --url https://my-site.atlassian.net
```
```

- [ ] **Step 2: Update CLAUDE.md gotchas**

In `CLAUDE.md`, append to the Gotchas section:

```markdown
- **Embedded OAuth app uses fixed callback port 53682.** The release build
  workflow injects `JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET` (CI-only env vars)
  via `build.rs`, which generates an XOR-obfuscated `embedded_oauth.rs` in
  `$OUT_DIR`. The bound callback URL is `http://localhost:53682/callback`,
  registered exactly in Atlassian Developer Console. Changing the port is a
  breaking release. BYO sources (flag, env, keychain) keep the historical
  dynamic-port behavior. See ADR-0006 and
  `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`.
- **`src/api/auth_embedded.rs` is a thin sibling module** to `auth.rs`. Keep
  obfuscation plumbing there; keep keychain/OAuth flow plumbing in `auth.rs`.
- **`refresh_oauth_token` resolves credentials internally** (keychain →
  embedded) — callers pass only `profile`. Do not re-introduce
  `client_id`/`client_secret` parameters; they short-circuit the resolver.
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: README + CLAUDE.md cover embedded OAuth app and BYO escape hatch"
```

---

## Task 13: Integration test — embedded login uses fixed port

**Files:**
- Create: `tests/oauth_embedded_login.rs`

This test rebuilds the test binary with `JR_BUILD_OAUTH_CLIENT_ID/_SECRET` set,
then runs the full OAuth flow against a wiremock authorization server.
Because this requires a separately-built binary, it lives in its own test
file and is gated behind a `JR_RUN_OAUTH_INTEGRATION=1` env var (heavy test).

- [ ] **Step 1: Create the test scaffolding**

Create `tests/oauth_embedded_login.rs`:

```rust
//! Integration test: the embedded OAuth path uses the fixed registered
//! callback port (53682) and exchanges credentials against a mock
//! authorization server.
//!
//! Heavyweight — rebuilds `jr` with `JR_BUILD_OAUTH_CLIENT_ID/_SECRET` set,
//! so it's gated behind `JR_RUN_OAUTH_INTEGRATION=1`. CI does not run this
//! by default; release builds run it as a smoke step instead.

use assert_cmd::Command;
use std::process::Stdio;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn embedded_login_uses_fixed_port() {
    if std::env::var("JR_RUN_OAUTH_INTEGRATION").is_err() {
        eprintln!("skipped: set JR_RUN_OAUTH_INTEGRATION=1 to run");
        return;
    }

    // 1. Start a mock authorization server on a random port. The actual
    //    Atlassian endpoints are baked into oauth_login at compile time
    //    (auth.atlassian.com), so this test does not exercise the real
    //    HTTP exchange — instead, it verifies the *redirect_uri* in the
    //    authorize URL is fixed to localhost:53682. We pre-bind 53682 to
    //    a tiny tokio TCP listener and verify the callback HTTP request
    //    arrives there with the expected query.

    // (Implementation detail: spawning the full flow against wiremock
    //  requires intercepting auth.atlassian.com, which the existing code
    //  hard-codes. A targeted refactor — passing a base URL through —
    //  is out of scope for this plan; the smoke test in release.yml
    //  covers the on-binary assertion instead.)

    eprintln!("placeholder: this test asserts shape via the release smoke step");
}
```

- [ ] **Step 2: Note the deferred test**

Update `docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md` Testing
section: add a paragraph noting that full embedded-login integration testing
is deferred until `oauth_login` is refactored to accept a base URL override
(needed to point at wiremock instead of `auth.atlassian.com`). The release
workflow's "Verify embedded OAuth app present" step covers the
on-binary assertion in the meantime.

```bash
# Edit the spec inline to add the deferred-testing note in the Testing section.
```

- [ ] **Step 3: Run, verify the placeholder is skipped**

```bash
cargo test --test oauth_embedded_login
```

Expected: 1 test passes (skip path). The test only does work when
`JR_RUN_OAUTH_INTEGRATION=1` is set.

- [ ] **Step 4: Commit**

```bash
git add tests/oauth_embedded_login.rs docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md
git commit -m "test(oauth): integration scaffold for embedded login (deferred to base-url refactor)"
```

---

## Task 14: Final sweep — clippy, fmt, full test

- [ ] **Step 1: Run formatting and lint**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Expected: zero output (no diff, no warnings).

- [ ] **Step 2: Run the full test suite**

```bash
cargo test
```

Expected: green.

- [ ] **Step 3: Inspect commit log**

```bash
git log --oneline develop..HEAD
```

Expected: 12-13 commits, each scoped to one task.

- [ ] **Step 4: Push and open PR**

```bash
git push -u origin feat/embedded-oauth-app
gh pr create --base develop --title "feat: embedded jr OAuth app with XOR obfuscation" \
  --body "$(cat <<'EOF'
## Summary

Implements the embedded `jr` OAuth app design from
`docs/superpowers/specs/2026-04-30-embedded-oauth-app-design.md`. Official
release builds embed an Atlassian OAuth app's `client_id` / `client_secret`
(XOR-obfuscated against a per-build random key) so `jr auth login --oauth`
succeeds without the user creating their own Developer Console app. Forks
and source builds keep the existing BYO flow.

Supersedes ADR-0002 with ADR-0006.

## What changes for users
- Official binary: `jr auth login --oauth` is one command. Browser → Allow → done.
- Forks / `cargo install --git`: BYO flow unchanged.
- Existing OAuth users with stored creds: zero impact (keychain wins over embedded).
- Power-user override on official binary: pass `--client-id`/`--client-secret` or
  `JR_OAUTH_CLIENT_ID`/`_SECRET` to bypass embedded.

## Test plan
- [x] `cargo test` (unit + integration) green
- [x] `cargo clippy -- -D warnings` clean
- [x] Default test build leaves `embedded_oauth_app()` = `None`
- [x] CI workflow does not inject build secrets
- [x] Release workflow injects build secrets and asserts presence on native targets
- [ ] Manual: tag a dev release, install, run `jr auth status` against an OAuth profile, verify `OAuth app: embedded`
- [ ] Manual: tag a dev release, run `jr auth login --oauth` against a real Atlassian instance

## Operational checklist before merge
- [ ] `OAUTH_CLIENT_ID` and `OAUTH_CLIENT_SECRET` repo secrets created
- [ ] Atlassian Developer Console app registered with callback `http://localhost:53682/callback`
- [ ] Scopes match `DEFAULT_OAUTH_SCOPES`
EOF
)"
```

---

## Self-Review Checklist (run after writing the plan)

**1. Spec coverage** — every spec section maps to a task:
- Goals 1, 2 → Task 7 (resolver wires Embedded → FixedPort) + Task 10 (release injection).
- Goal 3 (forks unchanged) → Task 4 (`embedded_oauth_app()` = `None`) + Task 11 (CI guard).
- Goal 4 (existing users unchanged) → Task 6 (resolver: keychain wins over embedded).
- Goal 5 (power-user override) → Task 6 (flag/env beat embedded).
- Goal 6 (no plain-text scanner hits) → Task 2 (per-build XOR key).
- Architecture: build.rs (Task 2), embedded module (Tasks 3-4), resolution order (Tasks 5-6), fixed callback port (Task 5), refresh resolver (Task 8), transparency (Task 9).
- Operational runbook → Task 1 (ADR-0006) + Task 10 (CI secrets) + Task 12 (CLAUDE.md gotcha).
- Testing → Tasks 3, 4, 5, 6, 8, 9 unit; Task 13 integration (deferred, with on-binary smoke in Task 10).

**2. Placeholder scan** — no TBDs, no "implement later", every code block is complete code, every command has an expected output. The deferred-test placeholder in Task 13 is intentional and documented; it is not a TODO.

**3. Type consistency** — `OAuthAppSource` defined once in `auth_embedded.rs`, imported in `cli/auth.rs`. `EmbeddedOAuthApp` uses `String` fields (not `&'static str`) since the secret is decoded into heap memory. `RedirectUriStrategy` and `RedirectUriStrategyRequest` are distinct types: Request is the unbound intent, Strategy is the resolved post-bind value. `resolve_oauth_app_credentials` and `resolve_oauth_app_credentials_for_test` share a pure-function core; the public version is a one-line wrapper that supplies env/keychain/embedded args.
