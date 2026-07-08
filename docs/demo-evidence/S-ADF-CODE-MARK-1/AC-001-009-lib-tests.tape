Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-001-009-lib-tests.gif
Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-001-009-lib-tests.webm

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

# AC-001..AC-009: BC-7.2.015 unit anchors (GREEN post-fix)
# Covers: assert_marks_eq + assert_link_mark_with_href helpers,
#         test_markdown_inline_code_mark_and_composition rewrite,
#         test_bc_7_2_015_{plain_code_baseline,strong,em,strike,subsup,link,
#         mixed_range,multi_mark,alert_wrapper}, prop_bc_7_2_015_*

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-ADF-CODE-MARK-1"
Enter
Sleep 300ms
Show

Type "cargo test --lib -- test_bc_7_2_015_ test_markdown_inline_code_mark_and_composition prop_bc_7_2_015"
Enter
Sleep 12s
Sleep 2s
