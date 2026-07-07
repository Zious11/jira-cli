Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-001-canonical-green.gif
Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-001-canonical-green.webm

Require bash
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

# AC-001: Canonical GREEN — replicate CI topology
# Mount factory-artifacts worktree, run guard (exit 0), remove mount, verify clean

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-BC-CITATION-GUARD-1"
Enter
Sleep 300ms
Show

Type "git worktree add --detach .factory origin/factory-artifacts 2>&1 | tail -1"
Enter
Sleep 4s

Type "bash scripts/check-bc-citation-symbols.sh"
Enter
Sleep 3s

Type "git worktree remove --force .factory && git status --short && echo 'mount-removed-clean'"
Enter
Sleep 3s
