//! Shared assertion helpers for integration tests.
//!
//! Promoted from `tests/json_error_shape.rs` (S-639-1, "Test Infrastructure — F3
//! Story Deliverables"). The original definition in `tests/json_error_shape.rs`
//! was deleted; its three call sites now re-import from this module.

/// Assert the `--output json` error-envelope contract: stderr is
/// `{"error":"…","code":<expected_code>}`, stdout is empty, and the process
/// exits with `expected_code`.
///
/// Key order in the parsed JSON value is `serde_json::Map`'s default iteration
/// behavior (alphabetical, since this crate does not enable the `preserve_order`
/// feature) — this is an implementation detail of the `serde_json::Value` map
/// type, NOT a contractual guarantee of the wire format. Callers MUST parse
/// fields individually (e.g. `parsed["error"]`, `parsed["code"]`) and MUST NOT
/// assert on literal key order or an exact serialized string.
pub fn assert_json_error_envelope(output: &std::process::Output, expected_code: i32, label: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Exit code.
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "{label}: expected exit {expected_code}; stderr={stderr} stdout={stdout}"
    );

    // stdout must be empty — channel-separation invariant (#526).
    assert!(
        stdout.trim().is_empty(),
        "{label}: stdout must be empty on error (channel-separation #526); stdout={stdout}"
    );

    // stderr must be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
        panic!("{label}: stderr must be valid JSON when --output json set: {e}\nstderr: {stderr}")
    });

    // `error` field must be a non-empty string.
    assert!(
        parsed["error"].as_str().is_some_and(|s| !s.is_empty()),
        "{label}: JSON envelope must have non-empty 'error' field; got: {parsed}"
    );

    // `code` field must match the exit code.
    assert_eq!(
        parsed["code"].as_i64(),
        Some(expected_code as i64),
        "{label}: JSON envelope 'code' must be {expected_code}; got: {parsed}"
    );
}
