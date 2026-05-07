# AC-001: OAuth URL correctness (BC-3.4.001)
# handle_open MUST use instance_url() — not base_url() — when composing the browse URL.
# Given base_url = api.atlassian.com/ex/jira/<cloudId> and instance_url = mycompany.atlassian.net,
# the resulting URL must contain mycompany.atlassian.net and NOT api.atlassian.com.

Output docs/demo-evidence/S-0.01/AC-001-oauth-url-correctness.gif
Output docs/demo-evidence/S-0.01/AC-001-oauth-url-correctness.webm

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

Type "# AC-001: OAuth URL correctness — instance_url() used, not base_url()"
Enter
Sleep 500ms

Type "cargo test --test issue_open test_bc_3_4_001_oauth_uses_instance_url -- --nocapture --test-threads=1"
Enter
Sleep 30s
