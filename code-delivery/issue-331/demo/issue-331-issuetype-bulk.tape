Output issue-331-issuetype-bulk.gif
Output issue-331-issuetype-bulk.webm

Require jr

Set FontFamily "Menlo"
Set FontSize 13
Set Width 920
Set Height 540
Set Theme "Dracula"
Set Padding 16
Set PlaybackSpeed 0.85

# Setup -- hide env prep from viewer
Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/issue-331"
Enter
Sleep 300ms
Type "export JR_BASE_URL=http://127.0.0.1:1"
Enter
Type "export JR_AUTH_HEADER='Basic eA=='"
Enter
Type "export PATH=$PWD/target/debug:$PATH"
Enter
Sleep 300ms
Show

# ----------------------------------------------------------------
# DEMO 1: BC-3.4.019 -- Cross-project guard fires offline (exit 64)
# ----------------------------------------------------------------
Type "# BC-3.4.019: --type guard rejects cross-project keys"
Enter
Sleep 600ms

Type "jr issue edit FOO-1 BAR-2 --type Bug --no-input 2>&1; echo exit=$?"
Enter
Sleep 2500ms

# ----------------------------------------------------------------
# DEMO 2: EC-3.4.019-5 -- Guard fires even in --dry-run (exit 64)
# ----------------------------------------------------------------
Type "# EC-3.4.019-5: guard fires BEFORE dry-run (JSON error, exit 64)"
Enter
Sleep 600ms

Type "jr issue edit FOO-1 BAR-2 --type Bug --dry-run --output json --no-input 2>&1; echo exit=$?"
Enter
Sleep 2500ms

# ----------------------------------------------------------------
# DEMO 3: BC-3.4.018 -- Same-project dry-run shows issueType key
# ----------------------------------------------------------------
Type "# BC-3.4.018: same-project dry-run emits camelCase issueType"
Enter
Sleep 600ms

Type "jr issue edit FOO-1 FOO-2 --type Bug --dry-run --output json --no-input; echo exit=$?"
Enter
Sleep 2500ms
