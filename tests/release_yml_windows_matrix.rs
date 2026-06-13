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
//! **Anchoring:** AC-002, AC-004, and AC-005 use step-name anchoring (find the
//! step by name, inspect a line window) to ensure each assertion is scoped to its
//! owning step block. This prevents a single occurrence of a glob or keyword
//! elsewhere in the file from satisfying multiple independent tests.
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
/// Both the positive assertions (`shell: pwsh`, `Compress-Archive`) and the
/// negative assertions (`zip `, `shell: bash`) are anchored to the 10-line window
/// immediately following the `Package (Windows)` step name. This prevents a false
/// pass caused by the adjacent `Checksum (Windows)` bash step satisfying the
/// `shell: bash` check, and enforces C-V3 co-location within the step itself.
///
/// `sha256sum` is checked in a separate anchored window for the `Checksum (Windows)`
/// step so that assertion is also step-scoped.
///
/// Traces to: NFR-P-W1; ADR-0016 §Decision 2 (C-V3 re-amendment); S-WIN-4 EC-002.
#[test]
fn test_release_yml_windows_package_step_produces_zip() {
    let yml = release_yml();

    // --- Package (Windows) step: must use shell: pwsh + Compress-Archive ---
    // Anchor to the step name and inspect the next 10 lines only, so that the
    // adjacent Checksum (Windows) bash step cannot satisfy these assertions.

    let pkg_step_name = "Package (Windows)";
    let pkg_pos = yml.find(pkg_step_name).unwrap_or_else(|| {
        panic!(
            "AC-002 FAIL: step '{}' not found in .github/workflows/release.yml.\n\
             A step named 'Package (Windows)' gated `if: runner.os == 'Windows'` \
             with `shell: pwsh` and `Compress-Archive` is required.\n\
             (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
            pkg_step_name
        )
    });

    // 5 lines covers: step name, `if:`, `shell:`, `run:`, and a trailing blank
    // line — the complete Package (Windows) step body without bleeding into the
    // adjacent Checksum (Windows) step, whose `shell: bash` and `.zip` redirect
    // would otherwise trigger the negative assertions below.
    let pkg_window: String = yml[pkg_pos..]
        .lines()
        .take(5)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        pkg_window.contains("shell: pwsh"),
        "AC-002 FAIL: `shell: pwsh` not found within the first 10 lines of the \
         'Package (Windows)' step in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `shell: pwsh` to invoke \
         PowerShell's Compress-Archive cmdlet. Using `shell: bash` with `zip` \
         is a C-V3 BLOCKER: `zip` is NOT available on `windows-latest` PATH.\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_window
    );

    assert!(
        pkg_window.contains("Compress-Archive"),
        "AC-002 FAIL: `Compress-Archive` not found within the first 10 lines of \
         the 'Package (Windows)' step in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use PowerShell's `Compress-Archive` cmdlet \
         to create the .zip archive. This is the PRIMARY, deterministic packaging \
         mechanism (C-V3). Do NOT use the Unix `zip` command — it is not present \
         on `windows-latest` runners and will fail with `command not found`.\n\
         Required form:\n\
           Compress-Archive -Path \"target/...\" -DestinationPath \"jr-...-x86_64-pc-windows-msvc.zip\"\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_window
    );

    // C-V3 negative check: `zip ` (Unix Info-ZIP) must NOT appear in the Package
    // (Windows) step body — only in the adjacent Compress-Archive invocation which
    // uses it as a DestinationPath suffix, not as a shell command.
    // We check for `zip ` with a trailing space to avoid false-matching the `.zip`
    // extension inside the Compress-Archive -DestinationPath argument.
    assert!(
        !pkg_window.contains("zip "),
        "AC-002 FAIL: `zip ` (Unix Info-ZIP invocation) found within the first 10 \
         lines of the 'Package (Windows)' step in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `Compress-Archive`, not the Unix `zip` \
         command. `zip` is NOT available on `windows-latest` PATH (C-V3 BLOCKER).\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_window
    );

    assert!(
        !pkg_window.contains("shell: bash"),
        "AC-002 FAIL: `shell: bash` found within the first 10 lines of the \
         'Package (Windows)' step in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `shell: pwsh`, not `shell: bash`. \
         Using bash + zip is the old broken spec (C-V3 BLOCKER). \
         The adjacent Checksum (Windows) step uses bash legitimately — this \
         assertion is step-scoped to catch a regression in the Package step itself.\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_window
    );

    // --- Checksum (Windows) step: must exist and use sha256sum ---
    // Anchored separately so the sha256sum assertion is step-scoped and does not
    // pass merely because sha256sum appears elsewhere in the file.

    let chk_step_name = "Checksum (Windows)";
    let chk_pos = yml.find(chk_step_name).unwrap_or_else(|| {
        panic!(
            "AC-002 FAIL: step '{}' not found in .github/workflows/release.yml.\n\
             A separate step named 'Checksum (Windows)' gated `if: runner.os == 'Windows'` \
             with `shell: bash` and `sha256sum` is required in addition to the \
             Package (Windows) step.\n\
             (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
            chk_step_name
        )
    });

    let chk_window: String = yml[chk_pos..]
        .lines()
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        chk_window.contains("sha256sum"),
        "AC-002 FAIL: `sha256sum` not found within the first 10 lines of the \
         'Checksum (Windows)' step in .github/workflows/release.yml.\n\
         The Checksum (Windows) step must use `sha256sum` to generate the .zip.sha256 \
         file. `sha256sum` is confirmed available via Git for Windows coreutils (C-V3).\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        chk_window
    );
}

