Output AC-005-020-move-to-success.gif
Output AC-005-020-move-to-success.webm

Set FontSize 14
Set Width 1400
Set Height 560
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19880"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s604-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s604-3-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s604-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s604-3-cfg/jr /tmp/jr-s604-3-cac"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-005 (BC-8.2.002 postcondition 2): --move-to Frontend -> target resolves
# BEFORE the DELETE; DELETE .../component/{sourceId}?moveIssuesTo=<targetId>
# fires exactly once on success.
Type "# AC-005: --move-to Frontend -> resolve target, snapshot, then DELETE"
Enter
Sleep 200ms
Type "jr component delete Backend --project FOO --move-to Frontend --no-input"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 1500ms

# AC-020 (BC-8.2.008 success shape): --output json returns EXACTLY
# {deleted, movedIssuesTo, affectedIssueCount, affectedIssues} matching the
# snapshot taken before the DELETE.
Type "# AC-020: --output json success shape (movedIssuesTo = target id)"
Enter
Sleep 200ms
Type "jr component delete Backend --project FOO --move-to Frontend --no-input --output json"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 2200ms
