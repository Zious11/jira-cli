Output AC-006-007-replace-gate.gif
Output AC-006-007-replace-gate.webm

Set FontSize 14
Set Width 1400
Set Height 560
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19878"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-3-cac"
Env JR_STDIN_IS_TTY "1"
Env RUST_LOG ""
Env PATH "/tmp/jr-s576-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-3-cfg /tmp/jr-s576-3-cac && echo '%PDF demo' > /tmp/report.pdf"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-007: non-interactive gate (no TTY stdin without --yes → exit 64)
Type "# AC-007: non-interactive mode without --yes → exit 64 (BC-3.9.017 EC-3.9.017-9)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing --no-input"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-007: --yes bypasses gate entirely
Type "# AC-007: --yes bypasses gate — no prompt (non-interactive bypass)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing --yes"
Enter
Sleep 1200ms
Sleep 1500ms

# AC-007: confirmation gate — proceed with y (interactive, JR_STDIN_IS_TTY=1)
Type "# AC-007: interactive gate — typing 'y' proceeds (BC-3.9.014 eprint! mechanics)"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing"
Enter
Sleep 800ms
Type "y"
Enter
Sleep 1500ms
Sleep 1500ms

# AC-007: gate cancel with n
Type "# AC-007: typing 'n' cancels → 'Upload cancelled.' exit 0"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing"
Enter
Sleep 800ms
Type "n"
Enter
Sleep 800ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
