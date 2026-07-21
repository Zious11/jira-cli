Output AC-001-002-003-010-single-gate.gif
Output AC-001-002-003-010-single-gate.webm

Set FontSize 14
Set Width 1400
Set Height 560
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19879"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-4-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-4-cac"
Env JR_STDIN_IS_TTY "1"
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

# AC-002/AC-001: interactive gate — 'y' proceeds to DELETE (VP-576-002 confirm)
Type "# AC-002 VP-576-002: gate confirm — 'y' proceeds; pre-prompt GET fetches filename"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001"
Enter
Sleep 900ms
Type "y"
Enter
Sleep 1200ms
Sleep 1500ms

# AC-003: single-AID JSON shape
Type "# AC-003: --yes --output json → {deleted:true, id:...}"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 --yes --output json"
Enter
Sleep 800ms
Sleep 1500ms

# AC-002: gate cancel with 'n' (VP-576-002 cancel)
Type "# AC-002 VP-576-002: gate cancel — 'n' → 'Deletion cancelled.' exit 0"
Enter
Sleep 200ms
Type "jr issue attachment delete 99002"
Enter
Sleep 900ms
Type "n"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-010: non-interactive without --yes → exit 64 (DEC-174)
Type "# AC-010: --no-input without --yes → exit 64"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 --no-input"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
