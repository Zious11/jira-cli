---
name: jira-cli CI/CD Audit
version: "1.0"
date: "2026-05-04"
status: AUDIT-COMPLETE
mode: audit-only
snapshot_sha: dea166471e22eff55974d7675593469b37048c5f
product_version: v0.5.0-dev.7
---

# CI/CD Audit — jira-cli (jr)

Brownfield audit. The repository is functional at v0.5.0-dev.7. This document
inventories existing workflows, checks them against VSDD requirements, and
identifies gaps for Phase 3 remediation. No workflow files were modified.

---

## §1: Existing Workflows Inventory

**Workflow files found:** 2 (`.github/workflows/ci.yml`, `.github/workflows/release.yml`)
**Last updated:** 2026-05-16 — §1.1 job catalog updated to include `mutants` job (issue #346, F2 arch update)
**Supplementary:** `.github/dependabot.yml`, `.github/CODEOWNERS`

---

### 1.1 `ci.yml` (70 LOC)

**Purpose:** Continuous integration gate on all pushes and PRs.

**Triggers:**
- `push` to branches `main` and `develop`
- `pull_request` (all branches, all events)

**Jobs:**

| Job | Runner | What it does |
|---|---|---|
| `fmt` | `ubuntu-latest` | `cargo fmt --all -- --check` — format check, zero tolerance |
| `clippy` | `ubuntu-latest` | `cargo clippy --all --all-features --tests -- -D warnings` — zero-warnings policy |
| `test` | `ubuntu-latest`, `macos-latest` (matrix) | `cargo test --all-features` — unit + integration tests on two platforms |
| `msrv` | `ubuntu-latest` | `cargo check --all-features` against `dtolnay/rust-toolchain@1.85.0` — MSRV pin verification |
| `deny` | `ubuntu-latest` | `EmbarkStudios/cargo-deny-action@v2` — license allowlist + advisory check |
| `coverage` | `ubuntu-latest` | `cargo llvm-cov` → `lcov.info` → Codecov upload (`fail_ci_if_error: false`) |
| `mutants` | `ubuntu-latest` | `cargo-mutants` mutation testing — scoped to `src/cli/issue/create.rs`, `src/api/jira/bulk.rs`, `src/types/jira/bulk.rs`; PR-only trigger (`if: github.event_name == 'pull_request'`); `--in-diff <diff-file>` (diff written via `git diff origin/${{ github.base_ref }}...HEAD > "$DIFF_FILE"`; cargo-mutants v27 requires a file path, not a git ref); scope via `.cargo/mutants.toml::examine_globs`; 90% kill-rate target enforced via inline shell; `timeout-minutes: 60`; installed via `cargo install cargo-mutants` (no new SHA-pin surface). See §1.1a for full specification. |

**Caching:** `Swatinem/rust-cache@v2` on `clippy`, `test`, `msrv`, `coverage` jobs.

**What it does NOT do:**
- No explicit timeout values on any job or step (except the `mutants` job — see §1.1a)
- No Semgrep / CodeQL static analysis
- No SBOM generation
- No secrets scanning
- No action SHA pinning (uses `@v6`, `@v2`, `@v7` tags, not SHA hashes)
- No `cargo audit` (cargo-deny covers advisories but is a different tool)
- `coverage` upload uses `fail_ci_if_error: false` — codecov failures are silent

---

### 1.1a `mutants` Job — Full Specification

**Added by:** Issue #346 (F2 arch update, 2026-05-16)

**Purpose:** Mutation testing on the bulk create/edit modules to detect weak test assertions — tests that pass even when the implementation is silently broken by small code mutations (negated conditions, removed returns, swapped operators). Complements unit tests, integration tests, and proptests as a meta-verification layer.

**Tooling:** `cargo-mutants` — a binary tool installed via `cargo install cargo-mutants` in the CI step. It is NOT a Cargo dependency and MUST NOT be added to `[dev-dependencies]` in `Cargo.toml`. See `CLAUDE.md` AI Agent Notes for the canonical prohibition.

**Trigger:**
```yaml
if: github.event_name == 'pull_request'
```
Runs on PRs to `develop` only. Does NOT run on direct push to `develop` or `main`, bounding blast radius to the PR review phase. Pattern mirrors the existing `security` job guard.

**Scope:** Three files, fixed in `.cargo/mutants.toml` and enforced via `examine_globs` (no `--file` flags in the CI invocation). Note: cargo-mutants v27 reads configuration from `.cargo/mutants.toml`, not `.mutants.toml` at repo root.
- `src/cli/issue/create.rs`
- `src/api/jira/bulk.rs`
- `src/types/jira/bulk.rs`

**Diff-mode:** `--in-diff <diff-file>` — only mutates lines that are changed in the PR diff. Note: cargo-mutants v27 requires `--in-diff` to receive a file path (not a git ref directly); the CI workflow writes the diff via `git diff origin/${{ github.base_ref }}...HEAD > "$DIFF_FILE"` before invoking cargo-mutants. The diff-file path uses `${{ runner.temp }}/pr-${{ github.run_id }}.diff` for run-unique safety. Local invocation: use `mktemp -t pr.diff.XXXXXX`. This amortizes the per-mutant cost: a PR touching 50 lines runs in minutes rather than hours. Full-file mutation (without `--in-diff`) is reserved for local baseline runs.

**Kill-rate target:** 90% (hardcoded in the workflow shell script at `.github/workflows/ci.yml` Check kill rate step; cargo-mutants v27 does not expose a fail-under threshold via TOML config). Threshold enforcement is a shell one-liner in the CI step reading `mutants.out/caught.txt` and `mutants.out/missed.txt`. Kill rate is computed as `caught / (caught + missed + timeout)` per cargo-mutants v27 convention. Unviable mutants (build errors under mutation) are excluded from the denominator.

**Timeout:** `timeout-minutes: 60` at the job level. This satisfies the GAP-2 timeout gap for this job specifically. The `--in-diff` scoping keeps actual runtime well under this ceiling for typical PRs; the limit guards against runaway full-corpus mutation if the diff guard fails.

**Caching:** `Swatinem/rust-cache@v2` (shared with other jobs). `mutants.out/` is NOT cached — results must be fresh per run.

**Whitelist convention:** `#[mutants::skip]` attribute with a mandatory justification comment on the same or preceding line:
```rust
// mutants::skip: this branch is unreachable under normal inputs; covered by property test X
#[mutants::skip]
fn invariant_guard(...) { ... }
```
The convention and rationale are codified in `docs/specs/cargo-mutants-policy.md`.

**Deferral policy:** If the initial baseline run reveals surviving mutants below the 90% threshold, the PR is NOT blocked. Instead:
1. For each surviving-mutant cluster: either add `#[mutants::skip]` with a mandatory justification comment, OR file a follow-up issue per uncovered region.
2. `track-debt` entries are filed for each whitelisted case.
3. The 90% gate becomes enforced in full after the baseline issue set is addressed.

**Security surface:** No new secrets required. No new network calls from the harness. No `GITHUB_TOKEN` permission escalation. NFR-S-E compliance maintained: installs via `cargo install` in a `run:` step (not a `uses:` action reference), introducing zero new SHA-pin surface.

**Artifacts produced:** `mutants.out/` directory (gitignored via `.gitignore` entry added in issue #346). Not uploaded as a CI artifact; results are visible in the job log.

**Policy document:** `docs/specs/cargo-mutants-policy.md` — codifies the skip convention, kill-rate rationale, and deferral policy. Required reading before applying any `#[mutants::skip]` annotation.

---

### 1.2 `release.yml` (159 LOC)

**Purpose:** Build and publish release binaries when a `v*` tag is pushed.

**Triggers:** `push` of tags matching `v*`

**Permissions:** `contents: write` (required for GitHub Release creation)

**Jobs:**

| Job | Runner(s) | What it does |
|---|---|---|
| `build` | `macos-latest` (×2), `ubuntu-latest` (×2) | Cross-platform build across 4 targets |
| `release` | `ubuntu-latest` | Downloads all build artifacts; creates GitHub Release via `softprops/action-gh-release@v2` |

**Build matrix targets:**
- `x86_64-apple-darwin` (macOS)
- `aarch64-apple-darwin` (macOS, native)
- `x86_64-unknown-linux-gnu` (Linux)
- `aarch64-unknown-linux-gnu` (Linux, via `cross`)

**Notable behaviors:**
- Injects `JR_BUILD_OAUTH_CLIENT_ID`/`JR_BUILD_OAUTH_CLIENT_SECRET` from GitHub Secrets into build env (ADR-0006 embedded OAuth)
- Cross-compilation via `cargo install cross` fetched from GitHub at runtime (supply chain risk: no SHA pin on cross install source)
- Post-build embedded-OAuth smoke verifies `jr auth status` reports `OAuth app: embedded` on native targets
- SHA256 sum generated per tarball (Linux: `sha256sum`, macOS: `shasum -a 256`)
- Pre-release auto-detection via `contains(github.ref_name, '-')`
- Auto-generated release notes via `softprops/action-gh-release@v2`

**What it does NOT do:**
- No GPG / sigstore / cosign binary signing
- No SBOM attachment to release
- No action SHA pinning
- No explicit job timeout values
- `cross` installed via `cargo install ... --git https://github.com/cross-rs/cross` without version lock at runtime (the install itself is not reproducible)

---

### 1.3 `dependabot.yml`

**Purpose:** Automated dependency update PRs.

**Ecosystems monitored:**
- `cargo` — weekly, max 5 open PRs
- `github-actions` — weekly, max 5 open PRs

**What it does NOT do:**
- No grouping rules (each dep gets its own PR)
- No auto-merge configuration
- No ignore list for noisy dependencies

---

### 1.4 `CODEOWNERS`

```
* @Zious11
```

All files require review from `@Zious11`. This satisfies the "code owner approval on PRs" requirement from `CLAUDE.md`.

---

## §2: VSDD CI/CD Requirements Checklist

| Requirement | Status | Evidence |
|---|---|---|
| Build on PR + push to main/develop | **PRESENT** | `ci.yml` triggers: `push: branches [main, develop]` + `pull_request` |
| Tests run (unit + integration) | **PRESENT** | `ci.yml` `test` job: `cargo test --all-features` on ubuntu + macos matrix |
| Clippy zero-warnings policy | **PRESENT** | `ci.yml` `clippy` job: `cargo clippy --all --all-features --tests -- -D warnings` (`ci.yml:25`) |
| Format check (`cargo fmt --all -- --check`) | **PRESENT** | `ci.yml` `fmt` job (`ci.yml:16-17`) |
| cargo-deny supply chain audit | **PRESENT** | `ci.yml` `deny` job: `EmbarkStudios/cargo-deny-action@v2` (`ci.yml:47-52`) |
| Branch protection on main + develop (CI required + code owner) | **PRESENT-PARTIAL** | `CODEOWNERS` file present with `@Zious11`. Branch protection API settings must be verified live (see §5). CI status checks are expected to be registered. |
| Release workflow (on tag) | **PRESENT** | `release.yml` triggers on `v*` tags; builds 4 targets; creates GitHub Release |
| Dependabot / Renovate for dependency updates | **PRESENT** | `.github/dependabot.yml` covers both cargo and github-actions ecosystems |
| Security scan (cargo-audit / Semgrep / CodeQL) | **PRESENT-PARTIAL** | `cargo-deny` covers the advisories database (equivalent to `cargo-audit` for known CVEs). No Semgrep, no CodeQL, no standalone `cargo-audit` run. |
| License scanning | **PRESENT** | `deny.toml` defines an explicit license allowlist; enforced via `cargo-deny-action` in CI |
| Secrets scanning | **MISSING** | No `gitleaks`, `trufflehog`, `git-secrets`, or GitHub secret scanning configuration present |
| Job timeout values | **MISSING** | No `timeout-minutes:` on any job in `ci.yml` or `release.yml` |
| Action SHA pinning | **MISSING** | All actions use version tags (`@v6`, `@v2`, `@v7`, `@v8`, `@stable`, `@1.85.0`) rather than full SHA hashes |
| MSRV verification in CI | **PRESENT** | `ci.yml` `msrv` job: `dtolnay/rust-toolchain@1.85.0` + `cargo check --all-features` |
| Coverage reporting | **PRESENT** | `ci.yml` `coverage` job: `cargo llvm-cov` → Codecov |
| Mutation testing (meta-verification layer) | **PRESENT** | `ci.yml` `mutants` job (added issue #346): `cargo-mutants` scoped to bulk + edit modules via `.cargo/mutants.toml::examine_globs`; PR-only; `--in-diff <diff-file>` mode (v27 file-path form); 90% kill-rate target (`caught / (caught + missed + timeout)`); `timeout-minutes: 60`. Policy in `docs/specs/cargo-mutants-policy.md`. |

**Summary counts:**
- PRESENT: 10
- PRESENT-PARTIAL: 2
- MISSING: 3

---

## §3: Pre-commit Hooks

**Finding: No pre-commit hook framework is configured.**

Searched for: `.pre-commit-config.yaml`, `lefthook.yml`, `lefthook.yaml`, `.lefthook.yml`, `.husky/` directory. None found in the repository root.

**What this means:** There is no automated enforcement of format, lint, or test at commit time locally. The CI pipeline in `ci.yml` is the only enforcement gate. Developers can commit code that fails `cargo fmt` or `cargo clippy` locally and only discover the failure after a push triggers CI.

**`rust-toolchain.toml`** pins `channel = "stable"` with components `rustfmt`, `clippy`, `llvm-tools-preview` — the tools are available, but nothing enforces their use pre-commit.

**Current local dev workflow (inferred):** Developers must manually run `cargo fmt`, `cargo clippy`, and `cargo test` before pushing. `CLAUDE.md` documents the commands but enforces nothing.

---

## §4: Gap Analysis

### GAP-1: No action SHA pinning (HIGH)

**What's missing:** All GitHub Actions references use mutable version tags (`actions/checkout@v6`, `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2`, `softprops/action-gh-release@v2`, `actions/upload-artifact@v7`, `actions/download-artifact@v8`, `codecov/codecov-action@v6`, `taiki-e/install-action@cargo-llvm-cov`). A tag can be force-pushed to point at malicious content at any time.

**Why it matters (NFR-S-B cross-reference):** Pass 4 §2.9 confirms that `JR_BUILD_OAUTH_CLIENT_ID`/`JR_BUILD_OAUTH_CLIENT_SECRET` are injected into the release build environment. If any action in the release pipeline is compromised via tag re-pointing, the OAuth client secret is directly accessible to attacker-controlled code in the runner environment. This is a supply-chain attack vector with high-value credential exposure.

**Recommended action:** Pin every action to its full commit SHA. Use `renovate` or `dependabot` (github-actions ecosystem already configured) to keep SHAs current. Example:
```yaml
uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2
```

**Severity: HIGH**

> **Post-Pass-2 reconciliation note:** Severity rebased to HIGH from CRITICAL post-Pass-2 reconciliation: rare event but high impact (direct OAuth client secret exposure). NFR-S-E action SHA pinning registered as R-H6 in risk-register.md.

---

### GAP-2: No job timeout values (HIGH)

**What's missing:** No `timeout-minutes:` on any job in `ci.yml` or `release.yml`.

**Why it matters:** Without timeouts, a hung build (e.g., wiremock test waiting for a port that never opens, or a cross-compilation that deadlocks) will consume the full GitHub Actions billing window (6 hours default) before failing. For a project with a 4-target release matrix, a timeout-runaway costs 24 runner-hours per incident. The release workflow is particularly exposed because `cargo install cross --git ...` involves a network fetch with no timeout guard.

**Recommended action:** Add `timeout-minutes: 30` at the job level for all CI jobs; `timeout-minutes: 60` for release build jobs (cross-compilation is slower). Add `timeout-minutes: 5` for the standalone `fmt` and `deny` jobs.

**Severity: HIGH**

---

### GAP-3: No secrets scanning (HIGH)

**What's missing:** No secrets scanning in CI and no GitHub repository-level secret scanning configured.

**Why it matters (NFR-S-B cross-reference):** Pass 4 NFR-S-B (HIGH) identifies `JR_AUTH_HEADER` as an env-var override that bypasses keychain — a pattern that could accidentally leak credentials if a developer inadvertently commits a `.env` file or shell history. The project handles OAuth tokens and API tokens. Without secrets scanning, a committed credential would not be detected until manually noticed. Pass 4 §2.9 also documents that `OAUTH_CLIENT_ID`/`OAUTH_CLIENT_SECRET` are CI secrets — a developer working on embedded OAuth could accidentally echo them in a debug step.

**Recommended action:** Enable GitHub secret scanning at the repository level (Settings > Security > Secret scanning). Add `gitleaks` as a step in `ci.yml` security scan job for local pre-push detection.

**Severity: HIGH**

---

### GAP-4: No SBOM generation (MEDIUM)

**What's missing:** No Software Bill of Materials attached to releases. Pass 4 §2.8 explicitly verified: no `cargo-cyclonedx`, `cargo-sbom`, `syft`, or SPDX generation in any workflow.

**Why it matters (NFR catalog §2.8, item 7):** `jr` handles OAuth tokens and Jira credentials. Downstream security teams (enterprise users) may require SBOM for compliance. With 332 transitive dependencies, the supply chain surface is non-trivial. SBOM publication also enables automated vulnerability tracking via tools like Dependency-Track.

**Recommended action:** Add `cargo-cyclonedx` or `syft` as a step in `release.yml` after build; attach the SBOM JSON as a release artifact. This is a Phase 3 addition to `release.yml`.

**Severity: MEDIUM**

---

### GAP-5: No release binary signing (MEDIUM)

**What's missing:** Binaries are distributed as tarballs with SHA256 checksums, but no cryptographic signature proves provenance. Pass 4 §2.10 explicitly confirms: "No GPG/sigstore signing."

**Why it matters (NFR catalog §2.8, item 8):** SHA256 sums verify integrity (the file was not corrupted in transit) but not authenticity (the file came from this repository's CI). An attacker who can publish a GitHub Release (e.g., via a compromised maintainer token) can replace binaries and generate matching SHA256 sums. Sigstore/cosign signing via GitHub OIDC provides non-repudiable provenance without requiring key management.

**Recommended action:** Add `sigstore/cosign-installer` + `cosign sign-blob` in `release.yml` after the Package step. Attach `.sig` files alongside tarballs. This is a policy decision (spec/policy) plus a CI/CD change.

**Severity: MEDIUM**

---

### GAP-6: `cargo-deny multiple-versions = "warn"` not blocking (LOW)

**What's missing:** `deny.toml` sets `multiple-versions = "warn"`, meaning duplicate transitive crate versions are flagged but do not fail the build.

**Why it matters (NFR catalog §2.8, item 12):** With 332 transitive dependencies, duplicate crate versions expand the attack surface and binary size. The `warn` setting means CI passes even with many duplicates. Making this `deny` would require deduplication effort but would enforce a tighter supply chain.

**Recommended action:** Inventory current duplicates (`cargo deny check bans 2>&1 | grep warning`), deduplicate where feasible, then raise to `multiple-versions = "deny"`. Or document the accepted duplicates via explicit `[[bans.skip]]` entries and raise to `deny`. This is a config-only change to `deny.toml`.

**Severity: LOW**

---

### GAP-7: No pre-commit hook framework (LOW)

**What's missing:** No `lefthook`, `pre-commit`, or `husky` configuration. Developers can commit non-formatted or clippy-failing code; CI is the only gate.

**Why it matters:** Developer feedback loop is slower (CI roundtrip vs. local instant check). Particularly relevant because `CLAUDE.md` prohibits lint suppression and requires format compliance — these are enforced only after push.

**Recommended action:** Add `lefthook` with hooks for `cargo fmt --check` and `cargo clippy -- -D warnings` on pre-commit. Low maintenance overhead. Opt-in via `lefthook install`.

**Severity: LOW**

---

## §5: Branch Protection State

Branch protection cannot be verified live from this audit (no `gh` CLI access in this dispatch). The required state, per `CLAUDE.md` ("Protected branches: main and develop require CI to pass and code owner approval on PRs. Admins can bypass."), is:

**Required protection state for `develop` and `main`:**

```json
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "CI / fmt",
      "CI / clippy",
      "CI / test (ubuntu-latest)",
      "CI / test (macos-latest)",
      "CI / msrv (1.85.0)",
      "CI / deny (licenses + vulnerabilities)"
    ]
  },
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "require_code_owner_reviews": true
  },
  "enforce_admins": false,
  "restrictions": null
}
```

**Note on coverage job:** The `coverage` job uploads to Codecov with `fail_ci_if_error: false`. It should NOT be a required status check since it can succeed even with partial network failures.

**Verification command (for state-manager to run):**
```bash
gh api repos/Zious11/jira-cli/branches/develop/protection
gh api repos/Zious11/jira-cli/branches/main/protection
```

---

## §6: Recommendations for Phase 3

### Fix in Phase 3 (required for security posture before feature delivery)

| Gap | Action | Effort |
|---|---|---|
| GAP-1 (HIGH): Action SHA pinning | Pin all 8 action references to full SHA in `ci.yml` and `release.yml`. Use `dependabot` (already configured for github-actions) to keep SHAs current. | 30 min |
| GAP-2 (HIGH): Job timeouts | Add `timeout-minutes:` to every job in both workflow files. | 15 min |
| GAP-3 (HIGH): Secrets scanning | Enable GitHub secret scanning at repository level (Settings toggle, no code change). Optionally add `gitleaks` step to a new `security.yml`. | 1 hour |

### Added in subsequent cycles (post-audit)

| Addition | Issue | Description |
|---|---|---|
| `mutants` CI job | #346 | Mutation testing on bulk + edit modules (see §1.1a). Complements unit tests, integration tests, and proptests as a meta-verification layer: detects weak assertions that would pass even when the implementation is silently broken. PR-only, `--in-diff <diff-file>` mode (v27 file-path form; scope via `.cargo/mutants.toml::examine_globs`), 90% kill-rate target (`caught / (caught + missed + timeout)`). |

### Defer as tech debt (Phase 3+ or post-v1.0)

| Gap | Rationale for deferral |
|---|---|
| GAP-4 (MEDIUM): SBOM | No current enterprise user requirement. Add when first enterprise customer requests it. `syft` step in `release.yml` is a 10-line addition when needed. |
| GAP-5 (MEDIUM): Binary signing | Requires policy decision on signing key management (OIDC keyless via Sigstore is the path of least resistance). Defer to post-v1.0 release workflow hardening. |
| GAP-6 (LOW): `multiple-versions = deny` | Requires dependency inventory and deduplication sprint. Not blocking for correctness. Defer to a dedicated maintenance sprint. |
| GAP-7 (LOW): Pre-commit hooks | Purely developer ergonomics. Opt-in via `lefthook install` is low-risk. Defer or add as a contributor documentation item. |

---

## §7: Cross-Reference with NFR Catalog

From Pass 4 R4 NFR catalog (44 total concerns). The following supply-chain and security NFRs have direct CI/CD implications:

| NFR ID | Severity | Description | Requires CI/CD change? | Action type |
|---|---|---|---|---|
| NFR-S-B | HIGH | `JR_AUTH_HEADER` env-var auth bypass; secrets exposed in insecure CI envs | Yes — indirect: action SHA pinning (GAP-1) limits attack vector | CI/CD change (GAP-1) + spec note |
| NFR-S-A | MEDIUM | No PKCE in OAuth flow | No — purely a code/spec change | Spec/policy change only |
| Supply chain (deny.toml) | MEDIUM | `unknown-registry = "warn"` / `unknown-git = "warn"` in deny.toml not blocking | Yes — raise deny.toml settings to `deny` | Config change to `deny.toml` (no new workflow needed) — see ADR-0003 / GAP-6 note below |
| Pass 4 §2.8 item 7 | MEDIUM | No SBOM generation in CI | Yes — add SBOM step in `release.yml` | CI/CD change (GAP-4, deferred) |
| Pass 4 §2.8 item 8 | MEDIUM | No GPG/sigstore binary signing | Yes — add cosign step in `release.yml` | CI/CD change (GAP-5, deferred) + policy decision |
| Pass 4 §2.8 item 12 | LOW | `multiple-versions = "warn"` not blocking | Yes — change `deny.toml` `[bans]` setting | Config change to `deny.toml` (GAP-6, deferred) |

**Items requiring only spec/policy changes (no CI/CD file modification):**
- NFR-S-A (PKCE): A code change in `api/auth.rs`. CI runs tests on merge, which would cover this once implemented. No new workflow needed.
- NFR-S-C (`--verbose` PII redaction): A code change in `api/client.rs` (`redact_body()` helper). No CI/CD modification. See nfr-catalog.md §NFR-S-C.
- Supply chain deny.toml (`unknown-registry`/`unknown-git`): A one-line change in `deny.toml`. Existing `deny` job in `ci.yml` immediately enforces it. This concern is separate from NFR-S-C (which is `--verbose` PII); it is classified under the supply-chain dimension per ADR-0003 / GAP-6.

**Items requiring CI/CD file modification:**
- GAP-1 (action SHA pinning): `ci.yml` + `release.yml` — Phase 3 mandatory
- GAP-2 (job timeouts): `ci.yml` + `release.yml` — Phase 3 mandatory
- GAP-3 (secrets scanning): new `security.yml` or repository settings — Phase 3 mandatory
- GAP-4 (SBOM): `release.yml` addition — deferred
- GAP-5 (signing): `release.yml` addition — deferred

---

## Appendix A: Action Version Inventory (unpinned)

All actions present in the current workflows, as-found. These all require SHA pinning (GAP-1):

| Action | Tag used | Workflow | Risk |
|---|---|---|---|
| `actions/checkout` | `@v6` | `ci.yml`, `release.yml` | High — executes in all jobs |
| `Swatinem/rust-cache` | `@v2` | `ci.yml` (clippy, test, msrv, coverage) | Medium — cache write access |
| `dtolnay/rust-toolchain` | `@stable`, `@1.85.0` | `ci.yml` | Medium — installs toolchain |
| `taiki-e/install-action` | `@cargo-llvm-cov` | `ci.yml` | Medium — installs tool |
| `EmbarkStudios/cargo-deny-action` | `@v2` | `ci.yml` | Medium |
| `codecov/codecov-action` | `@v6` | `ci.yml` | Low (fail_ci_if_error: false) |
| `actions/upload-artifact` | `@v7` | `release.yml` | Medium |
| `actions/download-artifact` | `@v8` | `release.yml` | Medium |
| `softprops/action-gh-release` | `@v2` | `release.yml` | High — creates GitHub releases with `contents: write` |

**Highest-risk unpinned action:** `softprops/action-gh-release@v2` — runs with `contents: write` permission during releases and receives `OAUTH_CLIENT_ID`/`OAUTH_CLIENT_SECRET` in the same pipeline environment.
