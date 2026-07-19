Output AC-008-invalid-filter.gif
Output AC-008-invalid-filter.webm

Set FontSize 14
Set Width 1400
Set Height 350
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

# AC-008 (missing =): exit 64 before HTTP, shows accepted keys
Type "jr issue attachment list DEMO-1 --filter mime"
Enter
Sleep 600ms
Sleep 1800ms

# AC-008 (unknown key): exit 64, lists accepted keys
Type "jr issue attachment list DEMO-1 --filter type=image"
Enter
Sleep 600ms
Sleep 2000ms
