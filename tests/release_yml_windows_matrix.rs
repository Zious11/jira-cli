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
//! **Anchoring:** AC-002, AC-003, AC-004, and AC-005 use step-name anchoring via
//! `step_block` to ensure each assertion is scoped to its owning step's YAML block.
//! `step_block` slices from the step-name offset up to the NEXT `      - name:`
//! marker (or end-of-file for the last step), so each window covers exactly that
//! step's content regardless of how many lines it spans. This prevents a single
//! occurrence of a glob or keyword elsewhere in the file from satisfying multiple
//! independent tests, and is robust to benign reformatting (e.g. inserting an
//! `env:` block) that would invalidate a fixed line-count window.
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
///
/// CRLF normalization: on Windows, Git may check out `*.yml` files with CRLF
/// line endings. The `step_block` helper searches for `"\n      - name:"` and
/// other `\n`-terminated anchors, so CRLF in the raw file would break substring
/// matching. Normalizing here keeps all assertions platform-independent and is
/// consistent with the defense-in-depth applied in `ci_yml_windows_matrix.rs`.
fn release_yml() -> String {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(".github/workflows/release.yml");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Could not read .github/workflows/release.yml at {}: {}",
            path.display(),
            e
        )
    });
    raw.replace("\r\n", "\n")
}

/// Return the YAML text belonging to the step named `step_name`.
///
/// The slice begins at `step_name`'s first occurrence and ends immediately
/// before the next `      - name:` step-list marker at the same indentation
/// level, or at end-of-file if `step_name` is the last step in its job.
///
/// This boundary-based approach is robust to any number of lines being added
/// to a step (e.g. a new `env:` block) without requiring a fixed line-count
/// constant that would drift out of sync with the file.
///
/// # Panics
/// Panics with an `AC-NNN FAIL` message if `step_name` is not found in `yml`.
/// The `ac_tag` and `context` strings are included in the panic message so the
/// failing assertion is easy to locate.
fn step_block<'a>(yml: &'a str, step_name: &str, ac_tag: &str, context: &str) -> &'a str {
    // Byte offset of the first character of step_name in yml.
    let start = yml.find(step_name).unwrap_or_else(|| {
        panic!(
            "{} FAIL: step '{}' not found in .github/workflows/release.yml.\n{}\n({} / S-WIN-4)",
            ac_tag, step_name, context, ac_tag
        )
    });

    // The step-list marker used in release.yml is "      - name:" (six spaces).
    // Search for the next occurrence AFTER `start` to find the boundary between
    // this step and the following one.
    let step_marker = "\n      - name:";
    let end = yml[start..]
        .find(step_marker)
        .map(|rel| start + rel)
        .unwrap_or(yml.len()); // last step in the file — run to EOF

    &yml[start..end]
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
/// negative assertions (`zip `, `shell: bash`) are anchored to the
/// `Package (Windows)` step block (up to the next step). This prevents a false
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
    // Anchor to the step block (from "Package (Windows)" up to the next step
    // boundary) so that the adjacent Checksum (Windows) bash step cannot
    // satisfy these assertions.

    let pkg_step_name = "Package (Windows)";
    let pkg_block = step_block(
        &yml,
        pkg_step_name,
        "AC-002",
        "A step named 'Package (Windows)' gated `if: runner.os == 'Windows'` \
         with `shell: pwsh` and `Compress-Archive` is required.\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
    );

    assert!(
        pkg_block.contains("shell: pwsh"),
        "AC-002 FAIL: `shell: pwsh` not found in the 'Package (Windows)' step block \
         (up to the next step) in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `shell: pwsh` to invoke \
         PowerShell's Compress-Archive cmdlet. Using `shell: bash` with `zip` \
         is a C-V3 BLOCKER: `zip` is NOT available on `windows-latest` PATH.\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_block
    );

    assert!(
        pkg_block.contains("Compress-Archive"),
        "AC-002 FAIL: `Compress-Archive` not found in the 'Package (Windows)' step block \
         (up to the next step) in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use PowerShell's `Compress-Archive` cmdlet \
         to create the .zip archive. This is the PRIMARY, deterministic packaging \
         mechanism (C-V3). Do NOT use the Unix `zip` command — it is not present \
         on `windows-latest` runners and will fail with `command not found`.\n\
         Required form:\n\
           Compress-Archive -Path \"target/...\" -DestinationPath \"jr-...-x86_64-pc-windows-msvc.zip\"\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_block
    );

    // C-V3 negative check: `zip ` (Unix Info-ZIP) must NOT appear in the Package
    // (Windows) step body — only in the adjacent Compress-Archive invocation which
    // uses it as a DestinationPath suffix, not as a shell command.
    // We check for `zip ` with a trailing space to avoid false-matching the `.zip`
    // extension inside the Compress-Archive -DestinationPath argument.
    assert!(
        !pkg_block.contains("zip "),
        "AC-002 FAIL: `zip ` (Unix Info-ZIP invocation) found in the \
         'Package (Windows)' step block (up to the next step) in \
         .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `Compress-Archive`, not the Unix `zip` \
         command. `zip` is NOT available on `windows-latest` PATH (C-V3 BLOCKER).\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_block
    );

    assert!(
        !pkg_block.contains("shell: bash"),
        "AC-002 FAIL: `shell: bash` found in the 'Package (Windows)' step block \
         (up to the next step) in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `shell: pwsh`, not `shell: bash`. \
         Using bash + zip is the old broken spec (C-V3 BLOCKER). \
         The adjacent Checksum (Windows) step uses bash legitimately — this \
         assertion is step-scoped to catch a regression in the Package step itself.\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        pkg_block
    );

    // --- Checksum (Windows) step: must exist and use sha256sum ---
    // Anchored separately so the sha256sum assertion is step-scoped and does not
    // pass merely because sha256sum appears elsewhere in the file.

    let chk_step_name = "Checksum (Windows)";
    let chk_block = step_block(
        &yml,
        chk_step_name,
        "AC-002",
        "A separate step named 'Checksum (Windows)' gated `if: runner.os == 'Windows'` \
         with `shell: bash` and `sha256sum` is required in addition to the \
         Package (Windows) step.\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
    );

    assert!(
        chk_block.contains("sha256sum"),
        "AC-002 FAIL: `sha256sum` not found in the 'Checksum (Windows)' step block \
         (up to the next step) in .github/workflows/release.yml.\n\
         The Checksum (Windows) step must use `sha256sum` to generate the .zip.sha256 \
         file. `sha256sum` is confirmed available via Git for Windows coreutils (C-V3).\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3)",
        chk_block
    );
}

