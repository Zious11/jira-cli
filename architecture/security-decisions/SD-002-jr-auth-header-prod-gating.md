# SD-002: JR_AUTH_HEADER Production Gating

**Status:** RESOLVED (canonized to Option B — `#[cfg(debug_assertions)]`, 2026-05-07)
**Owner:** Phase 3 SECURITY-DECIDE
**Deadline:** Resolved at Phase 1 → 2 gate (2026-05-04); canonized during S-0.05 implementation (2026-05-07)
**References:** NFR-S-B (nfr-catalog.md), R-H2 (risk-register.md), `src/api/client.rs:64-66`
**Version:** 1.0.1

---

## Context

`JiraClient::build_headers` reads `JR_AUTH_HEADER` unconditionally in the production binary at `client.rs:64-66`. Any process that inherits this environment variable from its parent (e.g., a CI runner, a shell script, or a test harness) bypasses keychain authentication entirely. This is intentional for integration tests (`JiraClient::new_for_test`), but the env-var check has no `#[cfg(test)]` gate — it is active in release builds.

**Risk surface:** In CI/CD environments (GitHub Actions, Jenkins) where env vars are shared between steps or jobs, a leaked `JR_AUTH_HEADER` containing a valid `Authorization: Bearer <token>` value would allow any step that runs `jr` to authenticate without the keychain. This is a privilege escalation vector in multi-tenant or shared-runner environments.

---

## Options

### Option A: Gate behind `#[cfg(test)]` (test-only)

- Wrap the `JR_AUTH_HEADER` read at `client.rs:64-66` in `#[cfg(test)]`.
- `JR_AUTH_HEADER` would then be a compile-time test-only mechanism.
- **Impact on tests:** Integration tests currently use `JiraClient::new_for_test(base_url, auth_header)` which passes the header directly as a constructor argument — these are unaffected. Any test that sets `JR_AUTH_HEADER` as an env var directly would break; search for such tests before applying.
- **Migration concern:** Removes a potentially useful debugging escape hatch for power users.
- **Critical limitation (discovered S-0.05, 2026-05-07):** Subprocess integration tests using `Command::cargo_bin("jr").env("JR_AUTH_HEADER", ...)` compile and spawn a separate `jr` binary. That binary is a release-mode-equivalent process — `cfg(test)` is NOT active in it. Therefore `#[cfg(test)]` in the library code does NOT cover the subprocess pattern. ~151 subprocess integration tests across ~20 test files would break if this option were applied literally.

### Option B (original): Require simultaneous `JR_BASE_URL`

- Only honor `JR_AUTH_HEADER` when `JR_BASE_URL` is also set.
- Reasoning: A rogue process inheriting `JR_AUTH_HEADER` alone is blocked; a test harness always sets both.
- Lowest-risk migration — no behavior change for integration tests (which always set both vars via `JiraClient::new_for_test`).
- Does not fully eliminate the risk in CI environments where `JR_BASE_URL` is also leaked, but substantially narrows the window.
- **Status:** NOT chosen. Superseded by Option B-revised below.

### Option B-revised (CHOSEN — canonized 2026-05-07): Gate behind `#[cfg(debug_assertions)]`

- Wrap the `JR_AUTH_HEADER` read at `client.rs:64-66` in `#[cfg(debug_assertions)]`.
- In `cargo build --release`, `debug_assertions` is false — the env-var read block is excluded from the release binary entirely.
- In `cargo test` (default debug profile), `debug_assertions` is true — the env-var read IS present, so both in-process and subprocess integration tests continue to work.
- **Impact on tests:** Zero migration required. All ~151 subprocess integration tests using `Command::cargo_bin("jr").env("JR_AUTH_HEADER", ...)` continue to pass because `cargo_bin` produces a debug binary.
- **Security equivalence:** The release-binary threat is fully mitigated. `debug_assertions = false` in release profile is a Rust compiler invariant — it cannot be overridden at runtime. The threat model concern (CI pipeline / shared runner inheriting `JR_AUTH_HEADER` into a deployed release binary) is eliminated identically to Option A.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| TBD  | PENDING  | Awaiting Phase 3 security review |
| **Decide-by** | **Phase 1 → 2 gate** | Required before Phase 2 story decomposition begins (ADV-P2-009) |
| 2026-05-04 | Option A — `#[cfg(test)]` compile-time gate | Categorical security guarantee — code excluded from release binary. Test migration cost bounded (most tests use `JiraClient::new_for_test` already). Rust 1.80+ check-cfg validates conditional-compilation specs. |
| 2026-05-07 | **CANONIZED to Option B-revised — `#[cfg(debug_assertions)]`** | During S-0.05 implementation, Red Gate analysis (AC-004 audit) found ~151 subprocess integration tests using `Command::cargo_bin("jr").env("JR_AUTH_HEADER", ...)`. These spawn a debug binary without `cfg(test)` active, so Option A would have broken all of them. Option B-revised (`#[cfg(debug_assertions)]`) achieves identical release-binary security with zero test migration cost. Security goal is equivalently met: release binaries do not honor `JR_AUTH_HEADER`. |

