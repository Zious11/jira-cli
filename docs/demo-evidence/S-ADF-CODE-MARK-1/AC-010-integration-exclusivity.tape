Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-010-integration-exclusivity.gif
Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-010-integration-exclusivity.webm

Require cargo
Set FontFamily "Menlo"
Set FontSize 15
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 24
Set TypingSpeed 40ms
Set Shell "bash"

# AC-010: H-NEW-ADF-010 Calls A-D wiremock integration tests
# tests/adf_code_mark_exclusivity.rs — platform path:
#   Call A (strong+code), Call B (subsup+code), Call C (link preserved), Call D (mixed-range)

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-ADF-CODE-MARK-1"
Enter
Sleep 300ms
Show

Type "cargo test --test adf_code_mark_exclusivity"
Enter
Sleep 12s
Sleep 2s
