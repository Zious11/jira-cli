---
document_type: demo-evidence-report
product: "jr (Jira CLI)"
story_id: S-0.05
pipeline_run: "2026-05-07"
demo_type: cli
recording_tool: vhs
status: complete
---

# Demo Evidence Report — S-0.05

## Product: jr (Jira CLI)
## Story: S-0.05 — Gate JR_AUTH_HEADER behind cfg(debug_assertions)
## Pipeline Run: 2026-05-07
## Branch: feat/jr-auth-header-cfg-test-gate

---

## Per-AC Demo Recordings

| AC | Test | Description | GIF | WebM | Tape | Size (webm) | Status |
|----|------|-------------|-----|------|------|-------------|--------|
| AC-001 | test_sd_002_new_for_test_honors_auth_header | cfg(debug) builds still honor JR_AUTH_HEADER via new_for_test | [gif](AC-001-debug-builds-honor-env-var.gif) | [webm](AC-001-debug-builds-honor-env-var.webm) | [tape](AC-001-debug-builds-honor-env-var.tape) | 304K | recorded |
| AC-002 (in-process) | test_sd_002_cfg_test_is_active_in_test_binary | cfg!(test) is true in test binary | [gif](AC-002-in-process-cfg-test-active.gif) | [webm](AC-002-in-process-cfg-test-active.webm) | [tape](AC-002-in-process-cfg-test-active.tape) | 346K | recorded |
| AC-002 (source) | test_sd_002_cfg_test_gate_present_in_source | #[cfg(debug_assertions)] annotation present in src/api/client.rs | [gif](AC-002-source-inspection-gate-present.gif) | [webm](AC-002-source-inspection-gate-present.webm) | [tape](AC-002-source-inspection-gate-present.tape) | 355K | recorded |
| AC-003 | test_sd_002_new_for_test_signature_unchanged | new_for_test(String, String) -> JiraClient signature unchanged | [gif](AC-003-new-for-test-signature-unchanged.gif) | [webm](AC-003-new-for-test-signature-unchanged.webm) | [tape](AC-003-new-for-test-signature-unchanged.tape) | 357K | recorded |
| AC-004 | test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers | Zero in-process env::var("JR_AUTH_HEADER") readers in tests/ | [gif](AC-004-audit-zero-in-process-readers.gif) | [webm](AC-004-audit-zero-in-process-readers.webm) | [tape](AC-004-audit-zero-in-process-readers.tape) | 370K | recorded |

---

## Combined Demo

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| 5/5 SD-002 green | All auth_header_release_gate tests pass together | [gif](AC-combined-all-sd-002-pass.gif) | [webm](AC-combined-all-sd-002-pass.webm) | [tape](AC-combined-all-sd-002-pass.tape) | 747K | recorded |

---

## Bonus Demo — Release Binary Security Verification

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| BONUS | strings(release jr) grep count = 0 for JR_AUTH_HEADER | [gif](BONUS-release-binary-grep-verification.gif) | [webm](BONUS-release-binary-grep-verification.webm) | [tape](BONUS-release-binary-grep-verification.tape) | 737K | recorded |

The bonus demo runs `cargo build --release --quiet && strings target/release/jr | grep -c 'JR_AUTH_HEADER'` and shows the result is `0`. This is direct evidence that the `#[cfg(debug_assertions)]` gate compiled the `JR_AUTH_HEADER` env-var read out of the release binary — the primary security goal of SD-002.

---

## AC Coverage Summary

| AC | Story | Requirement | Demos |
|----|-------|-------------|-------|
| AC-001 | S-0.05 | Debug builds still honor JR_AUTH_HEADER (new_for_test path) | AC-001 + combined |
| AC-002 | S-0.05 | cfg(debug_assertions) gate present in source AND active in test binary | AC-002-in-process + AC-002-source + combined |
| AC-003 | S-0.05 | new_for_test signature unchanged (String, String) -> JiraClient | AC-003 + combined |
| AC-004 | S-0.05 | Zero in-process JR_AUTH_HEADER readers in tests/ | AC-004 + combined |
| BONUS | S-0.05 | Release binary does not contain JR_AUTH_HEADER string | BONUS |

---

## Visual Review Summary

| Demo | AC | Outcome | Notes |
|------|----|---------|-------|
| AC-001-debug-builds-honor-env-var | AC-001 | 1 passed; 0 failed | new_for_test delivers auth header to mock server |
| AC-002-in-process-cfg-test-active | AC-002 | 1 passed; 0 failed | cfg!(test) == true confirmed in test binary |
| AC-002-source-inspection-gate-present | AC-002 | 1 passed; 0 failed | #[cfg(debug_assertions)] found within 5 lines of env-var read |
| AC-003-new-for-test-signature-unchanged | AC-003 | 1 passed; 0 failed | Compile-time signature check passes |
| AC-004-audit-zero-in-process-readers | AC-004 | 1 passed; 0 failed | grep finds no env::var("JR_AUTH_HEADER") in tests/ |
| AC-combined-all-sd-002-pass | ALL | 5 passed; 0 failed | Full suite green |
| BONUS-release-binary-grep-verification | BONUS | count = 0 | JR_AUTH_HEADER absent from release binary strings |

---

## Toolchain

| Tool | Version | Status |
|------|---------|--------|
| VHS | 0.11.0 | installed |
| cargo | 1.94.0 (85eff7c80 2026-01-15) | installed |
| Playwright | N/A | not applicable (CLI product) |
| Font | Menlo (system) | installed |

---

## PR Embedding Snippet

```markdown
## Demo Evidence — S-0.05: cfg(debug_assertions) gate for JR_AUTH_HEADER

### Combined: 5/5 SD-002 tests green
![AC-combined-all-sd-002-pass](docs/demo-evidence/S-0.05/AC-combined-all-sd-002-pass.gif)

### BONUS: JR_AUTH_HEADER absent from release binary
![BONUS-release-binary-grep-verification](docs/demo-evidence/S-0.05/BONUS-release-binary-grep-verification.gif)

Full per-AC recordings: [docs/demo-evidence/S-0.05/](docs/demo-evidence/S-0.05/)
```

---

## Implementation Notes

- **Gate chosen: Option B** — `#[cfg(debug_assertions)]` rather than `#[cfg(test)]`. The orchestrator approved this deviation because `#[cfg(test)]` would have broken ~150 existing subprocess integration tests (those use `.env("JR_AUTH_HEADER", ...)` on `Command::cargo_bin("jr")`, which spawns a debug binary compiled without `cfg(test)`). `#[cfg(debug_assertions)]` achieves the same release-binary security goal while preserving the subprocess test pattern.
- **Commits covered:** `3cc49d6` (Red Gate tests), `afd0950` (cfg(debug_assertions) implementation)
- **All 5 SD-002 tests pass; 600+ lib + ~151 subprocess tests preserved; clippy and fmt clean**
