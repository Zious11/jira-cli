# Red Gate Log — S-345

## Story
S-345: Extract label-coalesce JSON builder into pure function + proptest

## Pattern
Standard Red Gate (assertion-error fail before implementation). The proptest
references the stub function `build_labels_edited_fields` which returns
`{"STUB_INTENTIONALLY_WRONG": true}`. The proptest's first invariant assertion
(top-level "labels" key MUST be present) fires immediately.

## Outcome
- Step A.1 first run: PROPTEST FAILED with assertion error referencing BC-3.4.006
  (CORRECT — discriminates the contract). Compile succeeded.
- Implementer (next dispatch) will replace the stub body; proptest expected to
  go green on first run after that.

## Evidence
- Stub function: src/cli/issue/create.rs (just above handle_edit_bulk_labels, gated #[cfg(test)] during stub phase)
- Proptest module: src/cli/issue/create.rs #[cfg(test)] mod build_labels_proptests
- Worktree commit (Red Gate): 195dc3a

## Verbatim Red Gate proof

```
running 1 test
test cli::issue::create::build_labels_proptests::build_labels_edited_fields_invariants ... FAILED

---- cli::issue::create::build_labels_proptests::build_labels_edited_fields_invariants stdout ----

thread '...' panicked at src/cli/issue/create.rs:1509:47:
BC-3.4.006: top-level 'labels' key MUST be present

Test failed: BC-3.4.006: top-level 'labels' key MUST be present.
minimal failing input: adds = ["a"], removes = []
    successes: 0
    local rejects: 0
    global rejects: 0

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 701 filtered out
```
