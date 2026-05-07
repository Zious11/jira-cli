# AC-003 (NFR-O-A): client.rs uses tracing::debug! for verbose events
# test_s_1_03_client_uses_tracing_debug
# test_s_1_03_client_no_verbose_request_eprintln
#
# Verifies that src/api/client.rs uses tracing::debug! (not raw eprintln!)
# for method+URL and rate-limit verbose output. The [verbose] eprintln!
# request logging sites must be replaced by structured tracing events.
#
# Pre-fix FAILS: tracing::debug! absent; eprintln!("[verbose] ... method()") present
# Post-fix PASSES: tracing::debug! used; request-level verbose eprintln! removed
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-003-client-tracing-debug.gif
Output docs/demo-evidence/S-1.03/AC-003-client-tracing-debug.webm

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

Type "# AC-003: client.rs uses tracing::debug! (not eprintln!) for verbose events"
Enter
Sleep 300ms

Type "cargo test --test observability test_s_1_03_client_uses_tracing_debug test_s_1_03_client_no_verbose_request_eprintln -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — tracing::debug! replaces [verbose] eprintln!"
Enter
Sleep 2s
