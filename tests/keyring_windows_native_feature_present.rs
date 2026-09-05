//! Red-Gate tests for S-WIN-3: keyring `windows-native` feature and deny.toml compatibility.
//!
//! These are source-text assertion tests (manifest grepping) that pin three acceptance criteria:
//!
//! - AC-001: `Cargo.toml` lists `windows-native` in the keyring features array.
//! - AC-002: `deny.toml` contains `[[bans.skip]]` blocks for `windows-sys` 0.60,
//!   `windows-targets` 0.53, and at least one `windows_*` arch crate at 0.53.
//!
//! Pattern mirrors `tests/base_url_release_gate.rs` (source-text assertions).
//! Neither test executes Credential Manager code — they assert manifest content only.
//!
//! # Why these tests exist
//!
//! keyring v3.6.3 with `windows-native` pulls `windows-sys = "0.60"` and its transitive
//! arch crates (`windows-targets 0.53.5`, `windows_x86_64_msvc 0.53.1`, etc.) as
//! transitive dependencies (C-V2(b), `.factory/research/windows-build-f4-preflight-verification.md`).
//! The project's `deny.toml` sets `bans.multiple-versions = "deny"`. Without the 0.60 skip
//! AND the transitive 0.53 skips, `cargo deny check` fails. All manifest changes MUST land
//! in the same commit.
//!
//! # How AC-002 assertions work (F-104)
//!
//! Each skip assertion parses `deny.toml` into discrete `[[bans.skip]]` blocks and requires
//! that the target name and version co-appear **within the same block**. This avoids a
//! false-positive that a 5-line sliding-window approach could produce when two adjacent,
//! unrelated `[[bans.skip]]` blocks happen to straddle a window boundary.
//!
//! # Test inventory
//!
//! | Test | AC | What it pins |
//! |------|----|--------------|
//! | `test_keyring_has_windows_native_feature` | AC-001 | `"windows-native"` present in keyring features in `Cargo.toml` |
//! | `test_deny_toml_has_windows_sys_0_60_skip` | AC-002 | `[[bans.skip]]` block for `windows-sys` version `"0.60"` in `deny.toml` |
//! | `test_deny_toml_has_windows_targets_0_53_skip` | AC-002 | `[[bans.skip]]` block for `windows-targets` version `"0.53"` in `deny.toml` |
//! | `test_deny_toml_has_windows_arch_0_53_skip` | AC-002 | at least one `[[bans.skip]]` block for a `windows_*` arch crate at version `"0.53"` in `deny.toml` |
//!
//! # S-cycle4-dpapi-storage-fix additions (AC-020, F3 re-review Finding #2)
//!
//! Two additional manifest source-text assertions, added in the SAME commit
//! as `Cargo.toml`'s new `[target.'cfg(windows)'.dependencies]` section and
//! `deny.toml`'s updated `windows-sys` `"0.60"` `[[bans.skip]]` `reason`
//! field, mirroring this file's existing `parse_bans_skip_blocks`/
//! `block_contains_name_and_version` pattern rather than inventing a second
//! one:
//!
//! | Test | AC | What it pins |
//! |------|----|--------------|
//! | `test_cargo_toml_direct_windows_sys_dependency_pins_same_0_60_version` | AC-020 | the new direct `windows-sys` entry under `[target.'cfg(windows)'.dependencies]` declares version `"0.60"` — the SAME line already present transitively via keyring's `windows-native` feature, never a version bump introduced alongside the promotion (BC-1.4.037 Invariant 3) |
//! | `test_deny_toml_windows_sys_0_60_reason_mentions_dpapi_module` | AC-020 | the existing `windows-sys` `"0.60"` `[[bans.skip]]` block's `reason` field names this module (`auth_windows_store` or "DPAPI"), never left stale naming only the pre-cycle-004 justification |
//!
//! This test does not itself run `cargo tree` (no such invocation exists
//! elsewhere in this test suite's pattern) — it is a manifest-text pin
//! consistent with this file's established convention, not a build-graph
//! diff.

