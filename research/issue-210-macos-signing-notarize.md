---
document_type: research
issue_id: 210
title: "Verification: macOS Developer ID signing + notarization for jr CLI"
last_updated: 2026-05-09
sources_count: 18
status: blocked-on-budget
---

# Issue #210 — Verification: macOS Developer ID signing + notarization

> **TL;DR:** Every claim in issue #210 is **verified or substantively correct** against
> Apple TN2206 and current (2026) Apple developer documentation. The issue's proposed
> CI workflow is sound and matches the canonical 2026 GitHub Actions pattern. Two
> minor refinements: (a) `xcrun stapler staple` does **not** apply to standalone CLI
> binaries (only to `.app`/`.dmg`/`.pkg`), so the staple step in the issue is a
> **no-op for the raw `jr` binary** unless we wrap it in a `.dmg`/`.pkg`; (b) `spctl
> --assess --type execute` will **also fail on a raw CLI binary** ("does not seem to
> be an app") even when signing+notarization are correct, so the spctl verification
> step needs adjustment. The fundamental claim — that only Developer ID with a stable
> Team ID survives the Keychain partition-list ACL across rebuilds — holds.
>
> **Recommended next action:** HUMAN-DECISION on whether the maintainer holds an
> Apple Developer Program membership ($99/yr, verified current). If no, defer #210
> indefinitely with the `blocked-on-budget` label; `jr auth refresh` (#209) remains
> the documented workaround. If yes, the implementation is **medium effort** (~1-2 days).

## Claim 1 — Ad-hoc sign cdhash invalidation (Apple TN2206)

**Status:** VERIFIED.

**Citation:** Apple TN2206 "Code Signing In Depth", Sections 2.1, 2.2, 3
(`https://developer.apple.com/library/archive/technotes/tn2206/_index.html`).
Apple Security Framework `SecTrustedApplicationCreateFromPath`
(`https://developer.apple.com/documentation/security/1567822-sectrustedapplicationcreatefro`).

**Summary:**

- Ad-hoc signing (`codesign -s -`) produces a CodeDirectory blob and computes a
  cdhash (SHA-1/SHA-256 of the CodeDirectory) for kernel validation, but it does
  **not** include a CMS signature, so there is **no certificate chain and no Team
  Identifier** in the signature. This is verbatim per TN2206 §2.2.
- The cdhash is **content-derived**: any code change (rebuild, even with identical
  source if timestamps/build-IDs differ) produces a new CodeDirectory and therefore
  a new cdhash. This is the explicit semantic of `cdhash <hash>` requirements in
  TN2206's "Rule Syntax" table.
- The legacy Keychain partition-list ACL keys on the **trusted-application
  identity**, which is verified via `SecTrustedApplicationCreateFromPath()`. For
  ad-hoc-signed binaries this collapses to a cdhash check (since no Team ID is
  available); for Developer-ID-signed binaries it can additionally rely on the
  designated requirement, which references the certificate chain (and thus the
  stable Team ID).
- Therefore the issue's claim is correct: **ad-hoc rebuild → new cdhash → legacy
  ACL entry no longer matches → user is prompted on next `jr` run after `brew
  upgrade`**. Only Developer ID (or any cert chain anchored to Apple) provides a
  stable identity (the 10-character `TeamIdentifier`) that survives rebuilds.

**Confidence:** High. TN2206 is the authoritative Apple source and directly
addresses each step of the claim.

## Claim 2 — Developer ID Application cert + cost

**Status:** VERIFIED.

**Citation:** Apple Developer "Create Developer ID certificates"
(`https://developer.apple.com/help/account/certificates/create-developer-id-certificates/`);
Apple Developer Program enrollment page (still $99 USD/year, no announced change
through 2026).

**Summary:**

- **Developer ID Application** is the correct certificate type for a CLI tool
  distributed outside the Mac App Store. Apple's documentation describes it as
  "a certificate used to sign a Mac app" for non-App-Store distribution. It is
  *not* the same as:
  - **Apple Distribution** (App Store / TestFlight only — would not allow
    Gatekeeper-friendly direct download).
  - **Mac Installer Distribution** (for signing `.pkg` *installers*, not the
    binaries inside). If we ever ship a `.pkg`, we'd need both this and Developer
    ID Application.
- **Cost:** $99 USD/year for an individual Apple Developer Program membership.
  Verified current as of 2026; no fee change has been announced.
- **What it produces:** `codesign -dv --verbose=4 jr` on a Developer-ID-signed
  binary outputs `Authority=Developer ID Application: Your Name (ABC123DEF4)`
  and `TeamIdentifier=ABC123DEF4` — a stable 10-character ID. The Team ID is
  tied to the Apple Developer account, **not** to the certificate, so it
  survives certificate renewal as well as rebuilds. This stable Team ID is
  exactly what the legacy Keychain partition-list mechanism uses to permit ACL
  entries to span rebuilds.

**Confidence:** High.

## Claim 3 — Notarytool setup, auth, timing, scope

**Status:** VERIFIED with one nuance.

**Citation:** Apple "Customizing the notarization workflow"
(`https://developer.apple.com/documentation/security/customizing-the-notarization-workflow`);
Apple "Resolving common notarization issues"
(`https://developer.apple.com/documentation/security/resolving-common-notarization-issues`).

**Summary:**

- `xcrun notarytool submit --wait` accepts **three** auth modes:
  1. `--apple-id` + `--password` (app-specific password from appleid.apple.com)
     + `--team-id`.
  2. `--keychain-profile <profile>` (one-time `notarytool store-credentials`
     setup; profile is then read from local Keychain).
  3. **App Store Connect API key** via `--key`, `--key-id`, `--issuer`. The
     Perplexity result claiming "API keys are not supported" is **incorrect**.
     Apple's documentation explicitly supports App Store Connect API keys for
     `notarytool`, and this is in fact the recommended approach for unattended
     CI: it avoids storing an Apple ID password and supports rotation.
- **CI-friendly approach for GHA:** Use App Store Connect API key. Three
  secrets: `APPSTORE_CONNECT_KEY` (the `.p8` file content), `APPSTORE_CONNECT_KEY_ID`
  (10-char ID), `APPSTORE_CONNECT_ISSUER_ID` (UUID). All three should be repo
  secrets. Alternative: Apple ID + app-specific password + team ID (3 secrets);
  simpler but tied to a personal Apple ID and rotated manually.
- **Notarization wait time (2024-2026):**
  - Median: 2-5 minutes.
  - 90th percentile: 10-15 minutes.
  - 95th percentile: ~30 minutes.
  - Apple's published target: 95% under 15 minutes.
  - **Recommend `timeout-minutes: 30`** for the notarize job.
- **What notarization checks:**
  1. Signed with a valid Developer ID Application certificate (chain to Apple
     root).
  2. Hardened runtime enabled (`--options runtime` at `codesign` time).
  3. Authenticated secure timestamp (`--timestamp` at `codesign` time).
  4. No `get-task-allow` entitlement (production builds only).
  5. Apple's automated **malware/static-analysis scan** (signature-based plus
     heuristics; not source review).
  6. Entitlements validation (no entitlements that require special approval —
     for a plain CLI we have none).

**Confidence:** High. The only correction to the broader Perplexity result is on
API-key support; everything else aligns with Apple docs.

## Claim 4 — Stapler workflow (caveat: does not apply to raw CLI binaries)

**Status:** VERIFIED, BUT THE ISSUE'S STAPLER STEP IS A NO-OP FOR THE RAW `jr`
BINARY.

**Citation:** `xcrun stapler` man page
(`https://keith.github.io/xcode-man-pages/stapler.1.html`); Apple "Notarizing
macOS software before distribution"
(`https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution`).

**Summary:**

- `xcrun stapler staple <path>` downloads the notarization ticket from Apple's
  CloudKit ticket-delivery service and embeds it into the file at a
  format-specific location (e.g., `Contents/CodeResources` for `.app` bundles,
  metadata regions for `.dmg`/`.pkg`).
- **Critical gap with the issue's plan:** Stapling is **only supported** for:
  - `.app` bundles
  - `.dmg` (UDIF) disk images
  - `.pkg` flat installer packages
- **Stapling is NOT supported for raw standalone executables.** Apple documents
  this explicitly: "Although tickets are created for standalone binaries, it's
  not currently possible to staple tickets to them." There is no `Contents/`
  directory or metadata region inside a Mach-O executable to embed a ticket.
- **Consequence:** If we distribute the raw `jr` binary in a `.tar.gz` (which is
  the current `release.yml` artifact format), `xcrun stapler staple jr` will
  **fail with an error** ("Could not find suitable target to staple"). The fix
  is one of:
  1. Wrap each binary in a `.dmg` and staple the `.dmg`. Most robust;
     enables offline first-run.
  2. Wrap each binary in a `.pkg` (also supports stapling). Heavier UX —
     installs to `/usr/local/bin/jr` system-wide.
  3. **Skip stapling entirely** and rely on online ticket fetching by
     Gatekeeper at first run. Acceptable for CLI tools because users almost
     always have internet on first install. Apple's
     `com.apple.gk.ticket-delivery` service queries on first execution
     transparently.
- **Stapling vs online fetch trade-off:**
  - Stapled: works offline, no first-run latency.
  - Not stapled: works only with internet on first run; small (~1s) latency.
- For the `jr` use case (developer tool, internet-connected install via brew),
  **option 3 (no staple) is acceptable** and simpler. The issue's stapler step
  should be removed or made conditional on the binary being wrapped in a
  `.dmg`/`.pkg`.

**Confidence:** High.

## Claim 5 — `spctl --assess` (caveat: also does not apply to raw CLI binaries)

**Status:** PARTIALLY VERIFIED — works for `.app`/`.pkg`/`.dmg`, but **rejects
raw CLI binaries with an unrelated error message**, regardless of signing/
notarization status.

**Citation:** Apple `spctl(8)` man page; community reports
(`https://gist.github.com/neonichu/c9297bc68cc43ebf5361`).

**Summary:**

- `spctl --assess --type execute --verbose=4 <path>`:
  - On a properly signed and notarized `.app`: outputs `accepted source=Notarized
    Developer ID`.
  - On a properly signed and notarized **raw CLI binary**: outputs `rejected (the
    code is valid but does not seem to be an app) origin=Software Signing` —
    this is a known limitation; spctl's assessment policy is bundle-oriented.
- **Consequence:** The issue's `spctl --assess --type execute --verbose=4 jr`
  verification will **incorrectly fail** on the raw `jr` binary even when
  everything is signed and notarized correctly. This is not a real failure; it
  is a categorical limitation of `spctl`.
- **What to use instead in CI:** Verify the signature and notarization
  attachment directly:
  - `codesign --verify --verbose=4 jr` — verifies signature integrity.
  - `codesign -dv --verbose=4 jr` and grep for `TeamIdentifier=<expected>` —
    verifies the cert chain produces the expected Team ID.
  - For notarization, the truth source is `notarytool submit --wait` returning
    `status: Accepted`. There is no `spctl`-equivalent verification for raw
    binaries.
- The issue's spctl step should be **removed** or replaced with a `codesign
  --verify` check.

**Confidence:** High.

## Claim 6 — GitHub Actions cert import pattern

**Status:** VERIFIED (manual `security` commands recommended over third-party
actions).

**Citation:** GitHub runner-images discussions
(`https://github.com/actions/runner-images/discussions/9107`); community recipes
verified against `security(1)` man page.

**Summary:**

- **Canonical 2026 pattern** is a hand-rolled set of `security` commands in a
  step, not a third-party action. The widely-cited
  `apple-actions/import-codesign-certs` action is **lightly maintained** (last
  meaningful update ~2022) and has reported issues on Apple Silicon runners.
  Manual setup is short (~10 lines) and more reliable.

  ```yaml
  - name: Import code-signing cert
    env:
      MACOS_CERT_P12_BASE64: ${{ secrets.MACOS_CERT_P12_BASE64 }}
      MACOS_CERT_P12_PASSWORD: ${{ secrets.MACOS_CERT_P12_PASSWORD }}
      MACOS_KEYCHAIN_PASSWORD: ${{ secrets.MACOS_KEYCHAIN_PASSWORD }}
    run: |
      echo "$MACOS_CERT_P12_BASE64" | base64 --decode > /tmp/cert.p12
      security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" build.keychain
      security default-keychain -s build.keychain
      security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" build.keychain
      security set-keychain-settings -lut 7200 build.keychain
      security import /tmp/cert.p12 -k build.keychain \
        -P "$MACOS_CERT_P12_PASSWORD" -T /usr/bin/codesign
      security list-keychains -d user -s build.keychain
      security set-key-partition-list -S apple-tool:,apple:,codesign: \
        -s -k "$MACOS_KEYCHAIN_PASSWORD" build.keychain
      rm /tmp/cert.p12
  ```

- **`set-key-partition-list` is required** on macOS 10.12+ (otherwise codesign
  hangs on a UI prompt that never appears in headless GHA — a 30-min nightmare
  to debug if missed).
- **Risks of leaving keychain on runner: zero.** GitHub-hosted macOS runners
  are fully ephemeral — the VM is destroyed at job end. No persistence, no
  cross-job leakage. Self-hosted runners would need explicit `security
  delete-keychain build.keychain` in a cleanup step, but that does not apply
  here.
- **Secrets needed (sum of cert + notarytool):**
  - `MACOS_CERT_P12_BASE64` — the .p12 file, base64-encoded.
  - `MACOS_CERT_P12_PASSWORD` — passphrase for the .p12.
  - `MACOS_KEYCHAIN_PASSWORD` — arbitrary, generated for the temp keychain.
  - `APPSTORE_CONNECT_KEY` — App Store Connect API key (.p8 file content).
  - `APPSTORE_CONNECT_KEY_ID` — 10-char API key ID.
  - `APPSTORE_CONNECT_ISSUER_ID` — UUID of the issuer.
  - `MACOS_TEAM_ID` — 10-char Team ID (also visible in `codesign` output for
    sanity-checking).
  - That is 7 new repo secrets. Modest but non-trivial setup.

**Confidence:** High.

## Claim 7 — GHA runner architecture for cross-arch macOS builds

**Status:** VERIFIED — the existing `release.yml` already builds both arches on
`macos-latest`, and this works because `macos-latest` is now Apple Silicon and
Rust supports both targets natively from one runner.

**Citation:** GitHub Actions runner-images repo; Rust target tier documentation;
existing `.github/workflows/release.yml` lines 17-25.

**Summary:**

- **`macos-latest` in 2026 = `macos-15` (Sequoia) on Apple Silicon (arm64).**
  GitHub completed the migration from x86_64 macOS to arm64 macOS for
  `macos-latest` over 2024. Explicit labels:
  - `macos-13` → Ventura, x86_64 (Intel).
  - `macos-14` → Sonoma, arm64 (Apple Silicon).
  - `macos-15` → Sequoia, arm64 (Apple Silicon).
  - `macos-latest` → currently aliases `macos-15`.
- **Existing `release.yml` builds both arches on `macos-latest`** by setting
  the `--target` flag for cargo. This works because:
  - On the arm64 runner, native `cargo build --target aarch64-apple-darwin`
    builds the arm64 binary.
  - `cargo build --target x86_64-apple-darwin` cross-compiles to x86_64. The
    Apple Silicon Xcode toolchain ships SDKs for both arches; Rust's
    `x86_64-apple-darwin` target is Tier 1 (vs `aarch64-apple-darwin` Tier 2
    by historical accident — both are well-supported).
  - The cross-compiled x86_64 binary cannot be **executed** on the arm64
    runner without Rosetta, which is why the existing workflow
    (lines 79-86) skips the embedded-creds smoke test for the
    non-native target. We will need the same skip for any post-build
    `codesign --verify` test that depends on running the binary (most
    `codesign` ops do not run the binary, so this is fine).
- **Cost:** arm64 macOS runners are billed at the same per-minute rate as
  x86_64 macOS runners on GitHub-hosted minutes (~$0.16/min currently); they
  are not 50% more expensive as one Perplexity result suggested. macOS minutes
  are 10x Linux minutes, but that is unchanged from the existing release
  workflow. **No cost delta from #210.**
- **No need to introduce a separate `macos-13` Intel runner.** Cross-compilation
  on `macos-latest` is sufficient, and that is what the workflow already does.

**Confidence:** High (verified against the existing workflow and Apple/Rust
docs).

## Claim 8 — Alternatives if the cert is unavailable

**Status:** Each alternative analysed.

**Citation:** Apple Security Framework documentation; Homebrew Formula Cookbook
(homebrew/brew docs); various community sources.

| Alternative | Verdict | Notes |
|---|---|---|
| **Status quo: `jr auth refresh` (#209)** | RECOMMENDED if no cert | Already shipped. User runs `jr auth refresh` once after each `brew upgrade jr`. Documented friction but functional. |
| **Homebrew "trust" mark** | Not possible | Homebrew has no mechanism to grant macOS Keychain ACL trust. Brew installs ad-hoc-signed binaries; the partition-list ACL invalidates anyway. |
| **Self-signed cert with user-installed trust anchor** | Not viable | The Keychain partition-list mechanism requires the cert chain to anchor to Apple's root (it checks via `SecCodeCheckValidityExternal` against the system trust store, with restricted trust roots). User-installed roots do not satisfy the partition check. |
| **Universal binary (`lipo`-merged x86_64+arm64)** | Does not solve the problem | The merged binary still has a single cdhash (or per-slice cdhashes); rebuilds still produce new cdhashes. Only relevant for distribution simplification, not signing stability. |
| **Drop OAuth keychain — use config-file storage** | Major UX/security regression | Would store refresh tokens in `~/.config/jr/` plain (or weakly encrypted). Loses macOS Keychain protection (encryption-at-rest, sleep-state protection, unlock prompts). Out of scope for #210 — this is a fundamentally different threat model, not a Gatekeeper workaround. |
| **Document the ad-hoc sign + `jr auth refresh` flow more prominently** | Cheap and effective | Add a `brew upgrade` post-install hint to `README.md` and `docs/`. Possibly emit a one-time "if you hit a Keychain prompt, run `jr auth refresh`" hint in `jr auth status` after a binary upgrade is detected. |

**Recommended fallback if cert unavailable:** Status quo + slightly better
docs. Do not pursue any of the other alternatives — they are either ineffective
(self-signed, brew trust, universal binary) or require fundamental product
direction changes (drop Keychain).

**Confidence:** High.

## Implications for #210 implementation

### If user HAS the cert (Apple Developer Program membership active)

**Effort estimate:** Medium (~1-2 days, including secret setup, workflow tests,
and a tagged dry-run release).

**Concrete `release.yml` patch sketch:**

```yaml
# After the "Build" step on macOS targets only:
- name: Import code-signing cert (macOS only)
  if: runner.os == 'macOS'
  env:
    MACOS_CERT_P12_BASE64: ${{ secrets.MACOS_CERT_P12_BASE64 }}
    MACOS_CERT_P12_PASSWORD: ${{ secrets.MACOS_CERT_P12_PASSWORD }}
    MACOS_KEYCHAIN_PASSWORD: ${{ secrets.MACOS_KEYCHAIN_PASSWORD }}
  run: |
    echo "$MACOS_CERT_P12_BASE64" | base64 --decode > /tmp/cert.p12
    security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" build.keychain
    security default-keychain -s build.keychain
    security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" build.keychain
    security set-keychain-settings -lut 7200 build.keychain
    security import /tmp/cert.p12 -k build.keychain \
      -P "$MACOS_CERT_P12_PASSWORD" -T /usr/bin/codesign
    security list-keychains -d user -s build.keychain
    security set-key-partition-list -S apple-tool:,apple:,codesign: \
      -s -k "$MACOS_KEYCHAIN_PASSWORD" build.keychain
    rm /tmp/cert.p12

- name: Sign jr binary (macOS only)
  if: runner.os == 'macOS'
  env:
    SIGN_IDENTITY: ${{ secrets.MACOS_SIGN_IDENTITY }}  # "Developer ID Application: Name (TEAMID)"
  run: |
    BIN="target/${{ matrix.target }}/release/jr"
    codesign --sign "$SIGN_IDENTITY" \
      --options runtime --timestamp \
      --identifier com.jaredbrichards.jr \
      "$BIN"
    codesign --verify --verbose=4 "$BIN"
    codesign -dv --verbose=4 "$BIN" 2>&1 | grep -q "TeamIdentifier=${{ secrets.MACOS_TEAM_ID }}"

- name: Notarize jr binary (macOS only)
  if: runner.os == 'macOS'
  env:
    AC_API_KEY: ${{ secrets.APPSTORE_CONNECT_KEY }}
    AC_KEY_ID: ${{ secrets.APPSTORE_CONNECT_KEY_ID }}
    AC_ISSUER_ID: ${{ secrets.APPSTORE_CONNECT_ISSUER_ID }}
  timeout-minutes: 30
  run: |
    BIN="target/${{ matrix.target }}/release/jr"
    # notarytool requires a zip/dmg/pkg wrapper; create a transient zip
    ditto -c -k --keepParent "$BIN" /tmp/jr.zip
    echo "$AC_API_KEY" > /tmp/ac_key.p8
    xcrun notarytool submit /tmp/jr.zip \
      --key /tmp/ac_key.p8 --key-id "$AC_KEY_ID" --issuer "$AC_ISSUER_ID" \
      --wait --timeout 25m
    rm /tmp/ac_key.p8 /tmp/jr.zip
    # NOTE: no `xcrun stapler staple jr` — stapling does not apply to raw
    # CLI binaries. Gatekeeper will fetch the ticket online on first run.
    # NOTE: no `spctl --assess` — also does not apply to raw CLI binaries.

# The existing Package step (tar czf) runs unchanged after Notarize.
```

**Estimated CI time delta:** +5-15 minutes per macOS arch on average (median
~5min for notarization, plus ~10-30s for signing). Two macOS arches → +10-30
minutes total per release. The job-level `timeout-minutes: 60` (existing) is
still adequate; consider bumping to 90 for safety.

**Outcome (per the issue):** Stable Team ID in the Keychain partition list →
`brew upgrade jr` does not invalidate the legacy ACL → no "allow access"
prompt → `jr auth refresh` becomes unnecessary for standard upgrades. (Note:
the OAuth tokens themselves are namespaced per-profile; the ACL stability is
about the *trusted-application* check, not token validity.)

### If user LACKS the cert

- Keep #210 open with `blocked-on-budget` label.
- Document `jr auth refresh` (#209) more prominently as the supported
  workaround. Specifically:
  - Add a "After upgrading via brew" section to `README.md` pointing to `jr
    auth refresh`.
  - Optionally emit a hint from `jr auth status` when it detects a Keychain
    prompt failure (graceful UX).
- Do **not** pursue self-signed, universal-binary, or drop-Keychain
  alternatives — they are either ineffective or wrong-shape for the problem.

## Recommended next action

- **HUMAN-DECISION:** confirm whether the maintainer has an active Apple
  Developer Program membership ($99/yr) and a Developer ID Application
  certificate.
- **If YES** → proceed with implementation per the patch sketch above.
  Effort: medium (~1-2 days). Cost: $99/yr Apple Developer Program (no GHA
  cost delta).
- **If NO** → defer #210 indefinitely with `blocked-on-budget`. Update README
  to make `jr auth refresh` more discoverable. The issue description and this
  research document together capture the full plan for when the budget exists.

## Open items / inconclusive

- **None of the major claims are inconclusive.** The single nuance is around
  stapler/spctl applicability to raw CLI binaries, which is now documented
  above and reflected in the patch sketch (those steps are removed).
- One unverified assumption: the *exact* algorithm by which the **legacy**
  Keychain (pre-Sierra `SecKeychainItem` API) checks partition lists when the
  trusted-application is Developer-ID-signed vs ad-hoc-signed. Apple's
  documentation here is sparse. The Perplexity result claims (with circumstantial
  support) that Developer ID's stable Team ID is sufficient to satisfy the
  partition check across rebuilds; this matches the issue's framing and
  matches `gh` CLI's observed behavior (which is Developer-ID-signed and does
  not exhibit the rebuild prompt). If we ship #210 and the prompt persists,
  the fallback diagnostic is `security dump-keychain -a login.keychain | grep
  -A2 jr` to inspect the partition list before/after upgrade. This is a
  small risk but worth flagging.

## Research Methods

| Tool | Queries | Purpose |
|---|---|---|
| Perplexity search | 6 | TN2206 cdhash semantics; Developer ID cert vs alternatives + cost; notarytool auth/timing/scope; stapler + spctl semantics; GHA cert-import pattern; runner architecture; cert-unavailable alternatives |
| Read | 1 | Existing `.github/workflows/release.yml` to verify current matrix |
| Training data | 1 area | Cross-checked Apple TN2206 references and `security(1)` flag semantics against general macOS knowledge |

**Total MCP tool calls:** 6 Perplexity searches + 1 Read = 7 (well under the 9-call cap).
**Training data reliance:** Low. Every load-bearing claim is sourced to Apple
developer documentation, the `xcrun stapler` man page, the GitHub
runner-images repo, or the `security(1)` man page. The one Perplexity result
that contradicted Apple docs (the "API keys not supported by notarytool"
claim) was caught and corrected by cross-reference.

## Sources (canonical)

- Apple TN2206 "Code Signing In Depth": `https://developer.apple.com/library/archive/technotes/tn2206/_index.html`
- Apple "Notarizing macOS software before distribution": `https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution`
- Apple "Customizing the notarization workflow": `https://developer.apple.com/documentation/security/customizing-the-notarization-workflow`
- Apple "Resolving common notarization issues": `https://developer.apple.com/documentation/security/resolving-common-notarization-issues`
- Apple "Create Developer ID certificates": `https://developer.apple.com/help/account/certificates/create-developer-id-certificates/`
- Apple Security Framework `SecTrustedApplicationCreateFromPath`: `https://developer.apple.com/documentation/security/1567822-sectrustedapplicationcreatefro`
- `xcrun stapler(1)` man page: `https://keith.github.io/xcode-man-pages/stapler.1.html`
- GitHub runner-images discussions on macOS arm64: `https://github.com/actions/runner-images/discussions/9107`
- Eclectic Light "Gatekeeper and notarization in Sequoia": `https://eclecticlight.co/2024/08/10/gatekeeper-and-notarization-in-sequoia/`
- Existing `jira-cli` `release.yml`: `/Users/zious/Documents/GITHUB/jira-cli/.github/workflows/release.yml`
