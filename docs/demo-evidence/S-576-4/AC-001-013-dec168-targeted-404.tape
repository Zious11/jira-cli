Output AC-001-013-dec168-targeted-404.gif
Output AC-001-013-dec168-targeted-404.webm

Set FontSize 14
Set Width 1400
Set Height 480
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19879"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-4-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-4-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s576-4-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-4-cfg /tmp/jr-s576-4-cac"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-001/AC-013 DEC-168: targeted 404 — canonical prefix then body
Type "# AC-001 DEC-168: targeted 404 exits 64 + prefix then body (NOT silent)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99404 --yes 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-001: AID validation before any HTTP (non-numeric)
Type "# AC-001 BC-3.9.015 precondition: AID validation fires BEFORE gate/HTTP"
Enter
Sleep 200ms
Type "jr issue attachment delete abc 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-001: AID validation — empty-looking value
Type "# AC-001: non-numeric AID with leading zeros not purely numeric"
Enter
Sleep 200ms
Type "jr issue attachment delete abc123 --yes 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-013 DEC-168: 404 on bulk path = DIFFERENT (benign skip, NOT exit 64)
Type "# AC-013: contrast — bulk path 404 is benign skip (asymmetry from targeted)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99998 --yes --output json 2>&1"
Enter
Sleep 800ms
Sleep 2000ms
