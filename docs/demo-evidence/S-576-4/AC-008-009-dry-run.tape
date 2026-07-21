Output AC-008-009-dry-run.gif
Output AC-008-009-dry-run.webm

Set FontSize 14
Set Width 1400
Set Height 560
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

# AC-008: --dry-run single-AID human (no DELETE, hint to stderr)
Type "# AC-008: --dry-run single-AID — no gate, no DELETE, hint (EC-3.9.020-3)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 --dry-run 2>&1"
Enter
Sleep 500ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-008: --dry-run single-AID JSON
Type "# AC-008: --dry-run single-AID JSON — {attachments:[{id}], dryRun:true, ids:[...]}"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 --dry-run --output json"
Enter
Sleep 600ms
Sleep 1500ms

# AC-009: --dry-run bulk (--issue + --older-than) — [ID,Filename,Size,Created] table
Type "# AC-009: --dry-run bulk (--issue) — [ID,Filename,Size,Created] table (EC-3.9.020-1/2)"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than 30d --dry-run 2>&1"
Enter
Sleep 1000ms
Sleep 1500ms

# AC-009: --dry-run bulk JSON
Type "# AC-009: --dry-run bulk JSON — {attachments:[{filename,id}], dryRun:true, ids:[...]}"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than 30d --dry-run --output json"
Enter
Sleep 1000ms
Sleep 1500ms

# AC-009: --dry-run multi-AID with metadata fan-out
Type "# AC-009: --dry-run multi-AID — per-AID metadata fan-out (filename column)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 99002 --dry-run"
Enter
Sleep 1000ms
Sleep 2000ms
