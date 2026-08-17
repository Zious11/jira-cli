Output docs/demo-evidence/S-604-2/AC-005-bad-assignee-type-exit-2.gif
Output docs/demo-evidence/S-604-2/AC-005-bad-assignee-type-exit-2.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1000
Set Height 300
Set Theme "Dracula"
Set Padding 20

Hide
Type "export PATH=/Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-604-2/target/debug:$PATH"
Enter
Sleep 200ms
Type "export JR_AUTH_HEADER='Basic dGVzdDp0ZXN0'"
Enter
Sleep 200ms
Show

# AC-005: --assignee-type BOGUS -> clap exit 2 (ValueEnum rejection), zero HTTP (DEC-188)
# possible values now show SCREAMING_SNAKE: COMPONENT_LEAD, PROJECT_LEAD, UNASSIGNED, PROJECT_DEFAULT
Type "jr component create --project FOO Backend --assignee-type BOGUS"
Enter
Sleep 2s
