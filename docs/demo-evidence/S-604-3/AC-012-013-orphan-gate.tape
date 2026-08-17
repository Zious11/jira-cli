Output AC-012-013-orphan-gate.gif
Output AC-012-013-orphan-gate.webm

Set FontSize 14
Set Width 1400
Set Height 640
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Require jr

Env JR_BASE_URL "http://127.0.0.1:19880"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s604-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s604-3-cac"
Env JR_STDIN_IS_TTY "1"
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

# AC-012 (BC-8.2.006, interactive): the confirm prompt names the component
# AND the real, snapshot-derived affected-issue count. Decline (N/Enter
# default) -> exit 0, zero DELETE.
Type "# AC-012 (interactive): decline the --orphan confirm -> exit 0, zero DELETE"
Enter
Sleep 200ms
Type "jr component delete Orphaned --project FOO --orphan"
Enter
Sleep 900ms
Type "N"
Enter
Sleep 700ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 1200ms

# AC-012 (confirm branch): 'y' proceeds to the DELETE.
Type "# AC-012 (interactive): confirm the --orphan prompt -> proceeds to DELETE"
Enter
Sleep 200ms
Type "jr component delete Orphaned --project FOO --orphan"
Enter
Sleep 900ms
Type "y"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 1200ms

# AC-013 (BC-8.2.006, non-interactive / VP-COMPONENT-006): --orphan without
# --yes -> exit 64, message contains the REAL snapshot-derived count (5),
# never a placeholder.
Type "# AC-013: non-interactive --orphan without --yes -> exit 64, real count"
Enter
Sleep 200ms
Type "jr component delete Orphaned --project FOO --orphan --no-input"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 1200ms

# AC-013 (--yes bypass): --yes present -> proceeds without any prompt.
Type "# AC-013: --orphan --yes -> proceeds without a prompt"
Enter
Sleep 200ms
Type "jr component delete Orphaned --project FOO --orphan --yes --no-input"
Enter
Sleep 900ms
Type "echo exit: $?"
Enter
Sleep 400ms
Sleep 2000ms
