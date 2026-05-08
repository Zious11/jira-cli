# AC-004 (H-005): malformed config TOML → exit 78 + file bytes unchanged
# test_s_1_06_h_005_malformed_config_exits_78_file_unchanged
#
# Pins: auth login --oauth with a malformed config.toml must:
#   - exit 78 (ConfigError)
#   - surface "toml" or "parse" in stderr
#   - leave config file bytes IDENTICAL (no clobber)
#   - not panic
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-004-malformed-config.gif
Output docs/demo-evidence/S-1.06/AC-004-malformed-config.webm

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

Type "# AC-004: malformed config exits 78, file bytes unchanged"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_005 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — exit 78 + no-clobber green"
Enter
Sleep 2s
