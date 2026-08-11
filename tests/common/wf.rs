//! A spanned document model for GitHub Actions workflow YAML, built by walking
//! `saphyr_parser::Parser`'s low-level `Event` stream.
//!
//! # Why this exists (S-CIGATE-3)
//!
//! `tests/ci_gate_completeness.rs` (and its siblings `tests/backfill_matrix_parity.rs`,
//! `tests/ci_yml_windows_matrix.rs`) built their structural assertions about
//! `.github/workflows/*.yml` on a hand-rolled `str::lines()` lexer
//! (`tests/common/yaml.rs`'s old `extract_job_block`/`extract_key_name_at_indent`/
//! `collect_mapping_key_set`). Three consecutive rounds of adversarial review each
//! found a NEW way to defeat that approximation (comment-indent truncation, BOM,
//! `? key` explicit-key syntax, lone CR, NEL/LS/PS, and `&anchor`/`!!tag` node
//! properties) at a flat, non-decaying rate — evidence the lexer has an unbounded
//! number of remaining gaps, not a small finite list. This module replaces the
//! approximation with a real YAML 1.2 parse, so the *class* of gap closes
//! structurally instead of being patched member-by-member.
//!
//! # Hard constraint: event-stream only, never `saphyr::Yaml`
//!
//! This module constructs YAML structure exclusively from `saphyr_parser::Parser`'s
//! `Event` stream (`saphyr_parser::Event`, `saphyr_parser::Span`, `saphyr_parser::Marker`).
//! It never constructs a `saphyr::Yaml` / `saphyr::YamlLoader` document. The
//! high-level API is a trap for this purpose: it silently collapses duplicate
//! mapping keys (last-wins, no error), erases scalar quoting style, and resolves
//! aliases — each of which is exactly the signal later passes of this story need
//! to see. See `S-CIGATE-3-ci-yml-real-yaml-parser.md`'s "Parser Decision" section.
//!
//! # Crate-behavior traps this module works around (verified against a real
//! `saphyr-parser` 0.0.11 build, not inferred from its docs — several of its own
//! doc comments are wrong):
//!
//! - **`Marker::index()` returns a CHARACTER index, not a byte index**, despite its
//!   own rustdoc claiming "in bytes" (the *struct field's* doc comment, "in chars",
//!   is the one that's correct). `.github/workflows/ci.yml` contains non-ASCII
//!   characters (em dashes, arrows, section signs, `≥`), so byte and char offsets
//!   diverge partway through the file. [`char_byte_table`] builds a char-index →
//!   byte-index lookup once per parsed document; every `Marker::index()` value is
//!   routed through it before being used to slice `&str`.
//! - **`Marker::col()` is 0-indexed**, despite its own rustdoc claiming 1-indexed
//!   (`Marker::line()` genuinely IS 1-indexed — the two accessors are inconsistently
//!   documented). A key's column tells us how many characters precede it on its
//!   physical line, which is what lets us recover "the start of the line
//!   containing this key" without re-deriving an indent-width assumption.
//! - **The parser does not reject duplicate mapping keys.** Duplicate key names
//!   arrive as separate `Event::Scalar` events in document order, not collapsed —
//!   detecting a duplicate is the caller's job (a later pass's concern; this module
//!   only guarantees the events are visible, not deduplicated).
//! - **Aliases are not resolved.** An alias arrives as `Event::Alias(anchor_id)`,
//!   never silently substituted with its target's value.
//! - **Block scalars come back dedented.** A `run: |` step body's parsed
//!   [`Event::Scalar`] text has the workflow's leading indentation stripped; where
//!   the *original* source text matters (e.g. reproducing a byte-for-byte slice),
//!   callers must slice the source string using the event's [`Span`], never the
//!   parsed scalar value.
//!
//! # What this module does NOT attempt
//!
//! It is not a general YAML object model — no flow-style detection, no per-scalar
//! quote-style exposure at this layer (that lives on the raw `Event::Scalar`, which
//! callers can still reach if a future pass needs it), no non-mapping document root
//! support beyond `Option`-safe failure. It exists to give
//! [`extract_job_block`](super::yaml::extract_job_block) — and later passes of
//! S-CIGATE-3 — a real parse tree to walk keys/spans on, replacing line-position
//! arithmetic with tree membership.

use saphyr_parser::{Event, Parser, ScanError, Span};
use std::ops::Range;

/// Re-exported so callers that need to inspect a [`Value::Scalar`]'s
/// quoting style (e.g. to preserve today's byte-pin strictness — see
/// `Value`'s own doc comment) don't need a second, separate
/// `saphyr_parser` import line for a type this module's own public API
/// already embeds.
pub use saphyr_parser::ScalarStyle;

/// A parsed GitHub Actions workflow document.
///
/// Exposes root-level keys directly (not only `jobs`) because some existing
/// guards are workflow-root-scoped (e.g. a top-level `defaults:` override, or
/// the workflow's own top-level `env:` block) and cannot be reached by a
/// job-scoped lookup by construction — see `tests/ci_gate_completeness.rs`'s
/// `test_ci_yml_has_no_workflow_level_shell_override` and
/// `test_ci_yml_workflow_level_env_key_set_is_pinned` for the two existing
/// tests that need this.
#[derive(Debug)]
pub struct WfDoc {
    /// The document root mapping's direct key names, in source order.
    /// Duplicates are NOT deduplicated (see module docs — that is a
    /// deliberate design choice, not an oversight).
    pub root_keys: Vec<String>,
    /// Every entry found under the root's `jobs:` mapping, in source order.
    /// Empty if the document has no top-level `jobs:` key, or if `jobs:`'s
    /// value is not itself a mapping.
    pub jobs: Vec<Job>,
}

