//! Windows DPAPI-encrypted-file fallback for OAuth tokens that exceed
//! Windows Credential Manager's ~2560-byte blob-size ceiling (issue #759).
//!
//! # Design (ADR-0021)
//!
//! `store_oauth_tokens` (`src/api/auth.rs`) writes the OAuth access/refresh
//! pair to the system keyring first, unconditionally, on every platform.
//! Only when a `set_password` call returns `keyring::Error::TooLong` does it
//! route the WHOLE pair (never split across backends) to this module's
//! `store_pair`, which persists it as a DPAPI-encrypted file under
//! `%LOCALAPPDATA%\jr\secrets\<profile>\oauth-tokens.dat`. `load_oauth_tokens`
//! and the `clear_profile_*` functions gain matching read/remove branches.
//!
//! This module is split along a pure/impure seam so the routing predicate,
//! the on-disk envelope framing, and the path-traversal guard are all
//! unit-testable on any OS/CI runner — only the two DPAPI FFI calls
//! (`dpapi::protect`/`dpapi::unprotect`) are Windows-only and require a real
//! Windows target to execute.
//!
//! See `ADR-0021-windows-oauth-secret-storage-dpapi-fallback.md` for the
//! full accepted design, including the adversarial-review corrections this
//! module's shape reflects (stale-keyring-shadow closure, delete-before-store
//! ordering, age-gated temp-file cleanup, the host-independent path-guard
//! recognizer, and the `CRYPTPROTECT_UI_FORBIDDEN`/USER-scope-only DPAPI flag
//! decision).
//!
//! # STUB NOTICE (S-cycle4-dpapi-storage-fix, Two-Step Red Gate, BC-5.38.001)
//!
//! Every non-trivial function body in this module is `todo!()`. Real logic
//! lands in this story's TDD implementation step, after test-writer's
//! failing tests exist. Signatures, module structure, and the cfg(windows) /
//! cfg(not(windows)) split are final per ADR-0021 — only the bodies are
//! deferred.

use crate::profile::Profile;
use std::path::PathBuf;

/// Pure, cross-platform-testable envelope encode/decode + on-disk framing
/// (ADR-0021 §3). No I/O — operates entirely on in-memory byte buffers, so
/// it is unit-testable on any OS/CI runner.
pub(crate) mod envelope {
    /// JSON-serialize `{version, access, refresh}` to plaintext bytes.
    pub fn encode(access: &str, refresh: &str) -> Vec<u8> {
        let _ = (access, refresh);
        todo!("BC-1.4.037 postcondition 1 — implemented in the TDD Green step")
    }

    /// Parse plaintext bytes back to `(access, refresh)`. A structurally
    /// malformed payload (bad JSON, missing field) must be a distinct
    /// `Err`, never silently coerced into an absent/empty pair.
    pub fn decode(bytes: &[u8]) -> anyhow::Result<(String, String)> {
        let _ = bytes;
        todo!("BC-1.4.037 postcondition 1 — implemented in the TDD Green step")
    }

    /// Prepend the 4-byte magic (`b"JROD"`) + 1-byte outer version to the
    /// DPAPI-protected ciphertext, producing the on-disk file contents.
    pub fn wrap(protected: Vec<u8>) -> Vec<u8> {
        let _ = protected;
        todo!("BC-1.4.037 postcondition 2 — implemented in the TDD Green step")
    }

    /// Validate the 5-byte header and return the remaining protected bytes.
    /// An unrecognized magic/version must be a distinct `Err`, never
    /// silently coerced into "no token."
    pub fn unwrap(file_bytes: &[u8]) -> anyhow::Result<&[u8]> {
        let _ = file_bytes;
        todo!("BC-1.4.037 postcondition 2 — implemented in the TDD Green step")
    }
}

/// Pure routing predicate (ADR-0021 §1): true IFF `err` is
/// `keyring::Error::TooLong(_, _)`. Never a hardcoded byte-budget guess —
/// routing is driven solely by `keyring`'s own typed error variant
/// (BC-1.4.035 Invariant 2). This function itself stays OS-agnostic and
/// unit-testable on any CI runner; its call site (`engage_dpapi_fallback`,
/// `src/api/auth.rs`) is what gates DPAPI engagement to `#[cfg(windows)]`
/// in production (BC-1.4.035 Invariant 3).
pub(crate) fn should_fallback_to_dpapi(err: &keyring::Error) -> bool {
    let _ = err;
    todo!("BC-1.4.035 invariant 2 — implemented in the TDD Green step")
}

/// Host-independent guard for a profile-derived path COMPONENT (never a
/// full path) about to be joined as `secrets/<profile>/oauth-tokens.dat`
/// (ADR-0021 §9, BC-1.4.040). Deliberately does NOT use
/// `std::path::Path`/`Component` — those parse path syntax according to the
/// compilation/runtime target's OS conventions, which would make this
/// guard's behavior (and test suite) depend on which OS `cargo test`
/// happens to run on. This is defense-in-depth behind the PRIMARY, live
/// `validate_profile_name` gate (BC-6.1.004/BC-6.1.005, `src/config.rs`).
pub(crate) fn reject_unsafe_profile_component(profile: &str) -> Result<(), ProfilePathEscape> {
    let _ = profile;
    todo!("BC-1.4.040 postconditions 1-7 — implemented in the TDD Green step")
}

/// Case-insensitive match against the 30-name Windows reserved device-name
/// set (ADR-0021 §9), evaluated against the profile's leading-space-trimmed
/// stem (the part before the first `.`, if any).
fn is_reserved_windows_device_name(profile: &str) -> bool {
    let _ = profile;
    todo!("BC-1.4.040 postconditions 5-7 — implemented in the TDD Green step")
}

