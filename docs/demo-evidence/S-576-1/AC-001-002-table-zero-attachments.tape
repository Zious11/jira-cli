Output AC-001-002-table-zero-attachments.gif
Output AC-001-002-table-zero-attachments.webm

Set FontSize 14
Set Width 1400
Set Height 420
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

# AC-001: table output — 6 columns (ID | Filename | Type | Size | Created | Author)
Type "jr issue attachment list DEMO-1"
Enter
Sleep 600ms
Sleep 2000ms

# AC-002: zero-attachment issue — empty stdout, stderr hint "No attachments on DEMO-2."
Type "jr issue attachment list DEMO-2"
Enter
Sleep 600ms
Sleep 2000ms
