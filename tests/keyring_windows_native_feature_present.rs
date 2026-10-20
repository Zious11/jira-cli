//! Red-Gate tests for S-WIN-3: keyring `windows-native` feature and deny.toml compatibility.
//!
//! These are manifest assertion tests (structural TOML parsing) that pin these acceptance criteria:
//!
//! - AC-001: `Cargo.toml` lists `windows-native` in the keyring features array.
//! - AC-002: `deny.toml` contains `[[bans.skip]]` blocks for `windows-sys` 0.60,
//!   `windows-targets` 0.53, and at least one `windows_*` arch crate at 0.53.
//!
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
//! # How the assertions work
//!
//! Every test parses `Cargo.toml` / `deny.toml` with the `toml` crate and inspects the
//! real declarations (the keyring dependency's `features` array, the `[[bans.skip]]`
//! array-of-tables entries by exact `name` + `version`). Comments can never satisfy or
//! break them.
//!
//! # Test inventory
//!
//! | Test | AC | What it pins |
//! |------|----|--------------|
//! | `test_keyring_has_windows_native_feature` | AC-001 | `"windows-native"` in the parsed keyring `features` array in `Cargo.toml` |
//! | `test_deny_toml_has_windows_sys_0_60_skip` | AC-002 | `[[bans.skip]]` entry `windows-sys` `"0.60"` in `deny.toml` |
//! | `test_deny_toml_has_windows_targets_0_53_skip` | AC-002 | `[[bans.skip]]` entry `windows-targets` `"0.53"` in `deny.toml` |
//! | `test_deny_toml_has_windows_arch_0_53_skip` | AC-002 | `[[bans.skip]]` entry `windows_x86_64_msvc` `"0.53"` in `deny.toml` |
//!
//! # S-cycle4-dpapi-storage-fix additions (AC-020), retargeted to 0.61 by #838
//!
//! jr's own DPAPI wrapper (`src/api/auth_windows_store.rs`, ADR-0021) declares
//! `windows-sys` as a direct `[target.'cfg(windows)'.dependencies]` entry. It was
//! declared at `"0.60"` in #759 and deliberately bumped to `"0.61"` in #838. These
//! tests parse the manifests structurally (the `toml` crate), so comments can never
//! satisfy or break them:
//!
//! | Test | What it pins |
//! |------|--------------|
//! | `test_cargo_toml_direct_windows_sys_declares_0_61_with_dpapi_features` | the real `windows-sys` key in the windows-target dependency table requires `"0.61"` and enables `Win32_Security_Cryptography` |
//! | `test_cargo_lock_contains_direct_windows_sys_version_already_in_graph` | `Cargo.lock` already holds a `windows-sys` 0.61.x package (the "zero new graph nodes" claim) |
//! | `test_deny_toml_has_windows_sys_0_61_skip` | `deny.toml` has a `[[bans.skip]]` entry for `windows-sys` `"0.61"` |
//!
//! Manifest-text pins only; no `cargo tree` invocation.

/// Parse a repo manifest embedded at compile time into a TOML table.
fn parse_manifest(name: &str, text: &str) -> toml::Table {
    text.parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("{name} must parse as TOML: {e}"))
}

/// Return `true` if `deny.toml` has a `[[bans.skip]]` entry with exactly this `name` and `version`.
fn deny_has_skip(name: &str, version: &str) -> bool {
    let deny = parse_manifest("deny.toml", include_str!("../deny.toml"));
    deny.get("bans")
        .and_then(|b| b.get("skip"))
        .and_then(|s| s.as_array())
        .expect("deny.toml must have [[bans.skip]] entries")
        .iter()
        .any(|s| {
            s.get("name").and_then(|n| n.as_str()) == Some(name)
                && s.get("version").and_then(|v| v.as_str()) == Some(version)
        })
}

/// AC-001 -- the `keyring` dependency's parsed `features` array in `Cargo.toml` contains
/// `"windows-native"` (Windows Credential Manager backend, ADR-0016 Decision 5b), alongside
/// the apple/linux backends.
#[test]
fn test_keyring_has_windows_native_feature() {
    let manifest = parse_manifest("Cargo.toml", include_str!("../Cargo.toml"));
    let features: Vec<&str> = manifest
        .get("dependencies")
        .and_then(|d| d.get("keyring"))
        .and_then(|k| k.get("features"))
        .and_then(|f| f.as_array())
        .expect("Cargo.toml must declare keyring under [dependencies] with a `features` array")
        .iter()
        .filter_map(|f| f.as_str())
        .collect();
    assert!(
        features.contains(&"windows-native"),
        "keyring features must include \"windows-native\" so the Windows release build links; got {features:?}"
    );
}

