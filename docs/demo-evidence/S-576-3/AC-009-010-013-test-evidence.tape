Output AC-009-010-013-test-evidence.gif
Output AC-009-010-013-test-evidence.webm

Set FontSize 14
Set Width 1400
Set Height 600
Set Theme "Dracula"
Set Shell bash
Set TypingSpeed 30ms

Env JR_BASE_URL "http://127.0.0.1:19878"
Env JR_AUTH_HEADER "Basic dGVzdDp0ZXN0"
Env JR_CONFIG_DIR "/tmp/jr-s576-3-cfg"
Env JR_CACHE_DIR "/tmp/jr-s576-3-cac"
Env RUST_LOG ""
Env PATH "/tmp/jr-s576-3-bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Hide
Type "mkdir -p /tmp/jr-s576-3-cfg /tmp/jr-s576-3-cac"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-013: CLI surface — flags registered in --help
Type "# AC-013: CLI surface guard — upload flags visible in --help"
Enter
Sleep 200ms
Type "jr issue attachment upload --help 2>&1 | grep -E 'replace-existing|--yes|--dry-run|--public|--internal'"
Enter
Sleep 400ms
Sleep 1500ms

# AC-009: no JSM import in attachments.rs (review-time grep)
Type "# AC-009: no api::jsm import in attachments.rs (forbidden dependency)"
Enter
Sleep 200ms
Type "grep -n 'api::jsm' /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-3/src/cli/issue/attachments.rs && echo 'FAIL' || echo 'PASS: no jsm imports found'"
Enter
Sleep 400ms
Sleep 1200ms

# AC-010: mutants.toml examine_globs covers attachments.rs
Type "# AC-010: mutants.toml examine_globs includes attachments.rs"
Enter
Sleep 200ms
Type "grep 'attachments' /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-3/.cargo/mutants.toml"
Enter
Sleep 400ms
Sleep 1200ms

# Run attachment_upload test suite (26 tests)
Type "# AC-009/010/013: cargo test attachment_upload — 26 tests green"
Enter
Sleep 200ms
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-576-3 && cargo test --test attachment_upload 2>&1 | tail -8"
Enter
Sleep 15000ms
Sleep 2000ms
