Output AC-006-012-delete-ordering.gif
Output AC-006-012-delete-ordering.webm

Set FontSize 14
Set Width 1400
Set Height 560
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19878"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-3-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s576-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-3-cfg /tmp/jr-s576-3-cac && echo '%PDF demo' > /tmp/report.pdf"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-006 VP-576-003: DELETE-before-POST ordering (--verbose shows method+URL)
Type "# AC-006 VP-576-003: --verbose shows DELETE(s) before POST ordering invariant"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing --yes --verbose 2>&1"
Enter
Sleep 1500ms
Sleep 2500ms

# AC-006: 404 benign skip (stale AID during delete phase — does not abort upload)
Type "# AC-006: 404 on DELETE is benign skip — upload still proceeds"
Enter
Sleep 200ms
Type "# (DEMO-1 AID 99404 would 404-skip; upload continues)"
Enter
Sleep 200ms
Sleep 1500ms

# AC-012: zero-match → no gate, no delete, direct upload (BC-3.9.018 idempotent)
Type "# AC-012: --replace-existing zero-match (DEMO-2 has no attachments) → silent idempotent upload"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-2 /tmp/report.pdf --replace-existing --yes"
Enter
Sleep 1000ms
Sleep 2000ms

# AC-012: --output json on zero-match shows upload result (no wouldDelete)
Type "# AC-012: zero-match --output json — bare array (no delete phase)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-2 /tmp/report.pdf --replace-existing --yes --output json"
Enter
Sleep 1000ms
Sleep 2000ms