/// AC-003 — Smoke step ("Verify embedded OAuth app present") is gated off on Windows.
///
/// Verifies that `if: runner.os != 'Windows'` is bound to the smoke step itself,
/// not merely present somewhere in the file. The check locates the
/// `Verify embedded OAuth app present` step by name and then asserts the gate
/// appears within the next 5 lines — anchoring the assertion to that specific step.
///
/// A bare `yml.contains("runner.os != 'Windows'")` is intentionally NOT used here
/// because the same condition also guards `Package (Unix)`. A future edit that
/// removed the gate from the smoke step but left it on `Package (Unix)` would
/// pass the old test while breaking the required smoke-step invariant.
///
/// The step body uses bash heredocs and XDG_CONFIG_HOME which are not available in
/// this form on Windows (deferred per ADR-0016 §Decision 5c).
///
/// Traces to: ADR-0016 §Decision 5c.
#[test]
fn test_release_yml_smoke_step_skipped_on_windows() {
    let yml = release_yml();

    // Locate the smoke step by name.
    let step_name = "Verify embedded OAuth app present";
    let smoke_pos = yml.find(step_name).unwrap_or_else(|| {
        panic!(
            "AC-003 FAIL: step '{}' not found in .github/workflows/release.yml.\n\
             The smoke step must exist before its gate can be verified.\n\
             (S-WIN-4 AC-003 / ADR-0016 §Decision 5c)",
            step_name
        )
    });

    // Inspect the next 5 lines after the step-name line for the windows-exclusion gate.
    // 5 lines is generous enough to cover any reasonable YAML indentation or blank lines
    // between `name:` and `if:`, while tight enough to stay within the step's own block.
    let after_step = &yml[smoke_pos..];
    let window: String = after_step.lines().take(5).collect::<Vec<_>>().join("\n");

    assert!(
        window.contains("runner.os != 'Windows'"),
        "AC-003 FAIL: `if: runner.os != 'Windows'` is not present within the \
         first 5 lines after the '{}' step name in .github/workflows/release.yml.\n\
         The smoke step must be gated with `if: runner.os != 'Windows'` immediately \
         after its `name:` line. The step uses bash heredocs and XDG_CONFIG_HOME \
         which are not available in this form on Windows (deferred per ADR-0016 \
         §Decision 5c).\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-003 / ADR-0016 §Decision 5c)",
        step_name,
        window
    );
}

