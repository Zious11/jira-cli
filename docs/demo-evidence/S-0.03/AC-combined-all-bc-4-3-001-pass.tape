# Combined: All 5 BC-4.3.001 tests pass (3 unit + 2 integration)
# Runs all tests matching "bc_4_3_001" across both lib and integration harnesses.
# Demonstrates complete coverage of the multi-workspace composite key fix.

Output docs/demo-evidence/S-0.03/AC-combined-all-bc-4-3-001-pass.gif
Output docs/demo-evidence/S-0.03/AC-combined-all-bc-4-3-001-pass.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 500
Set FontFamily "Menlo"
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 40ms
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.03-multi-workspace-asset-hashmap-key"
Enter
Sleep 500ms
Show

Type "# Combined: all 5 BC-4.3.001 tests (3 unit + 2 integration) — expect 5/5 PASS"
Enter
Sleep 500ms

Type "cargo test bc_4_3_001 -- --nocapture --test-threads=1 2>&1 | grep -E 'running|test .* \.\.\.|test result'"
Enter
Sleep 60s
