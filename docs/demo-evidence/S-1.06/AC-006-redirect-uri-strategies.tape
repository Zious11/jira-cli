# AC-006 (H-029): redirect URI strategies (3 test functions)
# test_s_1_06_h_029_embedded_redirect_uri
# test_s_1_06_h_029_byo_redirect_uri_dynamic_port
# test_s_1_06_h_029_embedded_callback_port_const_is_53682
#
# Pins:
#   - Embedded app: redirect_uri = http://127.0.0.1:53682/callback (IPv4, fixed port)
#   - BYO DynamicPort: redirect_uri = http://localhost:<port>/callback (not 53682)
#   - EMBEDDED_CALLBACK_PORT const = 53682 (breaking-release guard)
#
# Expected: 3 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-006-redirect-uri-strategies.gif
Output docs/demo-evidence/S-1.06/AC-006-redirect-uri-strategies.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.06-oauth-flow-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-006: redirect URI strategies — embedded 127.0.0.1:53682 vs BYO DynamicPort"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_029 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 3 passed; 0 failed — redirect URI strategies green"
Enter
Sleep 2s
