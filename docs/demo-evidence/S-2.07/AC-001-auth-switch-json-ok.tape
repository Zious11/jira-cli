# AC-001 / BC-7.3.004: `jr auth switch default --output json` → exit 0 + JSON success
# Covers: test_auth_switch_returns_json_ok
# Pins: handle_switch emits {"profile":"default","action":"switch","ok":true} when --output json
#       Exit code 0, JSON on stdout.
#
# Demo approach: run the process-spawn test via cargo test --nocapture so the
# actual `jr auth switch default --output json` invocation output is visible.
# The test uses XDG-isolated tempdir and test-scoped keychain — safe to run on any machine.
#
# Output: AC-001-auth-switch-json-ok.gif + AC-001-auth-switch-json-ok.webm

Output docs/demo-evidence/S-2.07/AC-001-auth-switch-json-ok.gif
Output docs/demo-evidence/S-2.07/AC-001-auth-switch-json-ok.webm

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

Type "# AC-001: jr auth switch default --output json -> JSON success on stdout"
Enter
Sleep 300ms

Type "cargo test --test auth_output_json test_auth_switch_returns_json_ok -- --nocapture --test-threads=1 2>&1"
Enter
Wait+Screen /test result: ok/
Sleep 2s
