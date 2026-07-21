Output AC-002-011-error-taxonomy.gif
Output AC-002-011-error-taxonomy.webm

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

# AC-002: file-not-found pre-check (exit 64, before HTTP)
Type "# AC-002: file-not-found pre-check — exit 64 before any HTTP"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /nonexistent.pdf"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-002: stdin rejected before HTTP
Type "# AC-002: stdin '-' rejected before HTTP (EC-3.9.001-6)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 -"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-011: 404 issue not found
Type "# AC-011: 404 issue not found — exit 1"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-3 /tmp/report.pdf 2>&1"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-011: 401 not authenticated
Type "# AC-011: 401 not authenticated — exit 1 + auth hint"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-4 /tmp/report.pdf 2>&1"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-011: 413 file too large — actionable message
Type "# AC-011: 413 file too large — actionable message (EC-3.9.012-3)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-6 /tmp/report.pdf 2>&1"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
