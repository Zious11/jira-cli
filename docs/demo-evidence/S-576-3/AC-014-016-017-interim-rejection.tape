Output AC-014-016-017-interim-rejection.gif
Output AC-014-016-017-interim-rejection.webm

Set FontSize 14
Set Width 1400
Set Height 520
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

# AC-017: --public rejected (interim guard; REMOVED-AT-S5)
Type "# AC-017: --public rejected with interim message (REMOVED at S-576-5)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --public 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-017: --internal rejected (interim guard)
Type "# AC-017: --internal rejected with same interim message"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --internal 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-017: rejection fires BEFORE file-pre-check
Type "# AC-017: interim rejection precedes file pre-check (nonexistent file + --public)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /nonexistent.pdf --public 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-016: upload to JSM issue with no flag → platform POST (no servicedeskapi calls)
Type "# AC-016: JSM issue (no --public/--internal) → platform POST, zero servicedeskapi calls"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --verbose 2>&1"
Enter
Sleep 1200ms
Sleep 2000ms