/// Parse `deny.toml` text into a `Vec` of individual `[[bans.skip]]` block texts.
///
/// Each element in the returned `Vec` is the raw text of one `[[bans.skip]]` block,
/// spanning from (and including) the `[[bans.skip]]` header line up to (but not
/// including) the next `[[bans.skip]]` header or end-of-file. Comment lines inside
/// a block are included as-is so that callers can search the full block text.
fn parse_bans_skip_blocks(deny_toml: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Option<String> = None;

    for line in deny_toml.lines() {
        if line.trim() == "[[bans.skip]]" {
            // Flush any previous block.
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(String::from("[[bans.skip]]\n"));
        } else if let Some(ref mut block) = current {
            block.push_str(line);
            block.push('\n');
        }
    }
    // Flush the last block.
    if let Some(block) = current {
        blocks.push(block);
    }
    blocks
}

/// Return `true` if any `[[bans.skip]]` block in `blocks` contains BOTH `name_needle`
/// and `version_needle` (both matched as substrings within the same block).
fn block_contains_name_and_version(
    blocks: &[String],
    name_needle: &str,
    version_needle: &str,
) -> bool {
    blocks
        .iter()
        .any(|block| block.contains(name_needle) && block.contains(version_needle))
}

/// AC-001 — `Cargo.toml` lists `windows-native` in the keyring features array.
///
/// Confirms the keyring dependency declaration includes `"windows-native"` alongside
/// the existing `"apple-native"` and `"linux-native"` features (ADR-0016 §Decision 5b).
///
/// Strategy: load `Cargo.toml` at compile time via `include_str!`, locate the keyring
/// dependency line, and verify `"windows-native"` appears in it.
#[test]
fn test_keyring_has_windows_native_feature() {
    let cargo_toml = include_str!("../Cargo.toml");

    // Find the keyring dependency line(s). It may be a multi-line table entry but the
    // feature list is on a single line (inline array). We look for the line that declares
    // the keyring dep and contains the features key.
    let keyring_line = cargo_toml
        .lines()
        .find(|l| l.contains("keyring") && l.contains("features"))
        .expect(
            "Could not locate a `keyring` dependency line with `features` in Cargo.toml. \
             Has the keyring dep been restructured? Update this test if the format changed.",
        );

    assert!(
        keyring_line.contains("\"windows-native\""),
        "S-WIN-3 AC-001 VIOLATION: `\"windows-native\"` not found in the keyring features \
         declaration in Cargo.toml.\n\
         Expected: `keyring = {{ version = \"3\", features = [..., \"windows-native\"] }}`\n\
         Found line: {keyring_line}\n\
         The `windows-native` feature enables Windows Credential Manager backend via keyring v3.6.3 \
         (ADR-0016 §Decision 5b). All three platform features — apple-native, linux-native, \
         windows-native — must be listed so the Windows release build links correctly.",
    );
}

