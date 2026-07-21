Output AC-008-dry-run.gif
Output AC-008-dry-run.webm

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
Type "mkdir -p /tmp/jr-s576-3-cfg /tmp/jr-s576-3-cac && echo '%PDF demo' > /tmp/report.pdf"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-008: --dry-run human preview (gate suppressed, list GET fires)
Type "# AC-008: --dry-run preview — gate suppressed, list GET fires, no mutations"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing --dry-run"
Enter
Sleep 1200ms
Sleep 2000ms

# AC-008: --dry-run JSON output (camelCase shape)
Type "# AC-008: --dry-run JSON — dryRun/wouldDelete/wouldUpload camelCase shape"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --replace-existing --dry-run --output json"
Enter
Sleep 1200ms
Sleep 2500ms

# AC-008 EC-3.9.020-6: --dry-run without --replace-existing → exit 2 (clap)
Type "# AC-008 EC-3.9.020-6: --dry-run requires --replace-existing → exit 2"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /tmp/report.pdf --dry-run 2>&1 | head -3"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-008: pre-flight NOT suppressed on dry-run (non-existent file still exits 64)
Type "# AC-008: pre-flight file check NOT suppressed on dry-run"
Enter
Sleep 200ms
Type "jr issue attachment upload DEMO-1 /nonexistent.pdf --replace-existing --dry-run"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 2000ms