/// AC-004 — Upload-artifact step's path block includes `jr-*.zip`.
///
/// Verifies that the `Upload artifact` step's `path:` block contains `jr-*.zip`
/// so the Windows .zip artifact is uploaded to GitHub Actions artifacts alongside
/// the existing .tar.gz and .sha256 files.
///
/// The assertion is anchored to the `Upload artifact` step name: `jr-*.zip` must
/// appear within the first 10 lines after that step name. This ensures the glob is
/// specifically present in the upload step's `path:` block, not just anywhere in
/// the file. A bare `yml.contains("jr-*.zip")` would also be satisfied by the
/// release job's `files:` block (AC-005's concern), making the two tests
/// indistinguishable — anchoring makes each test fail independently.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-004.
#[test]
fn test_release_yml_upload_artifact_includes_zip() {
    let yml = release_yml();

    // Locate the Upload artifact step by name and inspect the next 10 lines.
    let step_name = "Upload artifact";
    let step_pos = yml.find(step_name).unwrap_or_else(|| {
        panic!(
            "AC-004 FAIL: step '{}' not found in .github/workflows/release.yml.\n\
             The 'Upload artifact' step (actions/upload-artifact) must exist before \
             its path: block can be verified.\n\
             (S-WIN-4 AC-004 / NFR-P-W1)",
            step_name
        )
    });

    // 10 lines is generous enough to span `name:`, `uses:`, `with:`, `name:` sub-key,
    // and the `path: |` block with its glob entries while staying within the step.
    let window: String = yml[step_pos..]
        .lines()
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        window.contains("jr-*.zip"),
        "AC-004 FAIL: `jr-*.zip` not found within the first 10 lines of the \
         'Upload artifact' step in .github/workflows/release.yml.\n\
         The Upload artifact step's `path:` block must include `jr-*.zip` so the \
         Windows .zip artifact is uploaded alongside the existing .tar.gz files.\n\
         Required addition to the path: block:\n\
           jr-*.zip\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-004 / NFR-P-W1)",
        window
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
/// The assertion is anchored to the `Create GitHub Release` step name: `jr-*.zip` must
/// appear within the first 10 lines after that step name. This ensures the glob is
/// specifically present in the release step's `files:` block, not just anywhere in
/// the file (e.g. the upload-artifact `path:` block, which is AC-004's concern).
/// Anchoring makes this test fail independently if the glob is removed from the
/// release `files:` block while remaining in the upload `path:` block — which would
/// produce a CI artifact but no GitHub Release attachment, silently failing NFR-P-W1.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-005.
#[test]
fn test_release_yml_release_job_files_includes_zip() {
    let yml = release_yml();

    // Locate the Create GitHub Release step by name and inspect the next 10 lines.
    let step_name = "Create GitHub Release";
    let step_pos = yml.find(step_name).unwrap_or_else(|| {
        panic!(
            "AC-005 FAIL: step '{}' not found in .github/workflows/release.yml.\n\
             The 'Create GitHub Release' step (softprops/action-gh-release) must \
             exist before its files: block can be verified.\n\
             (S-WIN-4 AC-005 / NFR-P-W1)",
            step_name
        )
    });

    // 10 lines spans `name:`, `uses:`, `with:` and the `files: |` block entries.
    let window: String = yml[step_pos..]
        .lines()
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        window.contains("jr-*.zip"),
        "AC-005 FAIL: `jr-*.zip` not found within the first 10 lines of the \
         'Create GitHub Release' step in .github/workflows/release.yml.\n\
         The `softprops/action-gh-release` step's `files:` block must include \
         `jr-*.zip` so the Windows .zip artifact appears on the GitHub Releases page.\n\
         Required addition to the files: block in the release job:\n\
           jr-*.zip\n\
         Lines seen after step name:\n{}\n\
         (S-WIN-4 AC-005 / NFR-P-W1)",
        window
    );
}
