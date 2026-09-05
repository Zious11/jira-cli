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
