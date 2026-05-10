Output docs/demo-evidence/issue-110-pr2/D-004-no-parent-multi-key-rejected.gif
Output docs/demo-evidence/issue-110-pr2/D-004-no-parent-multi-key-rejected.webm

Require jr

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set TypingSpeed 50ms
Set Theme "Catppuccin Mocha"
Set Padding 20

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-110-pr2'
Enter
Sleep 500ms
Type 'export JR_BASE_URL=http://127.0.0.1:9'
Enter
Type 'export JR_AUTH_HEADER="Basic dGVzdA=="'
Enter
Sleep 300ms
Show

Type './target/release/jr issue edit FOO-1 FOO-2 --no-parent --no-input; echo "exit $?"'
Enter
Sleep 3s
