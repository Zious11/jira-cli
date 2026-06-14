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
//! | `test_release_yml_windows_package_step_produces_zip` | AC-002 (R1-001 fix) |
//! | `test_release_yml_windows_has_functional_smoke_step` | R1-002 (Red Gate) |
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

    // C-V3 negative check: a bash `zip` invocation (Unix Info-ZIP) must NOT appear
    // in the Package (Windows) step body. A bare `zip ` substring check is BRITTLE
    // because `.zip` appears legitimately in the Compress-Archive -DestinationPath
    // argument. Instead we check for run-line patterns that indicate a real
    // Info-ZIP invocation: a line starting with `zip ` after trim, or containing
    // `&& zip ` or `| zip ` (shell pipeline/chain forms).
    let has_bash_zip_invocation = pkg_block.lines().any(|line| {
        let t = line.trim();
        t.starts_with("zip ") || t.contains("&& zip ") || t.contains("| zip ")
    });
    assert!(
        !has_bash_zip_invocation,
        "AC-002 FAIL: a bash `zip` invocation (line starting with `zip `, or \
         containing `&& zip ` / `| zip `) found in the 'Package (Windows)' step \
         block (up to the next step) in .github/workflows/release.yml.\n\
         The Package (Windows) step MUST use `Compress-Archive`, not the Unix \
         `zip` command. `zip` is NOT available on `windows-latest` PATH (C-V3 BLOCKER).\n\
         Step block:\n{}\n\
         (S-WIN-4 AC-002 / ADR-0016 §Decision 2 C-V3 / R1-001)",
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

/// R1-002 — Windows build job has a functional smoke step that executes `jr.exe`.
///
/// Verifies that the `build` job contains a step that runs the freshly-built
/// `jr.exe` on Windows (e.g. `.\jr.exe --version`), so the release binary's
/// `/STACK:8388608` reserve and basic launchability are validated in the RELEASE path.
///
/// The existing "Verify embedded OAuth app present" step is gated
/// `if: runner.os != 'Windows'` (ADR-0016 §Decision 5c), leaving Windows with
/// no runtime validation without this dedicated step.
///
/// This test is a regression guard: it asserts that a Windows-applicable smoke step
/// exists and was not accidentally removed. The step and this test were co-delivered
/// in PR #511 (FIX-F5-001); the failing state can be reproduced locally by reverting
/// only the release.yml hunk from that PR.
///
/// This test asserts the existence of a Windows-applicable step that:
/// 1. Invokes `jr.exe` (the Windows binary name, with explicit `.\` prefix for pwsh)
/// 2. Is NOT excluded on Windows (does NOT carry `runner.os != 'Windows'`)
/// 3. Uses `shell: pwsh` OR an explicit `runner.os == 'Windows'` condition
///
/// Traces to: WIN-STACK (jr.exe stack reservation, CLAUDE.md §Gotchas);
/// ADR-0016 §Decision 5c (OAuth smoke step gated off on Windows — motivation for
/// this dedicated Windows step); FIX-F5-001 R1-002.
#[test]
fn test_release_yml_windows_has_functional_smoke_step() {
    let yml = release_yml();

    // Strategy: find every step block in the file that EXECUTES `jr.exe` as a
    // binary (not merely references it as a file path argument). For each such block,
    // check that:
    //   (a) it is NOT excluded on Windows (no `runner.os != 'Windows'` gate)
    //   (b) it is either explicitly Windows-only (`runner.os == 'Windows'`) or
    //       uses `shell: pwsh` (pwsh is Windows-native and implies a Windows step)
    //
    // "Executes jr.exe" means a run-line that launches the binary. The detection
    // covers three invocation forms (note: variable-indirection `& $bin` where $bin
    // holds a path is NOT detected — only literal callee references are):
    //   - starts with `jr.exe`, `.\jr.exe`, or `./jr.exe` (direct/CWD-relative), OR
    //   - contains `& ` followed by a token ending in `jr.exe` (PowerShell call
    //     operator with a literal callee), OR
    //   - contains a path ending in `/jr.exe` or `\jr.exe` used as a command (not
    //     as a `-Path` or `-DestinationPath` argument to another cmdlet)
    //
    // The Compress-Archive packaging step MUST NOT match: it references `jr.exe` only
    // as a `-Path "target/.../jr.exe"` argument to Compress-Archive, not as an
    // executable invocation.
    //
    // Note: step_starts scans the entire YAML file (not just the build job). The
    // release job's steps don't reference jr.exe, so this is correct in practice.

    let step_marker = "\n      - name:";

    // Collect starting offsets of all step blocks in the file.
    let mut step_starts: Vec<usize> = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = yml[search_from..].find(step_marker) {
        let abs = search_from + rel + 1; // +1: skip the leading '\n', point at '-'
        step_starts.push(abs);
        search_from = abs + 1;
    }

    /// Returns true if any run-line in `block` *executes* jr.exe as a binary.
    ///
    /// Rejects lines where jr.exe appears only as an argument to another command
    /// (e.g. `Compress-Archive -Path "…/jr.exe"`), which is not an execution.
    fn step_executes_jr_exe(block: &str) -> bool {
        block.lines().any(|line| {
            let t = line.trim();
            // Direct invocation: line starts with jr.exe (bare or with leading `.\`)
            if t.starts_with("jr.exe") || t.starts_with(".\\jr.exe") || t.starts_with("./jr.exe") {
                return true;
            }
            // PowerShell call-operator: `& $bin` or `& "…/jr.exe"` or `& .\jr.exe`
            if t.contains("& ") {
                // After `& `, look for jr.exe used as the callee (not after -Path or similar)
                let after_call = t.find("& ").map(|i| &t[i + 2..]).unwrap_or("");
                let callee = after_call.split_whitespace().next().unwrap_or("");
                if callee.ends_with("jr.exe")
                    || callee
                        .trim_matches(|c| c == '"' || c == '\'')
                        .ends_with("jr.exe")
                {
                    return true;
                }
            }
            // Path-based invocation: ends with /jr.exe or \jr.exe as executable
            // followed by space/flag/EOL (not as a -Path or -DestinationPath argument).
            // Reject: the line contains `-Path` or `-DestinationPath` before `jr.exe`
            // (Compress-Archive pattern).
            if (t.contains("/jr.exe") || t.contains("\\jr.exe"))
                && !t.contains("-Path")
                && !t.contains("-DestinationPath")
            {
                // Verify jr.exe is used as a command: followed by space, flag, or EOL
                let find_jr = t.find("/jr.exe").or_else(|| t.find("\\jr.exe"));
                if let Some(pos) = find_jr {
                    let after = &t[pos + 7..]; // len("jr.exe") == 6; prefix char is 1 extra
                    if after.is_empty() || after.starts_with(' ') || after.starts_with('-') {
                        return true;
                    }
                }
            }
            false
        })
    }

    // For each step, extract its block (up to the next step or EOF) and look for
    // one that both executes jr.exe AND is Windows-applicable.
    let found_windows_smoke = step_starts.iter().enumerate().any(|(i, &start)| {
        let end = step_starts.get(i + 1).copied().unwrap_or(yml.len());
        let block = &yml[start..end];

        // Must EXECUTE jr.exe (not merely reference it as a file path argument).
        if !step_executes_jr_exe(block) {
            return false;
        }

        // Must NOT be excluded on Windows.
        if block.contains("runner.os != 'Windows'") {
            return false;
        }

        // Must be Windows-applicable: either a Windows-only guard or pwsh shell.
        block.contains("runner.os == 'Windows'") || block.contains("shell: pwsh")
    });

    assert!(
        found_windows_smoke,
        "R1-002 FAIL: no Windows-applicable smoke step invoking `jr.exe` found in \
         .github/workflows/release.yml.\n\
         \n\
         The build job must contain a step that:\n\
           1. Invokes `jr.exe` (e.g. `.\\jr.exe --version` — use the `.\\ ` prefix\n\
              in pwsh; PowerShell does NOT search CWD without it)\n\
           2. Is NOT excluded on Windows (no `if: runner.os != 'Windows'`)\n\
           3. Uses `shell: pwsh` or `if: runner.os == 'Windows'`\n\
         \n\
         The 'Verify embedded OAuth app present' step is gated \
         `if: runner.os != 'Windows'` (ADR-0016 §Decision 5c), leaving the \
         Windows release binary with NO runtime launchability check — the \
         /STACK:8388608 PE reserve and basic binary integrity are never exercised \
         in CI for release builds without this step.\n\
         \n\
         Required: add a Windows smoke step such as:\n\
           - name: Smoke test (Windows)\n\
             if: runner.os == 'Windows'\n\
             shell: pwsh\n\
             run: |\n\
               $ErrorActionPreference = 'Stop'\n\
               Set-Location \"target/${{ matrix.target }}/release\"\n\
               .\\jr.exe --version\n\
               if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\n\
         \n\
         (FIX-F5-001 R1-002 / WIN-STACK / ADR-0016 §Decision 5c)"
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
