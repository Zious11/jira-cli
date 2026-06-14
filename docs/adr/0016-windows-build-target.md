---
document_type: architecture-decision-record
adr_number: "0016"
status: Accepted
date: 2026-06-13
supersedes: []
superseded_by: []
related: ["ADR-0003", "ADR-0006", "BC-6.1.001", "BC-6.1.002", "BC-1.4.027", "BC-6.1.014", "BC-6.2.016", "BC-6.2.017"]
---

# ADR-0016: Windows Build Target (x86_64-msvc, .zip, %APPDATA%/%LOCALAPPDATA% Paths, Credential Manager, Windows CI)

## Status

**Accepted** (2026-06-12). Scoped to cycle-001 windows-build feature (v0.6.0 target).

## Context

As of v0.6.0-dev.1 (commit 587206e), `jr` ships pre-built binaries for four targets:
`x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, and
`aarch64-unknown-linux-gnu`. No Windows artifact exists. The `release.yml` matrix
and all packaging steps are written for Unix runners with bash, `tar`, and `sha256sum`.

Windows users must build from source (`cargo build --release`), which is a significant
barrier. Windows is a primary platform for many Jira users; the absence of a pre-built
binary represents a distribution gap.

F1 delta analysis (`.factory/cycles/cycle-001/windows-build/delta-analysis.md`) identified
the following concrete blockers:

1. **keyring crate:** `windows-native` feature absent — credential storage silently fails
   at runtime on Windows (null backend).
2. **release.yml:** no Windows matrix row; packaging uses `tar czf` and `jr` (no `.exe`);
   smoke step uses `XDG_CONFIG_HOME` and heredoc — both fail on Windows runners.
3. **config/cache paths:** `dirs::home_dir().join(".config")` / `.join(".cache")` produce
   non-idiomatic Windows paths (`%USERPROFILE%\.config\jr`) instead of conventional
   AppData locations.
4. **Test isolation:** integration tests set `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` for path
   isolation, but `dirs` ignores XDG on Windows — tests would write to the real user
   directories in Windows CI.
5. **No Windows CI job:** regressions on Windows are undetectable until a release tag is
   pushed.

Five decisions were presented at the human gate on 2026-06-12. All five were confirmed.
This ADR records them as locked decisions.

## Decisions

### Decision 1: Target `x86_64-pc-windows-msvc` ONLY this cycle

**Decision:** Ship one Windows target: `x86_64-pc-windows-msvc`. Native build on
`windows-latest` GitHub Actions runner via `rustup target add`.

**Deferred:** `aarch64-pc-windows-msvc` — no native runner available; cross-compilation
for MSVC targets is not supported by `cross-rs`; requires a separate toolchain investigation.
Deferred to a future cycle.

**Not in scope:** `i686-pc-windows-msvc` (32-bit) — no user demand identified.
`x86_64-pc-windows-gnu` (MinGW) — requires MinGW runtime DLL; inferior distribution story
for end-users.

**Rationale:** MSVC toolchain is available natively on `windows-latest`. `reqwest + rustls`
(pure Rust TLS, per ADR-0003) performs the TLS handshake and crypto in pure Rust
(aws-lc-rs/ring) — no OpenSSL and no Schannel for the TLS protocol itself. This gives
a redistributable binary with no TLS-stack DLL dependency.

**Qualification (reqwest 0.13 default behavior):** reqwest 0.13 changed the `rustls`
default to verify certificate roots via `rustls-platform-verifier`, which on Windows
delegates root discovery to the Windows certificate store (CryptoAPI). This means there
is a soft runtime dependency on the Windows trust store for certificate root validation —
not for the TLS protocol stack itself. This is the upstream default and is acceptable:
`jr` constructs the HTTP client with a bare `Client::builder()…build()` in
`src/api/client.rs` (no explicit TLS config), so the 0.13 default applies. Atlassian
endpoints (`*.atlassian.net`) chain to publicly-trusted roots present in the Windows
store on any standard Windows installation.

The alternative (`rustls-tls-webpki-roots` feature) would bundle Mozilla roots at build
time, eliminating any OS-store reliance, but loses enterprise-CA and CRL support and
pins roots at compile time. That trade-off is not warranted here; the upstream default is
the correct choice.

**ADR-0003 cross-reference:** The existing choice of `rustls` over `native-tls` (ADR-0003)
was made explicitly for cross-compilation reasons. That decision pays off here: no TLS
linking issues on Windows.

**TLS implementation note (C-V5 inoculation, 2026-06-13):** On Windows, reqwest 0.13's
`rustls` feature activates `rustls-platform-verifier 0.6` (delegates root verification to
the Windows certificate store via CryptoAPI) with `aws-lc-rs` as the crypto provider —
NOT `webpki-roots` and NOT `ring`. This is confirmed by the reqwest v0.13.0 `Cargo.toml`
`rustls` feature entry (`dep:rustls-platform-verifier`, `__rustls-aws-lc-rs`). Cross-ref:
ADR-0003. Research: C-V5 in `.factory/research/windows-build-f4-preflight-verification.md`.

### Decision 2: Artifact format `.zip`

**Decision:** Package the Windows binary as `jr-<version>-x86_64-pc-windows-msvc.zip`
(contains `jr.exe`). Checksum as `jr-<version>-x86_64-pc-windows-msvc.zip.sha256`.

**Deferred:** `.tar.gz` of `jr.exe` — technically extractable via Git Bash or WSL, but
unfamiliar to pure-Windows users and not a Windows-conventional format.

**Rationale:** `.zip` is natively extractable on Windows 10+ via File Explorer and
PowerShell `Expand-Archive` without third-party tools. CLI tools on Windows (e.g.,
`ripgrep`, `fd`, `bat`, `delta`) universally distribute as `.zip`. This matches user
expectations. The `softprops/action-gh-release` step accepts arbitrary file types;
`.zip` uploads cleanly alongside the existing `.tar.gz` artifacts.

**Implementation note:** ~~Use `shell: bash` with `zip` (Git Bash) AND `sha256sum` (Git
Bash) — both are pre-installed on `windows-latest` GitHub-hosted runners via the Git for
Windows bundle. Specify `shell: bash` on the `run:` step; no further PATH manipulation is
needed.~~

> **AMENDED 2026-06-13 (F-WIN-F3-003):** ~~Adversarial review found the original wording
> listed `Compress-Archive` as a fallback without clarifying the primary mechanism, leaving
> an ambiguous "either works" statement that could cause implementer divergence. Resolution
> chosen: **Git Bash `zip` is the primary, deterministic packaging mechanism** for the
> Windows target, not a fallback. Rationale: `windows-latest` GitHub-hosted runners ship
> Git for Windows, which places `zip` and `sha256sum` in `C:\Program Files\Git\usr\bin`.
> This is documented in the `actions/runner-images` repository (windows-latest image
> manifest) and is the mechanism used by the majority of Rust CLI projects (ripgrep, fd,
> bat) that distribute `.zip` artifacts from `windows-latest` CI. PowerShell
> `Compress-Archive` remains a known alternative if the runner image ever changes, but it
> is NOT the primary mechanism for this cycle. Risk-acceptance note: because `zip`
> availability depends on the runner image (not a standard Windows OS feature), the story
> S-WIN-4 EC-002 risk is accepted as LOW given GitHub's image stability guarantees; no
> additional AC or runtime check is required. Story S-WIN-4 AC-002 wording is consistent
> with this decision — see story-writer note below.~~
>
> **SUPERSEDED 2026-06-13 (C-V3 re-amendment):** The F-WIN-F3-003 amendment above is
> factually incorrect and is superseded by this block. Pre-F4 verification research
> (`.factory/research/windows-build-f4-preflight-verification.md`, Claim C-V3) refuted
> the core premise: the Unix `zip` command is **NOT** available on `windows-latest`
> runners. Primary sources confirm: (1) the MSYS2 package index lists `zip` as a
> separately-installable package (`pacman -S zip`), NOT part of the coreutils/base set
> that Git for Windows bundles; (2) the Windows2022/Windows2025 runner-images manifests
> list `7zip` and `zstd` but do NOT list `zip` (the Info-ZIP Unix command); (3)
> `azure/azure-cli#27842` documents `bash: line N: zip: command not found` on these
> runners; (4) `actions/runner-images#9361` shows GitHub actively trims compression
> tooling. A bash step running `zip ...` on `windows-latest` will fail at that line.
>
> **Locked Decision 2 (as of C-V3):**
>
> The Windows release artifact is produced with **PowerShell `Compress-Archive`**
> (`shell: pwsh`) as the PRIMARY, deterministic `.zip` packaging mechanism — NOT Git Bash
> `zip`. `sha256sum` (Git for Windows coreutils, confirmed available) is used for the
> checksum, but MUST be in a **separate** `shell: bash` step so a missing `zip` binary
> cannot silently prevent checksum generation.
>
> Chosen implementation shape (two steps):
>
> ```yaml
> - name: Package (Windows)
>   if: runner.os == 'Windows'
>   shell: pwsh
>   run: |
>     Compress-Archive -Path "target/${{ matrix.target }}/release/jr.exe" `
>       -DestinationPath "jr-${{ github.ref_name }}-${{ matrix.target }}.zip"
>
> - name: Checksum (Windows)
>   if: runner.os == 'Windows'
>   shell: bash
>   run: |
>     sha256sum jr-${{ github.ref_name }}-${{ matrix.target }}.zip \
>       > jr-${{ github.ref_name }}-${{ matrix.target }}.zip.sha256
> ```
>
> `Compress-Archive` is always present on `windows-latest` (ships with PowerShell 5.1+,
> built into Windows). `sha256sum` is confirmed available from Git for Windows coreutils
> (C-V3). No external tool installation is required.
>
> **EC-002 reframing:** The `zip`-not-available case is no longer an "accepted LOW risk"
> — it is the definitive reason `Compress-Archive` is primary. Story S-WIN-4 EC-002 must
> be reframed from risk-acceptance of a LOW-probability failure to a statement that the
> `zip`-unavailable constraint is handled by construction (Compress-Archive is immune to it).
>
> Primary sources: MSYS2 package index (https://packages.msys2.org/package/zip);
> actions/runner-images Windows2022-Readme / Windows2025-Readme; azure/azure-cli#27842;
> actions/runner-images#9361. Research report: C-V3.

### Decision 3: Add Windows job to `ci.yml` (full CI regression protection)

**Decision:** Add `windows-latest` to the `test` matrix in `ci.yml`. The Windows CI job
runs `cargo test`, `cargo clippy -- -D warnings`, and implicitly depends on the
test-isolation seam (Decision 5) being implemented first.

**Rationale:** Windows CI on every PR provides regression protection. Without it, any
Windows-specific break is only detected when a release tag is pushed. The human gate
overrode the F1 recommendation (which had suggested release-build-only for v1) in favor
of full CI protection.

**Prerequisite:** The `JR_CONFIG_DIR`/`JR_CACHE_DIR` test-isolation seam (Decision 5)
must be implemented before Windows is added to `ci.yml`. Without the seam, tests set
`XDG_CONFIG_HOME`/`XDG_CACHE_HOME`, which `dirs` ignores on Windows, causing tests to
write to the real user profile.

**Scope note for `fmt` and `deny` jobs:** These run on `ubuntu-latest` only.

> **AMENDED 2026-06-13 (F-WIN-F3-001):** The original scope note above contained a
> factually false statement that was corrected by adversarial review before implementation.
> It claimed "the `clippy` job runs on `ubuntu-latest` only; Windows clippy is folded into
> the `test` matrix step." This is incorrect on two counts: (1) `clippy` and `test` are
> separate top-level jobs in `ci.yml` — the `test` job runs `cargo test --all-features`
> only and never ran clippy; (2) folding clippy into `test` would be architecturally wrong
> because Ubuntu clippy cannot lint `#[cfg(windows)]` branches — those branches are dead
> code on Linux and are silently skipped. See architecture-delta.md §4.1.

