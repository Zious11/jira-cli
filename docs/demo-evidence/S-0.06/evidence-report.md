---
document_type: demo-evidence-report
product: "jr (Jira CLI)"
story_id: S-0.06
pipeline_run: "2026-05-07"
demo_type: cli
recording_tool: vhs
status: complete
---

# Demo Evidence Report — S-0.06

## Product: jr (Jira CLI)
## Story: S-0.06 — --verbose-bodies flag + PII warning (SD-003)
## Pipeline Run: 2026-05-07
## Branch: feat/verbose-bodies-flag-and-pii-warning

---

## Per-AC Demo Recordings

| AC | Test | Description | GIF | WebM | Tape | Size (webm) | Status |
|----|------|-------------|-----|------|------|-------------|--------|
| AC-001 / H-NEW-VERBOSE-001 | test_sd_003_verbose_bodies_emits_pii_warning | --verbose-bodies emits 3-line PII warning to stderr | [gif](AC-001-pii-warning-emitted.gif) | [webm](AC-001-pii-warning-emitted.webm) | [tape](AC-001-pii-warning-emitted.tape) | 641K | recorded |
| AC-002 / H-NEW-VERBOSE-002 | test_sd_003_verbose_alone_suppresses_body_bytes | --verbose alone suppresses body bytes; suppression hint present | [gif](AC-002-verbose-suppresses-body.gif) | [webm](AC-002-verbose-suppresses-body.webm) | [tape](AC-002-verbose-suppresses-body.tape) | 651K | recorded |
| AC-003 | test_sd_003_verbose_plus_verbose_bodies_prints_body | --verbose + --verbose-bodies stacks: method+URL, PII warning, body bytes | [gif](AC-003-verbose-stacks-with-bodies.gif) | [webm](AC-003-verbose-stacks-with-bodies.webm) | [tape](AC-003-verbose-stacks-with-bodies.tape) | 661K | recorded |
| AC-004 | test_sd_003_verbose_bodies_alone_prints_body_without_url_line | --verbose-bodies alone: body + warning, no method+URL line | [gif](AC-004-verbose-bodies-alone.gif) | [webm](AC-004-verbose-bodies-alone.webm) | [tape](AC-004-verbose-bodies-alone.tape) | 667K | recorded |
| AC-005 | test_sd_003_help_mentions_verbose_bodies_flag | jr --help lists --verbose and --verbose-bodies with migration hint | [gif](AC-005-help-shows-both-flags.gif) | [webm](AC-005-help-shows-both-flags.webm) | [tape](AC-005-help-shows-both-flags.tape) | 629K | recorded |
| AC-006 | test_sd_003_changelog_has_breaking_change_entry | CHANGELOG.md has BREAKING CHANGE entry for --verbose body suppression | [gif](AC-006-changelog-breaking-change.gif) | [webm](AC-006-changelog-breaking-change.webm) | [tape](AC-006-changelog-breaking-change.tape) | 663K | recorded |

---

## Combined Demo

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| 6/6 SD-003 green | All verbose_bodies tests pass together | [gif](AC-combined-all-sd-003-pass.gif) | [webm](AC-combined-all-sd-003-pass.webm) | [tape](AC-combined-all-sd-003-pass.tape) | 1.4M | recorded |

---

## Bonus Demo — Live UX Showcase

| Demo | Description | GIF | WebM | Tape | Size (webm) | Status |
|------|-------------|-----|------|------|-------------|--------|
| BONUS | jr --help grep for verbose flags + PII warning text via test --nocapture | [gif](BONUS-live-ux.gif) | [webm](BONUS-live-ux.webm) | [tape](BONUS-live-ux.tape) | 1.1M | recorded |

The bonus demo shows two things in sequence:
1. `cargo run --quiet -- --help | grep -E 'verbose'` — both `--verbose` and `--verbose-bodies` appear in the help output
2. The PII warning test filtered with `grep -E 'WARNING|PII|jr|passed|failed'` — shows the actual 3-line warning text and the `1 passed` result

---

## AC Coverage Summary

