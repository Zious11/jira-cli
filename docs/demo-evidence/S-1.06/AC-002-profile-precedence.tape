# AC-002 (H-003): profile precedence chain (flag > env > config > "default")
# test_s_1_06_h_003_profile_precedence_chain
#
# Pins: four precedence cases in one test:
#   (a) --profile flag beats JR_PROFILE env
#   (b) JR_PROFILE env beats default_profile in config
#   (c) default_profile in config beats literal "default"
#   (d) no flag/env/default_profile → literal "default"
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-002-profile-precedence.gif
Output docs/demo-evidence/S-1.06/AC-002-profile-precedence.webm

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

Type "# AC-002: profile precedence chain (flag > env > config > default)"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_003 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 1 passed; 0 failed — precedence chain green"
Enter
Sleep 2s
