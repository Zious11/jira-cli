# Phase F6 — Targeted Hardening Report

- **Story:** S-E2E-1 — Live-Jira E2E testing in CI
- **Branch:** `feat/e2e-live-jira-testing`
- **Worktree:** `/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-E2E-1`
- **Base:** `origin/develop` (three-dot diff `origin/develop...HEAD`)
- **Date:** 2026-05-29
- **Verdict:** **PASS-WITH-NOTES** (notes are non-blocking: pre-existing cosmetic
  `cargo deny` warnings unrelated to this delta; mutation/fuzz/proof are N/A by construction)

---

## 1. Delta scope — zero src/ changes (confirmed)

`git diff origin/develop...HEAD --stat`:

```
 .github/workflows/e2e.yml           |  133 ++++
 CLAUDE.md                           |   11 +
 docs/specs/e2e-live-jira-testing.md |  241 +++++++
 tests/auth_header_release_gate.rs   |    9 +
 tests/e2e_live.rs                   | 1173 +++++++++++++++++++++++++++++++++++
 5 files changed, 1567 insertions(+)
```

- `git diff origin/develop...HEAD --name-only | grep '^src/'` → **NONE**
- `git diff origin/develop...HEAD --name-only | grep -E 'Cargo\.(toml|lock)'` → **NONE**

**Finding:** The delta is entirely test code (`tests/e2e_live.rs` — new; `tests/auth_header_release_gate.rs` — 1-line exclusion), CI workflow (`.github/workflows/e2e.yml` — new), and docs (`CLAUDE.md`, `docs/specs/`). **No production (`src/`) code changed. No dependencies added (Cargo.toml/Cargo.lock unchanged).**

The single `tests/auth_header_release_gate.rs` change is a security-test exclusion that adds `--exclude=e2e_live.rs` to the SD-002 "no in-process `JR_AUTH_HEADER` readers" audit. It is justified in-line: `e2e_live.rs` reads `JR_AUTH_HEADER` only to forward it to a `jr` subprocess (identical to `.env("JR_AUTH_HEADER", …)` on a `Command` builder) — it constructs no `JiraClient` and cannot reach a live site via the in-process path. The release-gate invariant for production code is unaffected.

---

## 2. Mutation testing — N/A (0 mutants), tool present

`cargo-mutants 27.0.0` is installed. Run against the in-diff scope:

```
DIFF_FILE=$(mktemp …) && git diff origin/develop...HEAD > "$DIFF_FILE" \
  && cargo mutants --in-diff "$DIFF_FILE" --list
→ INFO No mutants to filter        (exit 0)
```

**Mutant count: 0.** Rationale: `cargo-mutants` mutates production functions; the diff contains zero `src/` functions, so there is nothing to mutate. Mutation testing is **not applicable** for this delta — and this is the correct, empirically-confirmed outcome, not a skip. Likewise **fuzzing and formal proofs (Kani) have no in-diff target** for the same reason: no new/changed pure-core production code exists to fuzz or prove.

---

## 3. Full regression suite — PASS

`cargo test` (no `JR_RUN_E2E` set; the live suite is a deliberate no-op):

- **Exit:** 0
- **Aggregate across all test binaries:** **1493 passed; 0 failed; 43 ignored.**
- **`tests/e2e_live.rs` binary:** `3 passed; 0 failed; 12 ignored`. The 12 live tests are correctly gated (`ignored, set JR_RUN_E2E=1 and use --include-ignored to run against a live Jira site`); the 3 passing are the gate-invariant / pure-logic tests (`e2e_enabled_from`, `test_suite_is_noop_without_jr_run_e2e`) that must run in normal CI. **No live Jira site was contacted** — `JR_RUN_E2E` was intentionally left unset.

The double safety on the gate is present in `tests/e2e_live.rs`: env gate (`JR_RUN_E2E=1`) + `#[ignore]` + early-return guard (`if !e2e_enabled() { return; }`), plus an always-run invariant test that fails loudly if `JR_RUN_E2E=1` ever leaks into `ci.yml`.

