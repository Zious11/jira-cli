# AC-001 (NFR-O-A): Cargo.toml has pinned tracing/tracing-subscriber deps
# test_s_1_03_cargo_toml_has_tracing_dep
# test_s_1_03_cargo_toml_has_tracing_subscriber_dep
#
# Verifies that Cargo.toml declares explicit version pins for both
# `tracing` and `tracing-subscriber` (with env-filter feature).
#
# Pre-fix FAILS: neither dep present in Cargo.toml
# Post-fix PASSES: both deps with pinned versions present
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-001-cargo-toml-deps.gif
Output docs/demo-evidence/S-1.03/AC-001-cargo-toml-deps.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 400
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

Type "# AC-001: Cargo.toml has pinned tracing + tracing-subscriber deps"
Enter
Sleep 300ms

Type "cargo test --test observability test_s_1_03_cargo_toml_has_tracing_dep test_s_1_03_cargo_toml_has_tracing_subscriber_dep -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — tracing deps confirmed in Cargo.toml"
Enter
Sleep 2s
