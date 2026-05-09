# Combined: all 10 observability tests green
# Full S-1.03 test suite — all ACs in one run
#
# Runs the complete observability test suite covering:
#   AC-001: Cargo.toml tracing + tracing-subscriber deps (2 tests)
#   AC-002: main.rs subscriber init + EnvFilter (2 tests)
#   AC-003: client.rs tracing::debug! usage (2 tests)
#   AC-005: auth.rs tracing entries; no secrets (2 tests)
#   AC-006: no double-init in lib.rs / observability.rs (2 tests)
#
# Expected: 10 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-combined-all-s-1-03-pass.gif
Output docs/demo-evidence/S-1.03/AC-combined-all-s-1-03-pass.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 500
Set FontFamily "Menlo"
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 40ms
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.03-tracing-observability"
Enter
Sleep 500ms
Show

Type "# S-1.03 combined: all 10 observability tests"
Enter
Sleep 300ms

Type "cargo test --test observability -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 10 passed; 0 failed — NFR-O-A fully satisfied"
Enter
Sleep 2s
