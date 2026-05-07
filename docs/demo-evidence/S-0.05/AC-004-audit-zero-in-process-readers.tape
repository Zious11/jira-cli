# AC-004: audit finds zero in-process JR_AUTH_HEADER readers in tests/
# test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers
#
# Greps tests/ for env::var("JR_AUTH_HEADER") — any match is an in-process
# reader that must be migrated. All existing references are subprocess-only
# (.env("JR_AUTH_HEADER", ...) on cargo_bin builders) which are safe under
# the #[cfg(debug_assertions)] gate.
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-004-audit-zero-in-process-readers.gif
Output docs/demo-evidence/S-0.05/AC-004-audit-zero-in-process-readers.webm

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

Type "# AC-004: audit confirms zero in-process JR_AUTH_HEADER env::var() readers"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate test_sd_002_ac004_audit_no_in_process_jr_auth_header_readers -- --nocapture --test-threads=1"
Enter
Sleep 30s
