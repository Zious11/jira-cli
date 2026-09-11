//! Tests for BC-1.6.048 / BC-1.6.049 — `auth list` STATUS column / renderer
//! purity (S-cycle7-auth-state-derivation, Wave 1).
//!
//! These integration-crate tests cover AC-014 (renderer purity source-scan)
//! and do not require direct access to `pub(crate)` symbols.
//!
//! RED GATE: all tests in this file will fail assertion checks against the
//! current (pre-implementation) code, confirming:
//!   - render_list_table / render_list_json currently do NOT call
//!     derive_auth_state (AC-014 positive assertion)
//!   - the renderers currently DO contain `p.url.is_some()` (AC-014 negative
//!     assertion proving the defect exists)

/// BC-1.6.049 postcondition 2 / AC-014 (DEFAULT CI — renderer purity
/// source-scan; F3 adversary pass-1, finding F-1).
///
/// `render_list_table` and `render_list_json` must be PURE with respect to
/// credential probing — neither references `load_oauth_tokens`,
/// `load_api_token`, or any other keychain-reading symbol in its body.
/// ALL kind-specific probing must happen in `handle_list` (or the extracted
/// `collect_probe_results` helper it calls), not inside a renderer.
///
/// ALSO asserts the POSITIVE invariant: both renderers must call
/// `derive_auth_state` — the shared helper that replaces the old
/// `p.url.is_some()` ternary.
///
/// RED GATE failures on current code:
/// 1. The positive `derive_auth_state` assertion fails — renderers currently
///    call neither `derive_auth_state` nor any real credential probe.
/// 2. The negative `p.url.is_some()` assertion fails — renderers currently
///    contain this old defective check.
#[test]
fn test_bc_1_6_049_renderers_are_probe_free() {
    let list_src = include_str!("../src/cli/auth/list.rs");

    // ── Negative guards: keychain-reading symbols must NOT appear in renderer
    //    function bodies ──────────────────────────────────────────────────────
    //
    // We extract approximate function bodies by finding the fn declaration and
    // scanning forward to the next top-level declaration.

    let keychain_symbols = ["load_oauth_tokens", "load_api_token"];

    for &sym in &keychain_symbols {
        let table_body = extract_fn_body(list_src, "fn render_list_table");
        if let Some(body) = table_body {
            assert!(
                !body.contains(sym),
                "BC-1.6.048 purity violation (AC-014): render_list_table references \
                 `{sym}` — keychain probing must be in handle_list/collect_probe_results, \
                 not the renderer. This is the F-1 fix."
            );
        }

        let json_body = extract_fn_body(list_src, "fn render_list_json");
        if let Some(body) = json_body {
            assert!(
                !body.contains(sym),
                "BC-1.6.048 purity violation (AC-014): render_list_json references \
                 `{sym}` — keychain probing must be in handle_list/collect_probe_results, \
                 not the renderer. This is the F-1 fix."
            );
        }
    }

    // ── Negative guard: `p.url.is_some()` ternary must NOT appear in either
    //    renderer body — this is the defective implementation that this story
    //    replaces with a derive_auth_state call. Currently it IS present, so
    //    this assertion FAILS → Red Gate. ─────────────────────────────────────
    let old_impl_pattern = "p.url.is_some()";

    let table_body = extract_fn_body(list_src, "fn render_list_table");
    if let Some(body) = table_body {
        assert!(
            !body.contains(old_impl_pattern),
            "BC-1.6.049 STATUS derivation defect still present (AC-014): \
             render_list_table still uses `p.url.is_some()` for STATUS — must be \
             replaced with a derive_auth_state call fed from the probe_results \
             parameter. This assertion IS the Red Gate for the #788 fix."
        );
    }

    let json_body = extract_fn_body(list_src, "fn render_list_json");
    if let Some(body) = json_body {
        assert!(
            !body.contains(old_impl_pattern),
            "BC-1.6.049 STATUS derivation defect still present (AC-014): \
             render_list_json still uses `p.url.is_some()` for STATUS — must be \
             replaced with a derive_auth_state call fed from the probe_results \
             parameter. This assertion IS the Red Gate for the #788 fix."
        );
    }

    // ── Positive guard: both renderers MUST call derive_auth_state ───────────
    //
    // This is the primary Red Gate assertion — fails on current code because
    // neither renderer calls derive_auth_state yet (it doesn't exist).
    let table_body = extract_fn_body(list_src, "fn render_list_table");
    let table_calls_derive = table_body
        .as_deref()
        .map(|b| b.contains("derive_auth_state"))
        .unwrap_or(false);

    let json_body = extract_fn_body(list_src, "fn render_list_json");
    let json_calls_derive = json_body
        .as_deref()
        .map(|b| b.contains("derive_auth_state"))
        .unwrap_or(false);

    assert!(
        table_calls_derive,
        "BC-1.6.049 invariant 2 FAIL (AC-014): render_list_table does not call \
         derive_auth_state. Current code uses p.url.is_some() — this IS the Red Gate \
         for AC-014."
    );
    assert!(
        json_calls_derive,
        "BC-1.6.049 invariant 2 FAIL (AC-014): render_list_json does not call \
         derive_auth_state. Current code uses p.url.is_some() — this IS the Red Gate \
         for AC-014."
    );
}

/// Extract the approximate body of a named function from source text.
///
/// Finds `fn_sig` in `src`, then returns the text from that point up to
/// the next `\npub ` or `\n/// ` marker (a coarse function-boundary heuristic
/// sufficient for source-scan assertions).
fn extract_fn_body(src: &str, fn_sig: &str) -> Option<String> {
    let start = src.find(fn_sig)?;
    let after = &src[start..];
    let end = after[1..]
        .find("\npub ")
        .or_else(|| after[1..].find("\n/// "))
        .map(|pos| pos + 1)
        .unwrap_or(after.len());
    Some(after[..end].to_string())
}