**Correct `clippy` job behaviour (supersedes the original scope note):** The `clippy`
job gains a `strategy.matrix.os` over `[ubuntu-latest, windows-latest]` and changes
`runs-on: ubuntu-latest` to `runs-on: ${{ matrix.os }}`. This is a **separate clippy job
matrixed over both platforms** — NOT folded into the `test` job. The Windows clippy run
is required to lint the `#[cfg(windows)]` path-resolution branches in `src/config.rs` and
`src/cache.rs` that are invisible to an Ubuntu runner. The `test` job independently adds
`windows-latest` to its own matrix (`cargo test --all-features` only, no clippy).
Runner-cost impact: two clippy runs per PR (ubuntu + windows) instead of one.
Cross-reference: architecture-delta.md §4.1; S-WIN-5 AC-006.

### Decision 4: Config/cache paths use idiomatic Windows conventions

**Decision:** On Windows, resolve configuration to `%APPDATA%\jr` (`dirs::config_dir()`)
and cache to `%LOCALAPPDATA%\jr` (`dirs::cache_dir()`). Unix behavior (`~/.config/jr`,
`~/.cache/jr/v1/`) is unchanged.

**Implementation:** `#[cfg(windows)]` / `#[cfg(not(windows))]` branches in:
- `src/config.rs::global_config_dir()`
- `src/cache.rs::cache_root()`

