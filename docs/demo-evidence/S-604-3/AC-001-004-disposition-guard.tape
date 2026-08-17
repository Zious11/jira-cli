Output AC-001-004-disposition-guard.gif
Output AC-001-004-disposition-guard.webm

Set FontSize 14
Set Width 1400
Set Height 520
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19880"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s604-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s604-3-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s604-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s604-3-cfg/jr /tmp/jr-s604-3-cac"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-001 (BC-8.2.001 postcondition 1): neither --move-to nor --orphan -> exit
# 64, stderr names BOTH flags. NAME-based source ("Backend") still resolves
# first (Invariant 1) but zero DELETE/snapshot-search calls occur.
Type "# AC-001: neither --move-to nor --orphan -> exit 64, ZERO DELETE"
Enter
Sleep 200ms
Type "jr component delete Backend --project FOO --no-input"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 1500ms

# AC-004 (BC-8.2.001 Invariant 1 documented exception): numeric source, no
# disposition -> the guard fires BEFORE any HTTP call is even reachable (the
# numeric-source confirming GET only fires once a disposition is supplied) --
# so this is the disposition-guard message, NOT "not found", with zero HTTP.
Type "# AC-004: numeric source, neither flag -> disposition guard, ZERO HTTP"
Enter
Sleep 200ms
Type "jr component delete 999999999 --no-input"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 2000ms
