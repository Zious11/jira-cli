# AC-001 (unit level): Bare OID key collides on shared OID — composite key does not
# Unit test verifies that a HashMap<String, _> (bare OID) loses data on collision,
# while HashMap<(String, String), _> (composite) preserves both workspace entries.

Output docs/demo-evidence/S-0.03/AC-001-unit-composite-key-verification.gif
Output docs/demo-evidence/S-0.03/AC-001-unit-composite-key-verification.webm

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

Type "# AC-001 unit: composite (workspace_id, oid) key preserves both workspace entries"
Enter
Sleep 500ms

Type "cargo test --lib test_bc_4_3_001_bare_oid_key_collides_on_shared_oid -- --nocapture --test-threads=1"
Enter
Sleep 30s