/// The resolved value of a mapping entry, as returned by [`Job::value_of`]
/// and [`Step::value_of`].
///
/// Added by S-CIGATE-3 pass D (`extract_job_display_name`'s rewrite):
/// `Job`/`Step`'s `keys: Vec<String>` exposes KEY presence only, not the
/// VALUE — and `extract_job_display_name` needs a job's `name:` value, not
/// just to know it has one. Deliberately general rather than a one-off
/// `Job::name_value()` accessor: a later S-CIGATE-3 pass rewriting the
/// gate-block byte pins (`extract_and_normalize_if_expr`,
/// `extract_and_normalize_sole_run_line`, and siblings) will need the exact
/// same shape — a scalar's resolved text AND its YAML style
/// (`Plain`/`SingleQuoted`/`DoubleQuoted`/...), so that a re-quoted
/// `if: "${{ always() }}"` still fails a pin built for the plain
/// `if: ${{ always() }}` spelling (this story's AC-004 quoting-fidelity
/// mandate). Building that as a generic primitive here, rather than
/// reinventing it per pin, is why this exists in `wf.rs` and not inline in
/// `ci_gate_completeness.rs`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A scalar node (`key: value`), covering ALL YAML scalar styles —
    /// plain, single/double-quoted, and the two block-scalar forms
    /// (`Literal`/`Folded`, `|`/`>`). Unlike the old line-based checkers
    /// this module replaces, no special-casing is needed here for block
    /// scalars or quote-escape sequences: `saphyr-parser` already resolves
    /// `text` to the real rendered value (dedented/folded/unescaped as the
    /// YAML 1.2 spec requires — see module docs on "Block scalars come
    /// back dedented").
    Scalar {
        /// The scalar's fully-resolved text.
        text: String,
        /// The YAML style this scalar was written in.
        style: ScalarStyle,
        /// The scalar's tag (e.g. `!!str`), if any, as `handle` + `suffix`
        /// concatenated (e.g. `"tag:yaml.org,2002:str"`). A tag does not
        /// change `text`'s value — `saphyr-parser` already resolved it —
        /// this is exposed only for a caller that needs to reject a
        /// specific tag form outright, the way `ci_gate_completeness.rs`'s
        /// round-16 node-property guards do for mapping KEYS.
        tag: Option<String>,
        /// The 1-indexed source line the scalar's span STARTS on, via
        /// `Marker::line()` (confirmed 1-indexed and accurate — see module
        /// docs; unlike `Marker::col()`/`index()`, no known doc-comment
        /// inaccuracy applies to `line()`).
        ///
        /// Added by S-CIGATE-3 pass B so a caller pinning a scalar's exact
        /// text byte-for-byte (e.g. `ci-gate`'s `if:`/`run:`/`NEEDS_JSON:`
        /// values) can still enforce the pre-parser checker's "single
        /// physical line only" constraint: `saphyr-parser` correctly
        /// resolves a YAML-folded plain scalar spanning several physical
        /// source lines down to one space-joined string, which is *new*
        /// capability the old line-based checker never had — accepting
        /// such input, even though its resolved text would still
        /// byte-match a pin, would still be a behavioral loosening versus
        /// today's hard `Err` on any multi-line continuation, so callers
        /// compare `start_line == end_line` to keep that exact rejection.
        start_line: usize,
        /// The 1-indexed source line the scalar's span ENDS on. See
        /// `start_line`.
        end_line: usize,
    },
    /// A YAML alias (`*name`) reference. This module does NOT resolve an
    /// alias to its anchor's value — that requires a document-wide
    /// anchor-id -> value table this module does not build (see module
    /// docs: "Aliases are not resolved"). A caller that cannot safely
    /// proceed without the real value should reject this variant outright
    /// rather than guess at it.
    Alias,
    /// Any other node kind (a nested mapping or sequence value). None of
    /// this story's guards need to read into one; callers that do should
    /// extend this module rather than fall back to string-scanning around
    /// it.
    Other,
}

/// One entry (`<job_id>: { ... }`) under `jobs:`.
#[derive(Debug)]
pub struct Job {
    /// The job's id (the mapping key under `jobs:`).
    pub id: String,
    /// The byte range in the original source text this job's block occupies.
    ///
    /// Deliberately mirrors the OLD line-based `extract_job_block`'s
    /// contract for equivalence purposes (see
    /// `tests/wf_model_equivalence.rs`): the range starts at byte 0 of the
    /// physical line containing this job's key (i.e. it includes the job
    /// key's leading indentation, recovered via `Marker::col()` — see module
    /// docs), and ends at the start of the next sibling job's line, or at
    /// `yaml.len()` for the last job in `jobs:` (verified: using the
    /// `jobs:` mapping's `MappingEnd` marker instead of `yaml.len()`
    /// undershoots by design — see module docs on `sync-upstream.yml`'s
    /// trailing block scalar).
    pub span: Range<usize>,
    /// This job's own direct mapping keys (`name`, `if`, `needs`,
    /// `runs-on`, `steps`, `env`, ...), in source order. Not deduplicated.
    pub keys: Vec<String>,
    /// Parallel to `keys` — `values[i]` is the resolved [`Value`] of
    /// `keys[i]`. Use [`Job::value_of`] rather than indexing this directly.
    pub values: Vec<Value>,
    /// This job's `steps:` sequence, if it has one and it is a sequence.
    /// Empty otherwise (no `steps:` key, or its value is not a sequence).
    pub steps: Vec<Step>,
}

impl Job {
    /// This job's resolved [`Value`] for its direct key `key`, if present.
    /// When `keys` contains `key` more than once (a duplicate mapping key —
    /// itself invalid YAML that GitHub Actions/`actionlint` reject at parse
    /// time; see `read_mapping`'s doc comment), returns the FIRST
    /// occurrence, mirroring every other first-match convention in this
    /// module.
    #[must_use]
    pub fn value_of(&self, key: &str) -> Option<&Value> {
        self.keys
            .iter()
            .position(|k| k == key)
            .map(|i| &self.values[i])
    }
}

/// One entry of a job's `steps:` sequence.
#[derive(Debug)]
pub struct Step {
    /// The step's `name:` value, if present and a scalar. `None` if the step
    /// has no `name:` key, or `name:`'s value is not a scalar.
    pub name: Option<String>,
    /// This step's own direct mapping keys (`name`, `run`, `uses`, `env`,
    /// `shell`, `with`, `if`, ...), in source order. Not deduplicated. Empty
    /// if the step entry is not itself a mapping (malformed workflow).
    pub keys: Vec<String>,
    /// Parallel to `keys` — `values[i]` is the resolved [`Value`] of
    /// `keys[i]`. Use [`Step::value_of`] rather than indexing this
    /// directly. Empty whenever `keys` is (i.e. whenever the step entry is
    /// not itself a mapping).
    pub values: Vec<Value>,
    /// The byte range in the original source text this step entry occupies
    /// (from its first event's span start to its last event's span end — NOT
    /// line-snapped the way [`Job::span`] is, since no caller depends on
    /// step-level line anchoring yet).
    pub span: Range<usize>,
}

