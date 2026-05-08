# AC-004 / H-033 / BC-3.7.004: ftp:// URL rejected at validation — exit 64, zero HTTP calls
# Covers: test_s_2_01_h_033_bc_3_7_004_ftp_url_rejected_before_network_with_exit_64
# Pins: scheme validation gate in src/cli/issue/links.rs::handle_remote_link
#
# Output: AC-004-ftp-url-rejected-before-network.gif + .webm

Output docs/demo-evidence/S-2.01/AC-004-ftp-url-rejected-before-network.gif
Output docs/demo-evidence/S-2.01/AC-004-ftp-url-rejected-before-network.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.01-bc-2-issue-read-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-004 / H-033: ftp:// URL rejected before any network call — exit 64 (UserError)"
Enter
Sleep 300ms

Type "cargo test --test issue_read_holdouts test_s_2_01_h_033 -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 60s

Type "# Result: 1 passed — exit 64, 'http or https' + 'ftp' in stderr, zero HTTP calls"
Enter
Sleep 2s