/// AC-002 -- `deny.toml` skips `windows-sys` 0.60 (keyring `windows-native` pulls it; it is
/// semver-incompatible with 0.45 from jni and 0.61 from the broad graph, and
/// `bans.multiple-versions = "deny"` would otherwise fail `cargo deny check`).
#[test]
fn test_deny_toml_has_windows_sys_0_60_skip() {
    assert!(
        deny_has_skip("windows-sys", "0.60"),
        "deny.toml must have a [[bans.skip]] entry for windows-sys version \"0.60\""
    );
}

/// AC-002 -- `deny.toml` skips `windows-targets` 0.53. windows-sys 0.60 (keyring) pulls
/// windows-targets 0.53.5 while windows-sys 0.45 (jni) pulls 0.42.2; those two are the
/// in-graph versions (0.52.6 is Cargo.lock-only and filtered out of cargo-deny's graph), so
/// the 0.53 skip is needed alongside the 0.42 skip. Dropping it fails `cargo deny check`
/// even though the windows-sys skip assertion would stay green.
#[test]
fn test_deny_toml_has_windows_targets_0_53_skip() {
    assert!(
        deny_has_skip("windows-targets", "0.53"),
        "deny.toml must have a [[bans.skip]] entry for windows-targets version \"0.53\""
    );
}

/// AC-002 -- `deny.toml` skips the representative arch crate `windows_x86_64_msvc` at 0.53
/// (windows-targets 0.53.5 pulls the per-arch crates at 0.53.1; windows-targets 0.42.2 via
/// jni pulls them at 0.42.2). All arch crates need equivalent skips; this one stands in
/// for the pattern.
#[test]
fn test_deny_toml_has_windows_arch_0_53_skip() {
    assert!(
        deny_has_skip("windows_x86_64_msvc", "0.53"),
        "deny.toml must have a [[bans.skip]] entry for windows_x86_64_msvc version \"0.53\""
    );
}

/// The direct `windows-sys` dependency (promoted by ADR-0021 / #759, bumped to 0.61 by
/// #838) must require `"0.61"` and enable the DPAPI feature
/// (`Win32_Security_Cryptography`, for `CryptProtectData`/`CryptUnprotectData`).
/// Asserted against the parsed `[target.'cfg(windows)'.dependencies]` table, so comments
/// cannot satisfy it.
#[test]
fn test_cargo_toml_direct_windows_sys_declares_0_61_with_dpapi_features() {
    let manifest = parse_manifest("Cargo.toml", include_str!("../Cargo.toml"));

    let dep = manifest
        .get("target")
        .and_then(|t| t.get("cfg(windows)"))
        .and_then(|t| t.get("dependencies"))
        .and_then(|t| t.get("windows-sys"))
        .expect("Cargo.toml must declare windows-sys under [target.'cfg(windows)'.dependencies]");

    let version = dep
        .get("version")
        .and_then(|v| v.as_str())
        .expect("windows-sys direct dependency must have a string `version`");
    assert_eq!(
        version, "0.61",
        "direct windows-sys requirement must be \"0.61\" (bumped from 0.60 in #838)"
    );

    let features: Vec<&str> = dep
        .get("features")
        .and_then(|f| f.as_array())
        .expect("windows-sys direct dependency must list `features`")
        .iter()
        .filter_map(|f| f.as_str())
        .collect();
    assert!(
        features.contains(&"Win32_Security_Cryptography"),
        "windows-sys must enable Win32_Security_Cryptography for the DPAPI wrapper; got {features:?}"
    );
}

/// The direct `windows-sys` 0.61 requirement must resolve to a package already in the
/// lockfile graph, keeping the "zero new dependency-graph nodes" claim (ADR-0021 §5) true.
#[test]
fn test_cargo_lock_contains_direct_windows_sys_version_already_in_graph() {
    let lock = parse_manifest("Cargo.lock", include_str!("../Cargo.lock"));
    let packages = lock
        .get("package")
        .and_then(|p| p.as_array())
        .expect("Cargo.lock must have [[package]] entries");

    let has_0_61 = packages.iter().any(|p| {
        p.get("name").and_then(|n| n.as_str()) == Some("windows-sys")
            && p.get("version")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v.starts_with("0.61."))
    });
    assert!(
        has_0_61,
        "Cargo.lock must contain a windows-sys 0.61.x package"
    );
}

/// `deny.toml` must keep a `[[bans.skip]]` entry for `windows-sys` `"0.61"` (the version the
/// broad graph and jr's direct DPAPI dependency use); with `multiple-versions = "deny"`,
/// dropping it fails `cargo deny check`. Structural check only; the `reason` prose is
/// intentionally not asserted.
#[test]
fn test_deny_toml_has_windows_sys_0_61_skip() {
    assert!(
        deny_has_skip("windows-sys", "0.61"),
        "deny.toml must have a [[bans.skip]] entry for windows-sys version \"0.61\""
    );
}
