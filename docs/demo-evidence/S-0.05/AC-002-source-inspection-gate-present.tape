# AC-002 (source inspection): #[cfg(debug_assertions)] annotation present in source
# test_sd_002_cfg_test_gate_present_in_source
#
# RED GATE test: verifies #[cfg(debug_assertions)] appears within 5 lines of
# the JR_AUTH_HEADER env-var read in src/api/client.rs.
# Pre-fix: no cfg gate annotation is present (FAILS).
# Post-fix: the gate wraps the env-var block (PASSES).
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-002-source-inspection-gate-present.gif
Output docs/demo-evidence/S-0.05/AC-002-source-inspection-gate-present.webm

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

Type "# AC-002 source-inspection: cfg(debug_assertions) gate present in client.rs"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate test_sd_002_cfg_test_gate_present_in_source -- --nocapture --test-threads=1"
Enter
Sleep 30s
