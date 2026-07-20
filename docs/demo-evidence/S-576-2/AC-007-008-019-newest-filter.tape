Output AC-007-008-019-newest-filter.gif
Output AC-007-008-019-newest-filter.webm

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
Type "rm -rf /tmp/jr-demo-newest /tmp/jr-demo-filter /tmp/jr-demo-zero"
Enter
Sleep 200ms
Type "mkdir -p /tmp/jr-demo-newest /tmp/jr-demo-filter /tmp/jr-demo-zero"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-008: --newest N selects by created descending
Type "# AC-008: --newest 2 selects 2 most-recent attachments (by created desc)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-70 --newest 2 --out-dir /tmp/jr-demo-newest"
Enter
Sleep 1200ms
Sleep 1000ms
Type "ls /tmp/jr-demo-newest/"
Enter
Sleep 400ms
Sleep 1500ms

# AC-007: --all + --filter mime=image/*
Type "# AC-007: --filter narrows batch selection to image/* files only"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-70 --all --filter 'mime=image/*' --out-dir /tmp/jr-demo-filter"
Enter
Sleep 1200ms
Sleep 1000ms
Type "ls /tmp/jr-demo-filter/"
Enter
Sleep 400ms
Sleep 1500ms

# AC-019: filtered-to-zero hint
Type "# AC-019: filter matching zero attachments on DEMO-70 → hint + exit 0"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-70 --all --filter 'name=*.xyz' --out-dir /tmp/jr-demo-zero"
Enter
Sleep 800ms
Sleep 2000ms
