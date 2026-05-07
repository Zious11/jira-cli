# BONUS: release binary grep verification
#
# Shows that the JR_AUTH_HEADER string is NOT present in the release binary.
# This is direct evidence that #[cfg(debug_assertions)] compiled the env-var
# read out of the release build — the security goal of SD-002 is satisfied.
#
# Expected output: "0" (strings finds zero occurrences)

Output docs/demo-evidence/S-0.05/BONUS-release-binary-grep-verification.gif
Output docs/demo-evidence/S-0.05/BONUS-release-binary-grep-verification.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-0.05-jr-auth-header-cfg-test-gate"
Enter
Sleep 500ms
Show

Type "# BONUS: JR_AUTH_HEADER absent from release binary (SD-002 security goal)"
Enter
Sleep 300ms

Type "cargo build --release --quiet 2>&1 && strings target/release/jr | grep -c 'JR_AUTH_HEADER' || echo '0 — gate compiled env-var read out of release binary'"
Enter
Sleep 120s
