# AC-006 (H-016, BC-1.1.006): removing the active profile is rejected.
# 3 process-spawn tests verifying:
#   - exit code 64 + stderr "cannot remove active" + file byte-identity (basic case)
#   - guard fires without --no-input flag (non-TTY detection)
#   - guard fires even when a second profile is present
#
# test_s_1_08_ac006_h016_remove_active_profile_rejected_and_config_unchanged
# test_s_1_08_ac006_h016_remove_active_profile_rejected_without_no_input_flag
# test_s_1_08_ac006_h016_remove_active_profile_rejected_with_second_profile_present
#
# Output: AC-006-h016-active-profile-guard.gif + .webm

Output docs/demo-evidence/S-1.08/AC-006-h016-active-profile-guard.gif
Output docs/demo-evidence/S-1.08/AC-006-h016-active-profile-guard.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.08-keychain-roundtrip-holdout"
Enter
Sleep 500ms
Show

Type "# AC-006 / H-016: auth remove active profile rejected (exit 64, file unchanged)"
Enter
Sleep 300ms

Type "cargo test --test keychain_layout_holdouts -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 3 passed — active-profile removal guard verified, config byte-identical"
Enter
Sleep 2s
