# AC-003: to_enrich HashMap (line 398) preserved — not modified by fix (BC-4.3.001)
# The to_enrich: HashMap<(String, String), ()> at list.rs:398 was already correct.
# This guard confirms it remains a composite (workspace_id, oid) key after the fix.

Output docs/demo-evidence/S-0.03/AC-003-to-enrich-composite-key-unchanged.gif
Output docs/demo-evidence/S-0.03/AC-003-to-enrich-composite-key-unchanged.webm

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

Type "# AC-003: to_enrich composite key (line 398) unchanged — already correct, preserved"
Enter
Sleep 500ms

Type "cargo test --lib test_bc_4_3_001_to_enrich_composite_key_unchanged -- --nocapture --test-threads=1"
Enter
Sleep 30s