The cache versioning root `v1/` is preserved on both platforms. Full paths:
- Windows config: `%APPDATA%\jr\config.toml`
  (`dirs::config_dir().unwrap_or(USERPROFILE\AppData\Roaming).join("jr")`)
- Windows cache: `%LOCALAPPDATA%\jr\v1\<profile>\`
  (`dirs::cache_dir().unwrap_or(USERPROFILE\AppData\Local).join("jr").join("v1").join(profile)`)

**`APPDATA`/`LOCALAPPDATA` defensive fallback:** The `unwrap_or_else` branch reads
`APPDATA` (or `LOCALAPPDATA`) directly as a last resort when `dirs` fails. The empty-string
case must be filtered the same way as the debug seam — `APPDATA=""` is treated as
unavailable and falls through to `PathBuf::from(".")` (the `./jr` defensive path). Use
`.ok().filter(|s| !s.is_empty()).map(PathBuf::from)` in the fallback closure. This keeps
the unset and empty cases both routing to the `./jr` defensive fallback: the `APPDATA`
(config) path is consistent with BC-6.1.014 EC-1; the `LOCALAPPDATA` (cache) path is
consistent with BC-6.2.016 EC-1. Concrete snippet in architecture-delta.md §1.2.
- macOS/Linux config: unchanged (`$XDG_CONFIG_HOME/jr` or `~/.config/jr`)
- macOS/Linux cache: unchanged (`$XDG_CACHE_HOME/jr/v1/<profile>` or `~/.cache/jr/v1/<profile>`)

**Rationale:** Windows users expect AppData locations. A `.config` folder in the home
directory is unusual and may be hidden by Windows Explorer. `%APPDATA%` (Roaming) is
correct for config (survives roaming profile sync in enterprise environments); `%LOCALAPPDATA%`
(Local) is correct for cache (machine-local, not synced).

`XDG_CONFIG_HOME`/`XDG_CACHE_HOME` semantics remain Unix-only. On Windows, these env vars
are not consulted (they are not meaningful Windows concepts). The new `JR_CONFIG_DIR`/
`JR_CACHE_DIR` debug seam (Decision 5) provides the isolation mechanism for tests on all
platforms.

**BC impact:** Windows config-path behavior is fully captured by the new BC-6.1.014.
BC-6.1.001 and BC-6.1.002 cover config MIGRATION semantics (`[instance]/[fields]` →
`[profiles.default]`, idempotent write-back) and are NOT path-discovery contracts; no
platform-conditional clause is required in those BCs. Cache path behavior on Windows is
captured by BC-6.2.016 and the BC-6.2.004 platform-conditional update.

### Decision 5: `JR_CONFIG_DIR`/`JR_CACHE_DIR` debug-only env seam for test isolation

**Decision:** Add `JR_CONFIG_DIR` and `JR_CACHE_DIR` debug-only environment variable
overrides, read at the path-resolution site in `global_config_dir()` and `cache_root()`.
Gated by `#[cfg(debug_assertions)]`, consistent with the existing `JR_BASE_URL` and
`JR_AUTH_HEADER` debug seam pattern (SD-002).

