Output docs/demo-evidence/S-3.03/AC-Bonus-1-mutex-layering-rule.gif
Output docs/demo-evidence/S-3.03/AC-Bonus-1-mutex-layering-rule.webm

Require cat

Set Shell "bash"
Set Width 1200
Set Height 600
Set FontSize 14
Set FontFamily "Menlo"
Set TypingSpeed 50ms

# Bonus: Mutex layering rule documented in refresh_coordinator.rs preamble.
# Outer std::sync::Mutex<HashMap<...>> held briefly (released before .await).
# Inner tokio::sync::Mutex<RefreshState> held across .await.
# tokio::sync::Mutex mandatory (no poisoning on panic).

Hide
Type 'cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.03'
Enter
Type 'clear'
Enter
Show

Type 'head -30 src/api/refresh_coordinator.rs'
Enter
Sleep 2s
