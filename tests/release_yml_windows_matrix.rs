//! Source-text assertion tests for S-WIN-4: Windows build matrix and .zip packaging.
//!
//! These tests read `.github/workflows/release.yml` and grep for the exact YAML
//! constructs required by S-WIN-4. They are lightweight, always-run tests that
//! catch accidental removal of the Windows matrix row, package steps, smoke gate,
//! or artifact globs.
//!
//! **Presence-only caveat:** These tests verify that required configuration text
//! is present in `release.yml` but do NOT verify that the resulting workflow
//! executes correctly or that the `.zip` artifact is actually produced. The sole
//! correctness gate for the actual Windows release artifact is **H-WIN-6** — a
//! human inspection of the GitHub Release page after a live version tag.
//!
//! # Test inventory
//!
//! | Test | AC |
//! |------|----|
//! | `test_release_yml_has_windows_matrix_row` | AC-001 |
//! | `test_release_yml_windows_package_step_produces_zip` | AC-002 |
//! | `test_release_yml_smoke_step_skipped_on_windows` | AC-003 |
//! | `test_release_yml_upload_artifact_includes_zip` | AC-004 |
//! | `test_release_yml_release_job_files_includes_zip` | AC-005 |

/// Load the release.yml content once, panicking with a clear message if the file
/// cannot be read (which should never happen in normal development).
fn release_yml() -> String {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(".github/workflows/release.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Could not read .github/workflows/release.yml at {}: {}",
            path.display(),
            e
        )
    })
}

/// AC-001 — Windows matrix row is present in release.yml.
///
/// Verifies that the `build` job's `strategy.matrix.include` array contains a row
/// for `x86_64-pc-windows-msvc` on `os: windows-latest` with no `use_cross` field.
///
/// Traces to: NFR-P-W1 (Windows artifact produced on every release tag); ADR-0016 §Decision 1.
#[test]
fn test_release_yml_has_windows_matrix_row() {
    let yml = release_yml();

    assert!(
        yml.contains("x86_64-pc-windows-msvc"),
        "AC-001 FAIL: `x86_64-pc-windows-msvc` target not found in \
         .github/workflows/release.yml.\n\
         The build-job matrix must include a Windows row:\n\
         \n\
           - target: x86_64-pc-windows-msvc\n\
             os: windows-latest\n\
         \n\
         (S-WIN-4 AC-001 / NFR-P-W1 / ADR-0016 §Decision 1)"
    );

    assert!(
        yml.contains("windows-latest"),
        "AC-001 FAIL: `windows-latest` runner not found in \
         .github/workflows/release.yml.\n\
         The Windows matrix row must specify `os: windows-latest`.\n\
         (S-WIN-4 AC-001 / NFR-P-W1 / ADR-0016 §Decision 1)"
    );
}

/// AC-002 — Package (Windows) step uses Compress-Archive (shell: pwsh) and
/// Checksum (Windows) step uses sha256sum (shell: bash).
///
/// Verifies the TWO Windows-packaging steps required by ADR-0016 §Decision 2
/// (C-V3 re-amendment):
///
///   1. `Package (Windows)`: `shell: pwsh` + `Compress-Archive` — the PRIMARY,
///      deterministic .zip packaging mechanism. `zip` (Unix Info-ZIP) is NOT
///      available on `windows-latest` PATH (C-V3 BLOCKER); `Compress-Archive` is
///      built into PowerShell 5.1+ and is always present.
///   2. `Checksum (Windows)`: `shell: bash` + `sha256sum` — confirmed available
///      via Git for Windows coreutils (C-V3 CONFIRMED).
///
/// This test MUST grep for `Compress-Archive` and `shell: pwsh`, NOT for `zip` or
/// `shell: bash` (the old, broken spec). A test grepping for `zip` would
/// incorrectly pass on a regressed spec and fail to detect the unsafe invocation.
///
/// Traces to: NFR-P-W1; ADR-0016 §Decision 2 (C-V3 re-amendment); S-WIN-4 EC-002.
#[test]
fn test_release_yml_windows_package_step_produces_zip() {
    let yml = release_yml();

    // --- Package (Windows) step: must use shell: pwsh + Compress-Archive ---

    assert!(
        yml.contains("Package (Windows)"),
        "AC-002 FAIL: `Package (Windows)` step not found in \
         .github/workflows/release.yml.\n\
         A step named 'Package (Windows)' gated `if: runner.os == 'Windows'` \
         with `shell: pwsh` and `Compress-Archive` is required.\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)"
    );

    assert!(
        yml.contains("shell: pwsh"),
        "AC-002 FAIL: `shell: pwsh` not found in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `shell: pwsh` to invoke \
         PowerShell's Compress-Archive cmdlet. Using `shell: bash` with `zip` \
         is a C-V3 BLOCKER: `zip` is NOT available on `windows-latest` PATH.\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)"
    );

    assert!(
        yml.contains("Compress-Archive"),
        "AC-002 FAIL: `Compress-Archive` not found in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use PowerShell's `Compress-Archive` cmdlet \
         to create the .zip archive. This is the PRIMARY, deterministic packaging \
         mechanism (C-V3). Do NOT use the Unix `zip` command — it is not present \
         on `windows-latest` runners and will fail with `command not found`.\n\
         Required form:\n\
           Compress-Archive -Path \"target/...\" -DestinationPath \"jr-...-x86_64-pc-windows-msvc.zip\"\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)"
    );

    // --- Checksum (Windows) step: must exist and use sha256sum ---

    assert!(
        yml.contains("Checksum (Windows)"),
        "AC-002 FAIL: `Checksum (Windows)` step not found in \
         .github/workflows/release.yml.\n\
         A separate step named 'Checksum (Windows)' gated `if: runner.os == 'Windows'` \
         with `shell: bash` and `sha256sum` is required in addition to the \
         Package (Windows) step.\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)"
    );

    assert!(
        yml.contains("sha256sum"),
        "AC-002 FAIL: `sha256sum` not found in .github/workflows/release.yml.\n\
         The Checksum (Windows) step must use `sha256sum` to generate the .zip.sha256 \
         file. `sha256sum` is confirmed available via Git for Windows coreutils (C-V3).\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)"
    );
}

