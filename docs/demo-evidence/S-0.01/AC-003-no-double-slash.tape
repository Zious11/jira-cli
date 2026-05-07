# AC-003: No double-slash in composed browse URL (BC-3.4.001)
# Trailing slash on instance_url must not produce //browse/ in the output.
# Given JR_BASE_URL=https://mycompany.atlassian.net/ (trailing slash),
# jr issue open PROJ-123 --url-only must emit https://mycompany.atlassian.net/browse/PROJ-123
# (single slash before "browse"), NOT https://mycompany.atlassian.net//browse/PROJ-123.

Output docs/demo-evidence/S-0.01/AC-003-no-double-slash.gif
Output docs/demo-evidence/S-0.01/AC-003-no-double-slash.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.01-fix-handle-open-oauth-instance-url"
Enter
Sleep 500ms
Show

Type "# AC-003: No double-slash — trailing slash in URL is stripped cleanly"
Enter
Sleep 500ms

Type "cargo test --test issue_open test_bc_3_4_001_no_double_slash -- --nocapture --test-threads=1"
Enter
Sleep 30s
