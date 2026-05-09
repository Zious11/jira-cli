# AC-005 (NFR-O-A): auth.rs has tracing entries; no secrets in tracing fields
# test_s_1_03_auth_has_tracing_entry_points
# test_s_1_03_auth_no_client_secret_in_tracing_fields
#
# Verifies that src/api/auth.rs emits tracing events at OAuth flow entry
# points (oauth_login, exchange_code_for_token, refresh_oauth_token) AND
# that client_secret / access_token / refresh_token values are never
# passed as tracing field arguments.
#
# Pre-fix FAILS: no tracing:: calls in auth.rs
# Post-fix PASSES: tracing events present; secret fields absent
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-005-auth-tracing-no-secrets.gif
Output docs/demo-evidence/S-1.03/AC-005-auth-tracing-no-secrets.webm

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

Type "# AC-005: auth.rs has tracing entries; no secrets logged"
Enter
Sleep 300ms

Type "cargo test --test observability test_s_1_03_auth_has_tracing_entry_points test_s_1_03_auth_no_client_secret_in_tracing_fields -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — auth tracing entries present; secrets safe"
Enter
Sleep 2s
