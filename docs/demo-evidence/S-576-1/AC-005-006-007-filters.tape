Output AC-005-006-007-filters.gif
Output AC-005-006-007-filters.webm

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

# AC-005: mime glob — image* crosses '/' (matches image/jpeg and image/png)
Type "jr issue attachment list DEMO-7 --filter mime=image*"
Enter
Sleep 600ms
Sleep 1800ms

# AC-006: name glob with ? — report-?.pdf matches single-char suffix only
Type "jr issue attachment list DEMO-7 --filter name=report-?.pdf"
Enter
Sleep 600ms
Sleep 1800ms

# AC-007: size-max — only attachments <= 2048 bytes (3 of 6 shown)
Type "jr issue attachment list DEMO-7 --filter size-max=2048"
Enter
Sleep 600ms
Sleep 1800ms
