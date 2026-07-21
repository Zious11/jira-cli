Output AC-006-007-011-issue-older-than.gif
Output AC-006-007-011-issue-older-than.webm

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

# AC-006/AC-011: --issue + --older-than combined (human mode — BC-3.9.019 canonical strings)
Type "# AC-006/AC-011: --issue --older-than human mode — pre/post HINT strings (BC-3.9.019)"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than 30d --yes 2>&1"
Enter
Sleep 1200ms
Sleep 1500ms

# AC-006: --output json — NEITHER hint string emitted to stderr
Type "# AC-006: --output json — BC-3.9.019 hints suppressed in JSON mode"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 --older-than 30d --yes --output json 2>&1"
Enter
Sleep 1000ms
Sleep 1500ms

# AC-006/AC-007: chrono filter in action (DEMO-10: 1 old 2025-12-01 + 1 recent 2026-07-20)
Type "# AC-007: chrono filter — DEMO-10: old(99001) + recent(99003); --older-than 30d matches only old"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-10 --older-than 30d --dry-run 2>&1"
Enter
Sleep 1000ms
Sleep 1500ms

# AC-006: zero-match — DEMO-2 empty → no attachments
Type "# AC-006: zero-match — DEMO-2 empty → 'No attachments older than...' exit 0"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-2 --older-than 30d --yes 2>&1"
Enter
Sleep 800ms
Sleep 2000ms
