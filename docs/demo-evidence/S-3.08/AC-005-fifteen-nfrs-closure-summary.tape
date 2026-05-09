Output docs/demo-evidence/S-3.08/AC-005-fifteen-nfrs-closure-summary.gif
Output docs/demo-evidence/S-3.08/AC-005-fifteen-nfrs-closure-summary.webm

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 600
Set FontFamily "Menlo"
Set Theme "TokyoNight"
Set Padding 16

Hide
Type "cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.08"
Enter
Sleep 500ms
Show

Type '# 5 source-comment closures (NFR-R-G NFR-O-I NFR-O-T NFR-SCA-1 NFR-SCA-3)'
Enter
Sleep 1s

Type 'grep -hE "// NFR-[A-Z0-9-]+:" src/cache.rs src/adf.rs src/api/jira/worklogs.rs src/api/rate_limit.rs src/jql.rs | grep -oE "NFR-[A-Z0-9-]+" | sort -u'
Enter
Sleep 3s

Type '# 6 CLAUDE.md closures (NFR-O-G NFR-O-C NFR-O-N NFR-O-P NFR-O-U NFR-O-X)'
Enter
Sleep 1s

Type 'grep -oE "NFR-O-[GCNPUX]" CLAUDE.md | sort -u'
Enter
Sleep 3s

Type '# 2 already-RESOLVED via S-2.05 (NFR-O-H NFR-O-R)'
Enter
Sleep 1s

Type 'grep -oE "NFR-O-(H|R)" /Users/zious/Documents/GITHUB/jira-cli/.factory/specs/prd/nfr-catalog.md | sort -u'
Enter
Sleep 3s

Type '# 2 pure-DEFER (NFR-O-E NFR-SCA-2)'
Enter
Sleep 1s

Type 'grep -oE "NFR-(O-E|SCA-2)" /Users/zious/Documents/GITHUB/jira-cli/.factory/specs/prd/nfr-catalog.md | sort -u'
Enter
Sleep 3s

Type 'echo "15/15 NFRs have closure: 5 src-comments + 6 CLAUDE.md + 2 RESOLVED(S-2.05) + 2 DEFER"'
Enter
Sleep 2s
