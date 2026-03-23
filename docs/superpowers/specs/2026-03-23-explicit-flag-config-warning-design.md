# Explicit Flag Config Warning

## Overview

When a user explicitly requests a feature via a CLI flag but the required config is missing, warn on stderr instead of silently skipping. The command still succeeds (exit 0) with degraded output.

Closes: GitHub issue #18.

## Problem

`jr issue list --points` with no `story_points_field_id` configured silently shows the table without a Points column. An AI agent or user has no way to know the flag was ignored or what to do about it.

This violates the CLAUDE.md convention: "Errors: Always suggest what to do next."

## Design

### Extract `resolve_show_points` helper

Extract the `show_points` + `sp_field_id` resolution into a small helper function in `src/cli/issue/list.rs` so it can be unit tested:

```rust
/// Resolve whether to show story points. Returns the field ID if points should be shown,
/// or None. Emits a warning to stderr if --points was requested but config is missing.
fn resolve_show_points<'a>(show_points: bool, sp_field_id: Option<&'a str>) -> Option<&'a str> {
    if show_points {
        match sp_field_id {
            Some(id) => Some(id),
            None => {
                eprintln!(
                    "warning: --points ignored. Story points field not configured. \
                     Run \"jr init\" or set [fields].story_points_field_id in ~/.config/jr/config.toml"
                );
                None
            }
        }
    } else {
        None
    }
}
```

### Update `handle_list`

Replace line 114:

```rust
let effective_sp = if show_points { sp_field_id } else { None };
```

with:

```rust
let effective_sp = resolve_show_points(show_points, sp_field_id);
```

### Behavior matrix

| Command | Config set? | Behavior | Exit code |
|---------|-----------|----------|-----------|
| `issue list --points` | Yes | Shows points column | 0 |
| `issue list --points` | No | Warning to stderr, list without points | 0 |
| `issue list` | Either | No warning, no points column | 0 |
| `issue list --points --output json` | No | Warning on stderr, clean JSON on stdout | 0 |
| `issue list --points --no-input` | No | Warning still fires (AI agents need it) | 0 |

### What doesn't change

- `issue create --points 5` / `issue edit --points 5` — already error via `resolve_story_points_field_id` (correct: can't proceed without config)
- `issue edit --no-points` — already errors via `resolve_story_points_field_id` (correct: needs field ID to clear the value)
- `sprint current` — auto-shows points when configured, silently skips when not (correct: not explicitly requested)
- `issue view` — auto-shows points when configured (correct: not explicitly requested)
- `extra` field list passed to `search_issues` — always populated from `sp_field_id` regardless of `--points`. This pre-existing behavior is unchanged and out of scope.

## Convention

This establishes a general pattern: **explicit flags that depend on optional config warn on stderr when config is missing, then degrade gracefully.** Silent skipping is acceptable for auto/implicit behavior (features shown automatically when config exists) but never for explicitly requested features. Apply this pattern to any future flag that optionally requires config.

Warning format: lowercase `warning:` prefix, plain text (no colors), always fires regardless of `--no-input` mode.

If more flags need this pattern in the future, extract to a reusable helper. One case today doesn't justify the abstraction (YAGNI).

## Files touched

| File | Change |
|------|--------|
| `src/cli/issue/list.rs` | Add `resolve_show_points` helper, update `handle_list`, add unit tests |

## Testing

Three unit tests on the pure `resolve_show_points` function:

1. `resolve_show_points_flag_false` — `show_points=false`, any config → returns `None`
2. `resolve_show_points_flag_true_config_present` — `show_points=true`, config=`Some("field_id")` → returns `Some("field_id")`
3. `resolve_show_points_flag_true_config_missing` — `show_points=true`, config=`None` → returns `None` (warning emitted to stderr, not captured in test)

No integration test changes needed. Existing integration tests don't exercise the `--points` flag path.