**Design:** In `src/config.rs::global_config_dir()`, before the OS branch:
```
#[cfg(debug_assertions)]
// Empty string is treated as unset — mandated by BC-6.2.017 EC-1/EC-5.
if let Some(dir) = std::env::var("JR_CONFIG_DIR").ok().filter(|s| !s.is_empty()) {
    return PathBuf::from(dir);
}
// then: #[cfg(windows)] dirs::config_dir() branch, else XDG/home branch
```

Identical pattern in `src/cache.rs::cache_root()` for `JR_CACHE_DIR`.

The seam is read BEFORE the `#[cfg(windows)]` / `#[cfg(not(windows))]` OS split,
so it provides a single cross-platform isolation mechanism. When `JR_CONFIG_DIR` is
set, neither the XDG path nor the Windows AppData path is consulted.

**Subprocess boundary:** Integration tests use `assert_cmd` (subprocess invocation of the
`jr` binary). Environment variables are the only cross-process isolation mechanism that
works here. In-process state cannot be used. The `JR_CONFIG_DIR`/`JR_CACHE_DIR` env vars
are set via `.env("JR_CONFIG_DIR", dir.path())` on the `Command` builder — exactly the
same pattern as the existing `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` usage, making migration
of test helpers mechanical.

**Existing XDG tests:** On macOS and Linux, tests can continue to set `XDG_CONFIG_HOME`/
`XDG_CACHE_HOME` OR migrate to `JR_CONFIG_DIR`/`JR_CACHE_DIR`. The new seam works on all
platforms. The existing XDG-based isolation also continues to work on Unix because the code
checks `XDG_CONFIG_HOME` in the `#[cfg(not(windows))]` branch. Both mechanisms coexist.

