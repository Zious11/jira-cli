# Pass 4 Deepening — Round 3: Resolution of NFR-R-E vs NEW-INV-229 Contradiction

**Mandate:** Resolve the contradiction between Pass 2 R5 (NEW-INV-229: bug confirmed) and Pass 4 R2 (NFR-R-E demoted/deferred: bug retracted). Verify at source.

---

## §1: Resolution of NFR-R-E

**Verdict: BUG-AT-DIFFERENT-SITE — Pass 2 R5 was RIGHT, Pass 4 R2 was WRONG.**

The bug exists exactly where Pass 2 R5 said it did: `src/cli/issue/list.rs` lines 395–463. Pass 4 R2 looked at the wrong file: `api/assets/linked.rs::enrich_assets` (lines 170–225). That function uses **index-based** distribution (`Vec<usize>` of indices, results returned as `(idx, result)` tuples — line 211, 218–224). It is correct. But it is NOT the only enrichment path.

The `cli/issue/list.rs::handle_list` enrichment path is an **independent re-implementation** of the same logic that takes a different (buggy) shortcut.

**Evidence (file:lines):**
- `cli/issue/list.rs:398` — `to_enrich: StdHashMap<(String, String), ()>` keyed by `(wid, oid)` ✓ (Pass 2 R5 correct)
- `cli/issue/list.rs:440` — async block returns `(oid, result)` — **drops `wid`**
- `cli/issue/list.rs:446` — `resolved: StdHashMap<String, ...>` keyed by `oid` alone ✓ (Pass 2 R5 correct)
- `cli/issue/list.rs:449` — `resolved.insert(oid, ...)` — last-write-wins across workspaces ✓ (Pass 2 R5 correct)
- `cli/issue/list.rs:456` — `resolved.get(oid)` — cannot disambiguate by workspace ✓ (Pass 2 R5 correct)

Pass 2 R5's cited line numbers (406, 446, 449) are accurate to within the structural tuple boundary (406 is the `let key = (wid, oid);` construction).

---

## §2: Asset Enrichment Topology — Definitive Characterization

There are **TWO distinct enrichment paths**:

### Path A: `api/assets/linked.rs::enrich_assets` (CORRECT)
- Input: `&mut [LinkedAsset]` (a single flat slice)
- Step 1 (line 172–177): `needs_enrichment: Vec<usize>` — indices into the slice
- Step 2 (line 199–214): futures map `&idx` → async returns `(idx, result)` tuple
- Step 3 (line 218–224): `for (idx, result) in results` — assigns back by **index**, not by `oid`
- **No HashMap keyed by `oid`. No mis-attribution risk.**

### Path B: `cli/issue/list.rs::handle_list` (BUGGY)
- Input: `Vec<Vec<LinkedAsset>>` (per-issue assets) — line 401: `issue_assets`
- Step 1 (line 398): `to_enrich: HashMap<(wid, oid), ()>` — dedup key
- Step 2 (line 399): `enrich_indices: Vec<(usize, usize)>` — (issue_idx, asset_idx) — **stored but only used for write-back, not for result distribution**
- Step 3 (line 429–443): futures iterate `to_enrich.keys()`, async returns `(oid, result)` — **wid dropped at line 440**
- Step 4 (line 446–451): `resolved: HashMap<String, (key, label, type)>` keyed by **oid only**
- Step 5 (line 454–462): write-back uses `resolved.get(oid)` — last writer for any duplicate oid across workspaces wins for ALL issues holding that oid.

**Mis-attribution scenario (concrete):** Two workspaces W1 and W2 both contain object_id=`88`. W1's object 88 = "Acme Corp". W2's object 88 = "Globex Inc". After `join_all`, `resolved["88"]` = whichever future completed last (non-deterministic — `join_all` preserves submission order, but iteration over `HashMap::keys()` at line 430 is non-deterministic). Write-back applies that single winner to BOTH issues.

---

## §3: cli/issue/list.rs Function-Level Audit (lines 395–463)

