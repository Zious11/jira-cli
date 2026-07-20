Output AC-017-019-structural-and-tests.gif
Output AC-017-019-structural-and-tests.webm

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

# AC-017 Layer 1: clap mutual-exclusion constraints → exit 2
Type "# AC-017: clap structural constraints exit 2 — no selector"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 2>&1 | head -3"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

Type "# --id + --all mutually exclusive → exit 2"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --id 30001 --all 2>&1 | head -3"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1200ms

# AC-017 Layer 2: handler --newest validation → exit 64
Type "# AC-017 Layer 2: --newest 0 handler guard → exit 64"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-10 --newest 0"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 800ms
Type "jr issue attachment download DEMO-10 --newest -3"
Enter
Sleep 600ms
Type "echo exit: $?"
Enter
Sleep 300ms
Sleep 1500ms

# AC-013 / AC-017: cargo test suite (29 tests GREEN)
Type "# AC-013: cargo test attachment_download — 29 tests GREEN"
Enter
Sleep 200ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-2"
Enter
Sleep 200ms
Show

Type "cargo test --test attachment_download 2>&1 | tail -6"
Enter
Sleep 10000ms
Sleep 2000ms
