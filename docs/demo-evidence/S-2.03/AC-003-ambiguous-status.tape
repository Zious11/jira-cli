# AC-003 / H-039 / BC-4.2.006: jr assets tickets --status PROG with two matches → exit 64
# Covers: test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64
# Pins: filter_tickets Ambiguous branch in src/cli/assets.rs + src/partial_match.rs
#
# The test spawns `jr assets tickets OBJ-1 --status PROG` against wiremock, where
# two ticket statuses ("In Progress", "Progressing") both contain "PROG".
# partial_match returns Ambiguous → jr exits 64, stderr names both candidates.
#
# Output: AC-003-ambiguous-status.gif + AC-003-ambiguous-status.webm

Output docs/demo-evidence/S-2.03/AC-003-ambiguous-status.gif
Output docs/demo-evidence/S-2.03/AC-003-ambiguous-status.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.03-bc-4-asset-enrichment-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-003 / H-039: --status PROG ambiguous (matches 'In Progress' + 'Progressing') → exit 64"
Enter
Sleep 300ms

Type "cargo test --test asset_holdouts test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — exit 64 (UserError), both status candidates listed in stderr"
Enter
Sleep 2s
