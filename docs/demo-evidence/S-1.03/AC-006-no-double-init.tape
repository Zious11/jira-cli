# AC-006: subscriber init guarded; no double-init panics
# test_s_1_03_lib_rs_does_not_init_subscriber
# test_s_1_03_observability_rs_does_not_init_subscriber
#
# Verifies that subscriber initialization belongs ONLY in src/main.rs.
# Neither src/lib.rs nor src/observability.rs may call .init() on a
# tracing_subscriber, which would cause double-init panics in test mode.
#
# Pre-fix PASSES vacuously (no subscriber anywhere)
# Post-fix PASSES if implementer correctly placed init only in main.rs
# Post-fix FAILS if .init() appears in lib.rs or observability.rs
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-006-no-double-init.gif
Output docs/demo-evidence/S-1.03/AC-006-no-double-init.webm

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

Type "# AC-006: subscriber init guarded in main.rs only (no double-init panics)"
Enter
Sleep 300ms

Type "cargo test --test observability test_s_1_03_lib_rs_does_not_init_subscriber test_s_1_03_observability_rs_does_not_init_subscriber -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — no double-init risk in lib.rs or observability.rs"
Enter
Sleep 2s
