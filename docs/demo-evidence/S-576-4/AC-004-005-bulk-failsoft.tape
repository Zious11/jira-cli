Output AC-004-005-bulk-failsoft.gif
Output AC-004-005-bulk-failsoft.webm

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

# AC-005: bulk success JSON
Type "# AC-005: bulk 2-AID success → {count:2, deleted:true, ids:[...]}"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99002 --yes --output json"
Enter
Sleep 800ms
Sleep 1500ms

# AC-004: missing --yes on multi-AID (BC-3.9.016 EC-3.9.016-8 string)
Type "# AC-004 BC-3.9.016: missing --yes on multi-AID → exit 64"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99002 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-004: benign 404 skip in bulk (99001 success + 99998 benign skip)
Type "# AC-004 BC-3.9.010: partial 404 benign skip → count:1 (not abort)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99998 --yes --output json 2>&1"
Enter
Sleep 800ms
Sleep 1500ms

# AC-004: non-404 aborts sequence (99001 success + 99403 abort)
Type "# AC-004 EC-3.9.010-4: non-404 error ABORTS sequence → exit 1"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99403 --yes 2>&1"
Enter
Sleep 700ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
