Output AC-014-015-016-cwe22-sanitization.gif
Output AC-014-015-016-cwe22-sanitization.webm

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
Type "rm -rf /tmp/jr-demo-cwe22 /tmp/jr-demo-degen && mkdir -p /tmp/jr-demo-cwe22 /tmp/jr-demo-degen"
Enter
Sleep 200ms
Type "rm -f /tmp/_CON.txt /tmp/CON.txt"
Enter
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# AC-014: CWE-22 path-traversal filename sanitized at batch path
Type "# AC-014: CWE-22 — '../../etc/passwd' filename sanitized to 'passwd'"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-60 --all --out-dir /tmp/jr-demo-cwe22"
Enter
Sleep 1200ms
Sleep 800ms
Type "ls /tmp/jr-demo-cwe22/"
Enter
Sleep 400ms
Sleep 2000ms

# AC-015: degenerate-name warning (filename='..') → id-as-filename fallback
Type "# AC-015: degenerate name '..' → warning + id-as-filename fallback"
Enter
Sleep 200ms
Type "jr issue attachment download DEMO-80 --all --out-dir /tmp/jr-demo-degen"
Enter
Sleep 1200ms
Sleep 800ms
Type "ls /tmp/jr-demo-degen/"
Enter
Sleep 400ms
Sleep 2000ms

# AC-016: Windows device-name escape at single-id call site
Type "# AC-016: single-id 'CON.txt' → escaped as '_CON.txt' (SEC-576-001)"
Enter
Sleep 200ms
Type "cd /tmp && jr issue attachment download DEMO-90 --id 30006"
Enter
Sleep 1200ms
Sleep 800ms
Type "ls -la _CON.txt"
Enter
Sleep 400ms
Sleep 2000ms