| AC | Story | Requirement | Holdout | Demos |
|----|-------|-------------|---------|-------|
| AC-001 | S-0.06 | --verbose-bodies emits PII warning to stderr (3 lines) | H-NEW-VERBOSE-001 | AC-001 + combined + bonus |
| AC-002 | S-0.06 | --verbose alone suppresses body bytes; suppression hint present | H-NEW-VERBOSE-002 | AC-002 + combined |
| AC-003 | S-0.06 | --verbose + --verbose-bodies stacks: method+URL AND body AND warning | — | AC-003 + combined |
| AC-004 | S-0.06 | --verbose-bodies alone: body + warning, NOT method+URL line | — | AC-004 + combined |
| AC-005 | S-0.06 | jr --help lists both flags with migration hint in --verbose description | — | AC-005 + combined + bonus |
| AC-006 | S-0.06 | CHANGELOG.md contains BREAKING CHANGE entry mentioning --verbose and --verbose-bodies | — | AC-006 + combined |

---

## Visual Review Summary

| Demo | AC | Outcome | Notes |
|------|----|---------|-------|
| AC-001-pii-warning-emitted | AC-001 | 1 passed; 0 failed | All 3 PII warning lines confirmed in stderr |
| AC-002-verbose-suppresses-body | AC-002 | 1 passed; 0 failed | Sentinel strings absent; suppression hint present |
| AC-003-verbose-stacks-with-bodies | AC-003 | 1 passed; 0 failed | [verbose] line + warning + body bytes all confirmed |
| AC-004-verbose-bodies-alone | AC-004 | 1 passed; 0 failed | Body + warning present; no method+URL line |
| AC-005-help-shows-both-flags | AC-005 | 1 passed; 0 failed | Both --verbose and --verbose-bodies in --help |
| AC-006-changelog-breaking-change | AC-006 | 1 passed; 0 failed | BREAKING CHANGE entry confirmed in CHANGELOG.md |
| AC-combined-all-sd-003-pass | ALL | 6 passed; 0 failed | Full suite green |
| BONUS-live-ux | AC-001 / AC-005 | live grep confirms | --help grep + PII warning text both visible |

---

## Toolchain

| Tool | Version | Status |
|------|---------|--------|
| VHS | 0.11.0 | installed |
| cargo | 1.94.0 (85eff7c80 2026-01-15) | installed |
| Playwright | N/A | not applicable (CLI product) |
| Font | Menlo (system) | installed |

---

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo build` | clean |
| `cargo build --release` | clean |
| `cargo test --test verbose_bodies` | 6/6 passed |
| `cargo clippy --all --all-features --tests -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

---

## PR Embedding Snippet

```markdown
## Demo Evidence — S-0.06: --verbose-bodies flag + PII warning

### Combined: 6/6 SD-003 tests green
![AC-combined-all-sd-003-pass](docs/demo-evidence/S-0.06/AC-combined-all-sd-003-pass.gif)

### BONUS: Live UX — --help grep + PII warning text
![BONUS-live-ux](docs/demo-evidence/S-0.06/BONUS-live-ux.gif)

Full per-AC recordings: [docs/demo-evidence/S-0.06/](docs/demo-evidence/S-0.06/)
```

---

## Implementation Notes

- **SD-003 resolution:** `--verbose` is now header-only (method + URL + status). Body bytes are suppressed unless `--verbose-bodies` is also passed.
- **PII warning:** 3-line warning emitted to stderr on client construction when `--verbose-bodies` is set — warns users not to pipe to AI-agent contexts or shared logs.
- **Flag orthogonality:** `--verbose` and `--verbose-bodies` are independent. `--verbose-bodies` alone prints body + warning without the method+URL line.
- **BREAKING CHANGE:** Documented in CHANGELOG.md. Users who relied on `--verbose` for body inspection must migrate to `--verbose --verbose-bodies`.
- **Holdouts satisfied:** H-NEW-VERBOSE-001 (AC-001) and H-NEW-VERBOSE-002 (AC-002) both verified by tests.
- **Commits covered:** `4d858b0` (Red Gate), `8641349` (implementation)
- **All 6 SD-003 tests pass; 600+ lib + integration tests preserved; clippy and fmt clean**
