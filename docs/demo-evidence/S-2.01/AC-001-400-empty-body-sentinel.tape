# AC-001 / H-030 / BC-7.3.001: 400 + empty body → "<empty response body>" in stderr
# Covers: test_s_2_01_h_030_bc_7_3_001_400_empty_body_shows_sentinel_in_stderr
# Pins: extract_error_message branch 5 (empty body sentinel) in src/api/client.rs
#
# Output: AC-001-400-empty-body-sentinel.gif + .webm

Output docs/demo-evidence/S-2.01/AC-001-400-empty-body-sentinel.gif
Output docs/demo-evidence/S-2.01/AC-001-400-empty-body-sentinel.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.01-bc-2-issue-read-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-001 / H-030: 400 + empty body → '<empty response body>' sentinel in stderr"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_030 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — empty-body sentinel verified"
Enter
Sleep 2s
