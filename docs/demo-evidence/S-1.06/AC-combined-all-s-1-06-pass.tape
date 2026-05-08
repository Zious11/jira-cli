# Combined demo: all 11 S-1.06 OAuth flow holdout tests green
#
# Runs the full oauth_flow_holdouts integration test suite showing 11/11 pass.
# Coverage:
#   AC-001 H-001: auth status no profiles → exit 0 + "No profiles configured"
#   AC-001 H-002: auth list --output json no profiles → exit 0 + []
#   AC-002 H-003: profile precedence chain (flag > env > config > default)
#   AC-003 H-004: auth refresh --no-input no URL → exit 64 + actionable hints
#   AC-004 H-005: malformed config TOML → exit 78 + file bytes unchanged
#   AC-005 H-022: scope-mismatch dispatch (3 sub-tests)
#   AC-006 H-029: redirect URI strategies (3 sub-tests)
#
# Expected: 11 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-combined-all-s-1-06-pass.gif
Output docs/demo-evidence/S-1.06/AC-combined-all-s-1-06-pass.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.06-oauth-flow-holdout-suite"
Enter
Sleep 500ms
Show

Type "# S-1.06: OAuth flow regression holdout suite — 11/11 tests"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts -- --nocapture --test-threads=1"
Enter
Sleep 90s

Type "# Result: 11 passed; 0 failed — full S-1.06 holdout suite green"
Enter
Sleep 2s
