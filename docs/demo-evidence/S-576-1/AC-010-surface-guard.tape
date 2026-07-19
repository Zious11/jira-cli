Output AC-010-surface-guard.gif
Output AC-010-surface-guard.webm

Set FontSize 14
Set Width 1400
Set Height 420
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Env PATH "/opt/homebrew/bin:/Users/zious/.cargo/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "cd /tmp/jr-s576-worktree"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-010: surface guard — attachment list paths/flags registered in SURFACE table, all 10 tests pass
Type "cargo test --test e2e_cli_surface_guard 2>&1 | tail -8"
Enter
Sleep 6000ms
Sleep 2000ms
