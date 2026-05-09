# AC-003 / BC-7.3.005: `jr auth switch ghost --output json` → exit 64 + JSON error on stderr
# Covers: test_auth_switch_unknown_profile_returns_json_error
# Pins: handle_switch propagates JrError::UserError; main.rs error handler wraps it as
#       {"error": "<msg>", "code": 64} to stderr when --output json is set.
#       Exit code 64. AC-003 extends H-020 coverage to auth subcommands.
#
# Demo approach: run the process-spawn test via cargo test --nocapture to show
# the full error-path behavior. The test uses an empty XDG-isolated config with
# no profiles defined, so "ghost" cannot exist.
#
# Output: AC-003-auth-switch-error-json.gif + AC-003-auth-switch-error-json.webm

Output docs/demo-evidence/S-2.07/AC-003-auth-switch-error-json.gif
Output docs/demo-evidence/S-2.07/AC-003-auth-switch-error-json.webm

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
Set WaitTimeout 120s

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.07-auth-output-json-and-policy"
Enter
Sleep 500ms
Show

Type "# AC-003: jr auth switch ghost --output json -> exit 64 + JSON error on stderr"
Enter
Sleep 300ms

Type "cargo test --test auth_output_json test_auth_switch_unknown_profile_returns_json_error -- --nocapture --test-threads=1 2>&1"
Enter
Wait+Screen /test result: ok/
Sleep 2s