---

## 4. Lints — PASS

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | CLEAN (exit 0) |
| Clippy | `cargo clippy --all-targets -- -D warnings` | CLEAN (exit 0, 0 warnings/errors) |

---

## 5. Security / supply-chain scans (full tree) — PASS-WITH-NOTES

### 5a. `cargo deny check`
- **Exit:** 0 — `advisories ok, bans ok, licenses ok, sources ok`
- **Note (non-blocking, pre-existing):** 3 cosmetic `license-not-encountered` warnings for `BSD-2-Clause`, `OpenSSL`, `Unicode-DFS-2016` allow-list entries in `deny.toml` that no current dependency uses. These are pre-existing deny.toml config entries, **unrelated to this delta** — the delta adds zero dependencies (Cargo.toml/Cargo.lock unchanged, confirmed in §1), so it introduces no new supply-chain surface and cannot have caused these warnings. Out of F6 scope.

### 5b. No secrets committed
`git diff origin/develop...HEAD | grep -iE "(api[_-]?token|secret|password|BEGIN .*PRIVATE KEY)"`:
- All matches are either `${{ secrets.* }}` GitHub Actions references, prose describing the secret-gating mechanism, or env-var **names** (`JR_E2E_API_TOKEN`, `JR_E2E_EMAIL`). **No literal credentials, tokens, passwords, or private keys are present.** Auth is composed at runtime from environment secrets (`Basic $(printf '%s:%s' "$JR_E2E_EMAIL" "$JR_E2E_API_TOKEN" | base64 …)`).

### 5c. No new dependencies
Cargo.toml / Cargo.lock are **not** in the delta (§1) → no new third-party code, no new advisory surface.

---

## 6. e2e.yml security posture (static re-attestation)

Re-confirmed present in `.github/workflows/e2e.yml` (F5-reviewed; re-attested here):

- **Egress-policy block:** `step-security/harden-runner@…v2.19.3` with `egress-policy: block` and a fail-closed pinned allow-list (wildcard `*.atlassian.net:443`; no real subdomain hard-coded). An unlisted host fails the job rather than leaking the credential.
- **No `pull_request_target`:** triggers are `push: [develop, main]`, `schedule` (nightly 06:00 UTC), and `workflow_dispatch` only. No `pull_request` / `pull_request_target` trigger.
- **Fork-PR guard (belt-and-suspenders):** job-level `if: github.event_name != 'pull_request'` even though no PR trigger exists.
- **Environment-gated secrets:** `environment: jira-e2e` — secrets are gated to the Environment + its deployment-branch policy (develop/main); fork PRs cannot read them. GitHub also withholds secrets from `pull_request` runs by default.
- **Least privilege:** `permissions: { contents: read }`.
- **Concurrency safety:** `group: jira-e2e`, `cancel-in-progress: false` — never cancels a run mid-flight (avoids orphaned in-progress issues with no teardown).
- **Always-run teardown:** `if: always()` close-only teardown step closes issues labelled for the run; `|| true` keeps a partial teardown from aborting; `timeout-minutes: 20`. All third-party actions are SHA-pinned.

---

## 7. Verdict

**PASS-WITH-NOTES.**

- Zero `src/` delta confirmed empirically; the feature is test + CI + docs only.
- Mutation testing returns 0 mutants (N/A by construction); fuzzing and formal proofs have no in-diff target for the same reason.
- Full regression green: 1493 passed / 0 failed / 43 ignored; live E2E suite a confirmed no-op (no live site contacted).
- Lints clean (fmt + clippy `-D warnings`).
- Security scans clean: `cargo deny` exit 0; no committed secrets; no new dependencies.
- e2e.yml security posture re-attested: egress block, no `pull_request_target`, environment-gated secrets, fork-PR guard, least-privilege, SHA-pinned actions.

**Notes (all non-blocking, none caused by this delta):** 3 pre-existing cosmetic `cargo deny` `license-not-encountered` warnings in `deny.toml`. Recommend proceeding to F7 (delta convergence + PR).
