# AC-001: cfg(debug) builds still honor JR_AUTH_HEADER
# test_sd_002_new_for_test_honors_auth_header
#
# Verifies JiraClient::new_for_test passes the auth header to outgoing HTTP
# requests. The #[cfg(debug_assertions)] gate on the env-var read must NOT
# break this constructor path — new_for_test takes auth_header as an
# argument, not from env.
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-001-debug-builds-honor-env-var.gif
Output docs/demo-evidence/S-0.05/AC-001-debug-builds-honor-env-var.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.05-jr-auth-header-cfg-test-gate"
Enter
Sleep 500ms
Show

Type "# AC-001: debug builds still honor JR_AUTH_HEADER via new_for_test"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate test_sd_002_new_for_test_honors_auth_header -- --nocapture --test-threads=1"
Enter
Sleep 30s
