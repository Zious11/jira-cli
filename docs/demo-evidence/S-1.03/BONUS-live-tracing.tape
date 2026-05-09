# BONUS: Live tracing in action — default WARN level vs RUST_LOG=debug
#
# Shows the runtime behavior of the tracing wire-up:
#   1. Default: WARN level — no tracing noise during normal operation
#   2. RUST_LOG=debug — tracing emits debug events (subscriber test confirms)
#
# Demonstrates that the subscriber init with EnvFilter actually works at runtime:
# RUST_LOG controls what gets logged, and running the observability test with
# RUST_LOG=debug proves the debug-level tracing infrastructure is live.
#
# Expected: 1 passed; 0 failed (subscriber init test with debug env)

Output docs/demo-evidence/S-1.03/BONUS-live-tracing.gif
Output docs/demo-evidence/S-1.03/BONUS-live-tracing.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 500
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

Type "# Default WARN level — tracing silent during normal CLI use"
Enter
Sleep 300ms

Type "cargo run --quiet -- --help 2>&1 | head -5"
Enter
Sleep 30s

Type ""
Enter
Sleep 500ms

Type "# RUST_LOG=debug — tracing-subscriber test with debug level enabled"
Enter
Sleep 300ms

Type "RUST_LOG=debug cargo test --test observability test_s_1_03_main_initializes_tracing_subscriber -- --nocapture --test-threads=1 2>&1 | head -15"
Enter
Sleep 60s

Type "# Tracing infrastructure live: RUST_LOG gates debug events at runtime"
Enter
Sleep 2s
