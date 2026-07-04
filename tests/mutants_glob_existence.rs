//! Guard 3 (DEC-150): validates every `examine_globs` entry in `.cargo/mutants.toml`
//! resolves to at least one real file via `glob::glob()` expansion.
//!
//! S-MUTANTS-SCOPE-GUARDS-1 / BC-5.38.001 todo!() stubs.
//! Helper bodies are todo!() per Red Gate discipline; test functions call the helpers
//! (ensuring helpers are not dead code) and fail RED because the helpers panic.
//!
//! Full test names follow `test_<verb>_<subject>_<expected_outcome>` convention per
//! `docs/specs/test-naming-convention.md`.

// ---------------------------------------------------------------------------
// Guard 3 helper functions — all todo!() per BC-5.38.001
// ---------------------------------------------------------------------------

/// Given a list of glob patterns, runs `glob::glob()` expansion on each and
/// returns the list of patterns that matched zero files.
///
/// BC-5.38.001: non-trivial body (I/O via glob, branching on results) — todo!() required.
/// BC-5.38.005 self-check: "If I include this real implementation, will the test for
/// this function pass trivially without any implementer work?" → YES (returns wrong value)
/// → must remain todo!().
fn validate_globs(entries: &[String]) -> Vec<String> {
    // Implementation: for each entry, expand
    //   glob::glob(&format!("{}/{}", env!("CARGO_MANIFEST_DIR").replace('\\', "/"), entry))
    // collecting entries where the pattern matched zero files.
    // Windows path note (F-C1 FIX): CARGO_MANIFEST_DIR uses backslashes on Windows;
    //   .replace('\\', "/") normalises before passing to glob 0.3.
    let _ = entries;
    todo!("validate_globs: not yet implemented — see S-MUTANTS-SCOPE-GUARDS-1")
}

/// Extracts the `examine_globs` array from a parsed TOML Value.
///
/// Panics with `MUTANTS-GLOBS-KEY-MISSING: examine_globs key not found in
/// .cargo/mutants.toml — key renamed, section restructured, or examine_globs
/// is present but empty` when the resulting Vec would be empty (key absent or
/// renamed, or array is empty).
///
/// BC-5.38.001: non-trivial body (TOML traversal, conditional panic) — todo!() required.
/// BC-5.38.005 self-check: returning todo!() prevents tests 1 and 4 from passing.
fn extract_examine_globs_or_panic(value: &toml::Value) -> Vec<String> {
    let _ = value;
    todo!("extract_examine_globs_or_panic: not yet implemented — see S-MUTANTS-SCOPE-GUARDS-1")
}

/// Asserts the `examine_globs` entry count meets the coverage floor.
///
/// Uses a single-source `const FLOOR: usize = 11` (MED-1-P22 FIX) in both the
/// comparison and the panic message format string so they cannot diverge.
///
/// Panics with `MUTANTS-GLOBS-COVERAGE-FLOOR: expected >= {FLOOR} examine_globs
/// entries, got {N}. Update this PIN when entries are intentionally removed
/// (the floor is a lower bound; additions never fire it).` when entries.len() < FLOOR.
///
/// // PIN: update when examine_globs adds/removes entries
///
/// BC-5.38.001: non-trivial body (comparison, conditional panic) — todo!() required.
/// BC-5.38.005 self-check: returning todo!() prevents tests 1 and 6 from passing.
fn assert_examine_globs_coverage_floor(entries: &[String]) {
    let _ = entries;
    todo!("assert_examine_globs_coverage_floor: not yet implemented — see S-MUTANTS-SCOPE-GUARDS-1")
}

// ---------------------------------------------------------------------------
// Test functions — full bodies calling todo!() helpers → all RED per Red Gate
// ---------------------------------------------------------------------------

