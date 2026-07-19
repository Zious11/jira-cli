Output AC-003-004-json-filter-hint.gif
Output AC-003-004-json-filter-hint.webm

Set FontSize 13
Set Width 1400
Set Height 700
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

# AC-003: JSON output — curated shape, no "self", has "contentUrl", author is {accountId,displayName}
Type "jr issue attachment list DEMO-1 --output json"
Enter
Sleep 600ms
Sleep 2500ms

# AC-004: filter-count hint fires when N < M (1 of 3 shown)
Type "jr issue attachment list DEMO-1 --filter mime=application/pdf"
Enter
Sleep 600ms
Sleep 2000ms