---

## Resolution

**Chosen option:** B-revised (`#[cfg(debug_assertions)]` compile-time gate) — canonized 2026-05-07

**Original gate decision (2026-05-04):** Option A (`#[cfg(test)]`) was selected at the Phase 1→2 gate based on perplexity research showing it provides the categorically strongest security posture: the env-var read is excluded from release-mode compiled binaries entirely, eliminating runtime exploitation vectors.

**Canonization rationale (2026-05-07):** During S-0.05 implementation, the Red Gate analysis (AC-004 audit) revealed that ~151 subprocess integration tests across ~20 test files use `Command::cargo_bin("jr").env("JR_AUTH_HEADER", ...)`. These tests invoke the `jr` binary as a child process. The binary spawned by `cargo_bin` is compiled in debug profile but is a separate process — `cfg(test)` is NOT active in the subprocess binary. A literal `#[cfg(test)]` gate in `client.rs` would therefore break all ~151 subprocess tests while providing no additional security benefit over `#[cfg(debug_assertions)]`.

**Why `#[cfg(debug_assertions)]` equivalently satisfies the security goal:**

1. **Release binary threat is fully mitigated.** `cargo build --release` always sets `debug_assertions = false` (Rust language guarantee). The env-var read block is excluded from all release binaries. The threat model concern — a CI pipeline or shared runner inheriting `JR_AUTH_HEADER` into a production-deployed `jr` binary — is eliminated.

2. **Debug binaries are not a production attack surface.** `jr` is a CLI tool distributed as a release binary. Debug builds are developer artifacts on developer machines. A developer machine that leaks `JR_AUTH_HEADER` in `debug_assertions = true` mode is not a meaningful threat in the `jr` threat model (single-user machine with no multi-tenant shared-runner context).

3. **Subprocess test pattern preserved.** `Command::cargo_bin("jr")` produces a debug binary. In debug profile, `debug_assertions = true`, so the env-var bypass remains active for subprocess tests. All ~151 existing subprocess integration tests continue to pass with zero migration effort.

4. **`#[cfg(debug_assertions)]` is a clean, well-understood Rust pattern** for "this code only exists in non-release builds." It is semantically correct for this use case.

**Implementation (canonized):**
```rust
// JR_AUTH_HEADER is debug-only: excluded from release builds via #[cfg(debug_assertions)] gate.
#[cfg(debug_assertions)]
if let Ok(header) = std::env::var("JR_AUTH_HEADER") {
    return Ok(header);
}
```

**Resolves DRIFT-002** — NFR-S-B holdout becomes definable now that fix path is fixed; queue for Phase 2 story decomposition.

## Resolution Requirement (fulfilled)

Before closing this SD, the Phase 3 implementer was required to:
1. Confirm which integration tests (if any) rely on `JR_AUTH_HEADER` as a bare env var (not via `new_for_test`). **Result:** ~151 subprocess tests use `.env("JR_AUTH_HEADER", ...)`. Zero in-process `env::var("JR_AUTH_HEADER")` calls exist in tests (all use `new_for_test` or subprocess pattern).
2. Choose and implement the gate. **Result:** Option B-revised (`#[cfg(debug_assertions)]`) implemented in S-0.05.
3. Add a test that verifies `JR_AUTH_HEADER` is NOT honored in the chosen constraint scenario. **Result:** `tests/auth_header_release_gate.rs` — 5 tests; Red Gate verified pre-fix; all pass post-fix.
4. Record the outcome in this document. **Result:** This canonization entry.
