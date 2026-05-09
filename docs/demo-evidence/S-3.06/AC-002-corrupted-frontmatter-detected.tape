Output AC002_corrupted_frontmatter_detected.gif
Output AC002_corrupted_frontmatter_detected.webm

Require bash

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 400
Set Theme "Dracula"
Set Padding 20
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli"
Enter
Sleep 500ms
Show

Type "sed -i.bak 's/definitional_count: 46/definitional_count: 9999/' .factory/specs/prd/bc-1-auth-identity.md"
Sleep 500ms
Enter
Sleep 2s

Type "bash .worktrees/S-3.06/scripts/check-spec-counts.sh"
Sleep 500ms
Enter
Sleep 5s

Type "mv .factory/specs/prd/bc-1-auth-identity.md.bak .factory/specs/prd/bc-1-auth-identity.md"
Sleep 500ms
Enter
Sleep 2s

Type "bash .worktrees/S-3.06/scripts/check-spec-counts.sh"
Sleep 500ms
Enter
Sleep 5s