/// Test 1: real-data canonical run — all 11 current examine_globs entries
/// resolve to real files; dead list is empty; coverage floor does not panic.
#[test]
fn test_resolve_all_examine_globs_entries_to_real_files() {
    let toml_src = include_str!("../.cargo/mutants.toml");
    let value = toml::from_str::<toml::Value>(toml_src).unwrap();
    // Helper panics with MUTANTS-GLOBS-KEY-MISSING if key absent or empty.
    let entries = extract_examine_globs_or_panic(&value);
    // Helper panics with MUTANTS-GLOBS-COVERAGE-FLOOR if entries.len() < FLOOR.
    assert_examine_globs_coverage_floor(&entries);
    let dead = validate_globs(&entries);
    assert!(dead.is_empty(), "examine_globs entries resolve to no files: {:?}", dead);
}

/// Test 2: seeded-failure — nonexistent pattern returns a non-empty dead list.
/// Content pin confirms the exact dead pattern string appears (F-VA-28-5 FIX).
#[test]
fn test_reject_nonexistent_examine_globs_entry_returns_dead_list() {
    let dead = validate_globs(&["src/nonexistent_dummy_for_selftest.rs".to_string()]);
    assert!(!dead.is_empty(), "expected dead list to be non-empty");
    assert!(
        dead.iter().any(|p| p.contains("nonexistent_dummy_for_selftest")),
        "expected dead list to contain 'nonexistent_dummy_for_selftest', got: {:?}",
        dead
    );
}

/// Test 3: polarity mutant killer — inline TOML parse → extract → validate_globs
/// returns non-empty dead list with content pin (pass-3 M-1 FIX).
#[test]
fn test_validate_globs_via_toml_parse_returns_dead_entry() {
    let mock_toml = r#"examine_globs = ["src/nonexistent_dummy_for_selftest.rs"]"#;
    let value = toml::from_str::<toml::Value>(mock_toml).unwrap();
    // MUST use shared helper (tests 3/4/5/7/8 all call extract_examine_globs_or_panic).
    let entries = extract_examine_globs_or_panic(&value);
    let dead = validate_globs(&entries);
    assert!(!dead.is_empty(), "expected dead list to be non-empty");
    assert!(
        dead.iter().any(|p| p.contains("nonexistent_dummy_for_selftest")),
        "expected dead list to contain 'nonexistent_dummy_for_selftest', got: {:?}",
        dead
    );
}

/// Test 4: key-absent case — extract_examine_globs_or_panic panics with
/// MUTANTS-GLOBS-KEY-MISSING when examine_globs key is missing (F-MED-2 FIX / L-2 FIX).
#[test]
fn test_detect_missing_examine_globs_key_panics_with_key_missing_message() {
    let mock_toml = r#"foo = ["bar"]"#;
    let value = toml::from_str::<toml::Value>(mock_toml).unwrap();
    let result = std::panic::catch_unwind(|| {
        extract_examine_globs_or_panic(&value);
    });
    assert!(result.is_err(), "helper did not panic — expected MUTANTS-GLOBS-KEY-MISSING panic");
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(
        msg.contains("MUTANTS-GLOBS-KEY-MISSING"),
        "expected MUTANTS-GLOBS-KEY-MISSING in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("examine_globs key not found"),
        "expected 'examine_globs key not found' in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("is present but empty"),
        "expected 'is present but empty' in panic message, got: {}",
        msg
    );
}

/// Test 5: coverage floor RED probe (N=3) — assert_examine_globs_coverage_floor
/// panics with MUTANTS-GLOBS-COVERAGE-FLOOR; message contains expected >= 11 and
/// got 3 (MED-2 FIX / HIGH-1-P23 FIX / F-1(c) FIX).
#[test]
fn test_coverage_floor_panics_when_entries_below_threshold() {
    let mock_toml = r#"examine_globs = ["src/a.rs", "src/b.rs", "src/c.rs"]"#;
    let value = toml::from_str::<toml::Value>(mock_toml).unwrap();
    let entries = extract_examine_globs_or_panic(&value);
    let result = std::panic::catch_unwind(|| {
        assert_examine_globs_coverage_floor(&entries);
    });
    assert!(result.is_err(), "helper did not panic — expected MUTANTS-GLOBS-COVERAGE-FLOOR panic");
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(
        msg.contains("MUTANTS-GLOBS-COVERAGE-FLOOR"),
        "expected MUTANTS-GLOBS-COVERAGE-FLOOR in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("expected >= 11"),
        "expected 'expected >= 11' in MUTANTS-GLOBS-COVERAGE-FLOOR panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("got 3"),
        "expected 'got 3' in MUTANTS-GLOBS-COVERAGE-FLOOR panic message, got: {}",
        msg
    );
}

