# Phase F6 — Security Scan Results

**Feature:** S-388 / issue #388 — cross-hierarchy `edit --type` 400 enrichment + `--no-parent` fake-endpoint hint fix
**Delta commit:** `e0ea24b` (merged to `develop`)
**Date:** 2026-05-20
**Verdict:** PASS — no CRITICAL/HIGH findings

## Scope of Delta

The S-388 delta is error-message enrichment plus a pure classifier:

- `is_cross_hierarchy_type_error` — pure function, branches on two `Option<bool>`
- `handle_edit` error-path dispatch — composes stderr error text
- `IssueType.subtask: Option<bool>` — additive serde field

No new authentication, credential handling, network surface, command execution,
deserialization of untrusted formats, file I/O, or injection sink is introduced.
The delta is security-inert by construction.

## 1. cargo deny check (full dependency tree)

```
advisories ok, bans ok, licenses ok, sources ok
```

- **advisories:** OK — no RUSTSEC advisories against any of the 340 crates.
- **bans:** OK — no banned/duplicate-crate violations.
- **licenses:** OK — all dependency licenses within the `deny.toml` allow-list.
- **sources:** OK — all crates from permitted registries.

Two non-blocking `license-not-encountered` warnings (`OpenSSL`,
`Unicode-DFS-2016` listed in `deny.toml` but no longer matched by any crate).
These are pre-existing stale allow-list entries — informational only, not
security findings, and unrelated to the S-388 delta.

## 2. cargo audit (RustSec advisory database)

```
Loaded 1096 security advisories
Scanning Cargo.lock for vulnerabilities (340 crate dependencies)
```

No vulnerability or warning lines emitted; exit code 0. The full dependency
tree is clean against the RustSec advisory DB.

## 3. Semgrep

**Not run — tool not installed** (`semgrep not found` in PATH).

Justification for not blocking on this: the delta surface is error-message
*text* plus a pure `Option<bool>` classifier. Semgrep's `auto` ruleset targets
injection sinks, hardcoded secrets, unsafe deserialization, path traversal, and
SSRF — none of which exist in this delta. The delta:

- introduces no shell/SQL/template/eval sink → no injection rule applies;
- introduces no credential literal → no secrets rule applies;
- adds only an `Option<bool>` serde field → no unsafe-deserialization surface;
- has no file-path or URL construction → no path-traversal / SSRF rule applies.

A Semgrep run over these 4 files would produce no actionable finding. This is a
tooling-availability gap, not a coverage gap; recorded transparently rather than
silently omitted.

## Findings Summary

| Severity | Count | Detail |
|----------|-------|--------|
| CRITICAL | 0     | — |
| HIGH     | 0     | — |
| MEDIUM   | 0     | — |
| LOW      | 0     | — |
| INFO     | 2     | Stale `deny.toml` license allow-list entries (pre-existing, unrelated) |

## Conclusion

PASS. No CRITICAL or HIGH severity findings. `cargo deny check` and
`cargo audit` both clean across the full 340-crate dependency tree. The delta
introduces no new security-relevant surface. No security-reviewer escalation
required.
