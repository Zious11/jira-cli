# AC-003: new_for_test signature unchanged
# test_sd_002_new_for_test_signature_unchanged
#
# Compile-time verification that JiraClient::new_for_test still accepts
# (String, String) -> JiraClient. If the signature changes, the test
# fails to compile. S-0.05 fix must NOT alter this public API.
#
# Expected: 1 passed; 0 failed

Output docs/demo-evidence/S-0.05/AC-003-new-for-test-signature-unchanged.gif
Output docs/demo-evidence/S-0.05/AC-003-new-for-test-signature-unchanged.webm

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

Type "# AC-003: new_for_test(String, String) -> JiraClient signature unchanged"
Enter
Sleep 300ms

Type "cargo test --test auth_header_release_gate test_sd_002_new_for_test_signature_unchanged -- --nocapture --test-threads=1"
Enter
Sleep 30s
