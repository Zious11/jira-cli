---
document_type: research-decision
wave: 2
gate_pass: 01
decisions: 4
producer: research-agent
date: 2026-05-08
recommendation_d1: A
recommendation_d2: separate
recommendation_d3: defer
recommendation_d4: C
overall_confidence: MEDIUM
---

# Wave 2 Integration-Gate Close — Process/Governance Decisions Research

Scope: 4 process decision points raised by the Wave 2 gate finding triage. The
user has chosen to **fix the spec-anchor BLOCKING items** and **defer the
non-blockers**. Open questions are HOW to fix.

External evidence quality varies considerably across the four decisions. Where
external precedent is weak, this report says so explicitly rather than
inflating confidence.

---

## Decision 1 — BC re-anchoring strategy for WV2-ADV-01

### Choice: **Option A** (re-anchor to BC-7.1.001 + add 4 sub-BCs in BC-7.4)

### Confidence: **HIGH**

### Top citations driving the recommendation

1. **Google AIP-122 (Resource Names) and AIP-162 (Revisions)** — when an
   existing canonical taxonomy section already covers a topic, AIP guidance
   prefers extending that section over inventing a new top-level identifier.
   New top-level IDs are reserved for "fundamentally new entities", not for
   contracts that fit cleanly under an existing topical heading. The same
   philosophy translates to BC taxonomies: BC-7.4 already exists to hold
   per-shape JSON pins (12 entries today, all per-command, including
   `move`, `assign`, `edit`, `link`, `unlink`, `sprint add`, `sprint remove`,
   `auth list`). Adding `auth login`, `auth switch`, `auth logout`,
   `auth remove` continues the established pattern. ([AIP-162](https://google.aip.dev/162))
2. **Project-internal evidence (BC-INDEX.md:472-487)** — BC-7.4 is the
   established home of per-shape JSON pins. Of the 12 existing entries, every
   one is a single-command shape pin (e.g., BC-7.4.011 `auth list table`).
   Option A continues that established pattern; Option B would invent a single
   cross-cutting BC inside BC-7.3 (Error Display), which is the wrong topical
   home for success-shape contracts.

### Trade-off summary

Option A produces 4 fine-grained sub-BCs that each pin one auth subcommand's
JSON shape. The cost is 4 new IDs to maintain (BC-7.4.013..016). The benefit
is taxonomic correctness: each BC corresponds to exactly one shape, mirroring
the BC-7.4.001..012 precedent. Option B saves 3 IDs but creates a single
"omnibus" BC inside BC-7.3 (Error Display) — a section about error semantics,
not success shapes. That structural mismatch is exactly the kind of
mis-anchoring that produced WV2-ADV-01 in the first place.

On future-churn risk (1c): Option A is *lower-churn* — if Wave 3 adds a
`request_id` field to one auth shape (say, `auth login`), only BC-7.4.013
needs an update. Option B's single shared BC would grow more abstract over
time as each shape diverges, eventually requiring a split anyway.

### Edge cases / caveats

- **External-source caveat**: my Perplexity reason call returned no relevant
  results on BC-taxonomy governance specifically; this recommendation rests
  primarily on AIP-162's resource-versioning analogy and the project's
  internal BC-7.4 precedent. The AIP analogy is not a perfect fit (AIPs
  govern API contracts, not test-anchor taxonomies), but the principle of
  "extend the topical section, don't invent a new top-level identifier"
  transfers cleanly.
- BC-7.4.013..016 are reserved IDs; verify no other in-flight story already
  claims them before the fix-PR.
- Option A requires touching BC-INDEX.md (4 new rows in BC-7.4 table) AND
  bc-7-output-render.md body (4 new BC body sections). Option B touches one
  body section. Net file count is similar; Option A is more *spec lines*
  but more *anchorically correct*.

---

## Decision 2 — One combined fix-PR or two separate

### Choice: **Separate**

### Confidence: **MEDIUM-HIGH**

### Top citations driving the recommendation

1. **Google's "Code Review Developer Guide" / eng-practices** — recommends
   "small CLs" reviewable in <1 hour and typically <200 LOC. Empirical study
   of 212K+ PRs (Augment Code, PropelCode) shows defect detection drops 70%
   for PRs >400 LOC and review time increases 2-3x. Your Fix-PR A (50 LOC,
   18 files) is well below the LOC ceiling but the **file count** drives
   cognitive load independently of LOC. Combining with Fix-PR B (1 file, 30
   LOC) crosses a *coherence boundary* (two distinct topics) without saving
   meaningful review time. ([Google eng-practices](https://google.github.io/eng-practices/review/reviewer/standard.html), [Augment Code review study](https://www.augmentcode.com/guides/code-review-best-practices-that-scale))
2. **Open-source convention** — Kubernetes' `kind/cleanup` label, Linux
   kernel `Documentation/` patches, and Rust RFC repo all batch by
   *thematic unit*, not by *gate-closure session*. A "spec-anchor sweep" and
   a "NFR catalog sweep" are two thematic units. The contributor guides for
   all three explicitly say "one logical change per PR". (No direct merge-log
   citation in retrieved sources; this draws on documented contributor-guide
   policies.)

### Trade-off summary

Combined PR pros: single revert unit, single merge for the gate, simpler
backport. Combined PR cons: mixed-topic diff, harder for a topic-specialist
reviewer to focus, larger context-switch cost during review. Separate PR
pros: each PR is single-topic and reviewable in <30 minutes, parallel
review possible, cleaner `git log --grep` for future archaeology. Separate
PR cons: two merges to track, slightly more orchestration overhead.

The "atomic gate close" benefit of combining is real but small — both PRs
can be merged within an hour of each other with linked descriptions ("closes
gate alongside #NNN"), achieving 90% of the atomic-revert benefit.

The **file count is the discriminator**: 18 files of 1-2 LOC each is
mechanically simple but reading 18 files imposes a real cognitive cost. The
NFR catalog sweep is in 1 file with denser (30-line) changes that need
careful per-row review. Combining mixes a "scan-many-files" task with a
"read-one-file-carefully" task — the worst of both worlds for the reviewer.

### Edge cases / caveats

- **External evidence is moderate**: Google's <200 LOC guideline is well
  established (multiple sources agree); the project-specific OSS-merge-log
  examples I asked for did not return concrete URLs in the retrieved
  results. Confidence is graded MEDIUM-HIGH (not HIGH) because of this.
- If your reviewer is the same person for both, the parallelism benefit
  evaporates — combined becomes a wash. Recommendation still holds because
  of the topic-coherence argument.
- 18 files of 1-2 LOC each is borderline mechanically simple enough that
  some shops would auto-merge under a `chore(spec-sweep)` label. If your
  team has such automation, combined is more attractive.

---

## Decision 3 — Create S-3.11 story stub now or wait

### Choice: **Defer to Wave 3 planning** (with one carve-out: WV2-SEC-01)

### Confidence: **MEDIUM**

### Top citations driving the recommendation

1. **Mike Cohn (Mountain Goat Software) on technical debt**: formal backlog
   items are warranted "when you've committed to paying it off at a specific
   time" and the debt is "significant enough to warrant priority". For
   smaller refactoring tasks (a few hours), a "technical backlog/informal
   items" approach is preferred. Your finding mix is heavily weighted toward
   small items (1 doc edit, 1 missing unit test, 1 internal-spec
   contradiction, 3 refactors, 1 spec edit) — these are precisely the
   "informal items" Cohn describes. ([Mountain Goat](https://www.mountaingoatsoftware.com/blog/three-strategies-for-fitting-refactoring-into-your-sprints))
2. **Project-internal evidence (`.factory/STATE.md` Drift Items table)** —
   the consistency review (line 460-477) explicitly verifies 5 of 5 sampled
   DEFERRED items have explicit targets, and 5 of 5 sampled RESOLVED items
   are supported by diff evidence. The drift-table mechanism is *empirically
   working* on this project. The risk of "lost follow-up" that motivates
   stub-now in less-disciplined shops doesn't apply here.

### Trade-off summary

Stub-now creates a `S-3.11-wave-2-followup-cleanup.md` artifact that needs
spec discipline (frontmatter, BC anchors, ACs) for 7+ small items. The
result is likely a "grab-bag" story — the anti-pattern Cohn explicitly warns
against (stories should be independent, small, testable). Defer-with-drift
postpones that decision to Wave 3 planning where capacity, scope, and
priority are weighed jointly. Your STATE.md drift-table track record (100%
sampled-RESOLVED rate) shows the mechanism doesn't lose items.

The carve-out: **WV2-SEC-01 (CWE-400) is genuine security debt**. Even though
unexploitable remotely, security items deserve an explicit target date in
the drift log (per NIST/OWASP guidance on input-validation gaps). Don't let
it sit in DEFERRED with "no target".

### Edge cases / caveats

- **External evidence is weak on quantified abandonment rates**: Perplexity
  did not return numerical studies of "TODO comment vs. formal-story closure
  rates". Confidence is MEDIUM (not HIGH) for that reason. The recommendation
  is grounded in (a) Cohn's qualitative guidance on grab-bag stories and
  (b) your project's own data (5/5 DEFERRED items have targets).
- If your project's drift-table discipline starts slipping (e.g., a future
  sample shows 3/5 with no targets), revisit this decision. The track record
  is the foundation; if it cracks, formal stories become necessary.
- The "S-3.10 was queued during S-2.06" precedent cuts both ways. S-3.10 was
  a *single, focused, cleanup-of-a-specific-deprecation* — 1 well-defined
  item. Bundling 7 mixed items into a hypothetical S-3.11 is structurally
  different.

---

## Decision 4 — WV2-SEC-01 placement (parse_duration_validate input cap)

### Choice: **Option C** (standalone tiny PR)

### Confidence: **LOW-MEDIUM**

### Top citations driving the recommendation

1. **Industry pattern for tiny security hardenings** — kernel security
   patches (e.g., CVE-2026-31431 "copy_fail", upstream commit
   `a664bf3d603d`) typically ship as standalone single-commit fixes,
   independently backportable. The standalone-PR pattern is the dominant
   form for small CWE-400 / CWE-20 hardenings. ([CERT-EU advisory 2026-005](https://cert.europa.eu/publications/security-advisories/2026-005/), [AlmaLinux blog on copy_fail](https://almalinux.org/blog/2026-05-01-cve-2026-31431-copy-fail/))
2. **Project-internal architectural argument** — the project explicitly
   uses **worktree isolation** between specs (`factory-artifacts`) and code
   (`develop`). Mixing factory-artifacts edits with `develop` code edits in
   one PR (Option A) violates that separation. Your CLAUDE.md branch
   conventions document `develop` as the code-merge target with feature
   branches `type/short-description`. A 5-line `fix:` patch fits perfectly
   in that pattern.

### Trade-off summary

Option A (fold into Fix-PR A) — pros: single merge for the gate. Cons:
breaks spec/code worktree separation; mixes review domains (factory
artifacts vs product code); makes `git log` harder to use for security-fix
archaeology.

Option B (bundle into S-3.11) — pros: one cleanup PR. Cons: a 5-line
security fix sits behind 6 other unrelated items waiting for next-wave
planning. Time-to-fix could be 2-4 weeks. For unexploitable MEDIUM CWE-400
that's acceptable but not ideal — small security debt should be discharged
fast when the fix is trivial.

Option C (standalone) — pros: matches industry pattern (kernel-style
standalone hardening commits); fits cleanly in `develop` branch flow with
a `fix(security):` conventional-commit prefix; reviewable in <5 minutes;
clean revert if needed; trivially backportable to any maintenance branch.
Cons: one more PR for the team to merge. The 5-minute review cost is much
smaller than the architectural cost of the alternatives.

### Edge cases / caveats

- **External evidence is weakest here**: Perplexity searches did not
  surface a perfect "5-line CWE-400 standalone PR" example from
  curl/Rust/Go/CPython logs. The kernel CVE-2026-31431 is the closest
  analog and it does follow the standalone-commit pattern, but it's
  larger than 5 lines. Confidence is LOW-MEDIUM accordingly.
- Lifecycle benchmark for unexploitable MEDIUM CWE-400 in CLI tools:
  retrieved data is sparse. Anecdotal pattern from kernel/curl is "ship
  in next maintenance window if trivial; ship in next release if not".
  5 lines is trivial; the implication is "ship soon, standalone".
- If your team's CI overhead per PR is high (e.g., 30+ minute matrix), the
  per-PR cost might tip toward Option B. Your current CI is 8/8 green and
  fast (most recent stories merged with single-cycle APPROVE), so this
  doesn't apply.
- One mitigation if Option C feels too small: include the WV2-SEC-01 fix
  + a regression-pin test (~20 LOC total) and title the PR
  `fix(security): cap parse_duration input length (CWE-400)`. That's a
  meaningful unit, not a "trivial" one.

---

## Final Summary — "If I had to pick all 4 right now"

Recommendation: **D1=A, D2=separate, D3=defer, D4=C**.

The unifying logic: each decision favors *minimum coupling* and
*topic-coherent units*. D1's Option A keeps each auth shape pinned to its
own per-shape BC, mirroring the project's existing BC-7.4.001..012 pattern
and avoiding a cross-cutting BC inside the wrong section. D2's separate
PRs respect the spec-anchor sweep and the NFR catalog sweep as two
distinct topical units, matching Google's small-CL guidance and OSS
contributor-guide norms. D3's defer-with-drift leverages your project's
empirically-working drift-table discipline (5/5 RESOLVED-with-evidence on
the consistency review's audit) instead of forcing 7 mixed items into a
grab-bag story. D4's standalone PR keeps the security fix out of the
spec-edit worktree boundary, matches the kernel-style standalone-hardening
pattern, and clears the security debt fast. The combined effect is one
spec-sweep PR + one NFR-sweep PR (both factory-artifacts) + one tiny
security PR (develop) closing the gate, with everything else logged for
Wave 3 planning.

Confidence calibration: D1 is HIGH because the AIP-162 analogy and the
project's BC-7.4 precedent both point the same direction. D2 is
MEDIUM-HIGH because Google's <200 LOC guideline is well-attested but
specific OSS merge-log examples weren't retrievable. D3 is MEDIUM because
Cohn's guidance is qualitative and abandonment-rate quantification was not
retrievable. D4 is LOW-MEDIUM because the kernel analog is the closest
external precedent and it's not a perfect fit; the recommendation rests
heavily on the project's worktree-isolation architecture.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity reason | 4 | Decision-by-decision reasoning queries (D1 BC governance, D2 PR structure, D3 agile cleanup story, D4 security fix placement) — 3 of 4 returned weak signal due to upstream search-result limitations |
| Perplexity search | 6 | Targeted lookups: Google AIP versioning, Google eng-practices PR size, Mike Cohn technical debt, CWE-400 lifecycle, OSS cleanup PR conventions, kernel input-cap commits |
| WebFetch | 0 | (not needed; Perplexity returned the AIP-162 / eng-practices URLs already) |
| WebSearch | 0 | (Perplexity covered) |
| Context7 | 0 | (no library API to verify) |
| Read (project-internal) | 6 | Read all 4 review files + BC-INDEX.md (BC-7.x section) + STATE.md for Drift Items table evidence (D3c) |
| Training data | 2 areas | OSS contributor-guide conventions (Kubernetes, Rust RFC, Linux Documentation/) where Perplexity returned irrelevant SEO/exam-question results; worktree-isolation architectural argument for D4 (project-internal CLAUDE.md is primary source) |

**Total MCP tool calls:** 10 (4 reason + 6 search)
**Training data reliance:** MEDIUM — D2's OSS-convention citations are a mix of training data (contributor-guide policies) and Perplexity-returned data (Google eng-practices empirical study). D4's standalone-PR pattern relies partially on training data because Perplexity could not surface the exact kernel/curl/Rust mailing-list patterns requested. D1 and D3 are well-grounded in retrieved sources + project-internal data.

**Inconclusive areas flagged**:
- D3 abandonment-rate quantification (no retrievable numerical study)
- D4 lifecycle-time benchmarks for unexploitable MEDIUM CWE-400 in CLI tools (sparse external data; recommendation grounded in architectural argument instead)
- BC-taxonomy-specific governance literature (none found; AIP-162 used as nearest analog)
