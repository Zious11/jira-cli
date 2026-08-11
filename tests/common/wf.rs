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
    /// This job's `steps:` sequence, if it has one and it is a sequence.
    /// Empty otherwise (no `steps:` key, or its value is not a sequence).
    pub steps: Vec<Step>,
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
    /// The byte range in the original source text this step entry occupies
    /// (from its first event's span start to its last event's span end — NOT
    /// line-snapped the way [`Job::span`] is, since no caller depends on
    /// step-level line anchoring yet).
    pub span: Range<usize>,
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

                    let (keys, steps) =
                        if matches!(events[entry.value_start].0, Event::MappingStart(..)) {
                            let (body_entries, _) = read_mapping(&events, entry.value_start);
                            let keys: Vec<String> =
                                body_entries.iter().map(|e| e.key.clone()).collect();
                            let steps = extract_steps(&events, &body_entries, &table);
                            (keys, steps)
                        } else {
                            (Vec::new(), Vec::new())
                        };

                    jobs.push(Job {
                        id: entry.key.clone(),
                        span: start_byte..end_byte,
                        keys,
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
            Event::Scalar(text, ..) => {
                let key = text.to_string();
                let key_span = events[i].1;
                let value_start = i + 1;
                let value_end = skip_node(events, value_start);
                entries.push(MapEntry {
                    key,
                    key_span,
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
            span,
        };
    }

    let (entries, _) = read_mapping(events, item_start);
    let keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
    let name = entries.iter().find(|e| e.key == "name").and_then(|e| {
        if let Event::Scalar(text, ..) = &events[e.value_start].0 {
            Some(text.to_string())
        } else {
            None
        }
    });

    Step { name, keys, span }
}