impl Step {
    /// This step's resolved [`Value`] for its direct key `key`, if present.
    /// Same first-match-on-duplicate convention as [`Job::value_of`].
    #[must_use]
    pub fn value_of(&self, key: &str) -> Option<&Value> {
        self.keys
            .iter()
            .position(|k| k == key)
            .map(|i| &self.values[i])
    }
}

impl WfDoc {
    /// Parse `yaml` into a [`WfDoc`].
    ///
    /// # Panics
    ///
    /// Panics loudly, naming the underlying [`ScanError`], if `yaml` is not
    /// well-formed YAML. This is a deliberate behavioral change from the old
    /// line-based `extract_job_block`, which returned `Some`/`None`
    /// regardless of document validity: silently returning `None` here would
    /// surface at call sites (several of which `.expect()`/`.unwrap()` the
    /// result) as a misleading "job not found" rather than the true "the
    /// document itself is malformed" cause.
    #[must_use]
    pub fn parse(yaml: &str) -> WfDoc {
        let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(yaml)
            .collect::<Result<Vec<_>, ScanError>>()
            .unwrap_or_else(|e| {
                panic!("wf.rs: failed to parse workflow YAML as valid YAML 1.2: {e}")
            });

        let table = char_byte_table(yaml);
        let byte_of = |char_idx: usize| -> usize {
            *table.get(char_idx).unwrap_or_else(|| {
                panic!(
                    "wf.rs: char index {char_idx} out of range for a document \
                     of {} chars — this indicates a bug in this module's \
                     event-index bookkeeping, not malformed input",
                    table.len().saturating_sub(1)
                )
            })
        };

        let Some(root_start) = events
            .iter()
            .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
        else {
            // No top-level mapping (e.g. an empty document, or a document
            // whose root is a scalar/sequence). Not a malformed-YAML case —
            // just an empty result, mirroring the old function's
            // return-`None`-on-no-match contract for "job not found".
            return WfDoc {
                root_keys: Vec::new(),
                jobs: Vec::new(),
            };
        };

        let (root_entries, _root_end) = read_mapping(&events, root_start);
        let root_keys: Vec<String> = root_entries.iter().map(|e| e.key.clone()).collect();

        let mut jobs = Vec::new();
        if let Some(jobs_entry) = root_entries.iter().find(|e| e.key == "jobs") {
            if matches!(events[jobs_entry.value_start].0, Event::MappingStart(..)) {
                let (job_entries, _) = read_mapping(&events, jobs_entry.value_start);
                let job_count = job_entries.len();
                for (idx, entry) in job_entries.iter().enumerate() {
                    let start_byte = byte_of(line_start_char_idx(&entry.key_span));
                    let end_byte = if idx + 1 < job_count {
                        byte_of(line_start_char_idx(&job_entries[idx + 1].key_span))
                    } else {
                        yaml.len()
                    };

                    let (keys, values, steps) =
                        if matches!(events[entry.value_start].0, Event::MappingStart(..)) {
                            let (body_entries, _) = read_mapping(&events, entry.value_start);
                            let keys: Vec<String> =
                                body_entries.iter().map(|e| e.key.clone()).collect();
                            let values: Vec<Value> = body_entries
                                .iter()
                                .map(|e| resolve_value(&events, e.value_start))
                                .collect();
                            let steps = extract_steps(&events, &body_entries, &table);
                            (keys, values, steps)
                        } else {
                            (Vec::new(), Vec::new(), Vec::new())
                        };

                    jobs.push(Job {
                        id: entry.key.clone(),
                        span: start_byte..end_byte,
                        keys,
                        values,
                        steps,
                    });
                }
            }
        }

        WfDoc { root_keys, jobs }
    }
}

/// Build a char-index → byte-index lookup table for `s`.
///
/// `table[i]` is the byte offset of the `i`-th character in `s`. An extra
/// trailing entry, `table[s.chars().count()]`, holds `s.len()` — the "one
/// past the last character" byte offset a `Marker` can legitimately point at
/// (e.g. a `MappingEnd`/`SequenceEnd`/`StreamEnd` at true end-of-file).
///
/// Required because `saphyr_parser::Marker::index()` returns a CHARACTER
/// index (see module docs) but every consumer in this codebase needs a BYTE
/// offset to slice the original `&str`.
fn char_byte_table(s: &str) -> Vec<usize> {
    let mut table: Vec<usize> = s.char_indices().map(|(byte_idx, _)| byte_idx).collect();
    table.push(s.len());
    table
}

/// Recover the char index of the start of the physical line containing
/// `span.start`, using `Marker::col()` (0-indexed — see module docs) rather
/// than re-scanning the source text for the preceding `\n`.
fn line_start_char_idx(span: &Span) -> usize {
    span.start.index() - span.start.col()
}

/// One direct entry of a YAML block or flow mapping, as read by
/// [`read_mapping`].
struct MapEntry {
    /// The (string) key. Non-scalar keys (aliases, nested mapping/sequence
    /// keys — the `? ... : ...` explicit-key form) are not supported by this
    /// module and cause [`read_mapping`] to panic; none of the workflow
    /// files this module parses use them.
    key: String,
    /// The span of the KEY event itself (not the value) — this is what
    /// [`line_start_char_idx`] needs to recover a job/step's line-anchored
    /// start position.
    key_span: Span,
    /// Whether the KEY scalar itself carries a YAML anchor (`&x`).
    ///
    /// Added by S-CIGATE-3 pass B for the round-16 node-property residual
    /// (`&x shell: cat {0}` / `!!str shell: cat {0}` — see
    /// [`super::find_key_node_properties`]): the old line-based
    /// `extract_key_name_at_indent` stopped parsing at the space after
    /// `&x`, saw no colon, and returned `None` — invisible to every
    /// key-set pin built on it. `saphyr-parser`'s event stream resolves
    /// the KEY correctly regardless (`Scalar("shell", anchor_id=1)`), so
    /// key-SET membership alone already catches a smuggled key by its own
    /// presence; this field additionally lets a caller reject a node
    /// property on a key that is ALREADY a legitimate member of a pinned
    /// key set (e.g. `&x run: ...`), which a text-only key-set comparison
    /// cannot see at all.
    key_has_anchor: bool,
    /// The KEY scalar's tag (e.g. `!!str`), if any, as `handle` + `suffix`
    /// concatenated. See `key_has_anchor`.
    key_tag: Option<String>,
    /// Event index of the first event of this entry's VALUE node.
    value_start: usize,
}

