Output AC-011-012-fail-soft.gif
Output AC-011-012-fail-soft.webm

Set FontSize 14
Set Width 1400
Set Height 520
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19877"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-2-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-2-cac"
Env PATH "/tmp/jr-s576-2-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-2-cfg /tmp/jr-s576-2-cac"
Enter
Sleep 200ms
Type "rm -rf /tmp/jr-demo-partial /tmp/jr-demo-partial-json"
Enter
Sleep 200ms
Type "mkdir -p /tmp/jr-demo-partial /tmp/jr-demo-partial-json"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-011: fail-soft — per-file warning, partial success, exit 1
Type "# AC-011: fail-soft batch — 1 of 2 content-GETs fails (500)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-50 --all --out-dir /tmp/jr-demo-partial"
Enter
Sleep 1200ms
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 800ms
Type "ls /tmp/jr-demo-partial/"
Enter
Sleep 400ms
Sleep 2000ms

# AC-012: JSON mode — warning still emits to stderr, manifest has only successes, exit 1
Type "# AC-012: JSON mode — warning on stderr, manifest = successes only, exit 1"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-50 --all --out-dir /tmp/jr-demo-partial-json --output json"
Enter
Sleep 1200ms
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
