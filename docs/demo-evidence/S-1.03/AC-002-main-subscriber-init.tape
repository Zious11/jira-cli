# AC-002 (NFR-O-A): main.rs initializes tracing-subscriber with EnvFilter
# test_s_1_03_main_initializes_tracing_subscriber
# test_s_1_03_main_uses_env_filter
#
# Verifies that src/main.rs calls tracing_subscriber:: init and uses
# EnvFilter / with_max_level / with_env_filter to gate log level.
#
# Pre-fix FAILS: tracing_subscriber:: and EnvFilter absent from main.rs
# Post-fix PASSES: both present in main.rs
#
# Expected: 2 passed; 0 failed

Output docs/demo-evidence/S-1.03/AC-002-main-subscriber-init.gif
Output docs/demo-evidence/S-1.03/AC-002-main-subscriber-init.webm

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

Type "# AC-002: main.rs initializes tracing-subscriber with EnvFilter"
Enter
Sleep 300ms

Type "cargo test --test observability test_s_1_03_main_initializes_tracing_subscriber test_s_1_03_main_uses_env_filter -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 2 passed; 0 failed — tracing_subscriber init + EnvFilter in main.rs"
Enter
Sleep 2s
