Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-012-citations-and-diff.gif
Output docs/demo-evidence/S-ADF-CODE-MARK-1/AC-012-citations-and-diff.webm

Require cargo
Require git
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 24
Set TypingSpeed 40ms
Set Shell "bash"

# AC-012: CLAUDE.md clause-(b) splice + claude_md_citations guard pass
# Part 1: claude_md_citations test passes (BC-X.13.001)
# Part 2: git diff shows BC-7.2.015 enforced-at-emission splice in CLAUDE.md

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-ADF-CODE-MARK-1"
Enter
Sleep 300ms
Show

Type "cargo test --test claude_md_citations -- test_claude_md_citations_resolve_to_real_files"
Enter
Sleep 10s

Type "git diff develop -- CLAUDE.md | grep -A1 'enforced at emission'"
Enter
Sleep 3s
Sleep 2s
