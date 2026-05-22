# Issue #398 (PR #399) — Copilot review finding verification

**Finding location:** `src/cli/issue/helpers.rs:65` (UUID pass-through branch of `resolve_team_field`)
**Date:** 2026-05-22
**Verdict:** **REFUTED** — the finding contradicts an explicit, adversary-converged, human-gated spec decision.

---

## 1. The Copilot finding (verbatim)

> "In the UUID pass-through branch, the 3rd tuple element (used as the 'resolved team name'
> for echoes / changed_fields) is set to the raw UUID. That means `jr issue edit/create
> --team <uuid>` will echo the UUID instead of a human-friendly display name, which
> undermines the 'resolved team display name' contract. Consider looking up the UUID in
> the cached/fetched teams list to return the display name when available (and only fall
> back to the UUID when it can't be resolved)."

---

## 2. The spec EXPLICITLY decides this — it is not an oversight

The PRD delta (`.factory/phase-f2-spec-evolution/prd-delta-398.md` §5) contains a
five-row table titled "Third-element (`team_name`) per return path — all five return
paths enumerated". The first row is the exact branch Copilot flags:

| Branch | Condition | `team_id` (element 2) | `team_name` (element 3) |
|--------|-----------|----------------------|------------------------|
| UUID-bypass | `is_team_uuid(team_name)` returns `true` | the raw UUID string | **the raw UUID string (same value; no lookup occurred)** |

The PRD delta §2 "Locked Design Decisions (human-gated)" table also locks:

> | Team echo value | RESOLVED display name — never the UUID or partial-match query |

These two statements are NOT in conflict — the spec resolves the apparent tension
explicitly. The "RESOLVED display name" rule governs the *partial-match* paths (`Exact`,
`ExactMultiple`, `Ambiguous`). The UUID-bypass path is a deliberately enumerated
**carve-out**: when a UUID is supplied, "the resolved value" *is* the UUID, because the
caller bypassed name resolution entirely.

The carve-out is restated in **three** separate BC bodies — each one independently and
deliberately:

- **BC-3.4.012** (`bc-3-issue-write.md:672`): "When `--team` value was passed as a raw
  UUID and the UUID-bypass path fires, `team_name` is the UUID itself (echo of the raw
  value the caller supplied)."
- **BC-3.4.012 Invariant 1** (line 698): "...never a UUID... **(unless the caller
  supplied a raw UUID, in which case the UUID is echoed)**."
- **BC-3.4.012 EC-3.4.012-1** (line 706): a dedicated edge case — "team echo shows the
  UUID (the raw caller-supplied value, since no name resolution occurred)."
- **BC-3.4.013 EC-3.4.013-2** (line 809): "`--team` value was a raw UUID (UUID-bypass
  path) → `changed_fields["team"]` is the UUID (the raw value supplied, since no name
  lookup occurred)."
- **BC-3.4.014** (line 884): "UUID-bypass: when the caller passes a raw UUID, the UUID is
  echoed as-is (no lookup occurred)."
- **BC-3.4.014 Invariant 1 + EC-3.4.014-1** (lines 912, 920): same carve-out, restated.

This carve-out survived **12 rounds of adversarial review** (frontmatter trace lines
31–52) plus a 2026-05-22 human-gate revision that broadened BC-3.4.014. At no round did
any reviewer challenge "UUID-bypass echoes the UUID". It is not an oversight that slipped
through — it is a converged, intentionally-and-repeatedly-affirmed design point.

The dedicated verification point **VP-398-001** exists specifically to pin this boundary
and is even rewritten (round 5, F-1) as a *direct unit-level `is_team_uuid` assertion* —
the spec authors treated the predicate boundary as load-bearing.

---

## 3. Why the UUID-bypass exists — it deliberately pre-empts the cache load

The rustdoc on `is_team_uuid` and the inline comment in `resolve_team_field`
(`helpers.rs:8-13`, `56-65`) state the purpose unambiguously:

> "Used to short-circuit the cache-name-match path for agents that already know a team's
> ID — `--team <uuid>` sends the value straight to the customfield without a cache lookup
> or name match."

> "UUID pass-through: if the caller already has a team UUID (agents, scripts), skip cache
> + name-match entirely. The customfield accepts the UUID directly — no lookup needed.
> **Pre-cache-load so a cold cache doesn't force a teams fetch just to validate an ID we
> already have.**"

The branch `return`s at line 64 **before** step 3 (`crate::cache::read_team_cache`) and
before any possibility of `fetch_and_cache_teams` (a GraphQL org-discovery call + a
`list_teams` API call — see `src/cli/team.rs:54-72`). The entire reason the branch is
positioned where it is (step 2, before step 3) is to guarantee that an agent/script
holding a UUID pays **zero** cache/network cost. Echoing the UUID is the natural,
zero-cost consequence of that design: no lookup happened, so there is no display name to
echo.

---

## 4. Copilot's suggested fix would defeat the bypass's purpose

Copilot proposes: "look up the UUID in the cached/fetched teams list ... fall back to
UUID when unresolvable." Evaluating the two readings:

**(a) Look up in the cache OR fetch → defeats the bypass.**
A full "resolve when available" lookup requires `read_team_cache`, and on a cold/expired
cache (7-day TTL) the only way to "resolve" is `fetch_and_cache_teams` — exactly the
GraphQL + REST round-trip the bypass exists to avoid. This directly contradicts the
rustdoc's stated intent and the PRD delta's "no lookup occurred" language. Rejected.

