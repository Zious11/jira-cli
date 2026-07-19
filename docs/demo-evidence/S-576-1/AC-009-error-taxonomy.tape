Output AC-009-error-taxonomy.gif
Output AC-009-error-taxonomy.webm

Set FontSize 14
Set Width 1400
Set Height 560
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Env JR_BASE_URL "http://127.0.0.1:19876"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-demo-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-demo-cac"
Env PATH "/tmp/jr-s576-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-demo-cfg /tmp/jr-s576-demo-cac"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-009: 404 → exit 64 "not found or not accessible"
Type "jr issue attachment list DEMO-3"
Enter
Sleep 500ms
Sleep 1000ms

# AC-009: 401 → exit 2 "Not authenticated. Run jr auth login"
Type "jr issue attachment list DEMO-4"
Enter
Sleep 500ms
Sleep 1000ms

# AC-009: 403 → exit 1 "Permission denied: cannot access issue"
Type "jr issue attachment list DEMO-5"
Enter
Sleep 500ms
Sleep 1000ms

# AC-009: 5xx → exit 1 "API error (500)"
Type "jr issue attachment list DEMO-6"
Enter
Sleep 500ms
Sleep 1000ms

# AC-009: network error → exit 1 "Could not reach" (override to unreachable port)
Type "JR_BASE_URL=http://127.0.0.1:19999 jr issue attachment list DEMO-1"
Enter
Sleep 800ms
Sleep 1500ms
