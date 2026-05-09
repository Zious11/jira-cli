Output docs/demo-evidence/S-3.03/AC-009-011-keyring-gated-ignored.gif
Output docs/demo-evidence/S-3.03/AC-009-011-keyring-gated-ignored.webm

Require cargo

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# AC-009, AC-010, AC-011 (keyring-gated): Tests exist and are #[ignore]-gated.
# Without JR_RUN_KEYRING_TESTS=1 they are skipped in default runs.
# Linux CI runs them with: JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=jr-s303-test
#   cargo test --test oauth_refresh_integration -- --ignored --test-threads=1

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'cargo test --test oauth_refresh_integration -- --ignored 2>&1 | tail -15'
Enter
Sleep 11s
Type 'echo ""'
Enter
Type 'echo "Linux CI gate: JR_RUN_KEYRING_TESTS=1 JR_SERVICE_NAME=jr-s303-test cargo test --test oauth_refresh_integration -- --ignored --test-threads=1"'
Enter
Sleep 2s
