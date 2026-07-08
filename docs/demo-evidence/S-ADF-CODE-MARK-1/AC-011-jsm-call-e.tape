Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-011-jsm-call-e.gif
Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-011-jsm-call-e.webm

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

# AC-011: H-NEW-ADF-010 Call E — JSM path parity
# tests/issue_create_jsm.rs::test_bc_7_2_015_call_e_jsm_path_subsup_code_mark_stripped
# Verifies: ^`code`^ via JSM path → marks == [code]; platform POST not called (.expect(0))

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-ADF-CODE-MARK-1"
Enter
Sleep 300ms
Show

Type "cargo test --test issue_create_jsm -- test_bc_7_2_015_call_e"
Enter
Sleep 12s
Sleep 2s