/// Read the direct entries of a block/flow mapping whose `MappingStart` event
/// is at `events[start]`.
///
/// Returns `(entries, end_idx)` where `end_idx` is the event index
/// immediately after the matching `MappingEnd` event.
///
/// Handles duplicate keys correctly BY CONSTRUCTION: the event stream never
/// collapses them (see module docs), so two entries with the same `key` text
/// simply both appear in the returned `Vec`, in source order. Detecting that
/// as a problem (or not) is left entirely to the caller — this function does
/// not judge duplicates one way or the other.
fn read_mapping(events: &[(Event<'_>, Span)], start: usize) -> (Vec<MapEntry>, usize) {
    assert!(
        matches!(events[start].0, Event::MappingStart(..)),
        "wf.rs: read_mapping called at event {start}, which is not a MappingStart: {:?}",
        events[start].0
    );
    let mut i = start + 1;
    let mut entries = Vec::new();
    loop {
        match &events[i].0 {
            Event::MappingEnd => return (entries, i + 1),
            Event::Scalar(text, _style, anchor_id, tag) => {
                let key = text.to_string();
                let key_span = events[i].1;
                let key_has_anchor = *anchor_id != 0;
                let key_tag = tag.as_ref().map(|t| format!("{}{}", t.handle, t.suffix));
                let value_start = i + 1;
                let value_end = skip_node(events, value_start);
                entries.push(MapEntry {
                    key,
                    key_span,
                    key_has_anchor,
                    key_tag,
                    value_start,
                });
                i = value_end;
            }
            other => panic!(
                "wf.rs: expected a scalar mapping key at event {i}, found {other:?} \
                 — non-scalar (alias/complex) mapping keys are not supported by \
                 this module and are not used by any workflow file it parses \
                 today; if this fires, a workflow file started using one",
            ),
        }
    }
}

/// Advance past exactly one YAML node (scalar, alias, sequence, or mapping —
/// recursing into nested sequences/mappings as needed) starting at
/// `events[start]`. Returns the event index immediately after the node's
/// closing event.
fn skip_node(events: &[(Event<'_>, Span)], start: usize) -> usize {
    match &events[start].0 {
        Event::Scalar(..) | Event::Alias(_) => start + 1,
        Event::SequenceStart(..) => {
            let (_, end) = read_sequence(events, start);
            end
        }
        Event::MappingStart(..) => {
            let (_, end) = read_mapping(events, start);
            end
        }
        other => panic!(
            "wf.rs: skip_node called at event {start} on an event that is not \
             the start of a node: {other:?} — this indicates a bug in this \
             module's event-index bookkeeping",
        ),
    }
}

/// Read the direct items of a sequence whose `SequenceStart` event is at
/// `events[start]`.
///
/// Returns `((item_start_idx, item_end_idx_exclusive)` pairs, end_idx)`
/// where `end_idx` is the event index immediately after the matching
/// `SequenceEnd` event.
fn read_sequence(events: &[(Event<'_>, Span)], start: usize) -> (Vec<(usize, usize)>, usize) {
    assert!(
        matches!(events[start].0, Event::SequenceStart(..)),
        "wf.rs: read_sequence called at event {start}, which is not a SequenceStart: {:?}",
        events[start].0
    );
    let mut i = start + 1;
    let mut items = Vec::new();
    loop {
        if matches!(events[i].0, Event::SequenceEnd) {
            return (items, i + 1);
        }
        let item_end = skip_node(events, i);
        items.push((i, item_end));
        i = item_end;
    }
}

/// Resolve the [`Value`] of a mapping entry's value node, which starts at
/// `events[value_start]`.
fn resolve_value(events: &[(Event<'_>, Span)], value_start: usize) -> Value {
    match &events[value_start].0 {
        Event::Scalar(text, style, _anchor_id, tag) => {
            let span = events[value_start].1;
            Value::Scalar {
                text: text.to_string(),
                style: *style,
                tag: tag.as_ref().map(|t| format!("{}{}", t.handle, t.suffix)),
                start_line: span.start.line(),
                end_line: span.end.line(),
            }
        }
        Event::Alias(_) => Value::Alias,
        _ => Value::Other,
    }
}

/// Build a job's `steps:` list, if it has a `steps:` key whose value is a
/// sequence.
fn extract_steps(
    events: &[(Event<'_>, Span)],
    job_body_entries: &[MapEntry],
    table: &[usize],
) -> Vec<Step> {
    let Some(steps_entry) = job_body_entries.iter().find(|e| e.key == "steps") else {
        return Vec::new();
    };
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return Vec::new();
    }
    let (items, _) = read_sequence(events, steps_entry.value_start);
    items
        .into_iter()
        .map(|(item_start, item_end)| build_step(events, item_start, item_end, table))
        .collect()
}

/// Build one [`Step`] from a `steps:` sequence item spanning event indices
/// `[item_start, item_end)`.
fn build_step(
    events: &[(Event<'_>, Span)],
    item_start: usize,
    item_end: usize,
    table: &[usize],
) -> Step {
    let start_byte = table[events[item_start].1.start.index()];
    let end_byte = table[events[item_end - 1].1.end.index()];
    let span = start_byte..end_byte;

    if !matches!(events[item_start].0, Event::MappingStart(..)) {
        // A `steps:` entry that isn't a mapping is malformed GitHub Actions
        // YAML, but this module's job is to model the document, not
        // validate it — return an empty-keys step rather than panicking.
        return Step {
            name: None,
            keys: Vec::new(),
            values: Vec::new(),
            span,
        };
    }

    let (entries, _) = read_mapping(events, item_start);
    let keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
    let values: Vec<Value> = entries
        .iter()
        .map(|e| resolve_value(events, e.value_start))
        .collect();
    let name = entries.iter().find(|e| e.key == "name").and_then(|e| {
        if let Event::Scalar(text, ..) = &events[e.value_start].0 {
            Some(text.to_string())
        } else {
            None
        }
    });

    Step {
        name,
        keys,
        values,
        span,
    }
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 pass B additions
//
// Everything below was added for `tests/ci_gate_completeness.rs`'s
// gate-block scalar/key-set pin migration (find_comment_start,
// extract_and_normalize_if_expr, extract_and_normalize_sole_run_line and
// siblings, extract_job_level_key_set, extract_gate_step_key_sets,
// extract_gate_env_key_set, the PINNED_GATE_* consts, and the G8 test
// `test_ci_gate_pass_fail_semantics_are_structurally_placed`). Purely
// additive — no existing `pub` item's signature changed (the one
// non-additive change, `Value::Scalar` gaining `start_line`/`end_line`
// fields, is backward compatible with every existing `Value::Scalar { text,
// .. }` match in this codebase, which already uses `..`).
// ---------------------------------------------------------------------------

/// A YAML node property (`&anchor` and/or `!tag`) found directly attached to
/// a mapping KEY scalar anywhere in a parsed document's tree (any depth —
/// job-level, step-level, `env:`-level, `with:`-level, and beyond).
///
/// See [`find_key_node_properties`]'s doc comment for why this exists
/// alongside (not instead of) tree-based key-SET membership checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyNodeProperty {
    /// The key's resolved text (e.g. `"shell"` for `&x shell: cat {0}`).
    pub key: String,
    /// Whether the key scalar carries a YAML anchor (`&x`).
    pub has_anchor: bool,
    /// The key scalar's tag (e.g. `!!str`), if any, as `handle` + `suffix`
    /// concatenated.
    pub tag: Option<String>,
}

/// Walk EVERY mapping key in `yaml`'s parse tree — at any depth, not only
/// the job/step levels `Job`/`Step` model directly — and return every key
/// scalar that carries a node property (anchor and/or tag) directly on
/// itself, in document order.
///
/// # Why this is a separate check from key-SET pins (S-CIGATE-3 AC-007,
/// round-16 residual)
///
/// A tree-based key-set comparison (`Job::keys`, `Step::keys`, or
/// [`step_mapping_child_keys`] below) already catches a NODE-PROPERTIED KEY
/// THAT IS NEW — `&x shell: cat {0}` adds a real `"shell"` key the parser
/// sees regardless of the anchor, so a pinned set that doesn't expect
/// `"shell"` there fails to match by ITS OWN TEXT alone, no node-property
/// awareness required. What a plain `Vec<String>` key-set comparison CANNOT
/// see is a node property attached to a key that is ALREADY a legitimate,
/// expected member of the pinned set — e.g. `&x run: some-other-command`:
/// the key set stays exactly `{"env","name","run"}`, textually identical to
/// the pin, while the value has silently gained an anchor a later alias
/// elsewhere in the same document (GitHub shipped anchor/alias support to
/// production Actions 2025-09-18) could reference. This function closes
/// that residual gap by scanning for the node property itself, independent
/// of whether the key's SET membership also happens to be correct.
///
/// # Panics
///
/// Same malformed-YAML-panics contract as [`WfDoc::parse`].
#[must_use]
pub fn find_key_node_properties(yaml: &str) -> Vec<KeyNodeProperty> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(yaml)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| panic!("wf.rs: failed to parse workflow YAML as valid YAML 1.2: {e}"));

    let Some(root_start) = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
    else {
        return Vec::new();
    };

    let mut found = Vec::new();
    collect_key_node_properties(&events, root_start, &mut found);
    found
}

/// Recursive helper for [`find_key_node_properties`]: records every
/// node-propertied key directly under the mapping starting at
/// `events[mapping_start]`, then recurses into every entry's VALUE (mapping
/// or sequence) to find nested ones too.
fn collect_key_node_properties(
    events: &[(Event<'_>, Span)],
    mapping_start: usize,
    out: &mut Vec<KeyNodeProperty>,
) {
    let (entries, _end) = read_mapping(events, mapping_start);
    for entry in &entries {
        if entry.key_has_anchor || entry.key_tag.is_some() {
            out.push(KeyNodeProperty {
                key: entry.key.clone(),
                has_anchor: entry.key_has_anchor,
                tag: entry.key_tag.clone(),
            });
        }
        recurse_into_value_for_node_properties(events, entry.value_start, out);
    }
}

/// Descend into a mapping entry's VALUE node (if it is itself a mapping or
/// sequence) looking for further node-propertied keys nested inside it.
fn recurse_into_value_for_node_properties(
    events: &[(Event<'_>, Span)],
    value_start: usize,
    out: &mut Vec<KeyNodeProperty>,
) {
    match &events[value_start].0 {
        Event::MappingStart(..) => collect_key_node_properties(events, value_start, out),
        Event::SequenceStart(..) => {
            let (items, _end) = read_sequence(events, value_start);
            for (item_start, _item_end) in items {
                recurse_into_value_for_node_properties(events, item_start, out);
            }
        }
        _ => {}
    }
}

impl WfDoc {
    /// Parse a SINGLE job's own block — text starting at `<job_id>:` and
    /// covering that job's whole mapping, exactly the shape
    /// [`super::yaml::extract_job_block`] returns — into a standalone
    /// [`Job`], WITHOUT requiring the `jobs:` wrapper [`WfDoc::parse`]
    /// expects at the document root.
    ///
    /// # Why this exists (S-CIGATE-3 pass B)
    ///
    /// `tests/ci_gate_completeness.rs`'s per-job pin functions (e.g.
    /// `extract_and_normalize_if_expr`) take a `job_block: &str` parameter
    /// — that shape is FROZEN: two `#[cfg(unix)]` tests
    /// (`test_ci_gate_decision_matches_job_level_if_for_every_needs_member`,
    /// `test_ci_gate_decision_is_arity_independent_for_unlisted_skips`) call
    /// them with `extract_job_block(&ci, job)`'s result for EVERY
    /// `ci-gate.needs` member, not only `ci-gate` itself, and must need zero
    /// edits. `WfDoc::parse` cannot be reused directly for a lone job
    /// block: it looks for a top-level `jobs:` key and a `job_block` slice
    /// has none — its own root key IS the job id. This function gives those
    /// pin functions the SAME tree-walked `Job` model (`keys`, `values`,
    /// `steps`, `value_of`) `WfDoc::parse`'s `.jobs` field already builds
    /// for the whole-file case, without re-deriving a parallel line-based
    /// scanner or reindenting `job_block` to synthesize a fake `jobs:`
    /// wrapper (reindenting every line to fabricate a deeper nesting level
    /// is exactly the kind of position-dependent trick this story's
    /// tree-based approach exists to avoid).
    ///
    /// # Panics
    ///
    /// Same malformed-YAML-panics contract as [`WfDoc::parse`]. Also panics
    /// if `job_block`'s root mapping does not have EXACTLY ONE entry (a
    /// `job_block` slice produced by `extract_job_block` always has this
    /// shape — a hand-constructed multi-entry string passed here indicates
    /// caller error, not a case to silently guess at).
    #[must_use]
    pub fn parse_single_job(job_block: &str) -> Job {
        let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
            .collect::<Result<Vec<_>, ScanError>>()
            .unwrap_or_else(|e| {
                panic!(
                    "wf.rs: parse_single_job: failed to parse job block YAML as valid YAML 1.2: {e}"
                )
            });
        let table = char_byte_table(job_block);

        let Some(root_start) = events
            .iter()
            .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
        else {
            panic!(
                "wf.rs: parse_single_job: job_block has no top-level mapping at all: {job_block:?}"
            );
        };
        let (root_entries, _root_end) = read_mapping(&events, root_start);
        if root_entries.len() != 1 {
            panic!(
                "wf.rs: parse_single_job: expected exactly one root entry (the \
                 job id), found {} — job_block should be a single \
                 `<job_id>: {{...}}` mapping entry, exactly as \
                 extract_job_block returns it, not: {job_block:?}",
                root_entries.len()
            );
        }
        let entry = &root_entries[0];

        let (keys, values, steps) =
            if matches!(events[entry.value_start].0, Event::MappingStart(..)) {
                let (body_entries, _) = read_mapping(&events, entry.value_start);
                let keys: Vec<String> = body_entries.iter().map(|e| e.key.clone()).collect();
                let values: Vec<Value> = body_entries
                    .iter()
                    .map(|e| resolve_value(&events, e.value_start))
                    .collect();
                let steps = extract_steps(&events, &body_entries, &table);
                (keys, values, steps)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };

        Job {
            id: entry.key.clone(),
            span: 0..job_block.len(),
            keys,
            values,
            steps,
        }
    }
}

/// Resolve the byte SPAN (into `job_block`) of the job-level value of key
/// `key`, PROVIDED that value is itself a mapping or sequence node (for a
/// scalar/alias value, use [`Job::value_of`] instead — this function exists
/// specifically for a composite value like `needs: [a, b, c]`, which has no
/// single `Event::Scalar` to source byte-for-byte pinned text from).
///
/// Returns `None` if `job_block`'s job has no `key` at its own (job) level,
/// or `key`'s value there is a scalar or alias rather than a mapping/
/// sequence.
///
/// # Why span-slicing instead of reconstructing from resolved values
///
/// Reconstructing `"[fmt, clippy, ...]"` from a `Vec` of resolved item
/// scalars would require this function to invent a join separator (`", "`)
/// that happens to match `ci.yml`'s current formatting — fragile in exactly
/// the way a real byte-for-byte pin must not be. Slicing `job_block` between
/// the value node's `SequenceStart`/`MappingStart` span-start and its
/// matching `SequenceEnd`/`MappingEnd` span-end instead recovers the
/// LITERAL source text verbatim (verified empirically: for `needs: [fmt,
/// clippy, ...]`, `SequenceStart`'s span starts at the `[` character and
/// `SequenceEnd`'s span ends immediately after the `]`) — this is still
/// tree-membership-derived (the span bounds come from the parsed node, not
/// a raw substring search), just applied to a composite value instead of a
/// single scalar's resolved text.
#[must_use]
pub fn job_level_value_span(job_block: &str, key: &str) -> Option<Range<usize>> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!(
                "wf.rs: job_level_value_span: failed to parse job block YAML as valid YAML 1.2: {e}"
            )
        });
    let table = char_byte_table(job_block);

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);

    let target = body_entries.iter().find(|e| e.key == key)?;
    match &events[target.value_start].0 {
        Event::MappingStart(..) | Event::SequenceStart(..) => {
            let value_end = skip_node(&events, target.value_start);
            let start_byte = table[events[target.value_start].1.start.index()];
            let end_byte = table[events[value_end - 1].1.end.index()];
            Some(start_byte..end_byte)
        }
        _ => None,
    }
}

/// Resolve the sorted, complete key set of the mapping VALUE of key `key`,
/// within the step whose OWN keys include `step_anchor_key` (e.g. `"run"`,
/// to anchor to the step actually carrying the gate decision), inside
/// `job_block`.
///
/// Unlike the pre-S-CIGATE-3 `extract_gate_env_key_set` (which scanned
/// BACKWARD from a `run:` line and therefore missed a legal `env:`-after-
/// `run:` reorder — see that function's own doc comment, round-14
/// SUGGESTION), this is order-independent BY CONSTRUCTION: it finds "the
/// step whose own mapping has both `step_anchor_key` and `key`" via tree
/// membership, not textual proximity, so `env:` may legally appear before
/// OR after `run:` on the same step without this function's result
/// changing.
///
/// Returns `None` if no step has `step_anchor_key`, that step has no `key`,
/// or `key`'s value there is not itself a mapping.
#[must_use]
pub fn step_mapping_child_keys(
    job_block: &str,
    step_anchor_key: &str,
    key: &str,
) -> Option<Vec<String>> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!("wf.rs: step_mapping_child_keys: failed to parse job block YAML as valid YAML 1.2: {e}")
        });

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let steps_entry = body_entries.iter().find(|e| e.key == "steps")?;
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return None;
    }
    let (items, _) = read_sequence(&events, steps_entry.value_start);

    for (item_start, _item_end) in items {
        if !matches!(events[item_start].0, Event::MappingStart(..)) {
            continue;
        }
        let (step_entries, _) = read_mapping(&events, item_start);
        if !step_entries.iter().any(|e| e.key == step_anchor_key) {
            continue;
        }
        let target = step_entries.iter().find(|e| e.key == key)?;
        if !matches!(events[target.value_start].0, Event::MappingStart(..)) {
            return None;
        }
        let (child_entries, _) = read_mapping(&events, target.value_start);
        let mut keys: Vec<String> = child_entries.iter().map(|e| e.key.clone()).collect();
        keys.sort();
        return Some(keys);
    }
    None
}

/// Resolve the [`Value`] of ONE named child (`child_key`) of the mapping
/// value of `mapping_key`, within the step whose own keys include
/// `step_anchor_key`, inside `job_block`. Sibling of
/// [`step_mapping_child_keys`] — that function answers "what keys does
/// this nested mapping have"; this one answers "what is the value of ONE
/// specific one of them" (e.g. the gate step's `env:` block's
/// `NEEDS_JSON:` child), including its `ScalarStyle`/tag/line-span for
/// AC-004 quoting-fidelity byte-pin comparison.
///
/// Returns `None` if no step has `step_anchor_key`, that step has no
/// `mapping_key`, `mapping_key`'s value there is not itself a mapping, or
/// that mapping has no `child_key`.
#[must_use]
pub fn step_mapping_child_value(
    job_block: &str,
    step_anchor_key: &str,
    mapping_key: &str,
    child_key: &str,
) -> Option<Value> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!("wf.rs: step_mapping_child_value: failed to parse job block YAML as valid YAML 1.2: {e}")
        });

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let steps_entry = body_entries.iter().find(|e| e.key == "steps")?;
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return None;
    }
    let (items, _) = read_sequence(&events, steps_entry.value_start);

    for (item_start, _item_end) in items {
        if !matches!(events[item_start].0, Event::MappingStart(..)) {
            continue;
        }
        let (step_entries, _) = read_mapping(&events, item_start);
        if !step_entries.iter().any(|e| e.key == step_anchor_key) {
            continue;
        }
        let mapping_entry = step_entries.iter().find(|e| e.key == mapping_key)?;
        if !matches!(events[mapping_entry.value_start].0, Event::MappingStart(..)) {
            return None;
        }
        let (child_entries, _) = read_mapping(&events, mapping_entry.value_start);
        let child = child_entries.iter().find(|e| e.key == child_key)?;
        return Some(resolve_value(&events, child.value_start));
    }
    None
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 pass C additions
//
// Everything below was added for the `ci-gate.needs` / job-graph migration
// cluster in `tests/ci_gate_completeness.rs`: `parse_needs_set`,
// `list_all_ci_yml_job_names`, `always_run_needs_members`,
// `matrix_needs_members`, `test_matrix_os_lists_remain_static_literals`, and
// `test_verify_msrv_job_pins_toolchain_and_rustup_toolchain_env`. These
// generalize pass A/B's job/step accessors to JOB-LEVEL NESTED mapping
// paths (`strategy.matrix.os`, `strategy.matrix.exclude`) that neither
// `Job::value_of` (one level only) nor the step-scoped
// `step_mapping_child_*` functions (anchored inside a `steps:` sequence
// item) can reach, plus a step accessor (`first_step_mapping_child_value`)
// that does not require a `step_anchor_key` disambiguator — needed because
// the `msrv` job has FOUR steps sharing a `uses:` key, only one of which
// has the `with.toolchain` child pass B's anchored
// `step_mapping_child_value` would need to disambiguate correctly.
// ---------------------------------------------------------------------------

/// Shared first step for every job-level-path accessor below: parse
/// `job_block` into its raw event stream, panicking (same contract as
/// [`WfDoc::parse`]) if it is not well-formed YAML 1.2.
fn parse_job_block_events<'a>(job_block: &'a str, caller: &str) -> Vec<(Event<'a>, Span)> {
    Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!("wf.rs: {caller}: failed to parse job block YAML as valid YAML 1.2: {e}")
        })
}