/// AC-002 (windows-sys 0.60) — `deny.toml` has a `[[bans.skip]]` block for `windows-sys` 0.60.
///
/// Confirms the required skip entry is present alongside the existing 0.45 and 0.61 skips.
/// Without this skip, adding `windows-native` to keyring features causes `cargo deny check`
/// to fail: keyring v3.6.3 pulls `windows-sys 0.60`, which is semver-incompatible with
/// the existing 0.45 (jni) and 0.61 (clap/tokio/reqwest) under 0.x semantics
/// (C-V2(b) BLOCKER finding, `.factory/research/windows-build-f4-preflight-verification.md`).
///
/// Strategy: parse `deny.toml` into discrete `[[bans.skip]]` blocks and require that
/// `name = "windows-sys"` and `version = "0.60"` co-appear **within the same block**.
/// This prevents a false-positive across adjacent unrelated blocks (F-104).
#[test]
fn test_deny_toml_has_windows_sys_0_60_skip() {
    let deny_toml = include_str!("../deny.toml");
    let blocks = parse_bans_skip_blocks(deny_toml);

    let found =
        block_contains_name_and_version(&blocks, r#"name = "windows-sys""#, r#"version = "0.60""#);

    assert!(
        found,
        "S-WIN-3 AC-002 VIOLATION: No `[[bans.skip]]` block for `windows-sys` version `\"0.60\"` \
         found in deny.toml.\n\
         \n\
         This skip is REQUIRED because keyring v3.6.3 with the `windows-native` feature pulls \
         `windows-sys = \"0.60\"` as a transitive dep (C-V2(b) BLOCKER, research verification \
         file: .factory/research/windows-build-f4-preflight-verification.md).\n\
         \n\
         With `bans.multiple-versions = \"deny\"` active and three semver-incompatible 0.x \
         versions in the graph (0.45 from jni, 0.60 from keyring windows-native, 0.61 from \
         clap/tokio/reqwest), `cargo deny check` will fail until this skip entry is added.\n\
         \n\
         The required entry (add alongside existing 0.45 and 0.61 skips):\n\
         \n\
         [[bans.skip]]\n\
         name = \"windows-sys\"\n\
         version = \"0.60\"\n\
         reason = \"...\"\n\
         \n\
         This change MUST be committed in the same commit as the `windows-native` feature \
         addition to Cargo.toml (S-WIN-3 File Structure Requirements).",
    );
}

/// AC-002 (windows-targets 0.53) — `deny.toml` has a `[[bans.skip]]` block for
/// `windows-targets` version `"0.53"`.
///
/// `windows-sys 0.60` (via keyring windows-native) transitively pulls `windows-targets 0.53.5`.
/// Without this skip, `cargo deny check` fails because `windows-targets` appears at three
/// semver-incompatible versions (0.42 from jni, 0.52 from ring, 0.53 from keyring windows-native).
/// cargo-deny requires N-1 versions skipped; 0.42 and 0.53 are skipped, leaving 0.52.6 as
/// the single canonical un-skipped version.
///
/// If this skip is dropped, `cargo deny check` fails but AC-002's windows-sys assertion
/// remains green — this test closes that gap (F-103).
///
/// Strategy: same block-boundary parsing as `test_deny_toml_has_windows_sys_0_60_skip` (F-104).
#[test]
fn test_deny_toml_has_windows_targets_0_53_skip() {
    let deny_toml = include_str!("../deny.toml");
    let blocks = parse_bans_skip_blocks(deny_toml);

    let found = block_contains_name_and_version(
        &blocks,
        r#"name = "windows-targets""#,
        r#"version = "0.53""#,
    );

    assert!(
        found,
        "S-WIN-3 AC-002 VIOLATION: No `[[bans.skip]]` block for `windows-targets` \
         version `\"0.53\"` found in deny.toml.\n\
         \n\
         `windows-sys 0.60` (via keyring v3.6.3 windows-native) transitively pulls \
         `windows-targets 0.53.5`. With three versions in Cargo.lock (0.42 from jni, \
         0.52 from ring, 0.53 from keyring), the 0.42 and 0.53 skips are load-bearing: \
         they leave 0.52.6 as the single canonical un-skipped version. Removing the 0.53 \
         skip causes `cargo deny check` to fail (two un-skipped versions remain: 0.52 + 0.53).\n\
         \n\
         Required entry:\n\
         \n\
         [[bans.skip]]\n\
         name = \"windows-targets\"\n\
         version = \"0.53\"\n\
         reason = \"...\"",
    );
}

/// AC-002 (windows_* arch crates 0.53) — `deny.toml` has at least one `[[bans.skip]]`
/// block for a `windows_*` arch crate at version `"0.53"` (e.g. `windows_x86_64_msvc`).
///
/// `windows-targets 0.53.5` (via keyring windows-native) pulls per-arch stub crates such as
/// `windows_x86_64_msvc 0.53.1`, `windows_aarch64_msvc 0.53.1`, etc. Without these skips,
/// `cargo deny check` fails for each arch crate individually. This test asserts the
/// most commonly-built arch crate (`windows_x86_64_msvc`) is covered, which is sufficient
/// to confirm the 0.53 arch-crate skip pattern is in place.
///
/// Strategy: same block-boundary parsing as the other AC-002 tests (F-104).
#[test]
fn test_deny_toml_has_windows_arch_0_53_skip() {
    let deny_toml = include_str!("../deny.toml");
    let blocks = parse_bans_skip_blocks(deny_toml);

    // Check that windows_x86_64_msvc 0.53 (the canonical x64 MSVC arch crate) is skipped.
    // If the 0.53 arch-crate skip pattern is present, this representative crate will be covered.
    let found = block_contains_name_and_version(
        &blocks,
        r#"name = "windows_x86_64_msvc""#,
        r#"version = "0.53""#,
    );

    assert!(
        found,
        "S-WIN-3 AC-002 VIOLATION: No `[[bans.skip]]` block for `windows_x86_64_msvc` \
         version `\"0.53\"` found in deny.toml.\n\
         \n\
         `windows-targets 0.53.5` (via keyring v3.6.3 windows-native) pulls per-arch stub \
         crates including `windows_x86_64_msvc 0.53.1`. Each arch crate appears at three \
         versions (0.42 from jni, 0.52 from ring, 0.53 from keyring); the 0.42 and 0.53 \
         skips are load-bearing. Removing them causes `cargo deny check` to fail for each \
         arch crate individually.\n\
         \n\
         Required entry (representative — all arch crates need equivalent skips):\n\
         \n\
         [[bans.skip]]\n\
         name = \"windows_x86_64_msvc\"\n\
         version = \"0.53\"\n\
         reason = \"...\"",
    );
}

/// AC-020 (F3 re-review Finding #2) — the new direct `windows-sys` entry
/// under `[target.'cfg(windows)'.dependencies]` in `Cargo.toml` declares the
/// SAME `"0.60"` version line already present transitively via keyring's
/// `windows-native` feature — never a version bump introduced alongside the
/// promotion (BC-1.4.037 Invariant 3: "zero new dependency-graph nodes").
/// Promoting an already-present transitive dependency to direct, at the
/// SAME version, adds no new crate/version pair to the resolved dependency
/// graph; a different version here would.
///
/// Strategy: locate the `[target.'cfg(windows)'.dependencies]` section
/// header, then the `windows-sys` dependency line within a bounded window
/// after it, and assert that line pins version `"0.60"` (matching the
/// existing `deny.toml` `windows-sys` `"0.60"` `[[bans.skip]]` entry's
/// version, asserted by `test_deny_toml_has_windows_sys_0_60_skip` above).
///
/// **Not RED as of this Red Gate commit — a documented exception, not a
/// gap.** `Cargo.toml`'s `[target.'cfg(windows)'.dependencies]` section
/// (Task 18) was already landed in the S-cycle4-dpapi-storage-fix stub
/// commit (51255e2a), before this test file existed — there is no runtime
/// code body for a pure manifest-text assertion to "turn green" at the TDD
/// Green step. This test PASSES today; it exists as durable regression
/// protection against a future accidental version bump on this line, per
/// AC-020's own text ("a manifest source-text assertion test... pins...
/// the declared windows-sys version... is the SAME 0.60.x line").
#[test]
fn test_cargo_toml_direct_windows_sys_dependency_pins_same_0_60_version() {
    let cargo_toml = include_str!("../Cargo.toml");

    let section_start = cargo_toml
        .find("[target.'cfg(windows)'.dependencies]")
        .expect(
            "S-cycle4-dpapi-storage-fix AC-020 VIOLATION: Cargo.toml has no \
             `[target.'cfg(windows)'.dependencies]` section. This section must promote \
             `windows-sys` to a direct dependency at the same 0.60.x version already pulled \
             transitively via keyring's `windows-native` feature (ADR-0021 §5, BC-1.4.037 \
             Invariant 3).",
        );

    // Bound the search to the lines immediately following the section
    // header so a later, unrelated `windows-sys` reference elsewhere in the
    // file (there is none today, but this keeps the assertion anchored)
    // cannot satisfy it.
    let window_end = (section_start + 800).min(cargo_toml.len());
    let section_window = &cargo_toml[section_start..window_end];

    let windows_sys_line = section_window
        .lines()
        .find(|l| l.contains("windows-sys"))
        .expect(
            "S-cycle4-dpapi-storage-fix AC-020 VIOLATION: no `windows-sys` dependency line \
             found within the `[target.'cfg(windows)'.dependencies]` section of Cargo.toml.",
        );

    assert!(
        windows_sys_line.contains("\"0.60\""),
        "S-cycle4-dpapi-storage-fix AC-020 VIOLATION (BC-1.4.037 Invariant 3, \"zero new \
         dependency-graph nodes\"): the direct `windows-sys` entry under \
         `[target.'cfg(windows)'.dependencies]` must pin version \"0.60\" — the SAME version \
         already pulled transitively via keyring's `windows-native` feature (see the \
         `deny.toml` `windows-sys` \"0.60\" `[[bans.skip]]` entry). A different version here \
         would introduce a NEW dependency-graph node, contradicting the promotion's own \
         zero-new-nodes justification.\n\
         Found line: {windows_sys_line}",
    );
}

/// AC-020 (F3 re-review Finding #2) — `deny.toml`'s existing `windows-sys`
/// `"0.60"` `[[bans.skip]]` block's `reason` field, re-read after the
/// `auth_windows_store` DPAPI change, contains a reference to this module
/// (`auth_windows_store` or "DPAPI") alongside its existing `keyring`/
/// `windows-native` wording — never left stale, naming only the
/// pre-cycle-004 justification.
///
/// Strategy: same block-boundary parsing as the other tests in this file
/// (`parse_bans_skip_blocks`/`block_contains_name_and_version`-style
/// pattern) — locate the `windows-sys` `"0.60"` block specifically, then
/// assert its text contains either `"auth_windows_store"` or `"DPAPI"`.
///
/// **Not RED as of this Red Gate commit — a documented exception, not a
/// gap** (same rationale as the Cargo.toml test above): `deny.toml`'s
/// `windows-sys` `"0.60"` `reason` field (Task 18) was already updated in
/// the stub commit to name this module's DPAPI usage. This test PASSES
/// today; it exists as durable regression protection against a future edit
/// silently reverting the reason field to its pre-cycle-004 wording.
#[test]
fn test_deny_toml_windows_sys_0_60_reason_mentions_dpapi_module() {
    let deny_toml = include_str!("../deny.toml");
    let blocks = parse_bans_skip_blocks(deny_toml);

    let target_block = blocks
        .iter()
        .find(|b| b.contains(r#"name = "windows-sys""#) && b.contains(r#"version = "0.60""#))
        .expect(
            "S-cycle4-dpapi-storage-fix AC-020 VIOLATION: could not locate the `windows-sys` \
             \"0.60\" [[bans.skip]] block in deny.toml (asserted present by \
             test_deny_toml_has_windows_sys_0_60_skip above).",
        );

    let mentions_dpapi_module =
        target_block.contains("auth_windows_store") || target_block.contains("DPAPI");

    assert!(
        mentions_dpapi_module,
        "S-cycle4-dpapi-storage-fix AC-020 VIOLATION (BC-1.4.037 Invariant 3): the `windows-sys` \
         \"0.60\" [[bans.skip]] block's `reason` field must name this module's DPAPI usage \
         (`auth_windows_store` or \"DPAPI\") alongside its existing keyring/windows-native \
         wording — it must never be left stale, naming only the pre-cycle-004 justification.\n\
         Block text:\n{target_block}",
    );
}
