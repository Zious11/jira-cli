//! Anti-recurrence guard for the `JR_RUN_KEYRING_TESTS` gate idiom.
//!
//! The canonical keyring-test guard is:
//! ```rust
//! if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") { ... }
//! ```
//!
//! The loose form `JR_RUN_KEYRING_TESTS").is_err()` is semantically wrong:
//! it skips only when the var is *unset*, so `JR_RUN_KEYRING_TESTS=0` (or
//! `=false`) would incorrectly *run* the keyring test, violating the
//! documented "`=1` to run" contract. This was CR-009 / KEYRING-GUARD-IDIOM-DRIFT.
//!
//! This file enforces the canonical form going forward by scanning every
//! `.rs` file under `src/` and `tests/` and failing if any line contains
//! `JR_RUN_KEYRING_TESTS` adjacent to `.is_err()`.

use std::path::Path;

/// Walk a directory tree and collect all `.rs` files.
fn collect_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

/// Returns true if the line contains both `JR_RUN_KEYRING_TESTS` and `.is_err()`.
///
/// Self-match is prevented by the `files.retain` call in the test, which
/// excludes this file by name before scanning begins.
fn line_uses_loose_guard(line: &str) -> bool {
    line.contains("JR_RUN_KEYRING_TESTS") && line.contains(".is_err()")
}

#[test]
fn test_keyring_guard_idiom_no_loose_is_err_form() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let tests_dir = manifest_dir.join("tests");

    // The name of this file — excluded from the scan so doc-comment examples
    // inside it don't trigger on themselves.
    let self_name = "keyring_guard_idiom.rs";

    let mut files = collect_rs_files(&src_dir);
    files.extend(collect_rs_files(&tests_dir));
    // Exclude this guard file itself (its doc comments reference the pattern).
    files.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n != self_name)
            .unwrap_or(true)
    });
    files.sort();

    let mut violations: Vec<String> = Vec::new();

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        for (line_no, line) in content.lines().enumerate() {
            if line_uses_loose_guard(line) {
                let rel = path
                    .strip_prefix(manifest_dir)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                violations.push(format!("{}:{}: {}", rel, line_no + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "KEYRING-GUARD-IDIOM-DRIFT: found {} loose `JR_RUN_KEYRING_TESTS` guard(s) \
         using `.is_err()` (runs when var=0/false). \
         Use `as_deref() != Ok(\"1\")` instead:\n{}",
        violations.len(),
        violations.join("\n")
    );
}
