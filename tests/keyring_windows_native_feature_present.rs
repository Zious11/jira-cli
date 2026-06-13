//! Red-Gate tests for S-WIN-3: keyring `windows-native` feature and deny.toml compatibility.
//!
//! These are source-text assertion tests (manifest grepping) that pin two acceptance criteria:
//!
//! - AC-001: `Cargo.toml` lists `windows-native` in the keyring features array.
//! - AC-002: `deny.toml` contains a `[[bans.skip]]` entry for `windows-sys` version `"0.60"`.
//!
//! Pattern mirrors `tests/base_url_release_gate.rs` (source-text assertions).
//! Neither test executes Credential Manager code — they assert manifest content only.
//!
//! # Why these tests exist
//!
//! keyring v3.6.3 with `windows-native` pulls `windows-sys = "0.60"` as a transitive
//! dependency (C-V2(b), `.factory/research/windows-build-f4-preflight-verification.md`).
//! The project's `deny.toml` sets `bans.multiple-versions = "deny"`. The existing skips
//! cover only `windows-sys` 0.45 and 0.61; adding `windows-native` WITHOUT a 0.60 skip
//! causes `cargo deny check` to fail. Both manifest changes MUST land in the same commit.
//!
//! # Test inventory
//!
//! | Test | AC | What it pins |
//! |------|----|--------------|
//! | `test_keyring_has_windows_native_feature` | AC-001 | `"windows-native"` present in keyring features in `Cargo.toml` |
//! | `test_deny_toml_has_windows_sys_0_60_skip` | AC-002 | `[[bans.skip]]` for `windows-sys` version `"0.60"` present in `deny.toml` |

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

/// AC-002 — `deny.toml` contains a `[[bans.skip]]` entry for `windows-sys` version `"0.60"`.
///
/// Confirms the required skip entry is present alongside the existing 0.45 and 0.61 skips.
/// Without this skip, adding `windows-native` to keyring features causes `cargo deny check`
/// to fail: keyring v3.6.3 pulls `windows-sys 0.60`, which is semver-incompatible with
/// the existing 0.45 (jni) and 0.61 (clap/tokio/reqwest) under 0.x semantics
/// (C-V2(b) BLOCKER finding, `.factory/research/windows-build-f4-preflight-verification.md`).
///
/// Strategy: load `deny.toml` at compile time via `include_str!` and verify that both
/// `name = "windows-sys"` and `version = "0.60"` appear in adjacent lines within the file,
/// indicating a `[[bans.skip]]` block for exactly this version exists.
#[test]
fn test_deny_toml_has_windows_sys_0_60_skip() {
    let deny_toml = include_str!("../deny.toml");

    // Locate a `[[bans.skip]]` block that covers windows-sys 0.60.
    // We scan the file in a sliding window: within any 5-line window,
    // both `name = "windows-sys"` and `version = "0.60"` must co-appear.
    // This is structurally equivalent to the existing 0.45 and 0.61 entries.
    let lines: Vec<&str> = deny_toml.lines().collect();

    let found = lines.windows(5).any(|window| {
        let has_windows_sys_name = window.iter().any(|l| l.contains(r#"name = "windows-sys""#));
        let has_version_0_60 = window.iter().any(|l| l.contains(r#"version = "0.60""#));
        has_windows_sys_name && has_version_0_60
    });

    assert!(
        found,
        "S-WIN-3 AC-002 VIOLATION: No `[[bans.skip]]` entry for `windows-sys` version `\"0.60\"` \
         found in deny.toml.\n\
         \n\
         This skip is REQUIRED (not conditional) because keyring v3.6.3 with the `windows-native` \
         feature pulls `windows-sys = \"0.60\"` as a transitive dep (C-V2(b) BLOCKER, research \
         verification file: .factory/research/windows-build-f4-preflight-verification.md).\n\
         \n\
         With `bans.multiple-versions = \"deny\"` active and three semver-incompatible 0.x versions \
         in the graph (0.45 from jni, 0.60 from keyring windows-native, 0.61 from clap/tokio/reqwest), \
         `cargo deny check` will fail until this skip entry is added.\n\
         \n\
         The required entry (add alongside existing 0.45 and 0.61 skips):\n\
         \n\
         [[bans.skip]]\n\
         name = \"windows-sys\"\n\
         version = \"0.60\"\n\
         reason = \"keyring v3.6.3 windows-native feature requires windows-sys 0.60 \
(Win32_Security_Credentials); jni (via rustls-platform-verifier) requires 0.45 and the broad \
graph (clap/tokio/reqwest) requires 0.61. Three semver-incompatible 0.x majors; unification \
blocked upstream until keyring bumps windows-sys.\"\n\
         \n\
         This change MUST be committed in the same commit as the `windows-native` feature \
         addition to Cargo.toml (S-WIN-3 File Structure Requirements).",
    );
}
