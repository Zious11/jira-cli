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
    let mut windows_smoke_block: Option<String> = None;
    for (i, &start) in step_starts.iter().enumerate() {
        let end = step_starts.get(i + 1).copied().unwrap_or(yml.len());
        let block = &yml[start..end];

        // Must EXECUTE jr.exe (not merely reference it as a file path argument).
        if !step_executes_jr_exe(block) {
            continue;
        }

        // Must NOT be excluded on Windows.
        if block.contains("runner.os != 'Windows'") {
            continue;
        }

        // Must be Windows-applicable: either a Windows-only guard or pwsh shell.
        if block.contains("runner.os == 'Windows'") || block.contains("shell: pwsh") {
            windows_smoke_block = Some(block.to_string());
            break;
        }
    }

    assert!(
        windows_smoke_block.is_some(),
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

    // R2-001: assert the smoke step contains fail-closed exit-code plumbing.
    //
    // In PowerShell 7 (pwsh), `$ErrorActionPreference = 'Stop'` does NOT cause a
    // non-zero exit code from a native executable (like jr.exe) to fail the step.
    // The explicit `if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }` guard is
    // load-bearing: without it, a crash in jr.exe would be silently swallowed and
    // the smoke step would pass regardless. This assertion pins that plumbing so a
    // mutation deleting the LASTEXITCODE/exit line turns this test RED.
    //
    // Both substrings must appear in the same step block already located above.
    let smoke_block = windows_smoke_block.as_deref().unwrap();

    assert!(
        smoke_block.contains("LASTEXITCODE"),
        "R2-001 FAIL: `LASTEXITCODE` not found in the Windows smoke step block in \
         .github/workflows/release.yml.\n\
         \n\
         In PowerShell 7 (pwsh), `$ErrorActionPreference = 'Stop'` does NOT propagate \
         a non-zero exit code from native executables — a crashing jr.exe would be \
         silently ignored. The step MUST contain an explicit LASTEXITCODE check:\n\
           if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\n\
         Without it, the smoke step passes even when jr.exe crashes (WIN-STACK).\n\
         Step block:\n{}\n\
         (FIX-F5-002 R2-001 / WIN-STACK / ADR-0016 §Decision 5c)",
        smoke_block
    );

    assert!(
        smoke_block.lines().any(|line| {
            let t = line.trim();
            // Match the fail-closed exit line: `if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }`
            // Require both "LASTEXITCODE" and "exit" on the same line so a mutation that
            // deletes this specific line (while leaving a bare `$LASTEXITCODE` reference
            // elsewhere) still turns this assertion RED.
            t.contains("LASTEXITCODE") && t.contains("exit")
        }),
        "R2-001 FAIL: no line containing both `LASTEXITCODE` and `exit` found in the \
         Windows smoke step block in .github/workflows/release.yml.\n\
         \n\
         The step must contain the fail-closed exit guard on a single line:\n\
           if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\n\
         This is required because pwsh does not fail the step on non-zero native-binary \
         exit codes without it. Deleting this line causes the smoke step to silently \
         swallow a jr.exe crash (WIN-STACK regression).\n\
         Step block:\n{}\n\
         (FIX-F5-002 R2-001 / WIN-STACK / ADR-0016 §Decision 5c)",
        smoke_block
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

/// R3-001 — A Windows-applicable step verifies that the embedded OAuth app is present in
/// the release binary.
///
/// ## Problem being guarded
///
/// The build step injects `JR_BUILD_OAUTH_CLIENT_ID` / `JR_BUILD_OAUTH_CLIENT_SECRET` for
/// ALL matrix targets unconditionally, including `x86_64-pc-windows-msvc`. However, the
/// only step that checks whether embedded credentials actually landed in the binary —
/// "Verify embedded OAuth app present" — is gated `if: runner.os != 'Windows'`, so it
/// never runs on the Windows runner. A misconfigured build.rs on the Windows runner
/// (e.g., env-vars silently dropped) could silently ship an unbranded `jr.exe` with
/// green CI (FIX-F5-003 R3-001).
///
/// ## What this test asserts
///
/// There must exist at least one step in `release.yml` that is ALL of:
///
/// 1. **Windows-applicable** — NOT gated with `if: runner.os != 'Windows'`.
///    A step present but excluded on Windows provides zero verification there.
/// 2. **Checks for embedded OAuth credentials** — inspects `embedded_oauth.rs` for
///    non-None constants (the `EMBEDDED_ID`/`EMBEDDED_SECRET_XOR` = None checks) OR
///    runs `jr.exe auth status` and confirms the `embedded` label in its output.
///    The Windows smoke step (`.\jr.exe --version`) satisfies condition 1 but NOT
///    condition 2 — it only tests launchability, not OAuth embedding.
/// 3. **Fork-safe** — gated on a `HAS_EMBED_SECRETS` condition (or equivalent) so
///    forks where `OAUTH_CLIENT_ID`/`OAUTH_CLIENT_SECRET` are unavailable cleanly
///    skip the check rather than hard-failing (mirrors the Unix verify step's
///    `HAS_EMBED_SECRETS` pattern).
///
/// The test FAILS against the current release.yml (which has no such Windows step)
/// and PASSES once one is added.
///
/// Traces to: FIX-F5-003 R3-001; ADR-0006 (embedded OAuth); ADR-0016 §Decision 5c.
#[test]
fn test_release_yml_verifies_embedded_oauth_on_windows() {
    let yml = release_yml();

    // Strategy: scan every step block in the file looking for one that:
    //   (a) Is NOT excluded on Windows (does not carry `runner.os != 'Windows'`)
    //   (b) Checks embedded OAuth credentials (inspects embedded_oauth.rs None-constants
    //       OR validates `jr.exe auth status` output contains `embedded`)
    //   (c) Is gated on build-secret presence (HAS_EMBED_SECRETS or equivalent)
    //       so forks without secrets skip cleanly
    //
    // The existing "Verify embedded OAuth app present" step FAILS criterion (a) —
    // it carries `if: runner.os != 'Windows'`. The "Smoke test (Windows)" step
    // FAILS criterion (b) — it only runs `.\jr.exe --version`, not an OAuth check.

    let step_marker = "\n      - name:";

    // Collect starting offsets of all step blocks in the file.
    let mut step_starts: Vec<usize> = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = yml[search_from..].find(step_marker) {
        let abs = search_from + rel + 1; // +1: skip the leading '\n', point at '-'
        step_starts.push(abs);
        search_from = abs + 1;
    }

    /// Returns true if `block` inspects `embedded_oauth.rs` for None-constant
    /// guards (the primary Windows-compatible check that does not require executing
    /// a cross-compiled binary).
    ///
    /// # R8-001 hardening (tightened from R5-001)
    ///
    /// A pure symbol-presence check (`block.contains("EMBEDDED_ID")`) is too weak:
    /// a future edit that comments out the None-match or inverts the regex would still
    /// pass. R5-001 tightened this to require `= None` alongside a constant name, but
    /// that is still insufficient: the step's `Write-Error` MESSAGE strings also contain
    /// `= None` (e.g. `'…EMBEDDED_ID = None — build.rs…'`), so neutering the actual
    /// `-match` detection lines while leaving the prose strings intact would produce a
    /// false-green on the R5-001 check.
    ///
    /// R8-001 binds to the actual detection CONSTRUCT by requiring at least one line
    /// that contains ALL THREE of:
    ///   - `-match`                      (PowerShell regex-match operator)
    ///   - `EMBEDDED_ID` OR `EMBEDDED_SECRET_XOR`  (the constant name)
    ///   - `None`                        (the sentinel value being tested)
    ///
    /// This isolates the assertion to the regex-match lines
    /// (`$content -match 'EMBEDDED_ID\s*:\s*Option.*=\s*None'`) and is immune to
    /// the prose `Write-Error` message strings that also happen to contain `= None`.
    ///
    /// (b) A fail-closed path — either `exit 1` or `Write-Error`, ensuring that when
    ///     None is detected the step actually fails. A step that detects None but only
    ///     emits a warning (or inverts the condition) would still pass without (b).
    ///
    /// The current release.yml "Embedded OAuth verification (Windows)" step satisfies
    /// both criteria (verified):
    ///   - Contains `-match … EMBEDDED_ID … None` on a single detection line
    ///   - Contains `Write-Error` and `exit 1` (the fail branch for each None match)
    fn block_checks_embedded_oauth_rs(block: &str) -> bool {
        // Criterion (a): None-detection CONSTRUCT — require at least one line in the
        // block that contains ALL of: the `-match` operator, a constant name
        // (EMBEDDED_ID or EMBEDDED_SECRET_XOR), and `None`. This binds to the actual
        // PowerShell regex-match lines rather than the Write-Error prose strings that
        // also contain `= None`, closing the R5-001 false-green vector (R8-001).
        let has_none_detection = block.lines().any(|line| {
            line.contains("-match")
                && (line.contains("EMBEDDED_ID") || line.contains("EMBEDDED_SECRET_XOR"))
                && line.contains("None")
        });

        // Criterion (b): Fail-closed path — the block must contain a hard-failure
        // mechanism so that detecting None actually causes the step to fail rather
        // than silently passing with a warning.
        let has_fail_path = block.contains("exit 1") || block.contains("Write-Error");

        has_none_detection && has_fail_path
    }

    /// Returns true if `block` runs `jr.exe auth status` and checks that the
    /// output contains `embedded` — the alternative Windows-executable check.
    fn block_checks_auth_status_embedded(block: &str) -> bool {
        // A step that runs `jr.exe auth status` AND checks for "embedded" in output
        // satisfies the OAuth-embedding verification requirement.
        (block.contains("jr.exe") || block.contains("jr.exe auth"))
            && block.contains("auth status")
            && block.contains("embedded")
    }

    /// Returns true if `block` uses a fork-skip gate equivalent to the Unix step's
    /// `HAS_EMBED_SECRETS` pattern.
    fn block_is_fork_safe(block: &str) -> bool {
        // The canonical fork-skip pattern is HAS_EMBED_SECRETS (set in the step's
        // env: block and checked in the run: script). Accept any of:
        //   - HAS_EMBED_SECRETS (exact match — mirrors the Unix step)
        //   - OAUTH_CLIENT_ID (checking the secret directly in a condition)
        //   - secrets.OAUTH_CLIENT_ID (GitHub expression form in an `if:`)
        block.contains("HAS_EMBED_SECRETS")
            || block.contains("OAUTH_CLIENT_ID")
            || block.contains("secrets.OAUTH_CLIENT_ID")
    }

    let mut found_windows_oauth_verify = false;
    let mut found_step_name = String::new();

    for (i, &start) in step_starts.iter().enumerate() {
        let end = step_starts.get(i + 1).copied().unwrap_or(yml.len());
        let block = &yml[start..end];

        // Criterion (a): must NOT be excluded on Windows.
        if block.contains("runner.os != 'Windows'") {
            continue;
        }

        // Criterion (b): must check embedded OAuth credentials.
        if !block_checks_embedded_oauth_rs(block) && !block_checks_auth_status_embedded(block) {
            continue;
        }

        // Criterion (c): must be fork-safe.
        if !block_is_fork_safe(block) {
            continue;
        }

        // All three criteria satisfied — record the step name for the error message.
        // Step blocks start with "- name: <step_name>"; extract the name for diagnostics.
        let name_line = block
            .lines()
            .find(|l| l.trim_start().starts_with("- name:"))
            .unwrap_or("(unknown step)");
        found_step_name = name_line.trim().to_string();
        found_windows_oauth_verify = true;
        break;
    }

    assert!(
        found_windows_oauth_verify,
        "R3-001 FAIL: no Windows-applicable embedded-OAuth verification step found in \
         .github/workflows/release.yml.\n\
         \n\
         The build step injects JR_BUILD_OAUTH_CLIENT_ID/JR_BUILD_OAUTH_CLIENT_SECRET \
         for ALL matrix targets including x86_64-pc-windows-msvc, but the only step \
         that verifies embedded credentials — 'Verify embedded OAuth app present' — is \
         gated `if: runner.os != 'Windows'` and never runs on Windows. A misconfigured \
         build.rs on the Windows runner could silently ship an unbranded jr.exe.\n\
         \n\
         A Windows-applicable verification step must satisfy ALL of:\n\
           1. NOT gated with `if: runner.os != 'Windows'`\n\
           2. Checks embedded OAuth presence — either:\n\
                (a) Inspects embedded_oauth.rs for EMBEDDED_ID/EMBEDDED_SECRET_XOR = None,\n\
                    using PowerShell Select-String or equivalent, OR\n\
                (b) Runs `jr.exe auth status` and confirms the output contains 'embedded'\n\
           3. Fork-safe: gated on HAS_EMBED_SECRETS (or secrets.OAUTH_CLIENT_ID)\n\
              so forks without build secrets skip cleanly rather than hard-failing\n\
         \n\
         The existing 'Smoke test (Windows)' step satisfies criterion 1 (Windows-only)\n\
         but fails criterion 2 — it only runs `.\\jr.exe --version` (launchability),\n\
         not an OAuth embedding check.\n\
         \n\
         Example fix: add a step such as:\n\
           - name: Verify embedded OAuth app present (Windows)\n\
             if: runner.os == 'Windows'\n\
             shell: pwsh\n\
             env:\n\
               HAS_EMBED_SECRETS: ${{ (secrets.OAUTH_CLIENT_ID != '' && \
         secrets.OAUTH_CLIENT_SECRET != '') && 'yes' || 'no' }}\n\
             run: |\n\
               if ($env:HAS_EMBED_SECRETS -ne 'yes') {{ exit 0 }}\n\
               $f = Get-ChildItem -Path \"target/${{ matrix.target }}/release/build\" \\\n\
                     -Filter 'embedded_oauth.rs' -Recurse | Select-Object -First 1\n\
               if (-not $f) {{ Write-Error 'embedded_oauth.rs not found'; exit 1 }}\n\
               $content = Get-Content $f.FullName -Raw\n\
               if ($content -match 'EMBEDDED_ID.*=.*None') \\\n\
                 {{ Write-Error 'EMBEDDED_ID is None'; exit 1 }}\n\
               if ($content -match 'EMBEDDED_SECRET_XOR.*=.*None') \\\n\
                 {{ Write-Error 'EMBEDDED_SECRET_XOR is None'; exit 1 }}\n\
               Write-Host 'embedded_oauth.rs has populated constants'\n\
         \n\
         (FIX-F5-003 R3-001 / ADR-0006 embedded OAuth / ADR-0016 §Decision 5c)"
    );

    // If we got here, a valid step was found — report it for CI log clarity.
    // (This branch is only reached when the test PASSES, i.e. after the fix is in.)
    let _ = found_step_name; // suppress unused-variable lint in the Red state
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
