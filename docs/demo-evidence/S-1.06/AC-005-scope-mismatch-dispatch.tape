# AC-005 (H-022): 401 scope-mismatch dispatch (3 test functions)
# test_s_1_06_h_022_scope_mismatch_lowercase_dispatches_insufficient_scope
# test_s_1_06_h_022_scope_mismatch_mixed_case_dispatches_insufficient_scope
# test_s_1_06_h_022_non_scope_401_and_403_do_not_dispatch_insufficient_scope
#
# Pins: 401 + "scope does not match" (any case) → InsufficientScope;
#       401 without scope phrase → NotAuthenticated (not InsufficientScope);
#       403 + scope phrase → ApiError (not InsufficientScope).
#
# Expected: 3 passed; 0 failed

Output docs/demo-evidence/S-1.06/AC-005-scope-mismatch-dispatch.gif
Output docs/demo-evidence/S-1.06/AC-005-scope-mismatch-dispatch.webm

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
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-1.06-oauth-flow-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-005: 401 scope-mismatch dispatch (3 sub-tests, H-022)"
Enter
Sleep 300ms

Type "cargo test --test oauth_flow_holdouts test_s_1_06_h_022 -- --nocapture --test-threads=1"
Enter
Sleep 60s

Type "# Result: 3 passed; 0 failed — scope-mismatch dispatch green"
Enter
Sleep 2s