/// Resolve the on-disk path for a profile's DPAPI-encrypted secret file.
/// On EVERY platform, the first statement invokes
/// `reject_unsafe_profile_component` (Architecture Compliance Rule; AC-018)
/// — this is what makes the guard's wiring exercised, and
/// regression-catchable, on an ordinary Linux/macOS CI runner, not only on a
/// real Windows machine.
fn file_path(profile: &Profile) -> Result<PathBuf, ProfilePathEscape> {
    let _ = profile;
    todo!("ADR-0021 §3/§9 — implemented in the TDD Green step")
}

/// Windows-only, thin `unsafe` FFI wrapper around `CryptProtectData`/
/// `CryptUnprotectData` (ADR-0021 §5/§8). This is the SOLE `unsafe` code in
/// this module tree (CLAUDE.md: "No unsafe code without explicit
/// justification in a comment") — two functions, each doing exactly: build
/// a `DATA_BLOB` from a Rust slice, call the FFI function, copy the output
/// `DATA_BLOB` into an owned `Vec<u8>`, free the output buffer via
/// `LocalFree`. `dwFlags` must set `CRYPTPROTECT_UI_FORBIDDEN` and must
/// NEVER set `CRYPTPROTECT_LOCAL_MACHINE` (USER scope only, ADR-0021 §8) —
/// this is a compiled-in invariant asserted by AC-014, not a runtime choice.
#[cfg(windows)]
mod dpapi {
    /// DPAPI-protect `plaintext` (USER scope, UI-forbidden).
    pub fn protect(plaintext: &[u8]) -> std::io::Result<Vec<u8>> {
        let _ = plaintext;
        todo!("BC-1.4.037 postcondition 4 — implemented in the TDD Green step (Windows-only)")
    }

    /// Inverse of [`protect`].
    pub fn unprotect(blob: &[u8]) -> std::io::Result<Vec<u8>> {
        let _ = blob;
        todo!("BC-1.4.037 postcondition 4 — implemented in the TDD Green step (Windows-only)")
    }
}

/// Atomic pair write: encode -> DPAPI-protect -> wrap -> temp-write(fsync)
/// -> rename, with age-gated `*.tmp-*` cleanup (ADR-0021 §3, AC-013). On
/// EVERY platform, the first statement is `file_path(profile)?` (ADR-0021
/// §9) — this invokes `reject_unsafe_profile_component` and is what makes
/// the guard's wiring at this entry point exercised on an ordinary
/// Linux/macOS CI runner (AC-018), not only on a real Windows machine.
/// `#[cfg(windows)]`: once the guard passes, the real DPAPI-protect/wrap/
/// temp-write/rename implementation runs. `#[cfg(not(windows))]`: once the
/// guard passes, always returns the honest-fail error immediately (DPAPI is
/// categorically unavailable; this path exists only so the cross-platform
/// call site in `store_oauth_tokens` compiles uniformly).
pub fn store_pair(profile: &Profile, access: &str, refresh: &str) -> anyhow::Result<()> {
    let _ = (profile, access, refresh);
    todo!("BC-1.4.035/BC-1.4.037 — implemented in the TDD Green step")
}

/// Load a pair from the DPAPI-encrypted file, if present. On EVERY
/// platform, the first statement is `file_path(profile)?` (ADR-0021 §9).
/// `#[cfg(not(windows))]`: once the guard passes, always returns `Ok(None)`.
/// `#[cfg(windows)]`: once the guard passes, `Ok(None)` if no file exists;
/// `Err` on a file that exists but could not be turned into a usable pair —
/// see [`CorruptSecretFile`] for the typed discrimination between "this
/// content is corrupt" and "this is a genuine backend/IO error," which the
/// caller (`load_oauth_tokens`, `src/api/auth.rs`) must never silently
/// coerce into "no token."
pub fn load_pair(profile: &Profile) -> anyhow::Result<Option<(String, String)>> {
    let _ = profile;
    todo!("BC-1.4.036 — implemented in the TDD Green step")
}

/// Remove the DPAPI file for `profile` if present. On EVERY platform, the
/// first statement is `file_path(profile)?` (ADR-0021 §9).
/// `#[cfg(not(windows))]`: once the guard passes, always returns `Ok(())`
/// immediately — no filesystem call is made. `#[cfg(windows)]`: once the
/// guard passes, delete the file if present; `NotFound` is success (mirrors
/// `delete_credential_tolerating_no_entry`'s `NoEntry`-is-success shape).
pub fn remove_if_present(profile: &Profile) -> anyhow::Result<()> {
    let _ = profile;
    todo!("BC-1.4.038 — implemented in the TDD Green step")
}

/// Host-independent rejection reasons for a profile-derived path component
/// (ADR-0021 §9, BC-1.4.040). Defense-in-depth behind the PRIMARY, live
/// `validate_profile_name` gate (BC-6.1.004/BC-6.1.005, `src/config.rs`) —
/// see that BC's body for the reclassification rationale (Pass-20
/// gate-audit correction, F2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfilePathEscape {
    Empty,
    DotSegment,
    NulByte,
    Separator,
    Colon,
    TrailingDotOrSpace,
    ReservedDeviceName,
}

/// Display text is a short, variant-specific phrase per ADR-0021 §4/§6's
/// `{reason}` placeholder — the exact wording is a load-bearing, tested
/// string (BC-1.4.040 postcondition 6) deferred to the TDD Green step, not
/// invented here.
impl std::fmt::Display for ProfilePathEscape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = f;
        todo!("BC-1.4.040 postcondition 6 — implemented in the TDD Green step")
    }
}

