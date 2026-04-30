# Embedded `jr` OAuth App — Design

**Status:** Draft
**Date:** 2026-04-30
**Branch:** `feat/embedded-oauth-app`
**Related ADR:** ADR-0006 (to be authored — supersedes ADR-0002)

## Problem

`jr` currently requires every OAuth user to register their own Atlassian OAuth 2.0
(3LO) application in the [Atlassian Developer Console](https://developer.atlassian.com/console/myapps/),
copy a `client_id` and `client_secret`, and pass them to `jr auth login --oauth`.
That registration step is a meaningful UX cliff for new users — the docs alone
take ~20 minutes to navigate, and most users abandon the flow and use API tokens
instead.

We want OAuth to "just work" out of the box for users of the official `jr`
binary, while preserving the bring-your-own-app (BYO) escape hatch for forks,
locked-down enterprise tenants, and source builds.

## Goals

1. `jr auth login --oauth` succeeds on a fresh install of an official binary
   without the user creating any Atlassian app.
2. The OAuth identity belongs to a single `jr`-branded Atlassian app; users
   see "jr" in the Atlassian consent screen.
3. Forks and source builds (`cargo install`, local `cargo build`) continue
   to work via the existing BYO flow.
4. Existing users with stored BYO credentials keep working without any
   re-authentication step.
5. Power users on enterprise tenants can override the embedded app with their
   own `--client-id` / `--client-secret` if their org policy requires it.
6. The embedded `client_secret` is not visible to GitHub's automated
   secret-scanning bots after release.

## Non-goals

- Truly hiding the `client_secret` from a motivated reverse engineer. That
  is impossible for any binary we ship; mitigation is rotation, not concealment
  (see [Operational Runbook](#operational-runbook)).
- Switching to a public-client / PKCE flow. Atlassian's 3LO endpoint still
  requires `client_secret` as of 2026-04
  ([source](https://developer.atlassian.com/cloud/oauth/getting-started/implementing-oauth-3lo/)).
- Supporting dynamic loopback ports for the embedded app. Atlassian's
  authorize endpoint requires an exact registered `redirect_uri` and does
  not implement RFC 8252 ([JRACLOUD-92180](https://jira.atlassian.com/browse/JRACLOUD-92180)).

## Architecture

### High-level component diagram

```
┌──────────────────────────────────────────────────────────────────┐
│ build.rs                                                         │
│  - reads $JR_BUILD_OAUTH_CLIENT_ID, $JR_BUILD_OAUTH_CLIENT_SECRET│
│  - generates random 32-byte XOR key                              │
│  - emits $OUT_DIR/embedded_oauth.rs with:                        │
│      pub const ID:        Option<&str>     = Some("...") | None  │
│      pub const SECRET_XOR: Option<&[u8]>   = Some(b"...") | None │
│      pub const SECRET_KEY: Option<&[u8;32]>= Some(b"...") | None │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼ include!()
┌──────────────────────────────────────────────────────────────────┐
│ src/api/auth/embedded.rs   (new module — small, focused)         │
│  pub struct EmbeddedOAuthApp { id, secret }                      │
│  pub fn embedded_oauth_app() -> Option<EmbeddedOAuthApp>         │
│      - lazy: OnceLock<Option<EmbeddedOAuthApp>>                  │
│      - on first call, XOR-decodes secret, returns app            │
│      - returns None when build had no embedded creds             │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│ src/cli/auth.rs::login_oauth                                     │
│  resolve_oauth_credentials(profile, args) -> (id, secret, source)│
│   1. flag (--client-id/--client-secret)                          │
│   2. env  (JR_OAUTH_CLIENT_ID / _SECRET)                         │
│   3. keychain (existing oauth_client_id / oauth_client_secret)   │
│   4. embedded::embedded_oauth_app()                              │
│   5. interactive prompt (TTY only; --no-input → error)           │
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│ src/api/auth.rs::oauth_login                                     │
│  takes new RedirectUriStrategy enum:                             │
│    - DynamicPort               (BYO, existing behavior)          │
│    - FixedPort(u16)            (embedded → 53682)                │
│  binds listener accordingly; everything else unchanged.          │
└──────────────────────────────────────────────────────────────────┘
```

### `build.rs`

A new top-level `build.rs`:

```rust
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_SECRET");

    let id     = env::var("JR_BUILD_OAUTH_CLIENT_ID").ok();
    let secret = env::var("JR_BUILD_OAUTH_CLIENT_SECRET").ok();

    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_oauth.rs");
    let body = match (id, secret) {
        (Some(id), Some(secret)) => {
            let key: [u8; 32] = generate_xor_key();
            let xored = xor(secret.as_bytes(), &key);
            format!(
                "pub const EMBEDDED_ID:         Option<&str>      = Some({id:?});\n\
                 pub const EMBEDDED_SECRET_XOR: Option<&[u8]>     = Some(&{xored:?});\n\
                 pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = Some(&{key:?});\n"
            )
        }
        _ => {
            "pub const EMBEDDED_ID:         Option<&str>      = None;\n\
             pub const EMBEDDED_SECRET_XOR: Option<&[u8]>     = None;\n\
             pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = None;\n"
                .to_string()
        }
    };
    fs::write(out_path, body).unwrap();
}
```

Properties this gives us:
- `cargo:rerun-if-env-changed` ensures incremental builds pick up rotated secrets.
- A fresh random XOR key is generated **per build**, so two release builds of
  the same source code produce different bytes in `.rodata`. Automated scanners
  matching on a known-pattern leaked secret get nothing.
- When env vars are unset, the file emits `None` constants — no embedded data,
  no behavior change for forks.

### Embedded module

`src/api/auth/embedded.rs` (new file; today `auth.rs` is one flat 942-line
module — adding embedded OAuth as a sibling submodule keeps the obfuscation
plumbing isolated from the keychain plumbing):

```rust
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/embedded_oauth.rs"));

pub struct EmbeddedOAuthApp {
    pub client_id:     &'static str,
    pub client_secret: String,
}

static EMBEDDED: OnceLock<Option<EmbeddedOAuthApp>> = OnceLock::new();

pub fn embedded_oauth_app() -> Option<&'static EmbeddedOAuthApp> {
    EMBEDDED
        .get_or_init(|| match (EMBEDDED_ID, EMBEDDED_SECRET_XOR, EMBEDDED_SECRET_KEY) {
            (Some(id), Some(xor), Some(key)) => {
                let secret = decode(xor, key);
                Some(EmbeddedOAuthApp { client_id: id, client_secret: secret })
            }
            _ => None,
        })
        .as_ref()
}

fn decode(xored: &[u8], key: &[u8; 32]) -> String {
    String::from_utf8(
        xored.iter().enumerate().map(|(i, b)| b ^ key[i % 32]).collect(),
    )
    .expect("embedded secret is not valid UTF-8 — build pipeline broken")
}
```

The `OnceLock` means decoding happens at most once; the plaintext lives in heap
memory after first use. We do not zeroize after refresh because the secret is
needed for every refresh-token grant, so it stays in memory for the process
lifetime.

### Resolution order in `cli/auth.rs::login_oauth`

Today's `login_oauth` resolves `client_id` and `client_secret` separately via
the helper `resolve_credential(value, env, flag, label, secret, no_input, hint)`.
The new resolution chain is encapsulated in a new helper:

```rust
enum OAuthAppSource { Flag, Env, Keychain, Embedded, Prompt }

fn resolve_oauth_app_credentials(
    flag_id: Option<String>,
    flag_secret: Option<String>,
    no_input: bool,
) -> Result<(String, String, OAuthAppSource)>
```

Order:
1. **Flag** (`--client-id`/`--client-secret`) — both must be flag-provided to
   count; one without the other falls through.
2. **Env** (`JR_OAUTH_CLIENT_ID`/`JR_OAUTH_CLIENT_SECRET`) — same pairing rule.
3. **Keychain** — `load_oauth_app_credentials()` returns the existing pair.
4. **Embedded** — `embedded::embedded_oauth_app()`.
5. **Prompt** — only when `!no_input` and no source above resolved.

Returning `OAuthAppSource` lets `jr auth status` report which source drove the
session (see [Transparency](#transparency-surface)).

### Resolution order in `api/auth.rs::refresh_oauth_token`

A separate, narrower resolver runs at refresh time:

```rust
fn resolve_refresh_app_credentials() -> Result<(String, String, OAuthAppSource)>
```

Order: **keychain → embedded**. No flag/env/prompt — refresh is a non-interactive
path triggered by 401 handling and `jr auth refresh`. Keychain wins so a
returning BYO user never silently flips to the embedded app mid-session;
embedded fallback covers users whose session was authenticated via the
embedded app (which does not persist `oauth_client_id` / `oauth_client_secret`
to the keychain).

### Fixed callback port for embedded source

A new enum:

```rust
enum RedirectUriStrategy {
    DynamicPort,        // BYO: bind 127.0.0.1:0, existing behavior
    FixedPort(u16),     // Embedded: bind 127.0.0.1:53682
}
```

Embedded → `FixedPort(53682)`. All other sources → `DynamicPort` (zero behavior
change). The strategy is determined by the resolver and threaded into
`oauth_login`.

When `FixedPort(53682)` is in use:
- Bind the local listener to `127.0.0.1:53682` (loopback, IPv4-only — Atlassian's
  authorize endpoint will redirect the browser to whatever string we put in
  `redirect_uri`; the resolver picks up `localhost` → `127.0.0.1`).
- Set `redirect_uri = "http://localhost:53682/callback"` to match the registered
  callback URL exactly. Using `localhost` (not `127.0.0.1`) keeps parity with
  the existing BYO dynamic-port code.
- On `EADDRINUSE`, error with:
  > `port 53682 is in use; the jr OAuth callback needs this port. Free it,
  > or use --client-id/--client-secret with your own OAuth app.`

**Port choice rationale:** 53682 is high (avoids privileged ranges), uncommon
(not in IANA Service Name Registry), and unlikely to clash with common dev
tools (3000/3306/5173/8000/8080/8443/9000/9090). The choice is locked in once
and pinned in the registered `redirect_uri`; changing it later is a breaking
release.

### Transparency surface

`jr auth status` gains an `oauth-app-source` row with one of:
- `flag` / `env` — set this invocation
- `keychain` — stored from a prior `jr auth login --oauth`
- `embedded` — using the baked-in `jr` app
- `(none)` — no OAuth credentials available; only relevant on a fresh install
  where the user hasn't run `auth login --oauth` yet

This lets users (and us, when triaging issues) tell which credentials drove
the live session without inspecting the keychain.

## Data flow

### First-time login on official binary (the new happy path)

```
$ jr auth login --oauth --profile prod --url https://acme.atlassian.net
   ↓
1. resolve_oauth_app_credentials → no flag/env/keychain → embedded → (id, secret, Embedded)
2. oauth_login(profile, id, secret, scopes, FixedPort(53682))
3. Browser opens https://auth.atlassian.com/authorize?... (redirect_uri=http://localhost:53682/callback)
4. User clicks "Allow" on the "jr" consent screen
5. Browser → http://localhost:53682/callback?code=...&state=...
6. jr exchanges code for tokens (client_id + decoded client_secret)
7. Tokens stored at <prod>:oauth-access-token / <prod>:oauth-refresh-token
8. Note: nothing written to keychain oauth_client_id/oauth_client_secret —
   embedded creds aren't persisted, they're re-decoded next launch
```

### First-time login on fork or source build (BYO unchanged)

```
$ jr auth login --oauth --profile prod --url https://acme.atlassian.net
   ↓
1. resolve_oauth_app_credentials → no flag/env/keychain → embedded is None → prompt
2. (Existing behavior: prompt for client_id, client_secret, store in keychain)
3. oauth_login(..., DynamicPort)  ← BYO flow unchanged
```

### Returning user with prior BYO login (existing user)

```
$ jr issue list   (background 401 → automatic token refresh)
   ↓
1. refresh_oauth_token resolves app creds via resolve_refresh_app_credentials:
     keychain → embedded
2. Keychain has the user's own client_id/secret → refresh uses those.
3. Their session keeps using their own app. No change.
```

### Returning user authenticated via embedded app (new path)

```
$ jr issue list   (background 401 → automatic token refresh)
   ↓
1. refresh_oauth_token resolves app creds via resolve_refresh_app_credentials:
     keychain → embedded
2. Keychain is empty (embedded login does not write keychain) → embedded slot.
3. Refresh proceeds against embedded client_id/secret.
```

`refresh_oauth_token` does **not** consult flag or env — those are login-time
inputs. The resolver order at refresh is intentionally narrower: **keychain →
embedded**. Keychain wins so a returning BYO user never silently flips to the
embedded app mid-session, and embedded provides the fallback for users whose
session was originally authenticated via the embedded app.

### Power-user override on official binary

```
$ JR_OAUTH_CLIENT_ID=mine JR_OAUTH_CLIENT_SECRET=hers \
    jr auth login --oauth --profile sandbox
   ↓
1. resolve → env wins → (mine, hers, Env)
2. oauth_login(..., DynamicPort)  ← env source uses dynamic port, just like BYO
3. Their app's redirect_uri must be registered for the port; same constraint
   as today.
```

## Error handling

| Scenario | Behavior |
|---|---|
| Embedded creds absent (`option_env!` returned None) AND no flag/env/keychain AND `--no-input` | Error: `"OAuth app credentials required. Set JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET, or pass --client-id/--client-secret. This binary was not built with embedded credentials."` |
| Embedded creds absent AND interactive | Existing prompt flow + `OAUTH_APP_HINT` (unchanged) |
| Port 53682 in use | Error message above, exit non-zero. No automatic fallback to dynamic port (would silently fail authorize anyway since `redirect_uri` won't match). |
| Embedded `client_secret` invalid (rotated server-side) | Atlassian returns `invalid_client` on `/oauth/token`. Surface as: `"OAuth login failed: this binary's embedded credentials have been rotated. Please update jr (brew upgrade / curl install). To use your own OAuth app instead, run with --client-id and --client-secret."` |
| Build pipeline injects malformed secret (non-UTF-8 after XOR) | `decode()` panics with `"embedded secret is not valid UTF-8 — build pipeline broken"`. Caught at first `embedded_oauth_app()` call. CI release-build smoke test should `jr auth login --oauth --no-input --help` (or similar) to trigger the panic before publishing. |

## Migration / backward compatibility

- **Existing BYO users** — zero impact. Their keychain entries `oauth_client_id`
  / `oauth_client_secret` win over embedded in the resolver. If they `auth
  refresh`, they keep using their app.
- **Existing API-token users** — zero impact. This change touches only the
  `--oauth` path.
- **Forks / source builds** — zero impact. `option_env!` returns `None`,
  embedded slot returns `None`, prompt flow runs.
- **Users who want to switch from BYO to embedded** — `jr auth logout
  --profile X` clears their keychain OAuth creds and tokens; next
  `auth login --oauth` falls through to embedded. Document in CHANGELOG.

## Operational runbook

### Initial setup (one-time)

1. Register a new Atlassian OAuth 2.0 (3LO) app in
   [Developer Console](https://developer.atlassian.com/console/myapps/):
   - Name: `jr`
   - Callback URL: `http://localhost:53682/callback`
     (also `http://127.0.0.1:53682/callback` if console allows two)
   - Scopes: `read:jira-work`, `write:jira-work`, `read:jira-user`,
     `offline_access` (matches `DEFAULT_OAUTH_SCOPES`)
2. Capture `client_id` (public) and `client_secret` (sensitive).
3. Add to GitHub repository secrets:
   - `OAUTH_CLIENT_ID` (no real need for masking but keep secret-like
     for consistency)
   - `OAUTH_CLIENT_SECRET` (masked; `secrets.OAUTH_CLIENT_SECRET` is
     auto-redacted in workflow logs)
4. Update `.github/workflows/release.yml` to inject:
   ```yaml
   env:
     JR_BUILD_OAUTH_CLIENT_ID: ${{ secrets.OAUTH_CLIENT_ID }}
     JR_BUILD_OAUTH_CLIENT_SECRET: ${{ secrets.OAUTH_CLIENT_SECRET }}
   ```
   on the `cargo build --release` step only. CI test runs do **not** get the
   secrets — tests run against the fork-equivalent unbranded build.
5. Smoke-test on a draft release before promoting.

### Rotation (when the secret is suspected compromised)

1. In Developer Console, regenerate the `client_secret`.
2. Atlassian invalidates all tokens issued with the old secret. Users on
   live sessions will see their next refresh fail (`invalid_client`).
3. Update `OAUTH_CLIENT_SECRET` in GitHub secrets.
4. Cut a patch release (`vX.Y.Z+1`) — version bump only, no code change
   (the `cargo:rerun-if-env-changed` directive ensures a fresh build
   embeds the new value).
5. Announce in the release notes:
   > "Re-authentication required: the embedded OAuth app was rotated.
   > Run `jr auth login --oauth` again, or fall back to API tokens."
6. Consider adding a brief `jr auth status` warning on detected
   `invalid_client` errors that suggests upgrading.

### What we deliberately do NOT do

- **No "phone home" telemetry.** The embedded app's identity is its own
  recognition signal; we don't need to track install counts.
- **No remote secret fetch on first launch.** We rejected this option (Q2-C)
  because the operational cost of running a forever server outweighs the
  marginal security benefit when rotation already exists.
- **No client_secret zeroization on drop.** The secret is needed for every
  token refresh; in-process plaintext is unavoidable.

## Testing strategy

### Unit tests
- `embedded_oauth_app()` returns `None` when constants are `None` — covered by
  default test build (no env vars set).
- `embedded_oauth_app()` returns `Some(_)` with the right values when constants
  are set — covered by a test that sets `JR_BUILD_OAUTH_CLIENT_ID/_SECRET`
  before `cargo test` (gated to a single dedicated test suite to avoid
  cross-contamination).
- `decode()` round-trips: XOR-encode a known plaintext + key, decode, expect
  equality.
- Resolution order: parameterize `resolve_oauth_app_credentials` over a fake
  source-set and assert the precedence chain.

### Integration tests
- BYO regression: `jr auth login --oauth` with `--client-id`/`--client-secret`
  flags continues to use `DynamicPort` and reaches the OAuth flow against
  wiremock — verify `redirect_uri` in the captured authorize call is `localhost:<dynamic>`.
- Embedded happy path: build a test binary with stub embedded creds, run
  `jr auth login --oauth --profile prod` against wiremock — verify
  `redirect_uri=http://localhost:53682/callback` in the captured authorize call.
- Port-busy regression: pre-bind 53682 in the test, expect the friendly
  error message and non-zero exit.
- Override regression: build a test binary WITH embedded creds, set
  `JR_OAUTH_CLIENT_ID/_SECRET`, run login, verify the env-provided values
  are used (not the embedded) and `redirect_uri` is dynamic.

### Snapshot test
- Output of `jr auth status` includes the new `oauth-app-source` field;
  insta snapshot covers each source.

## Open questions

1. **Dev-tag (pre-release) builds**: should `vX.Y.Z-dev.N` builds use the
   same OAuth app as stable, or a separate `jr-dev` app? Same for now;
   reconsider if pre-release abuse becomes visible.
2. **Distribution channels other than GitHub Releases.** A future Homebrew
   formula or Cargo registry publish would also need the build secrets
   threaded through. Out of scope here.
3. **`acli` reference comparison.** Perplexity could not surface how
   Atlassian's official `acli` CLI handles OAuth (whether it embeds
   credentials or requires user-registered apps). Worth a manual check
   during implementation to confirm we're not picking a path Atlassian
   explicitly rejected.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `client_secret` extracted from binary by motivated reverse engineer | High (always possible) | Medium (attacker can impersonate the `jr` app) | Rotation runbook above; XOR obfuscation defeats automated scanners only |
| Port 53682 in use breaks login | Low | Medium | Friendly error directs to BYO override |
| Atlassian rejects `redirect_uri` due to dev-console quirk we hit at registration | Medium | High (would block embedded path entirely) | Manually verify in console before release; fall back to BYO if blocked |
| Build pipeline regression — release built without secrets | Low | High (silent UX downgrade) | Add release smoke test asserting `embedded_oauth_app().is_some()` for tagged builds |

## Out of scope (future work)

- Migrating BYO users to the embedded app on next refresh — would require a
  silent re-consent flow, deferred until we see demand.
- Replacing `client_secret` with a bound DPoP / mTLS scheme — Atlassian
  doesn't support these; deferred indefinitely.
- ADR-0006 itself. The full ADR will be written alongside the implementation
  plan; this doc is the design that motivates it.
