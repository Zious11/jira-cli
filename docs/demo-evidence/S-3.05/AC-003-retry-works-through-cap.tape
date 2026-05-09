Output AC-003-retry-works-through-cap.gif
Output AC-003-retry-works-through-cap.webm

Require cargo

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "TokyoNight"
Set FontFamily "Menlo"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.05"
Enter
Sleep 500ms
Show

Type "cargo test --test asset_enrichment_concurrency test_enrich_assets_retry_works_through_cap 2>&1"
Enter
Sleep 13s

Sleep 2s