| Line | Operation | Type | Risk |
|---|---|---|---|
| 398 | `to_enrich` HashMap allocation | `HashMap<(String,String), ()>` | OK — composite key |
| 399 | `enrich_indices` Vec allocation | `Vec<(usize, usize)>` | OK |
| 401–411 | Build dedup set + indices list | — | OK — captures (wid, oid) correctly |
| 406 | `let key = (wid, oid);` | tuple | OK |
| 429 | `futures` from `to_enrich.keys()` | iterator | non-deterministic order |
| 432–436 | wid empty-check + fallback | — | OK |
| 440 | `(oid, result)` returned | **drops wid** | **BUG ROOT CAUSE** |
| 446 | `resolved: HashMap<String, ...>` | **oid-only key** | **BUG** |
| 449 | `resolved.insert(oid, ...)` | last-write-wins | **BUG** — collision lossy |
| 456 | `resolved.get(oid)` | oid lookup | **BUG** — wid context lost |

**Single-line fix:** change line 440 to `((wid, oid), result)`, line 446 key to `(String, String)`, line 449 insert key, line 456 lookup key. Plus carry `wid` into the async closure (already present at line 432).

---

## §4: Audit Log

### Correction to Pass 4 R2 (NFR-R-E demotion)

**Original Pass 4 R2 claim:** "Re-reading `api/assets/linked.rs::enrich_assets` (lines 213–237) shows the function uses indexed result distribution (Vec aligned by source order, NOT keyed by object_id). The bug does not exist at this site."

**Correction:** The claim is TRUE for `api/assets/linked.rs::enrich_assets`. But Pass 4 R2 stopped there and concluded the bug doesn't exist anywhere. **Pass 4 R2 audited the wrong site.** The bug Pass 2 R5 reported is in `cli/issue/list.rs`, an independent enrichment path that does NOT reuse `enrich_assets()`.

**Action:** Re-promote NFR-R-E to **HIGH severity**. Mark Pass 4 R2 demotion as `CONV-RETRACTED-R3`.

### Pass 2 R5 (NEW-INV-229) confirmed

All cited line numbers (406, 446, 449) match source. The invariant violation is real. NEW-INV-229 stands as filed.

---

## §5: Updated NFR Severity Matrix — NFR-R-E

| Field | Value |
|---|---|
| ID | NFR-R-E |
| Name | Asset enrichment workspace mis-attribution |
| Category | Reliability / Correctness |
| Site | `src/cli/issue/list.rs:395–463` (`handle_list` asset enrichment block) |
| NOT at | `src/api/assets/linked.rs:170–225` (`enrich_assets` is correct) |
| Severity | **HIGH** (re-promoted from DEFERRED) |
| Trigger | Same `objectId` exists across multiple Assets workspaces in a single `issue list --assets` result set |
| Impact | Display data corruption: wrong asset name/key/type shown on issues; non-deterministic which workspace wins (HashMap key iteration order) |
| Detection | Multi-workspace org with overlapping object IDs running `jr issue list --assets` |
| Fix complexity | Trivial — change result key from `String` to `(String, String)` at 4 sites (lines 440, 446, 449, 456) |
| Confidence | HIGH — verified at source, all line numbers match |
| Test gap | No integration test covers multi-workspace overlapping `objectId` (would require dual workspace fixtures) |

---

## §6: State Checkpoint

```yaml
pass: 4
round: 3
status: complete
mandate: resolve_NEW-INV-229_vs_NFR-R-E_demotion
verdict: BUG-AT-DIFFERENT-SITE
correct_round: pass-2-r5
incorrect_round: pass-4-r2
bug_site: src/cli/issue/list.rs:395-463
bug_not_at: src/api/assets/linked.rs:170-225
nfr_r_e_severity: HIGH (re-promoted)
new_inv_229_status: confirmed
novelty: SUBSTANTIVE
timestamp: 2026-05-04T00:00:00Z
next_round_needed: false (NFR-R-E resolved; remaining NFR work is convergence, not contradiction)
```

## Novelty Assessment

**Novelty: SUBSTANTIVE**

This round resolves a direct contradiction between two prior rounds, retracts an incorrect demotion (Pass 4 R2), re-promotes a HIGH-severity correctness defect, and pinpoints two independent enrichment paths where prior rounds conflated them as one. Removing this round's findings would leave the spec with a phantom-retracted real bug — directly model-changing.

## Convergence Declaration

Pass 4 R3 has produced a SUBSTANTIVE finding (contradiction resolved, NFR re-promoted). NFR-R-E is now closed at HIGH severity with site precisely localized. Remaining Pass 4 work is general convergence, not contradiction resolution — likely R4 will be NITPICK unless other latent contradictions surface.
