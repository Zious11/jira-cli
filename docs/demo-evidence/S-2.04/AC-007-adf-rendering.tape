# AC-007 / H-044 / BC-7.2.001: issue view with ADF description renders heading+paragraph, drops mention
# Covers: test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention
# Pins: ADF-to-text rendering in src/adf.rs — heading/paragraph text rendered, mention silently dropped
#
# The test spawns `jr issue view PROJ-1` against a wiremock issue endpoint with an ADF
# description containing heading, paragraph, codeBlock, and mention nodes.
# The output must include heading and paragraph text; mention text must NOT appear.
# No panic on any ADF node type (including unknown future node types).
#
# KNOWN-GAP: H-044. When NFR-O-I is implemented (mention → @displayName),
# the mention assertion must flip: assert!(stdout.contains("@John Smith")).
#
# Output: AC-007-adf-rendering.gif + AC-007-adf-rendering.webm

Output docs/demo-evidence/S-2.04/AC-007-adf-rendering.gif
Output docs/demo-evidence/S-2.04/AC-007-adf-rendering.webm

Require cargo

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 14
Set Width 1200
Set Height 600
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Padding 20
Set Framerate 30
Set TypingSpeed 40ms

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-2.04-bc-5-boards-sprints-holdout-suite"
Enter
Sleep 500ms
Show

Type "# AC-007 / H-044: issue view with ADF description renders text, drops mention"
Enter
Sleep 300ms

Type "cargo test --test boards_sprints_holdouts test_s_2_04_h_044_bc_7_2_001_adf_renders_heading_paragraph_drops_mention -- --nocapture --test-threads=1 2>&1"
Enter
Sleep 30s

Type "# Result: 1 passed — heading+paragraph in stdout, mention absent, exit 0"
Enter
Sleep 2s