impl std::error::Error for ProfilePathEscape {}

/// Marker: [`load_pair`] found a file but could not turn its contents into
/// a usable pair — DPAPI unprotect failed (wrong user, tamper), the 5-byte
/// wrap header was unrecognized (`envelope::unwrap`), or the decrypted
/// plaintext failed `envelope::decode` (malformed JSON, missing field).
/// Recovered by the caller via `e.downcast_ref::<CorruptSecretFile>()` —
/// the SAME type-based, never-string-matched discrimination pattern
/// [`DpapiFallbackFailed`] establishes for the write path (ADR-0021 §3/§4).
#[derive(Debug)]
pub(crate) struct CorruptSecretFile(pub String);

impl std::fmt::Display for CorruptSecretFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = f;
        todo!("ADR-0021 §4 message text — implemented in the TDD Green step")
    }
}

impl std::error::Error for CorruptSecretFile {}

/// Marker: the DPAPI-encrypted-file fallback itself failed after a keyring
/// `TooLong` (ADR-0021 §6) — distinguishes "the size-safe path is
/// genuinely broken" (disk full, DPAPI syscall failure, permission denied
/// on the secrets directory) from an ordinary locked-keychain condition on
/// the small-secret keyring path, which keeps the pre-existing "Unlock your
/// keychain" message. The message TEXT selected for this marker is out of
/// scope for this story (`S-cycle4-honest-fail-message`, BC-1.4.039) — this
/// module only produces and propagates the marker itself.
#[derive(Debug)]
pub(crate) struct DpapiFallbackFailed(pub String);

impl std::fmt::Display for DpapiFallbackFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = f;
        todo!("ADR-0021 §6 message text — implemented in the TDD Green step")
    }
}

impl std::error::Error for DpapiFallbackFailed {}

