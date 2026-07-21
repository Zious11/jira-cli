Output AC-012-014-015-clap-tests.gif
Output AC-012-014-015-clap-tests.webm

Set FontSize 14
Set Width 1400
Set Height 600
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

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

# AC-014: --issue without --older-than → exit 2
Type "# AC-014: --issue without --older-than → exit 2 (EC-3.9.016-9)"
Enter
Sleep 200ms
Type "jr issue attachment delete --issue DEMO-1 2>&1 | head -3"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-015: --older-than without --issue → exit 2 (EC-3.9.016-5)
Type "# AC-015: --older-than without --issue → exit 2 (EC-3.9.016-5)"
Enter
Sleep 200ms
Type "jr issue attachment delete --older-than 30d 2>&1 | head -3"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-015: positional AID + --issue conflict → exit 2 (EC-3.9.016-4)
Type "# AC-015: positional AID + --issue conflict → exit 2 (EC-3.9.016-4)"
Enter
Sleep 200ms
Type "jr issue attachment delete 99001 --issue DEMO-1 --older-than 30d 2>&1 | head -3"
Enter
Sleep 400ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-012: CLI surface guard — delete flags in --help
Type "# AC-012: CLI surface — delete flags in --help"
Enter
Sleep 200ms
Type "jr issue attachment delete --help 2>&1 | grep -E 'issue|older-than|--yes|--dry-run'"
Enter
Sleep 500ms
Sleep 1500ms

# AC-012/AC-013: cargo test attachment_delete (25 tests green)
Type "# AC-012/AC-013: cargo test attachment_delete — 25 tests green"
Enter
Sleep 200ms
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-4 && cargo test --test attachment_delete 2>&1 | tail -8"
Enter
Sleep 20000ms
Sleep 2000ms