**Migration scope for test files:** All test files that set `XDG_CONFIG_HOME` or
`XDG_CACHE_HOME` must be updated to also set (or switch to) `JR_CONFIG_DIR`/`JR_CACHE_DIR`
for Windows CI compatibility. The primary affected helper is `jr_isolated()` in
`tests/auth_output_json.rs:69` and the ad-hoc `.env("XDG_*")` calls in 37 other in-scope test files (38 total files set XDG vars; `tests/e2e_live.rs` is excluded as fully `#[ignore]`/`JR_RUN_E2E`-gated and never runs in the `windows-latest` CI matrix). Canonical enumeration: `grep -rlE 'XDG_CONFIG_HOME|XDG_CACHE_HOME' tests/`. The migration is mechanical: add `.env("JR_CONFIG_DIR", …)` / `.env("JR_CACHE_DIR", …)` alongside each existing `.env("XDG_*", …)` call site.

**Release gate:** `JR_CONFIG_DIR` and `JR_CACHE_DIR` have NO effect in release binaries
(`cargo build --release` sets `cfg(not(debug_assertions))`). The `#[cfg(debug_assertions)]`
gate is the same mechanical guarantee as `JR_BASE_URL`. A regression test
(`tests/config_dir_release_gate.rs`) pins this guarantee.

### Decision 5b: Keyring — Windows Credential Manager (`windows-native` feature)

**Decision:** Add `windows-native` to the `keyring` crate's feature list:
```toml
keyring = { version = "3", features = ["apple-native", "linux-native", "windows-native"] }
```

**Rationale:** Without `windows-native`, the keyring crate degrades to a null backend on
Windows — `jr auth login` appears to succeed but credentials are not persisted. The next
invocation fails with an auth error. This is a hard functional blocker. All three platform
features (`apple-native`, `linux-native`, `windows-native`) are `cfg`-gated per-OS at
compile time; listing all three is correct.

The per-profile keychain layout in `src/api/auth.rs` (service name `jr-jira-cli`,
per-profile keys `<profile>:oauth-access-token`, `<profile>:oauth-refresh-token`,
shared keys `email`, `api-token`) uses plain string identifiers that Windows Credential
Manager accepts without modification. No source changes to `auth.rs` are required.

**Gotcha — colon in profile-namespaced keys:** keyring `windows-native` stores credentials
as `CRED_TYPE_GENERIC` entries via `CredWriteW`. Windows Credential Manager validates only
that the target name is non-empty and within length limits — it does NOT prohibit the colon
character. The `<profile>:oauth-access-token` scheme is therefore portable to Windows
unchanged with no key sanitization required (verified against keyring-rs v3.6.3
`src/windows.rs` + Microsoft `CredWriteW` documentation, 2026-06-12).

~~**`deny.toml` note:** `windows-native` pulls in `windows-sys` (wincred backend). The
existing `deny.toml` already skips the `windows-sys 0.45` / `0.61.2` version conflict.
Adding `windows-native` may introduce a new windows-sys version. Verify `cargo deny check`
at implementation time and add a `[[bans.skip]]` entry if needed.~~

**C-V2(b) amendment (2026-06-13):** F4 pre-flight verification confirmed that keyring
v3.6.3 `windows-native` pulls **windows-sys 0.60**, not 0.61. The existing skip entries
cover only 0.45 and 0.61.2 — `0.60` is absent. A `[[bans.skip]]` entry for windows-sys
0.60 is **REQUIRED** (not conditional) in the same commit as enabling `windows-native`;
`cargo deny check` under `bans.multiple-versions = "deny"` exits 1 without it.
Source: `.factory/research/windows-build-f4-preflight-verification.md`, claim C-V2(b).

