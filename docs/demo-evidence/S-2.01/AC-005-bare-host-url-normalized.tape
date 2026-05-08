# AC-005 / H-034 / BC-3.7.001: bare host URL normalized with trailing slash in POST body
# Covers: test_s_2_01_h_034_bc_3_7_001_bare_host_url_normalized_with_trailing_slash
# Pins: url normalization via url::Url::parse in src/cli/issue/links.rs::handle_remote_link
#
# Output: AC-005-bare-host-url-normalized.gif + .webm

Output docs/demo-evidence/S-2.01/AC-005-bare-host-url-normalized.gif
Output docs/demo-evidence/S-2.01/AC-005-bare-host-url-normalized.webm

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

Type "# AC-005 / H-034: https://example.com normalized to https://example.com/ (trailing slash)"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_034 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — POST body contains normalized URL with trailing slash"
Enter
Sleep 2s
