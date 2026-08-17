Output AC-019-snapshot-drift-fail-closed.gif
Output AC-019-snapshot-drift-fail-closed.webm

Set FontSize 14
Set Width 1400
Set Height 480
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

# AC-019 (BC-8.2.007 Postcondition 5, fail-closed on drift / VP-COMPONENT-017):
# "DriftComp" is wired in mock-server.py to simulate the JRACLOUD-95368
# anti-loop-guard condition -- the snapshot search's own repeated-cursor
# guard aborts with has_more=true, which component delete treats as a fresh
# failure (JrError::SnapshotIncomplete, exit 1) rather than a partial
# success. The DELETE endpoint for this id is wired to return a loud 500 if
# ever reached, proving (in this recording) that it never was.
Type "# AC-019: snapshot pagination drift (JRACLOUD-95368) -> fail-closed, exit 1"
Enter
Sleep 200ms
Type "jr component delete DriftComp --project FOO --orphan --yes --no-input"
Enter
Sleep 1500ms
Type "echo exit: $?"
Enter
Sleep 500ms
Sleep 2500ms
