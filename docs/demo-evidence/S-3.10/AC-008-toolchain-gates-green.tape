Output AC008_toolchain_gates_green.gif
Output AC008_toolchain_gates_green.webm

Require cargo

Set FontFamily "Menlo"
Set FontSize 14
Set Width 900
Set Height 400
Set Theme "Dracula"
Set Padding 20

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.10"
Enter
Sleep 1s
Show

Type "cargo build --all-targets 2>&1 | tail -1 && cargo clippy --all-targets -- -D warnings 2>&1 | tail -1 && cargo fmt --all -- --check && echo ALL GREEN"
Enter
Sleep 30s