/// This job's own direct mapping entries (its single root entry's body), as
/// read by every job-level-path accessor below. `None` if `job_block` has
/// no top-level mapping at all, or its sole root entry's value is not
/// itself a mapping (a malformed job block).
fn job_body_entries(events: &[(Event<'_>, Span)]) -> Option<Vec<MapEntry>> {
    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(events, entry.value_start);
    Some(body_entries)
}

/// Descend `entries` through every segment of `path`, treating EACH
/// segment as a mapping key whose value is itself a mapping, returning the
/// FINAL segment's own entries. Returns `None` the moment any segment is
/// missing, or any segment's value is not a mapping.
fn descend_as_mappings(
    events: &[(Event<'_>, Span)],
    mut entries: Vec<MapEntry>,
    path: &[&str],
) -> Option<Vec<MapEntry>> {
    for segment in path {
        let target = entries.iter().find(|e| e.key == *segment)?;
        if !matches!(events[target.value_start].0, Event::MappingStart(..)) {
            return None;
        }
        let (next_entries, _) = read_mapping(events, target.value_start);
        entries = next_entries;
    }
    Some(entries)
}

/// Resolve the [`Value`] found by walking a JOB-LEVEL nested mapping PATH:
/// `path[0]` is a direct key of the job itself; each subsequent `path[i]`
/// is resolved as a child of the PREVIOUS segment's mapping value. Returns
/// the LAST segment's resolved [`Value`] — which may itself be
/// [`Value::Other`] (e.g. a further-nested mapping, or a sequence; see
/// [`job_level_nested_sequence_items`] for reading a sequence's own items).
///
/// # Why this exists (S-CIGATE-3 pass C)
///
/// Neither [`Job::value_of`] (one level only) nor the STEP-scoped
/// [`step_mapping_child_value`] (anchored inside a `steps:` sequence item)
/// can reach a job-level nested mapping chain like `strategy.matrix.os` —
/// GitHub Actions' build-matrix shape lives directly under the job, not
/// inside any step. Motivating caller:
/// `tests/ci_gate_completeness.rs::test_matrix_os_lists_remain_static_literals`.
///
/// Returns `None` if any NON-FINAL segment of `path` is missing or its
/// value is not itself a mapping (nothing further to descend into), or the
/// final segment itself is missing.
///
/// # Panics
///
/// Panics if `path` is empty (caller error — there is no key to resolve),
/// or if `job_block` is not well-formed YAML 1.2 (same contract as
/// [`WfDoc::parse`]).
#[must_use]
pub fn job_level_nested_value(job_block: &str, path: &[&str]) -> Option<Value> {
    assert!(
        !path.is_empty(),
        "wf.rs: job_level_nested_value: path must not be empty"
    );
    let events = parse_job_block_events(job_block, "job_level_nested_value");
    let body = job_body_entries(&events)?;
    let (prefix, last) = path.split_at(path.len() - 1);
    let entries = descend_as_mappings(&events, body, prefix)?;
    let target = entries.iter().find(|e| e.key == last[0])?;
    Some(resolve_value(&events, target.value_start))
}

/// Resolve the complete key set (source order, not deduplicated) of the
/// mapping found by walking a JOB-LEVEL nested mapping PATH — EVERY
/// segment, including the last, is resolved as a mapping (unlike
/// [`job_level_nested_value`], whose final segment may be any node kind).
///
/// Motivating caller: `strategy.matrix`'s own key set (does it declare
/// `exclude:`?) in
/// `tests/ci_gate_completeness.rs::test_matrix_os_lists_remain_static_literals`.
///
/// Returns `None` if any segment (including the last) is missing, or its
/// value is not itself a mapping.
///
/// # Panics
///
/// Same contract as [`job_level_nested_value`].
#[must_use]
pub fn job_level_nested_keys(job_block: &str, path: &[&str]) -> Option<Vec<String>> {
    assert!(
        !path.is_empty(),
        "wf.rs: job_level_nested_keys: path must not be empty"
    );
    let events = parse_job_block_events(job_block, "job_level_nested_keys");
    let body = job_body_entries(&events)?;
    let entries = descend_as_mappings(&events, body, path)?;
    Some(entries.iter().map(|e| e.key.clone()).collect())
}

/// Resolve a JOB-LEVEL nested mapping PATH's final segment as a SEQUENCE,
/// returning each item's resolved scalar text in source order. Every
/// segment before the last is resolved as a mapping (like
/// [`job_level_nested_value`]); the LAST segment's value must itself be a
/// sequence (block-list `- item` or flow `[a, b]` — tree membership makes
/// no distinction between the two forms, unlike the pre-S-CIGATE-3
/// line-based checker this replaces, which needed separate handling for
/// each).
///
/// Motivating callers: `needs:` (a single-segment path, `&["needs"]` —
/// `tests/ci_gate_completeness.rs::parse_needs_set`) and
/// `strategy.matrix.os` (a three-segment path —
/// `test_matrix_os_lists_remain_static_literals`).
///
/// Returns `None` if any segment is missing, a non-final segment's value
/// is not a mapping, or the final segment's value is not a sequence
/// (including a bare scalar — GitHub Actions permits `needs: single_job`
/// without brackets, which this function deliberately does NOT special-case
/// as a one-item list, mirroring the pre-S-CIGATE-3 checker's behavior).
///
/// # Panics
///
/// Same contract as [`job_level_nested_value`], plus: panics if any item
/// of the resolved sequence is not itself a scalar (an alias or nested
/// mapping/sequence item) — none of this file's callers' target keys
/// (`needs:`, `strategy.matrix.os`) ever legitimately contain one, and
/// silently skipping such an item would under-report true membership —
/// the same failure shape this story's whole rewrite exists to close.
#[must_use]
pub fn job_level_nested_sequence_items(job_block: &str, path: &[&str]) -> Option<Vec<String>> {
    assert!(
        !path.is_empty(),
        "wf.rs: job_level_nested_sequence_items: path must not be empty"
    );
    let events = parse_job_block_events(job_block, "job_level_nested_sequence_items");
    let body = job_body_entries(&events)?;
    let (prefix, last) = path.split_at(path.len() - 1);
    let entries = descend_as_mappings(&events, body, prefix)?;
    let target = entries.iter().find(|e| e.key == last[0])?;
    if !matches!(events[target.value_start].0, Event::SequenceStart(..)) {
        return None;
    }
    let (items, _) = read_sequence(&events, target.value_start);
    Some(
        items
            .into_iter()
            .map(|(item_start, _item_end)| match &events[item_start].0 {
                Event::Scalar(text, ..) => text.to_string(),
                other => panic!(
                    "wf.rs: job_level_nested_sequence_items: sequence item \
                     under `{}` is not a scalar: {other:?} — this indicates \
                     malformed or unsupported workflow YAML for this call \
                     site.",
                    path.join(".")
                ),
            })
            .collect(),
    )
}

/// Resolve the [`Value`] of ONE named child (`child_key`) of the mapping
/// value of `mapping_key`, within the FIRST step (in source order) whose
/// own `mapping_key` mapping actually CONTAINS `child_key`.
///
/// # Why this is a separate function from `step_mapping_child_value`
/// (S-CIGATE-3 pass B)
///
/// `step_mapping_child_value`'s `step_anchor_key` design assumes a single,
/// stable OTHER key uniquely identifies the step of interest — true for the
/// `ci-gate` gate step (only one step in that job has a `run:` key at all),
/// but not true in general. Motivating caller:
/// `tests/ci_gate_completeness.rs::test_verify_msrv_job_pins_toolchain_and_rustup_toolchain_env`
/// — the `msrv` job has FOUR steps that all carry a `uses:` key, only one
/// of which has a `with.toolchain` child. Anchoring on
/// `step_anchor_key = "uses"` would match the WRONG step
/// (`step-security/harden-runner`, source-order-first, whose own `with:`
/// has `egress-policy` but no `toolchain`) and — per
/// `step_mapping_child_value`'s own `?`-early-return-on-missing-child
/// behavior — return `None` for the whole job rather than continuing to the
/// step that actually has it. This function instead scans every step,
/// SKIPPING (not aborting on) a step whose `mapping_key` mapping exists but
/// lacks `child_key`.
///
/// Returns `None` if no step has a `mapping_key` mapping containing
/// `child_key`.
///
/// # Panics
///
/// Same malformed-YAML-panics contract as [`WfDoc::parse`].
#[must_use]
pub fn first_step_mapping_child_value(
    job_block: &str,
    mapping_key: &str,
    child_key: &str,
) -> Option<Value> {
    let events = parse_job_block_events(job_block, "first_step_mapping_child_value");

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let steps_entry = body_entries.iter().find(|e| e.key == "steps")?;
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return None;
    }
    let (items, _) = read_sequence(&events, steps_entry.value_start);

    for (item_start, _item_end) in items {
        if !matches!(events[item_start].0, Event::MappingStart(..)) {
            continue;
        }
        let (step_entries, _) = read_mapping(&events, item_start);
        let Some(mapping_entry) = step_entries.iter().find(|e| e.key == mapping_key) else {
            continue;
        };
        if !matches!(events[mapping_entry.value_start].0, Event::MappingStart(..)) {
            continue;
        }
        let (child_entries, _) = read_mapping(&events, mapping_entry.value_start);
        let Some(child) = child_entries.iter().find(|e| e.key == child_key) else {
            continue;
        };
        return Some(resolve_value(&events, child.value_start));
    }
    None
}
