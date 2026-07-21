Output AC-001-003-004-005-upload-success.gif
Output AC-001-003-004-005-upload-success.webm

Set FontSize 14
Set Width 1400
Set Height 520
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19878"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-3-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s576-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-3-cfg /tmp/jr-s576-3-cac && echo '%PDF demo' > /tmp/report.pdf && echo 'txt1' > /tmp/file1.txt && echo 'txt2' > /tmp/file2.txt"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-004: human-mode 4-column table (Filename, Size, ID, Created)
Type "# AC-004: single upload — 4-column echo table"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf"
Enter
Sleep 1200ms
Sleep 2000ms

# AC-003: bare JSON array with curated shape (self omitted, content→contentUrl)
Type "# AC-003: --output json — bare array, curated shape (contentUrl, no self)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --output json"
Enter
Sleep 1200ms
Sleep 2500ms

# AC-005: multi-file = single multipart POST
Type "# AC-005: multi-file = single POST, one row per file"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-MULTI /tmp/file1.txt /tmp/file2.txt"
Enter
Sleep 1200ms
Sleep 1500ms

# AC-001: stdin rejected
Type "# AC-001: stdin '-' rejected before HTTP (EC-3.9.001-6)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 -"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