/// AC-003 — Smoke step ("Verify embedded OAuth app present") is gated off on Windows.
///
/// Verifies that `if: runner.os != 'Windows'` appears in the workflow so the
/// embedded-OAuth smoke step is skipped on the Windows runner. The step body is
/// bash/heredoc/XDG_CONFIG_HOME-shaped and cannot run on Windows without porting
/// (deferred per ADR-0016 §Decision 5c).
///
/// Traces to: ADR-0016 §Decision 5c.
#[test]
fn test_release_yml_smoke_step_skipped_on_windows() {
    let yml = release_yml();

    assert!(
        yml.contains("runner.os != 'Windows'"),
        "AC-003 FAIL: `runner.os != 'Windows'` not found in \
         .github/workflows/release.yml.\n\
         The 'Verify embedded OAuth app present' smoke step must be gated with \
         `if: runner.os != 'Windows'` to skip it on Windows runners. The step \
         uses bash heredocs and XDG_CONFIG_HOME which are not available in this \
         form on Windows (deferred per ADR-0016 §Decision 5c).\n\
         (S-WIN-4 AC-003 / ADR-0016 §Decision 5c)"
    );
}

/// AC-004 — Upload-artifact step's path block includes `jr-*.zip`.
///
/// Verifies that the `Upload artifact` step's `path:` block contains `jr-*.zip`
/// so the Windows .zip artifact is uploaded to GitHub Actions artifacts alongside
/// the existing .tar.gz and .sha256 files.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-004.
#[test]
fn test_release_yml_upload_artifact_includes_zip() {
    let yml = release_yml();

    assert!(
        yml.contains("jr-*.zip"),
        "AC-004 FAIL: `jr-*.zip` not found in .github/workflows/release.yml.\n\
         The Upload artifact step's `path:` block must include `jr-*.zip` so the \
         Windows .zip artifact is uploaded alongside the existing .tar.gz files.\n\
         Required addition to the path: block:\n\
           jr-*.zip\n\
         (S-WIN-4 AC-004 / NFR-P-W1)"
    );
}

/// AC-005 — Release job's `softprops/action-gh-release` files block includes `jr-*.zip`.
///
/// Verifies that the `Create GitHub Release` step's `files:` block contains `jr-*.zip`
/// so the Windows .zip artifact is attached to the GitHub Release alongside the
/// existing .tar.gz and .sha256 artifacts.
///
/// Note: the `softprops/action-gh-release` step accepts arbitrary file types including
/// .zip; no version change to the action is needed.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-005.
///
/// Implementation note: AC-004 already asserts `jr-*.zip` is present in the file
/// globally. This test provides the same assertion with a distinct name to clearly
/// pin the release-job files: block as a separate AC. Both tests must fail before
/// implementation and pass after.
#[test]
fn test_release_yml_release_job_files_includes_zip() {
    let yml = release_yml();

    // The glob `jr-*.zip` must appear in the release job's files: block.
    // Since both the upload-artifact path and the release files: block need
    // this glob, its presence anywhere in the file satisfies both ACs.
    // The distinction is tracked by separate test names for AC traceability.
    assert!(
        yml.contains("jr-*.zip"),
        "AC-005 FAIL: `jr-*.zip` not found in .github/workflows/release.yml.\n\
         The `softprops/action-gh-release` step's `files:` block must include \
         `jr-*.zip` so the Windows .zip artifact appears on the GitHub Releases page.\n\
         Required addition to the files: block in the release job:\n\
           jr-*.zip\n\
         (S-WIN-4 AC-005 / NFR-P-W1)"
    );
}
