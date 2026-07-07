Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-003-dead-symbol-failure.gif
Output docs/demo-evidence/S-BC-CITATION-GUARD-1/AC-003-dead-symbol-failure.webm

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

# AC-003: Error path — dead symbol citation produces DEAD output + exit 1
# Corrupt bc file created by helper script (avoids backtick escaping in VHS)

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-BC-CITATION-GUARD-1"
Enter
Sleep 300ms
Type "bash docs/demo-evidence/S-BC-CITATION-GUARD-1/setup-ac003-demo.sh"
Enter
Sleep 500ms
Type "TMP=$(cat /tmp/ac003-tmpdir.txt)"
Enter
Sleep 200ms
Show

Type "cat $TMP/bc-dead.md"
Enter
Sleep 800ms

Type "BC_DIR=$TMP bash scripts/check-bc-citation-symbols.sh; echo exit:$?"
Enter
Sleep 4s
