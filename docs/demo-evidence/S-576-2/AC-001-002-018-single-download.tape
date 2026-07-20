Output AC-001-002-018-single-download.gif
Output AC-001-002-018-single-download.webm

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
Type "mkdir -p /tmp/jr-s576-2-cfg /tmp/jr-s576-2-cac && rm -f /tmp/architecture.pdf /tmp/_CON.txt"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-001: two-step streaming download + AC-018 success hint to stderr
Type "# AC-001 / AC-018: single-id download — streaming + success hint"
Enter
Sleep 200ms
Type "cd /tmp && jr issue attachment download DEMO-10 --id 30001"
Enter
Sleep 1200ms
Sleep 2000ms

Type "ls -lh architecture.pdf"
Enter
Sleep 500ms
Sleep 1500ms

# AC-002: --out preflight fires before HTTP (parent dir missing)
Type "# AC-002: --out preflight (parent dir missing) fires before HTTP"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id 30001 --out /tmp/nodir/output.pdf"
Enter
Sleep 600ms
Sleep 2000ms

# AC-002: file already exists → overwrite-refuse + --force bypass
Type "# AC-002: overwrite-refuse → Use --force to overwrite"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id 30001 --out /tmp/architecture.pdf"
Enter
Sleep 600ms
Sleep 1500ms
Type "jr issue attachment download DEMO-10 --id 30001 --out /tmp/architecture.pdf --force"
Enter
Sleep 1200ms
Sleep 2000ms