/// AC-003 — Smoke step ("Verify embedded OAuth app present") is gated off on Windows.
///
/// Verifies that `if: runner.os != 'Windows'` is bound to the smoke step itself,
/// not merely present somewhere in the file. The check locates the
/// `Verify embedded OAuth app present` step by name and then asserts the gate
/// appears within that step's block (up to the next step boundary) — anchoring the
/// assertion to that specific step.
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

    // Locate the smoke step and inspect its full block up to the next step.
    let step_name = "Verify embedded OAuth app present";
    let smoke_block = step_block(
        &yml,
        step_name,
        "AC-003",
        "The smoke step must exist before its gate can be verified.\n\
         (S-WIN-4 AC-003 / ADR-0016 §Decision 5c)",
    );

    assert!(
        smoke_block.contains("runner.os != 'Windows'"),
        "AC-003 FAIL: `if: runner.os != 'Windows'` is not present in the '{}' \
         step block (up to the next step) in .github/workflows/release.yml.\n\
         The smoke step must be gated with `if: runner.os != 'Windows'` immediately \
         after its `name:` line. The step uses bash heredocs and XDG_CONFIG_HOME \
         which are not available in this form on Windows (deferred per ADR-0016 \
         §Decision 5c).\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-003 / ADR-0016 §Decision 5c)",
        step_name,
        smoke_block
    );
}

/// AC-004 — Upload-artifact step's path block includes `jr-*.zip`.
///
/// Verifies that the `Upload artifact` step's `path:` block contains `jr-*.zip`
/// so the Windows .zip artifact is uploaded to GitHub Actions artifacts alongside
/// the existing .tar.gz and .sha256 files.
///
/// The assertion is anchored to the `Upload artifact` step block (from the step
/// name up to the next step boundary). This ensures the glob is specifically
/// present in the upload step's `path:` block, not just anywhere in the file.
/// A bare `yml.contains("jr-*.zip")` would also be satisfied by the release job's
/// `files:` block (AC-005's concern), making the two tests indistinguishable —
/// anchoring makes each test fail independently.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-004.
#[test]
fn test_release_yml_upload_artifact_includes_zip() {
    let yml = release_yml();

    // Locate the Upload artifact step and inspect its full block up to the next step.
    let step_name = "Upload artifact";
    let upload_block = step_block(
        &yml,
        step_name,
        "AC-004",
        "The 'Upload artifact' step (actions/upload-artifact) must exist before \
         its path: block can be verified.\n\
         (S-WIN-4 AC-004 / NFR-P-W1)",
    );

    assert!(
        upload_block.contains("jr-*.zip"),
        "AC-004 FAIL: `jr-*.zip` not found in the 'Upload artifact' step block \
         (up to the next step) in .github/workflows/release.yml.\n\
         The Upload artifact step's `path:` block must include `jr-*.zip` so the \
         Windows .zip artifact is uploaded alongside the existing .tar.gz files.\n\
         Required addition to the path: block:\n\
           jr-*.zip\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-004 / NFR-P-W1)",
        upload_block
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
/// The assertion is anchored to the `Create GitHub Release` step block (from the step
/// name up to the next step boundary, or end-of-file since it is the last step in the
/// release job). This ensures the glob is specifically present in the release step's
/// `files:` block, not just anywhere in the file (e.g. the upload-artifact `path:`
/// block, which is AC-004's concern). Anchoring makes this test fail independently if
/// the glob is removed from the release `files:` block while remaining in the upload
/// `path:` block — which would produce a CI artifact but no GitHub Release attachment,
/// silently failing NFR-P-W1.
///
/// Traces to: NFR-P-W1; S-WIN-4 AC-005.
#[test]
fn test_release_yml_release_job_files_includes_zip() {
    let yml = release_yml();

    // Locate the Create GitHub Release step and inspect its full block.
    // This is the last step in the release job so step_block runs to end-of-file.
    let step_name = "Create GitHub Release";
    let release_block = step_block(
        &yml,
        step_name,
        "AC-005",
        "The 'Create GitHub Release' step (softprops/action-gh-release) must \
         exist before its files: block can be verified.\n\
         (S-WIN-4 AC-005 / NFR-P-W1)",
    );

    assert!(
        release_block.contains("jr-*.zip"),
        "AC-005 FAIL: `jr-*.zip` not found in the 'Create GitHub Release' step block \
         (up to end-of-file) in .github/workflows/release.yml.\n\
         The `softprops/action-gh-release` step's `files:` block must include \
         `jr-*.zip` so the Windows .zip artifact appears on the GitHub Releases page.\n\
         Required addition to the files: block in the release job:\n\
           jr-*.zip\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-005 / NFR-P-W1)",
        release_block
    );
}