// ============================================================================
// Tests — S-cycle4-dpapi-storage-fix, Two-Step Red Gate, Step 2 (failing tests)
// ============================================================================
//
// This module is `pub(crate)` (see `src/api/mod.rs`), and every marker type
// and function under test here (`envelope::*`, `should_fallback_to_dpapi`,
// `reject_unsafe_profile_component`, `is_reserved_windows_device_name`,
// `file_path`, `store_pair`/`load_pair`/`remove_if_present`,
// `ProfilePathEscape`/`CorruptSecretFile`/`DpapiFallbackFailed`) is
// `pub(crate)` too — none of it is nameable from an external integration
// test crate (`tests/*.rs`), which is why every test in this module lives
// HERE, inline, rather than in `tests/`. This mirrors the existing
// `#[cfg(test)] mod tests` at the bottom of `src/api/auth.rs`.
//
// CI-tier legend (see this story's dispatch prompt / vp-delta.md for the
// full rationale per VP):
//   - HOST-PURE:      no `#[ignore]`, no env gate — runs in every `cargo test`.
//   - WINDOWS-ONLY:   `#[cfg(windows)]` — compiles out entirely on this host.
//   - loosely,        several HOST-PURE tests below are Red-Gate-failing via
//                      a `todo!()` panic rather than an assertion mismatch —
//                      both are valid Red Gate failure modes (compiles,
//                      does not pass).
#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> Profile {
        Profile::from(name.to_string())
    }

    // ------------------------------------------------------------------
    // envelope::{encode, decode} — AC-012, VP-AUTHDX-014 (round-trip half)
    // ------------------------------------------------------------------

    #[test]
    fn test_envelope_encode_decode_round_trip_basic() {
        let (a, r) = ("access-token-value", "refresh-token-value");
        let bytes = envelope::encode(a, r);
        let (da, dr) =
            envelope::decode(&bytes).expect("decode of freshly-encoded bytes must succeed");
        assert_eq!((da.as_str(), dr.as_str()), (a, r));
    }

    #[test]
    fn test_envelope_encode_decode_round_trip_empty_strings() {
        let bytes = envelope::encode("", "");
        let (da, dr) = envelope::decode(&bytes).expect("empty strings must round-trip");
        assert_eq!((da.as_str(), dr.as_str()), ("", ""));
    }

    /// AC-012: round-trip holds for values well above the 2560-byte
    /// Credential Manager ceiling — this is the entire point of the DPAPI
    /// fallback existing at all.
    #[test]
    fn test_envelope_encode_decode_round_trip_oversized_token() {
        let a = "a".repeat(5_000);
        let r = "r".repeat(9_000);
        let bytes = envelope::encode(&a, &r);
        let (da, dr) =
            envelope::decode(&bytes).expect("oversized pair must round-trip byte-for-byte");
        assert_eq!(da, a);
        assert_eq!(dr, r);
    }

    #[test]
    fn test_envelope_encode_decode_round_trip_non_ascii_utf8() {
        let (a, r) = ("access-\u{1F600}-emoji", "refresh-\u{00e9}-accent");
        let bytes = envelope::encode(a, r);
        let (da, dr) = envelope::decode(&bytes).expect("UTF-8 pair must round-trip");
        assert_eq!((da.as_str(), dr.as_str()), (a, r));
    }

    /// AC-012 / VP-AUTHDX-014 (corrupt half): structurally malformed input
    /// to `decode` must be a distinct `Err`, never a panic, never silently
    /// coerced into an empty/absent result.
    #[test]
    fn test_envelope_decode_rejects_structurally_malformed_json() {
        let bad = b"this is not json at all";
        assert!(
            envelope::decode(bad).is_err(),
            "AC-012 VIOLATION: envelope::decode must reject malformed JSON with a distinct Err"
        );
    }

    #[test]
    fn test_envelope_decode_rejects_json_missing_access_field() {
        let bad = br#"{"version":1,"refresh":"r"}"#;
        assert!(
            envelope::decode(bad).is_err(),
            "AC-012 VIOLATION: envelope::decode must reject a payload missing the access field"
        );
    }

    #[test]
    fn test_envelope_decode_rejects_json_missing_refresh_field() {
        let bad = br#"{"version":1,"access":"a"}"#;
        assert!(
            envelope::decode(bad).is_err(),
            "AC-012 VIOLATION: envelope::decode must reject a payload missing the refresh field"
        );
    }

    #[test]
    fn test_envelope_decode_rejects_empty_input() {
        assert!(
            envelope::decode(&[]).is_err(),
            "AC-012 VIOLATION: envelope::decode must reject empty input, not panic or coerce to absent"
        );
    }

    /// The "never panics on malformed input" half of AC-012 is exercised
    /// implicitly by every `.expect_err()`/`.is_err()` assertion above and
    /// below: any of them would themselves panic (via an unwrap on a
    /// panicking call) if `decode`/`unwrap` panicked instead of returning
    /// `Err`, so no separate `#[should_panic]`-style test is needed here —
    /// that shape would perversely start PASSING for the wrong reason
    /// pre-implementation (via `todo!()`'s panic) and FAIL once real,
    /// correct, non-panicking logic lands, which is backwards for a Red
    /// Gate test.
    #[test]
    fn test_envelope_decode_rejects_binary_garbage_with_high_bytes() {
        assert!(
            envelope::decode(b"\xff\xfe\x00garbage").is_err(),
            "AC-012 VIOLATION: envelope::decode must return Err (not panic) on binary garbage"
        );
    }

    // ------------------------------------------------------------------
    // envelope::{wrap, unwrap} — AC-012, VP-AUTHDX-014
    // ------------------------------------------------------------------

    #[test]
    fn test_envelope_wrap_unwrap_round_trip() {
        let protected = vec![1u8, 2, 3, 4, 5, 250, 251, 252];
        let wrapped = envelope::wrap(protected.clone());
        let unwrapped =
            envelope::unwrap(&wrapped).expect("unwrap of freshly-wrapped bytes must succeed");
        assert_eq!(unwrapped, protected.as_slice());
    }

    #[test]
    fn test_envelope_wrap_unwrap_round_trip_empty_ciphertext() {
        let wrapped = envelope::wrap(Vec::new());
        let unwrapped =
            envelope::unwrap(&wrapped).expect("unwrap of an empty-ciphertext wrap must succeed");
        assert!(unwrapped.is_empty());
    }

    /// BC-1.4.037 Postcondition 2: the wrap header begins with the 4-byte
    /// magic `b"JROD"`.
    #[test]
    fn test_envelope_wrap_prepends_jrod_magic() {
        let wrapped = envelope::wrap(vec![9, 9, 9]);
        assert!(
            wrapped.len() >= 5,
            "AC-012 VIOLATION: wrap() must prepend a 5-byte header (4-byte magic + 1-byte version)"
        );
        assert_eq!(
            &wrapped[0..4],
            b"JROD",
            "AC-012 VIOLATION: wrap() must prepend the b\"JROD\" magic bytes"
        );
    }

    #[test]
    fn test_envelope_unwrap_rejects_truncated_header() {
        // Fewer than 5 bytes — cannot possibly contain a valid header.
        assert!(
            envelope::unwrap(b"JR").is_err(),
            "AC-012 VIOLATION: unwrap() must reject a truncated (<5 byte) header"
        );
        assert!(envelope::unwrap(b"").is_err());
    }

    #[test]
    fn test_envelope_unwrap_rejects_bad_magic() {
        let mut wrapped = envelope::wrap(vec![1, 2, 3]);
        wrapped[0] = b'X'; // corrupt the magic
        assert!(
            envelope::unwrap(&wrapped).is_err(),
            "AC-012 VIOLATION: unwrap() must reject an unrecognized magic, never silently \
             coerced into \"no token\""
        );
    }

    #[test]
    fn test_envelope_unwrap_rejects_unrecognized_version() {
        let mut wrapped = envelope::wrap(vec![1, 2, 3]);
        // Byte index 4 is the 1-byte version, immediately after the 4-byte
        // magic. 0xFF is not expected to be a real, currently-issued
        // version.
        wrapped[4] = 0xFF;
        assert!(
            envelope::unwrap(&wrapped).is_err(),
            "AC-012 VIOLATION: unwrap() must reject an unrecognized version byte"
        );
    }

    // ------------------------------------------------------------------
    // should_fallback_to_dpapi — VP-AUTHDX-011 sub-property (1), pure,
    // exhaustive over the keyring::Error variant set (BC-1.4.035
    // Invariant 2). HOST-PURE.
    // ------------------------------------------------------------------

    #[test]
    fn test_should_fallback_to_dpapi_true_for_toolong() {
        let err = keyring::Error::TooLong("password".to_string(), 2560);
        assert!(
            should_fallback_to_dpapi(&err),
            "VP-AUTHDX-011 VIOLATION: should_fallback_to_dpapi must return true for TooLong"
        );
    }

    #[test]
    fn test_should_fallback_to_dpapi_false_for_every_other_variant() {
        let cases: Vec<keyring::Error> = vec![
            keyring::Error::NoEntry,
            keyring::Error::BadEncoding(vec![0xff, 0xfe]),
            keyring::Error::Invalid("attribute".to_string(), "reason".to_string()),
            keyring::Error::PlatformFailure(Box::new(std::io::Error::other("boom"))),
            keyring::Error::NoStorageAccess(Box::new(std::io::Error::other("locked"))),
            keyring::Error::Ambiguous(Vec::new()),
        ];
        for err in cases {
            assert!(
                !should_fallback_to_dpapi(&err),
                "VP-AUTHDX-011 VIOLATION: should_fallback_to_dpapi must return false for \
                 every non-TooLong variant. Failed for: {err:?}"
            );
        }
    }

    // ------------------------------------------------------------------
    // reject_unsafe_profile_component — AC-017, VP-AUTHDX-016(a)(b)(c).
    // HOST-PURE, NO #[cfg(windows)] gate anywhere in this section — the
    // whole point is that the recognizer, not the host OS, does the
    // rejecting (Pass-2 adversarial review Finding #1).
    // ------------------------------------------------------------------

    #[test]
    fn test_reject_unsafe_profile_component_accepts_ordinary_names() {
        for name in ["default", "sandbox-prod", "team_a", "a", "ABC123_-"] {
            assert!(
                reject_unsafe_profile_component(name).is_ok(),
                "AC-017 VIOLATION: ordinary profile name {name:?} must be accepted"
            );
        }
    }

    #[test]
    fn test_reject_unsafe_profile_component_rejects_empty_string() {
        assert_eq!(
            reject_unsafe_profile_component(""),
            Err(ProfilePathEscape::Empty)
        );
    }

    #[test]
    fn test_reject_unsafe_profile_component_rejects_dot_segments() {
        assert_eq!(
            reject_unsafe_profile_component("."),
            Err(ProfilePathEscape::DotSegment)
        );
        assert_eq!(
            reject_unsafe_profile_component(".."),
            Err(ProfilePathEscape::DotSegment)
        );
    }

    /// EC-1.4.040-1/2 (partial): a profile name merely CONTAINING `..` or
    /// `.` as a substring (not the FULL string) is not a dot-segment
    /// rejection by itself — though `"a..b"` has no separator either, so it
    /// must be ACCEPTED (there is no other rejection reason for it).
    #[test]
    fn test_reject_unsafe_profile_component_dot_substring_is_not_a_dot_segment_rejection() {
        assert!(
            reject_unsafe_profile_component("a..b").is_ok(),
            "AC-017: \"a..b\" contains \"..\" as a substring but is not the exact string \
             \".\" or \"..\" and has no separator — must be accepted"
        );
    }

    #[test]
    fn test_reject_unsafe_profile_component_rejects_embedded_nul_byte() {
        assert_eq!(
            reject_unsafe_profile_component("secret\0name"),
            Err(ProfilePathEscape::NulByte)
        );
    }

    /// EC-1.4.040-1/2/6: both separators, anywhere in the string, on every
    /// host — this alone also catches a UNC prefix via either separator.
    #[test]
    fn test_reject_unsafe_profile_component_rejects_both_separators_everywhere() {
        for name in [
            "../../etc/passwd",
            "sub/dir",
            "sub\\dir",
            "\\\\server\\share",
            "//server/share",
            "a/b",
            "a\\b",
        ] {
            assert_eq!(
                reject_unsafe_profile_component(name),
                Err(ProfilePathEscape::Separator),
                "AC-017 VIOLATION: {name:?} must be rejected as Separator, on every host, \
                 with NO #[cfg(windows)] gate"
            );
        }
    }

    /// EC-1.4.040-5: a Windows drive-letter prefix AND an NTFS Alternate
    /// Data Stream suffix both use `:` — rejected unconditionally on every
    /// host, not just Windows.
    #[test]
    fn test_reject_unsafe_profile_component_rejects_colon_everywhere() {
        for name in ["C:", "C:\\evil", "secret:$DATA", "name:", ":name"] {
            assert_eq!(
                reject_unsafe_profile_component(name),
                Err(ProfilePathEscape::Colon),
                "AC-017 VIOLATION: {name:?} must be rejected as Colon, on every host"
            );
        }
    }

    #[test]
    fn test_reject_unsafe_profile_component_rejects_trailing_dot_or_space() {
        assert_eq!(
            reject_unsafe_profile_component("profile."),
            Err(ProfilePathEscape::TrailingDotOrSpace)
        );
        assert_eq!(
            reject_unsafe_profile_component("profile "),
            Err(ProfilePathEscape::TrailingDotOrSpace)
        );
    }

    /// AC-017 / VP-AUTHDX-016(b): the FULL, authoritative 30-name reserved
    /// Windows device-name set (ADR-0021 §9) — asserted rejected on THIS
    /// host (macOS/Linux), with no `#[cfg(windows)]` gate, proving the
    /// recognizer (not the host OS) does the rejecting.
    #[test]
    fn test_reject_unsafe_profile_component_rejects_all_30_reserved_device_names() {
        let classic = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];
        let com = [
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        ];
        let lpt = [
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let superscript = [
            "COM\u{b9}",
            "COM\u{b2}",
            "COM\u{b3}",
            "LPT\u{b9}",
            "LPT\u{b2}",
            "LPT\u{b3}",
        ];

        let mut all: Vec<&str> = Vec::new();
        all.extend_from_slice(&classic);
        all.extend_from_slice(&com);
        all.extend_from_slice(&lpt);
        all.extend_from_slice(&superscript);
        assert_eq!(
            all.len(),
            30,
            "test bug: the reserved-name enumeration must total 30"
        );

        for name in &all {
            assert_eq!(
                reject_unsafe_profile_component(name),
                Err(ProfilePathEscape::ReservedDeviceName),
                "AC-017 VIOLATION: reserved device name {name:?} (bare) must be rejected"
            );
            // Case-insensitivity.
            let lower = name.to_lowercase();
            assert_eq!(
                reject_unsafe_profile_component(&lower),
                Err(ProfilePathEscape::ReservedDeviceName),
                "AC-017 VIOLATION: reserved device name {lower:?} (lowercase) must be rejected"
            );
            // EC-1.4.040-7: with a trailing extension — the stem match runs
            // after the trailing-dot/space rejection, so "CON.txt" (no
            // trailing dot/space of its own) reaches the stem check.
            let with_ext = format!("{name}.txt");
            assert_eq!(
                reject_unsafe_profile_component(&with_ext),
                Err(ProfilePathEscape::ReservedDeviceName),
                "AC-017 VIOLATION: reserved device name with extension {with_ext:?} must be rejected"
            );
        }
    }

    /// EC-1.4.040-10: a leading-space-prefixed reserved stem is still
    /// rejected as ReservedDeviceName (stem-normalization trims leading
    /// spaces before matching).
    #[test]
    fn test_reject_unsafe_profile_component_rejects_leading_space_reserved_stem() {
        for name in [" CON", " LPT1", " com1", "  NUL"] {
            assert_eq!(
                reject_unsafe_profile_component(name),
                Err(ProfilePathEscape::ReservedDeviceName),
                "AC-017 VIOLATION (EC-1.4.040-10): {name:?} must be rejected as ReservedDeviceName"
            );
        }
    }

    /// EC-1.4.040-10: a leading-space NON-reserved name has no separate
    /// rejection reason and remains accepted — the leading-space handling
    /// is scoped ONLY to reserved-stem normalization.
    #[test]
    fn test_reject_unsafe_profile_component_accepts_leading_space_nonreserved_name() {
        assert!(
            reject_unsafe_profile_component(" my-profile").is_ok(),
            "AC-017 (EC-1.4.040-10): a leading-space NON-reserved name must be accepted"
        );
    }

    /// Non-reserved names that merely CONTAIN a reserved stem as a
    /// substring (not the whole stem) must NOT be rejected — the match is
    /// against the STEM (part before the first '.'), not a substring scan.
    #[test]
    fn test_reject_unsafe_profile_component_does_not_reject_reserved_name_as_substring() {
        for name in ["CONsole", "myCON", "PRNter", "auxiliary"] {
            assert!(
                reject_unsafe_profile_component(name).is_ok(),
                "AC-017: {name:?} contains a reserved stem as a SUBSTRING only — must be accepted"
            );
        }
    }

    /// Direct unit coverage of the private `is_reserved_windows_device_name`
    /// helper (same-module visibility) — a second, independent view of the
    /// same 30-name set from `reject_unsafe_profile_component`'s own
    /// dedicated test above.
    #[test]
    fn test_is_reserved_windows_device_name_direct() {
        assert!(is_reserved_windows_device_name("CON"));
        assert!(is_reserved_windows_device_name("con"));
        assert!(is_reserved_windows_device_name("COM\u{b9}"));
        assert!(is_reserved_windows_device_name(" NUL")); // leading-space trimmed
        assert!(!is_reserved_windows_device_name("default"));
        assert!(!is_reserved_windows_device_name("CONsole"));
    }

    /// Design-conformance sanity check (Pass-2 adversarial review Finding
    /// #1): demonstrates the underlying claim the guard exists to defeat —
    /// `std::path::Path` on THIS (non-Windows) host treats a
    /// Windows-drive-letter/UNC/ADS string as a single opaque
    /// `Component::Normal`, which would be wrongly ACCEPTED by a
    /// `std::path`-based implementation. This does not call
    /// `reject_unsafe_profile_component` at all — it is a fixture proving
    /// the guard's `std::path`-avoidance is not a hypothetical concern on
    /// this CI host.
    #[test]
    fn test_design_conformance_std_path_would_wrongly_accept_windows_vectors_on_this_host() {
        use std::path::{Component, Path};
        for vector in ["C:\\evil", "\\\\server\\share", "name:$DATA"] {
            let mut components = Path::new(vector).components();
            let first = components.next();
            assert!(
                matches!(first, Some(Component::Normal(_))) && components.next().is_none(),
                "test fixture assumption violated: std::path was expected to treat {vector:?} \
                 as a single opaque Component::Normal on this host — if this changed, the \
                 design-conformance rationale in ADR-0021 §9 needs re-verifying, not \
                 reject_unsafe_profile_component itself"
            );
        }
        // The guard itself (tested above) correctly rejects every one of these.
        for vector in ["C:\\evil", "\\\\server\\share", "name:$DATA"] {
            assert!(reject_unsafe_profile_component(vector).is_err());
        }
    }

    // ------------------------------------------------------------------
    // Marker type Display text — BC-1.4.040 Postcondition 6 / ADR-0021 §4/§6
    // "{reason}" placeholder. HOST-PURE.
    // ------------------------------------------------------------------

    #[test]
    fn test_profile_path_escape_display_texts_are_nonempty_and_distinct() {
        let variants = [
            ProfilePathEscape::Empty,
            ProfilePathEscape::DotSegment,
            ProfilePathEscape::NulByte,
            ProfilePathEscape::Separator,
            ProfilePathEscape::Colon,
            ProfilePathEscape::TrailingDotOrSpace,
            ProfilePathEscape::ReservedDeviceName,
        ];
        let mut seen = std::collections::HashSet::new();
        for v in variants {
            let text = format!("{v}");
            assert!(
                !text.is_empty(),
                "BC-1.4.040 Postcondition 6 VIOLATION: {v:?}'s Display text must be non-empty"
            );
            assert!(
                seen.insert(text.clone()),
                "BC-1.4.040 Postcondition 6 VIOLATION: {v:?}'s Display text {text:?} must be \
                 distinct from every other variant's text"
            );
        }
    }

    #[test]
    fn test_corrupt_secret_file_display_includes_profile_name() {
        let e = CorruptSecretFile("my-profile".to_string());
        let text = format!("{e}");
        assert!(
            text.contains("my-profile"),
            "ADR-0021 §4 VIOLATION: CorruptSecretFile's Display text must name the profile. \
             Got: {text:?}"
        );
    }

    #[test]
    fn test_dpapi_fallback_failed_display_includes_message() {
        let e = DpapiFallbackFailed("disk full while writing secrets directory".to_string());
        let text = format!("{e}");
        assert!(
            text.contains("disk full"),
            "ADR-0021 §6 VIOLATION: DpapiFallbackFailed's Display text must include the \
             underlying failure detail. Got: {text:?}"
        );
    }

    // ------------------------------------------------------------------
    // Guard-wiring oracle — AC-018, VP-AUTHDX-016(d). HOST-PURE, calls the
    // three entry points DIRECTLY with a guard-failing name and asserts
    // `Err` downcastable to `ProfilePathEscape`, BEFORE any FS op or each
    // function's own OS-specific short-circuit. This is a SEPARATE test
    // from the pure-recognizer cases above (verifies WIRING, not
    // correctness in isolation).
    // ------------------------------------------------------------------

    #[test]
    fn test_guard_wiring_store_pair_rejects_bad_profile_before_anything_else() {
        let p = profile("a/b");
        let err = store_pair(&p, "access", "refresh")
            .expect_err("AC-018 VIOLATION: store_pair must reject a guard-failing profile name");
        assert!(
            err.downcast_ref::<ProfilePathEscape>().is_some(),
            "AC-018 VIOLATION: store_pair's Err for a guard-failing name must be downcastable \
             to ProfilePathEscape (never DpapiFallbackFailed or any other error). Got: {err:#}"
        );
    }

    #[test]
    fn test_guard_wiring_load_pair_rejects_bad_profile_before_anything_else() {
        let p = profile("con");
        let err = load_pair(&p)
            .expect_err("AC-018 VIOLATION: load_pair must reject a guard-failing profile name");
        assert!(
            err.downcast_ref::<ProfilePathEscape>().is_some(),
            "AC-018 VIOLATION: load_pair's Err for a guard-failing name must be downcastable \
             to ProfilePathEscape (never Ok(None)). Got: {err:#}"
        );
    }

    #[test]
    fn test_guard_wiring_remove_if_present_rejects_bad_profile_before_anything_else() {
        let p = profile("name:$DATA");
        let err = remove_if_present(&p).expect_err(
            "AC-018 VIOLATION: remove_if_present must reject a guard-failing profile name",
        );
        assert!(
            err.downcast_ref::<ProfilePathEscape>().is_some(),
            "AC-018 VIOLATION: remove_if_present's Err for a guard-failing name must be \
             downcastable to ProfilePathEscape (never Ok(())). Got: {err:#}"
        );
    }

    // ------------------------------------------------------------------
    // Cross-platform non-engagement — AC-007, AC-013, VP-AUTHDX-013.
    // #[cfg(not(windows))], HOST-PURE on this host.
    // ------------------------------------------------------------------

    #[cfg(not(windows))]
    #[test]
    fn test_non_windows_store_pair_returns_dpapi_fallback_failed_for_valid_name() {
        let p = profile("default");
        let err = store_pair(&p, "access", "refresh").expect_err(
            "AC-007/AC-013 VIOLATION: on #[cfg(not(windows))], store_pair must fail (DPAPI is \
             categorically unavailable) for a guard-PASSING profile name",
        );
        assert!(
            err.downcast_ref::<DpapiFallbackFailed>().is_some(),
            "AC-007/AC-013/AC-019 VIOLATION: the non-Windows store_pair failure must carry the \
             DpapiFallbackFailed marker specifically. Got: {err:#}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_non_windows_load_pair_returns_ok_none_for_valid_name() {
        let p = profile("default");
        assert_eq!(
            load_pair(&p).expect(
                "AC-007/AC-013 VIOLATION: load_pair must not error for a valid name on non-Windows"
            ),
            None,
            "AC-007/AC-013 VIOLATION: on #[cfg(not(windows))], load_pair must return Ok(None) \
             unconditionally for a guard-PASSING profile name"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn test_non_windows_remove_if_present_returns_ok_for_valid_name() {
        let p = profile("default");
        assert!(
            remove_if_present(&p).is_ok(),
            "AC-007/AC-013 VIOLATION: on #[cfg(not(windows))], remove_if_present must return \
             Ok(()) unconditionally for a guard-PASSING profile name"
        );
    }

    // ------------------------------------------------------------------
    // Property-based tests — AC-012/VP-AUTHDX-014 (envelope round-trip)
    // and AC-017/VP-AUTHDX-016 (guard exhaustive rejection). HOST-PURE.
    // ------------------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Bounded, OAuth-token-shaped generator: printable ASCII, 0-6000
        /// bytes so both small and well-above-2560-byte cases are covered
        /// without the pathological-length overhead of a fully unbounded
        /// generator (mirrors the `token_strategy`/O-3-style bounding
        /// convention already used in `src/api/auth.rs`'s proptest suites).
        fn oauth_token_strategy() -> impl Strategy<Value = String> {
            proptest::collection::vec(0x20u8..0x7f, 0..6000)
                .prop_map(|bytes| String::from_utf8(bytes).unwrap())
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// VP-AUTHDX-014 (1): `decode(encode(a, r)) == (a, r)` for any
            /// UTF-8 pair, including values well above 2560 bytes.
            #[test]
            fn prop_envelope_encode_decode_round_trip(
                access in oauth_token_strategy(),
                refresh in oauth_token_strategy(),
            ) {
                let bytes = envelope::encode(&access, &refresh);
                let (da, dr) = envelope::decode(&bytes).expect("round-trip must succeed");
                prop_assert_eq!(da, access);
                prop_assert_eq!(dr, refresh);
            }

            /// VP-AUTHDX-014 (1): `unwrap(wrap(p)) == p` for any protected
            /// byte slice.
            #[test]
            fn prop_envelope_wrap_unwrap_round_trip(protected in proptest::collection::vec(any::<u8>(), 0..8000)) {
                let wrapped = envelope::wrap(protected.clone());
                let unwrapped = envelope::unwrap(&wrapped).expect("round-trip must succeed");
                prop_assert_eq!(unwrapped, protected.as_slice());
            }

            /// VP-AUTHDX-016: for ANY string, if
            /// `reject_unsafe_profile_component` accepts it, the string is
            /// by construction a single, non-empty, non-dot-segment,
            /// no-separator, no-colon, no-NUL, no-trailing-dot-or-space,
            /// non-reserved-device-name opaque path segment (Invariant 3 —
            /// "acceptance ⇒ opaque single segment"). Adversarial generator
            /// biased toward the exact hazard classes the guard defends
            /// against, not a uniform arbitrary-Unicode fuzz (which would
            /// almost always land in one rejection bucket trivially).
            #[test]
            fn prop_reject_unsafe_profile_component_acceptance_implies_opaque_segment(
                s in prop_oneof![
                    3 => "[A-Za-z0-9_-]{0,20}",
                    1 => "[A-Za-z0-9 _./:\\\\]{0,20}",
                    1 => Just(".".to_string()),
                    1 => Just("..".to_string()),
                    1 => Just("".to_string()),
                ]
            ) {
                if reject_unsafe_profile_component(&s).is_ok() {
                    prop_assert!(!s.is_empty());
                    prop_assert_ne!(s.as_str(), ".");
                    prop_assert_ne!(s.as_str(), "..");
                    prop_assert!(!s.contains('\0'));
                    prop_assert!(!s.contains('/'));
                    prop_assert!(!s.contains('\\'));
                    prop_assert!(!s.contains(':'));
                    prop_assert!(!s.ends_with('.'));
                    prop_assert!(!s.ends_with(' '));
                    prop_assert!(!is_reserved_windows_device_name(&s));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Windows-only — VP-AUTHDX-010. #[cfg(windows)]: compiles out entirely
    // on this (non-Windows) host, per this story's dispatch instructions.
    // Not compile-verified on this host — see the Red Gate report.
    // ------------------------------------------------------------------

    #[cfg(windows)]
    mod windows_only_tests {
        use super::*;

        /// VP-AUTHDX-010 sub-property (a) — Windows-COMPILED, spike-
        /// independent: `dpapi::protect` must never set
        /// `CRYPTPROTECT_LOCAL_MACHINE` (0x4). Pinned via the
        /// security-relevant bit-clear only (never an exact `== 0` on the
        /// full flag word, since `CRYPTPROTECT_UI_FORBIDDEN` (0x1) is
        /// legitimately also set — ADR-0021 §8, Pass-5 adversarial review
        /// Finding #3).
        ///
        /// References a `DPAPI_PROTECT_FLAGS: u32` constant this story's
        /// Green step must add at `dpapi::protect`'s `dwFlags` call site
        /// (VP-AUTHDX-010's own verification-method text names this
        /// pattern: "a named `const DPAPI_PROTECT_FLAGS: u32` used as the
        /// sole `dwFlags` argument"). This constant does not exist in the
        /// stub as of this Red Gate commit, so this test will not compile
        /// until the implementer adds it — acceptable because the whole
        /// module is `#[cfg(windows)]`-gated and therefore not part of
        /// this (non-Windows) host's build at all.
        const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x4;

        #[test]
        fn test_dpapi_protect_flags_never_set_local_machine_bit() {
            assert_eq!(
                dpapi::DPAPI_PROTECT_FLAGS & CRYPTPROTECT_LOCAL_MACHINE,
                0,
                "VP-AUTHDX-010(a) VIOLATION: dpapi::protect's dwFlags must never set \
                 CRYPTPROTECT_LOCAL_MACHINE (0x4) — USER scope only."
            );
        }

        /// VP-AUTHDX-010 sub-property (b) — Windows-RUNTIME real round-trip,
        /// including a plaintext above the 2560-byte Credential Manager
        /// ceiling, and confirming the ciphertext is not equal to the
        /// plaintext (protection actually occurred).
        #[test]
        fn test_dpapi_protect_unprotect_real_round_trip() {
            let plaintext = b"a".repeat(4000);
            let protected = dpapi::protect(&plaintext).expect("DPAPI protect must succeed");
            assert_ne!(
                protected, plaintext,
                "VP-AUTHDX-010(b) VIOLATION: ciphertext must not equal plaintext"
            );
            let unprotected = dpapi::unprotect(&protected).expect("DPAPI unprotect must succeed");
            assert_eq!(unprotected, plaintext);
        }
    }
}