/// Test 6: coverage floor GREEN boundary (N=11) — assert_examine_globs_coverage_floor
/// does NOT panic at exactly N=11; proves inclusive boundary < 11 (F-1(d) FIX).
#[test]
fn test_coverage_floor_does_not_panic_at_exact_threshold() {
    let entries: Vec<String> = (1..=11).map(|i| format!("src/mock_{}.rs", i)).collect();
    let result = std::panic::catch_unwind(|| {
        assert_examine_globs_coverage_floor(&entries);
    });
    assert!(
        result.is_ok(),
        "floor must NOT fire at exactly N=11 (inclusive boundary), got panic"
    );
}

/// Test 7: key-present-but-empty array — extract_examine_globs_or_panic panics
/// with MUTANTS-GLOBS-KEY-MISSING (MED-3-P23 FIX / LOW-1-P23 FIX).
#[test]
fn test_detect_empty_examine_globs_array_panics_with_key_missing_message() {
    let mock_toml = r#"examine_globs = []"#;
    let value = toml::from_str::<toml::Value>(mock_toml).unwrap();
    let result = std::panic::catch_unwind(|| {
        extract_examine_globs_or_panic(&value);
    });
    assert!(result.is_err(), "helper did not panic — expected MUTANTS-GLOBS-KEY-MISSING panic");
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(
        msg.contains("MUTANTS-GLOBS-KEY-MISSING"),
        "expected MUTANTS-GLOBS-KEY-MISSING in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("examine_globs key not found"),
        "expected 'examine_globs key not found' in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("is present but empty"),
        "expected 'is present but empty' in panic message, got: {}",
        msg
    );
}

/// Test 8: coverage floor RED near-miss (N=10) — assert_examine_globs_coverage_floor
/// panics at N=10 (one below threshold); message contains expected >= 11 and got 10
/// (V-4-P24 FIX).
#[test]
fn test_coverage_floor_panics_at_ten_entries_below_threshold() {
    let mock_toml = r#"examine_globs = ["src/a.rs","src/b.rs","src/c.rs","src/d.rs","src/e.rs","src/f.rs","src/g.rs","src/h.rs","src/i.rs","src/j.rs"]"#;
    let value = toml::from_str::<toml::Value>(mock_toml).unwrap();
    let entries = extract_examine_globs_or_panic(&value);
    let result = std::panic::catch_unwind(|| {
        assert_examine_globs_coverage_floor(&entries);
    });
    assert!(result.is_err(), "helper did not panic — expected MUTANTS-GLOBS-COVERAGE-FLOOR panic");
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| err.downcast_ref::<&str>().copied())
        .unwrap();
    assert!(
        msg.contains("MUTANTS-GLOBS-COVERAGE-FLOOR"),
        "expected MUTANTS-GLOBS-COVERAGE-FLOOR in panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("expected >= 11"),
        "expected 'expected >= 11' in MUTANTS-GLOBS-COVERAGE-FLOOR panic message, got: {}",
        msg
    );
    assert!(
        msg.contains("got 10"),
        "expected 'got 10' in MUTANTS-GLOBS-COVERAGE-FLOOR panic message, got: {}",
        msg
    );
}

/// Test 9: coverage floor GREEN above threshold (N=12) — assert_examine_globs_coverage_floor
/// does NOT panic at N=12; together with Test 6 (N=11) closes the <= 11 tightening class
/// (F-VA-28-1 FIX).
#[test]
fn test_coverage_floor_does_not_panic_above_threshold() {
    let entries: Vec<String> = (1..=12).map(|i| format!("src/mock_{}.rs", i)).collect();
    let result = std::panic::catch_unwind(|| {
        assert_examine_globs_coverage_floor(&entries);
    });
    assert!(
        result.is_ok(),
        "floor must NOT fire above threshold (N=12), got panic"
    );
}
