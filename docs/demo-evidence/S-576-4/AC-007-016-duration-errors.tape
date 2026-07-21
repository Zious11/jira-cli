Output AC-007-016-duration-errors.gif
Output AC-007-016-duration-errors.webm

Set FontSize 14
Set Width 1400
Set Height 520
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

# AC-007: invalid duration format (EC-3.9.019-3 canonical error string)
Type "# AC-007: invalid duration → exit 64 (EC-3.9.019-3 canonical string)"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than bad --yes 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-007 P1-001a: multibyte trailing char (5€ → exit 64, not panic)
Type "# AC-007 P1-001a: multibyte char '5€' → exit 64 not panic"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than '5€' --yes 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-007 P2-001: overflow large value (1e12d → exit 64, not overflow-panic)
Type "# AC-007 P2-001: chrono-band overflow (1000000000000d) → exit 64 not panic"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than 1000000000000d --yes 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-016: 403 targeted delete → exit 1
Type "# AC-016: 403 targeted delete → exit 1"
Enter
Sleep 200ms
Type "jr issue attachment delete 99403 --yes 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-001/AC-016: AID validation (non-numeric) fires before any HTTP
Type "# AC-001: non-numeric AID → exit 64 before any HTTP (BC-3.9.015 precondition)"
Enter
Sleep 200ms
Type "jr issue attachment delete abc --yes 2>&1"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
