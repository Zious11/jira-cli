Output AC-003-009-013-error-taxonomy.gif
Output AC-003-009-013-error-taxonomy.webm

Set FontSize 14
Set Width 1400
Set Height 560
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
Type "clear"
Enter
Sleep 200ms
Show

# AC-003: non-numeric AID → exit 64
Type "# AC-003 / AC-009: non-numeric AID → exit 64"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id foo"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-009: AID 404 → exit 64
Type "# AID 404 → exit 64 (Attachment not found)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id 99404"
Enter
Sleep 800ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-009: KEY 404 on batch → exit 64
Type "# KEY 404 (batch) → exit 64 (Issue not found)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-30 --all --out-dir /tmp"
Enter
Sleep 800ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-009: 401 → exit 2
Type "# 401 → exit 2 (Not authenticated)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id 99401"
Enter
Sleep 800ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-002: --out-dir not-exist → exit 64 (preflight)
Type "# --out-dir not-exist → exit 64 (Output directory does not exist)"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --all --out-dir /tmp/nodir-xyz"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
