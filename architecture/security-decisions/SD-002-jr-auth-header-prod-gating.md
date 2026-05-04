# SD-002: JR_AUTH_HEADER Production Gating

**Status:** PENDING
**Owner:** Phase 3 SECURITY-DECIDE
**Deadline:** TBD — must resolve before Phase 3 gate
**References:** NFR-S-B (nfr-catalog.md), R-H2 (risk-register.md), `src/api/client.rs:64-66`

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

### Option B: Require simultaneous `JR_BASE_URL`

- Only honor `JR_AUTH_HEADER` when `JR_BASE_URL` is also set.
- Reasoning: A rogue process inheriting `JR_AUTH_HEADER` alone is blocked; a test harness always sets both.
- Lowest-risk migration — no behavior change for integration tests (which always set both vars via `JiraClient::new_for_test`).
- Does not fully eliminate the risk in CI environments where `JR_BASE_URL` is also leaked, but substantially narrows the window.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| TBD  | PENDING  | Awaiting Phase 3 security review |

---

## Resolution Requirement

Before closing this SD, the Phase 3 implementer must:
1. Confirm which integration tests (if any) rely on `JR_AUTH_HEADER` as a bare env var (not via `new_for_test`).
2. Choose Option A or Option B and implement it.
3. Add a test that verifies `JR_AUTH_HEADER` is NOT honored in the chosen constraint scenario.
4. Record the outcome in this document.
