# AC-002 (in-process): cfg!(test) is true in test binary
# test_sd_002_cfg_test_is_active_in_test_binary
#
# Confirms cfg!(test) evaluates to true in any cargo test binary.
# Combined with the source-inspection test, this confirms the
# #[cfg(debug_assertions)] gate compiles correctly for test builds.
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-002-in-process-cfg-test-active.gif
Output docs/demo-evidence/S-0.05/AC-002-in-process-cfg-test-active.webm

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

Type "# AC-002 in-process: cfg!(test) is active in test binary"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate test_sd_002_cfg_test_is_active_in_test_binary -- --nocapture --test-threads=1"
Enter
Sleep 30s