**Scope correction (implementation-confirmed, 2026-06-13):** The C-V2(b) research
correctly identified windows-sys 0.60 but did NOT trace the transitive fan-out a new
windows-sys minor mechanically introduces. S-WIN-3 implementation confirmed that windows-sys
0.60 (via windows-native) pulls `windows-targets 0.53.x` and 7 `windows_*` arch crates at
the 0.53.x tier. Combined with the pre-existing 0.42.x lineage (jni → windows-sys 0.45)
and 0.52.x lineage (ring → windows-sys 0.52), Cargo.lock carries windows-targets at three
versions. `cargo deny check` required exactly 17 `[[bans.skip]]` entries in total — not 1
— to pass: 1 (windows-sys 0.60) + the 0.42 tier (windows-targets 0.42 + 7 arch crates) +
the 0.53 tier (windows-targets 0.53 + 7 arch crates) = 1 + 2 × (1 + 7) = 17, leaving
0.52.6 as the single un-skipped canonical version. Note: `windows_i686_gnullvm` is NOT
skipped — it has no 0.42 tier (the i686-gnullvm stub did not exist in the 0.42 generation);
only 2 versions appear in Cargo.lock (0.52.6, 0.53.1) and `cargo deny` tolerates it; a
skip entry would be unmatched and would error. The DECISION (enable windows-native + add
required deny skips) is unchanged; only the documented scope of the skip set is corrected
here. See architecture-delta.md §5.3 and §10 process-gap note (PG-WIN3-001).

### Decision 5c: Embedded-OAuth smoke step gated off on Windows

**Decision:** The "Verify embedded OAuth app present" step in `release.yml` is gated with
`if: runner.os != 'Windows'` for this cycle.

**Rationale:** The step uses bash heredoc, `XDG_CONFIG_HOME=$HOME/.config`, `find`, and
`grep` — all Unix-shaped. Porting to PowerShell or Git Bash is feasible but out of scope
for v1. The `build.rs` BCryptGenRandom path (the Windows entropy source for XOR key
generation) is tested implicitly by the build succeeding: `compile_error!` fires on any
non-unix/non-windows host (build.rs line 69–74), and BCryptGenRandom failure at build time
is fatal. The embedded constant check is the gap; a follow-up cycle can port it.

**ADR-0006 cross-reference:** ADR-0006 documents the XOR obfuscation scheme. The smoke
step verifies that `build.rs` populated the obfuscated constants at build time. For
Windows v1, this verification is deferred. The binary is still correct if built with the
OAuth secrets env vars present.

## Consequences

**Positive:**
- Windows users get a pre-built `.zip` binary from GitHub Releases.
- Credential storage works correctly via Windows Credential Manager.
- Config and cache are at conventional Windows AppData locations — no user confusion.
- Windows CI on every PR catches regressions before release.
- The `JR_CONFIG_DIR`/`JR_CACHE_DIR` seam eliminates the XDG-only isolation fragility
  and works identically on all three platforms.

**Negative / Trade-offs:**
- `aarch64-pc-windows-msvc` is not available this cycle (deferred).
- The embedded-OAuth smoke step is not verified on Windows builds for v1.
- Adding `windows-latest` to `ci.yml` increases runner cost (~30 min per PR at 1.0× rate).
- Test files using `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` for isolation must be migrated
  (mechanical change, tracked in F4 implementation scope).
- Windows config-path behavior is fully captured by BC-6.1.014; no change to
  BC-6.1.001/BC-6.1.002 is outstanding (those BCs govern migration semantics, not
  path discovery).

**OAuth loopback (port 53682):** The `TcpListener::bind("127.0.0.1:53682")` in `auth.rs`
is cross-platform. On Windows, the Windows Firewall may prompt the user on first run to
allow the inbound connection. This is a documented user-facing consequence, not a code
change. The prompt is a one-time event per user profile.

## See Also

- ADR-0003 — `reqwest + rustls` (cross-platform TLS; underpins clean Windows build)
- ADR-0006 — Embedded OAuth app and XOR obfuscation (`build.rs` BCryptGenRandom path)
- BC-6.1.014 — Windows config path (new BC; fully captures Windows path-discovery behavior)
- BC-6.2.016, BC-6.2.017 — Windows cache path and debug seam (new BCs)
- BC-6.1.001, BC-6.1.002 — Config migration contracts (no Windows clause required; these
  govern `[instance]/[fields]` → `[profiles.default]` migration semantics only)
- BC-1.4.027 — Per-profile keychain keys contract (`<profile>:oauth-access-token` / `<profile>:oauth-refresh-token`; requires `windows-native` to hold)
- `.factory/cycles/cycle-001/windows-build/delta-analysis.md` — F1 scope analysis
- `docs/specs/` — feature spec for this cycle (F2 output)
- `.factory/cycles/cycle-001/windows-build/architecture-delta.md` — concrete design
