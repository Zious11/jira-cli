Output AC-004-005-006-010-batch-all.gif
Output AC-004-005-006-010-batch-all.webm

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
Type "mkdir -p /tmp/jr-s576-2-cfg /tmp/jr-s576-2-cac && rm -rf /tmp/jr-demo-batch /tmp/jr-demo-json"
Enter
Sleep 300ms
Type "mkdir -p /tmp/jr-demo-batch /tmp/jr-demo-json"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-005/007: --all batch download + SHA-1 prefixed filenames + summary
Type "# AC-005 / AC-007: --all batch download + SHA-1 prefixed names"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --all --out-dir /tmp/jr-demo-batch"
Enter
Sleep 1200ms
Sleep 1500ms

Type "ls /tmp/jr-demo-batch/"
Enter
Sleep 400ms
Sleep 2000ms

# AC-006 / AC-010: JSON manifest (raw filename, id, path, size)
Type "# AC-006 / AC-010: --output json manifest (raw filename + SHA-1 path)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --all --out-dir /tmp/jr-demo-json --output json"
Enter
Sleep 1200ms
Sleep 3000ms
