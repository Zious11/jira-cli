# AC-001: Two-workspace OID collision resolves distinct labels (BC-4.3.001)
# Given two workspaces ws-A and ws-B both containing an asset with oid = "OBJ-88"
# but different names ("Acme Corp" for ws-A, "Widgets Inc" for ws-B),
# each issue's asset field MUST show the correct workspace-specific name.
# Pre-fix: last-write-wins caused both issues to show "Widgets Inc".

Output docs/demo-evidence/S-0.03/AC-001-multi-workspace-no-collision.gif
Output docs/demo-evidence/S-0.03/AC-001-multi-workspace-no-collision.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1000
Set Height 400
Set FontFamily "Menlo"
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 40ms
Set PlaybackSpeed 1

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.03-multi-workspace-asset-hashmap-key"
Enter
Sleep 500ms
Show

Type "# AC-001: Two-workspace OID collision — distinct labels, no last-write-wins"
Enter
Sleep 500ms

Type "cargo test --test issue_list_assets test_bc_4_3_001_multi_workspace_no_collision -- --nocapture --test-threads=1"
Enter
Sleep 30s
