# Review Findings — S-3.05

**PR:** #316
**Story:** S-3.05 — Cap asset enrichment join_all concurrency with buffer_unordered(8)
**Reviewer:** pr-review-triage
**Template:** review-findings-template.md

---

## Convergence Table

| Cycle | Findings | Blocking | Fixed | Remaining | Verdict |
|-------|----------|----------|-------|-----------|---------|
| 1     | 0        | 0        | 0     | 0         | APPROVE |

---

## Cycle 1 Findings

**Verdict: APPROVE — zero blocking findings.**

### Review checklist executed

| Area | Check | Result |
|------|-------|--------|
| buffer_unordered semantics | Order-independent consumers confirmed (HashMap inserts at both call sites) | PASS |
| buffer_unordered semantics | `buffer_unordered` vs `buffered` — correct choice per verification | PASS |
| buffer_unordered semantics | Error propagation — collect-all matches join_all (no short-circuit) | PASS |
| buffer_unordered semantics | Cancellation safety — identical drop semantics to join_all | PASS |
| Const placement | `pub const MAX_CONCURRENT_ASSET_FETCHES: usize = 8` at top of linked.rs | PASS |
| Const placement | Imported into list.rs from linked.rs (single source of truth) | PASS |
| Import chain | `use futures::stream::{self, StreamExt}` in both files | PASS |
| Import ordering | rustfmt --check clean (no output = 0 violations) | PASS |
| Clippy | `cargo clippy -- -D warnings` clean (0 warnings, 0 errors) | PASS |
| Test naming | All 4 tests follow `test_<verb>_<subject>_<outcome>` per CLAUDE.md | PASS |
| No lint suppression | No `#[allow]` in production or test code (b319557 dropped it) | PASS |
| AC-001 test | 10 distinct mock endpoints, `expect(1)` per endpoint, `server.verify()` | PASS |
| AC-002 test | Timing-based: 50ms delay, 90ms threshold, 40ms margin both sides | PASS |
| AC-002 design note | Atomic-counter approach documented as non-viable (wiremock RwLock) | PASS |
| AC-003 test | 429 + Retry-After: 1 flap pattern, wiremock mount priority correct | PASS |
| AC-004 test | Isolated file (compile-error Red Gate isolation), value assertion | PASS |
| AC coverage | 4/4 ACs covered | PASS |
| H-038 | Regression pin not disturbed — dedup HashMap logic untouched | PASS |
| No new deps | `futures` already in [dependencies]; `Cargo.toml` not changed | PASS |
| Diff scope | Only linked.rs, list.rs, two test files, demo evidence | PASS |
| Unit tests | 612 unit tests pass (no regressions) | PASS |
| Integration tests | 4/4 S-3.05 integration tests pass | PASS |
| Security surface | No new IO surface, no user-controlled inputs, CWE-400 mitigated | PASS |

### Suggestions (non-blocking)

None. The implementation is idiomatic, well-documented, and the test design rationale
(wiremock RwLock finding documented inline) is exemplary test engineering.

---

## Notes

**AC-002 test design:** The story spec recommended `Arc<AtomicUsize>+fetch_max` but the
test-writer correctly discovered that wiremock 0.6.5 runs `Respond::respond` under a write
lock, serializing all respond() calls and making the peak counter always peak at 1. The
pivot to timing-based assertion using `ResponseTemplate::set_delay(50ms)` (which runs
outside the lock) is the correct solution. The 90ms threshold (40ms margin) is robust for
CI. This deviation from the spec is documented in the test file's module docstring and
is the correct engineering decision.

**Import ordering in list.rs:** `MAX_CONCURRENT_ASSET_FETCHES` appears before `cmdb_field_ids`
in the use-group. This passes `cargo fmt --check` because Rust/rustfmt sorts uppercase
before lowercase (ASCII order: `M` = 77 < `c` = 99). No issue.