**(b) Cache-only, no-fetch ("when available" = only if cache already warm).**
This is cheaper, but still **not free and still wrong**:

1. **It still contradicts the spec.** The carve-out is not "echo the UUID *unless* the
   cache happens to be warm" — it is unconditional: "the raw UUID string (same value; no
   lookup occurred)". A cache-warm-dependent display name makes the echo output
   **non-deterministic** — the same `jr issue edit FOO-1 --team <uuid>` command would
   echo `team → My Team` or `team → <uuid>` depending on whether `teams.json` exists on
   disk and is within its 7-day TTL. That is a worse contract than a stable,
   predictable UUID echo. Adversarial review explicitly values determinism here
   (the entire BTreeMap-ordering / snapshot-pinning apparatus in BC-3.4.012/013).
2. **It still costs a disk read** the bypass currently avoids (`read_team_cache` →
   `read_cache` → file open + `serde_json` parse of the full team list). Minor, but the
   bypass's whole contract is "zero lookup".
3. **It reintroduces a deserialization-failure surface.** `read_team_cache` can fail or
   return a stale-format miss (see CLAUDE.md cache-format gotchas). A UUID-bypass that
   currently *cannot* touch the cache would gain a new (cache-miss / parse-error) code
   path for no behavioral benefit on the dominant agent/script use case (where the cache
   may legitimately never be warm).

The `read_team_cache` path (`cache.rs:93`) is a single `read_cache` call — cheap in
absolute terms — but cost is not the deciding factor. The deciding factor is that **the
spec deliberately locked the UUID echo as the stable, lookup-free value**, and
introducing cache-conditional behavior would make a contractually-pinned output
non-deterministic.

---

## 5. CLI-convention note

Copilot frames this as "undermines the resolved-display-name contract". The framing is
inverted: the spec's contract is "echo what resolution produced". For a UUID input,
resolution produced the UUID. Echoing the caller's own input back is the standard CLI
convention for an ID-passthrough path — it confirms *exactly what the tool acted on*
without an extra round-trip or a chance of a lookup disagreeing with the value actually
sent to the API. `resolve_asset` in the same file (`helpers.rs:480`) follows the
identical pattern: a `SCHEMA-NUMBER` key passes through with no API call and the key
itself is what flows downstream. The codebase is internally consistent on this.
(External CLI-precedent search was not required — this is a closed spec-design question.)

---

## 6. Verdict and recommended response

**REFUTED.** The code at `helpers.rs:64` (`return Ok((field_id, team_name.to_string()))`)
is correct and matches the spec exactly. No code change is warranted.

The finding contradicts:
- PRD delta §5 third-element table, row 1 (explicit "the raw UUID string; no lookup occurred").
- PRD delta §2 locked design decisions (the "resolved display name" rule scoped to
  partial-match paths; UUID-bypass is the enumerated carve-out).
- BC-3.4.012 / BC-3.4.013 / BC-3.4.014 bodies, invariants, and edge cases — each
  independently states the UUID-echo carve-out.
- The `is_team_uuid` / `resolve_team_field` rustdoc, which defines the bypass's purpose
  as avoiding any cache/network lookup.

Adopting Copilot's fix would:
- (reading a) reintroduce the exact GraphQL+REST teams fetch the bypass was built to
  skip — a functional regression for the agent/script use case the bypass targets; or
- (reading b) make a contractually-pinned, snapshot-tested echo value non-deterministic
  (cache-warm-dependent), which is a worse contract than the current stable UUID echo.

**Recommended action on the PR:** Resolve the Copilot comment as "by design" with a one-
line pointer: *"UUID-bypass echoing the UUID is an explicit, human-gated spec carve-out —
see prd-delta-398.md §5 third-element table row 1, and BC-3.4.012 EC-3.4.012-1 /
BC-3.4.013 EC-3.4.013-2 / BC-3.4.014 EC-3.4.014-1. The bypass deliberately performs no
cache/network lookup; there is no display name to echo. Changing this would require
reopening the F2 spec."*

No spec change is needed — the spec already covers this exhaustively. If a future product
decision *did* want display-name resolution for UUID inputs, that would be a new F2 delta
(reopening issue #398 scope), not a PR-review fix.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Read | 4 | prd-delta-398.md, helpers.rs, bc-3-issue-write.md (BC bodies), cache.rs region |
| Grep | 2 | locate BC-3.4.012/013/014 + VP-398-001 bodies; locate read_team_cache / fetch_and_cache_teams / CachedTeam |
| Perplexity | 0 | Not required — closed spec-design question, all evidence in-repo |
| Tavily | 0 | Not required |
| Context7 | 0 | Not required |
| WebSearch / WebFetch | 0 | Not required |
| Training data | 1 area | General CLI ID-passthrough convention (§5) — corroborated by in-repo `resolve_asset` precedent, not relied upon as sole basis |

**Total MCP tool calls:** 0
**Training data reliance:** low — the verdict rests entirely on in-repo spec artifacts
(PRD delta + BC bodies + code rustdoc); the one CLI-convention remark is cross-checked
against an existing codebase pattern (`resolve_asset`).
