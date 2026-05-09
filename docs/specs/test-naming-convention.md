# Test Naming Convention

## The convention

Adopt `test_<verb>_<subject>_<expected_outcome>` for ALL new tests written from S-2.07 v2.0.0 onward.

Examples:
- `test_auth_switch_returns_json_ok`
- `test_auth_login_emits_json_when_output_json_set`
- `test_auth_switch_unknown_profile_returns_json_error`
- `test_parse_duration_validate_accepts_combined_units`
- `test_parse_duration_validate_rejects_garbage`

## Migration policy

Existing tests are NOT renamed. Two pre-existing styles coexist in the corpus (post-S-2.07):

- 108 tests use `test_<verb>_<subject>` (legacy prefixed)
- 212 tests use `<subject>_<verb>_<expected>` (legacy no-prefix)

A big-bang rename was rejected: too high churn, no behavioral benefit. New tests written from S-2.07 v2.0.0 onward use the canonical convention. Over time, the canonical convention will be the dominant style by attrition.

## Rationale

The `test_` prefix improves grep-ability across the mixed-convention corpus: `rg '^\s*fn test_' --type rust` reliably enumerates all canonical-convention tests across hundreds of test files. Without the prefix, distinguishing test functions from helper functions requires more elaborate filtering (e.g., walking attributes for `#[test]`).

The `<verb>_<subject>_<expected>` ordering puts the action first, then the target, then the asserted outcome — readable left-to-right as "test that [verb] [subject] [outcome]".

## Ecosystem note

Many large Rust crates (tokio, serde, regex) omit the `test_` prefix entirely. We adopt it specifically for grep-ability across our own mixed-convention corpus. The Rust API guidelines (https://rust-lang.github.io/api-guidelines/naming.html) and the Rust Book (https://doc.rust-lang.org/book/ch11-03-test-organization.html) do not prescribe a test-naming convention; the choice is project-local.

## Self-check

Before submitting any PR with new tests, verify:

```bash
rg '^\s*fn ' tests/<new-file>.rs src/<changed-files>.rs | rg -v '^\s*fn test_' | rg -B1 '#\[test\]'
```

This finds any `#[test]`-attributed function that does NOT start with `test_`. The output should be empty for any new file authored from S-2.07 v2.0.0 onward.
