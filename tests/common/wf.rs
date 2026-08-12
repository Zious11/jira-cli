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
        /// Whether the scalar carries a YAML anchor (`&x`) directly on
        /// itself (e.g. `run: &x cargo check`).
        ///
        /// Added by S-CIGATE-3 fix (B-1 / VALUE-SIDE-ANCHOR-GAP-UNCLOSED):
        /// `resolve_value` used to discard `anchor_id` entirely, so a
        /// value-side anchor on an otherwise byte-correct pinned scalar
        /// (`run: &x echo "${NEEDS_JSON}" | bash scripts/check-ci-gate.sh`)
        /// was invisible to every byte-pin assertion in
        /// `tests/ci_gate_completeness.rs` that reads a `Value::Scalar`'s
        /// `text`/`style`/`tag` — mirrors `tag`'s existing rationale above,
        /// just for the anchor half of a YAML node property instead of the
        /// tag half. See [`find_key_node_properties`]'s doc comment,
        /// "Scope correction" section, for the full before/after account.
        has_anchor: bool,
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
    /// contract for equivalence purposes (verified during the S-CIGATE-3
    /// migration against the now-deleted line-based scanner via a scratch
    /// harness, not a tracked test file; this module's own `#[cfg(test)]
    /// mod tests` below is the permanent regression coverage for the
    /// span-correctness properties this field depends on): the range
    /// starts at byte 0 of the
    /// physical line containing this job's key (i.e. it includes the job
    /// key's leading indentation, recovered via `Marker::col()` — see module
    /// docs), and ends at the start of the next sibling job's line.
    ///
    /// For the LAST job in `jobs:`, the end bound is (S-CIGATE-3
    /// adversarial pass 5, MEDIUM; fixed after the ADV-SC3-P5-MED
    /// finding): the start of the line containing the next ROOT-level key
    /// after `jobs:`, if one exists — `jobs:` need not be the workflow's
    /// final top-level key; `concurrency:`, `defaults:`, `env:`,
    /// `permissions:`, `on:` are all legal AFTER it, and the ORIGINAL
    /// unconditional `yaml.len()` bound silently swallowed any such
    /// trailing root key into the last job's own block. Only when `jobs:`
    /// genuinely IS the workflow's last root key does this fall back to
    /// `yaml.len()` — preserving the ORIGINAL rationale for that bound
    /// (verified: using the `jobs:` mapping's `MappingEnd` marker instead
    /// of `yaml.len()` undershoots by design — see module docs on
    /// `sync-upstream.yml`'s trailing block scalar) exactly for the case
    /// it was chosen to cover. See `WfDoc::parse`'s
    /// `test_wfdoc_parse_last_job_span_excludes_trailing_root_key` and
    /// `test_wfdoc_parse_last_job_span_includes_full_trailing_block_scalar_when_jobs_is_last_root_key`
    /// for the two regression tests pinning both directions.
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
    ///
    /// # Panics (S-CIGATE-3, ADV-SC3-P6-LOW-002)
    ///
    /// Panics if `key` appears more than once among this job's own direct
    /// keys, naming the job id, the duplicated key, and the occurrence
    /// count. Before this fix, this function was the module's ONE
    /// remaining silent-first-match mapping-child lookup — its own doc
    /// comment claimed to be "mirroring every other first-match convention
    /// in this module", but [`find_unique_entry`]'s doc comment (see its
    /// "Coverage correction" paragraph) states every OTHER mapping-child
    /// lookup in this module was converted away from first-match
    /// specifically because a duplicate key silently resolving to its
    /// first occurrence reopens the exact S-626-1 MSRV false-green class
    /// this module exists to prevent — there was no remaining convention
    /// left to mirror, and the claim that "a duplicate mapping key is
    /// itself invalid YAML GitHub Actions/`actionlint` reject at parse
    /// time" is precisely the external validation this module's other
    /// lookups (and `find_unique_entry` itself) explicitly refuse to rely
    /// on. This function now applies the same refusal [`Job::keys`] and
    /// [`Step::keys`] remain available directly for any caller that
    /// legitimately wants the raw, non-deduplicated list instead.
    #[must_use]
    pub fn value_of(&self, key: &str) -> Option<&Value> {
        let matches: Vec<usize> = self
            .keys
            .iter()
            .enumerate()
            .filter(|(_, k)| k.as_str() == key)
            .map(|(i, _)| i)
            .collect();
        assert!(
            matches.len() <= 1,
            "wf.rs: Job::value_of: job `{}` has {} occurrences of the \
             `{key}:` key at its own (job) level — this is invalid YAML (a \
             duplicate mapping key) that GitHub Actions and actionlint both \
             reject at parse time, but this checker refuses to silently \
             pick a winner rather than rely on that external validation. \
             Remove the duplicate `{key}:` key.",
            self.id,
            matches.len(),
        );
        matches.first().map(|&i| &self.values[i])
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
    /// (from its first event's span start to its last event's span end).
    ///
    /// # Load-bearing consumer (S-CIGATE-3 fix-burst-6,
    /// ADV-SC3-P4-MED-001): [`step_mapping_child_value_for_step`]
    ///
    /// This field is `step_mapping_child_value_for_step`'s SOLE identity
    /// key for resolving a caller-supplied `&Step` back to its own entry
    /// inside a freshly re-parsed `job_block` — the guard used, among
    /// other call sites, by `ci_gate_completeness.rs`'s `msrv`-job
    /// toolchain/`RUSTUP_TOOLCHAIN` checks (S-626-1 AC-3). It is
    /// deliberately NOT line-snapped the way [`Job::span`] is: line-
    /// snapping would round two DIFFERENT steps that happen to start on
    /// the same physical line (impossible in valid block-sequence YAML,
    /// but not a distinction span identity should depend on getting right
    /// by accident) — or, more relevantly, would desync this field from
    /// the exact byte-span identity formula `step_mapping_child_value_for_
    /// step` independently recomputes from the SAME `job_block` text when
    /// it re-parses it (see that function's own `byte_of`/`matched` logic)
    /// — from that consumer's expectations, silently breaking the guard
    /// with a confusing false-red (a legitimate step failing to resolve)
    /// rather than a loud one. Do NOT line-snap this field "for
    /// consistency with `Job::span`" without first updating that
    /// consumer's own span-recomputation to match.
    pub span: Range<usize>,
}

impl Step {
    /// This step's resolved [`Value`] for its direct key `key`, if present.
    ///
    /// # Panics
    ///
    /// Same duplicate-key-refusal contract as [`Job::value_of`] — see that
    /// function's doc comment for the full rationale (S-CIGATE-3,
    /// ADV-SC3-P6-LOW-002). Names the step's own `name:` value (or
    /// `<unnamed>` if it has none) rather than a job id.
    #[must_use]
    pub fn value_of(&self, key: &str) -> Option<&Value> {
        let matches: Vec<usize> = self
            .keys
            .iter()
            .enumerate()
            .filter(|(_, k)| k.as_str() == key)
            .map(|(i, _)| i)
            .collect();
        assert!(
            matches.len() <= 1,
            "wf.rs: Step::value_of: step `{}` has {} occurrences of the \
             `{key}:` key at its own (step) level — this is invalid YAML \
             (a duplicate mapping key) that GitHub Actions and actionlint \
             both reject at parse time, but this checker refuses to \
             silently pick a winner rather than rely on that external \
             validation. Remove the duplicate `{key}:` key.",
            self.name.as_deref().unwrap_or("<unnamed>"),
            matches.len(),
        );
        matches.first().map(|&i| &self.values[i])
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
        assert_single_document(&events, "WfDoc::parse");

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
        if let Some(jobs_entry) = find_unique_entry(&root_entries, "jobs", "WfDoc::parse") {
            if matches!(events[jobs_entry.value_start].0, Event::MappingStart(..)) {
                // `jobs:` need not be the workflow's FINAL top-level key —
                // `concurrency:`, `defaults:`, `env:`, `permissions:`,
                // `on:` are all legal AFTER `jobs:` (S-CIGATE-3 adversarial
                // pass 5, MEDIUM). `find_unique_entry` above already
                // proved there is at most one `jobs:` key among
                // `root_entries`, so locating IT by key text again here is
                // safe (no duplicate to silently pick a winner between).
                // If a root key follows `jobs:` in source order, the LAST
                // job's span must be bounded by that key's own line start
                // — never by `yaml.len()` — or the last job's block would
                // silently swallow it.
                let next_root_key_start_byte = root_entries
                    .iter()
                    .position(|e| e.key == "jobs")
                    .and_then(|jobs_idx| root_entries.get(jobs_idx + 1))
                    .map(|next_root_entry| byte_of(line_start_char_idx(&next_root_entry.key_span)));

                let (job_entries, _) = read_mapping(&events, jobs_entry.value_start);
                assert_no_duplicate_keys(&job_entries, "WfDoc::parse (jobs:)");
                let job_count = job_entries.len();
                for (idx, entry) in job_entries.iter().enumerate() {
                    let start_byte = byte_of(line_start_char_idx(&entry.key_span));
                    let end_byte = if idx + 1 < job_count {
                        byte_of(line_start_char_idx(&job_entries[idx + 1].key_span))
                    } else {
                        // Last job in `jobs:`: bound to the next ROOT
                        // key's line start when one follows `jobs:`
                        // (mirrors the inter-job boundary above, one level
                        // out — everything up to that line, including any
                        // trailing blank lines/comments and a trailing
                        // block scalar's full content, still belongs to
                        // this job). Only fall back to `yaml.len()` when
                        // `jobs:` genuinely IS the workflow's last root
                        // key — preserving the original, deliberately
                        // undershoot-proof behavior for that case (see
                        // `Job::span`'s own doc comment on
                        // `sync-upstream.yml`'s trailing block scalar).
                        next_root_key_start_byte.unwrap_or(yaml.len())
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

/// Assert that `events` — collected from ONE `Parser::new_from_str` call —
/// represents AT MOST one YAML document, i.e. zero or one
/// `Event::DocumentStart`. Panics, naming the actual count, if there is
/// MORE than one.
///
/// # Why this exists (S-CIGATE-3 fix-burst-3, ADV-SC3-P1-LOW-001)
///
/// `saphyr_parser::Parser` iterates every document in a `---`-separated
/// YAML STREAM, not just the first — this is correct, spec-compliant
/// behavior for a general-purpose YAML parser. Every parse entry point in
/// this module, however, hunts for the first `Event::MappingStart` (or the
/// first `Event::DocumentStart`, for this very check) and builds its
/// structure from that alone; nothing in this module was ever designed to
/// handle a multi-document stream. Before this assertion existed, a `---`
/// appended to `.github/workflows/ci.yml` followed by a second document
/// containing e.g. `defaults: run: shell: cat {0}` was silently invisible
/// to every guard built on [`WfDoc::root_keys`] — a real, if previously
/// unenumerated, regression versus the pre-S-CIGATE-3 line-based scanner,
/// which read every physical line in the file regardless of `---`
/// boundaries and would have seen the smuggled `defaults:` key. Verified
/// directly: `WfDoc::parse` on such a stream returned `root_keys ==
/// ["jobs"]` only, with zero indication the input actually had two
/// documents, before this function existed.
///
/// Called at the START of every parse entry point in this module,
/// immediately after collecting the event stream — before any structural
/// lookup runs — so a multi-document stream fails loudly and immediately
/// rather than silently proceeding against document 1 alone.
///
/// # Zero documents is not a bug (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-LOW-001)
///
/// A comment-only or whitespace-only YAML stream produces `StreamStart,
/// StreamEnd` with ZERO `Event::DocumentStart` events — this is
/// well-formed YAML, not malformed input, and [`WfDoc::parse`]'s own doc
/// comment already documents the empty-root case as "[n]ot a
/// malformed-YAML case — just an empty result". The original version of
/// this function asserted `doc_start_count == 1` exactly, so a
/// comment-only stream panicked here with a message blaming a
/// `---`-separated multi-document stream — the opposite of what actually
/// happened, and a real contradiction with `WfDoc::parse`'s own contract.
/// The check is therefore an upper bound (`<= 1`), not an equality: every
/// caller of this function already handles the "no top-level mapping"
/// case correctly on its own path (an `Option`-returning `?` early-return,
/// or — for [`WfDoc::parse_single_job`] specifically — its own
/// purpose-built "job_block has no top-level mapping at all" panic with an
/// accurate message), so there is nothing left for THIS function to reject
/// once the "more than one document" case is ruled out.
fn assert_single_document(events: &[(Event<'_>, Span)], caller: &str) {
    let doc_start_count = events
        .iter()
        .filter(|(ev, _)| matches!(ev, Event::DocumentStart(..)))
        .count();
    assert!(
        doc_start_count <= 1,
        "wf.rs: {caller}: expected at most one YAML document in the parsed \
         stream, found {doc_start_count} — a multi-document stream \
         (`---`-separated) is not supported by this module: every \
         structural lookup built on this event list would silently see \
         only the FIRST document, discarding the rest. Split the extra \
         document(s) into their own file, or extend this module to handle \
         a multi-document stream explicitly, rather than parsing one and \
         silently ignoring the others."
    );
}

/// Look up the single entry in `entries` whose key text equals `key`.
///
/// # Why this exists (S-CIGATE-3 fix-burst-3, ADV-SC3-P1-MED-004)
///
/// [`read_mapping`]'s own doc comment states plainly that a duplicate
/// mapping key is left entirely to the CALLER to judge — the event stream
/// never collapses one. Every lookup in this module that used to resolve
/// ONE named child of a mapping via `entries.iter().find(...)` silently
/// returned the FIRST match on a duplicate — the same "silently pick a
/// winner" shape the four scalar-pin call sites in
/// `tests/ci_gate_completeness.rs` (`extract_and_normalize_if_expr`,
/// `parse_needs_set`, `extract_and_normalize_sole_needs_line`,
/// `extract_and_normalize_sole_needs_json_line`) already refuse to do for
/// a SCALAR value's key. This function gives every MAPPING-CHILD lookup in
/// this module the same refusal. Verified bypass this closes: a second
/// root-level `env:` block appended to `ci.yml` (containing e.g. a
/// smuggled `BASH_ENV`) was silently invisible to
/// `extract_workflow_env_key_set` (via `root_level_nested_keys` →
/// `descend_as_mappings`), because the un-fixed `.find` picked the FIRST
/// `env:` and never looked at the second; the same shape applied to a
/// duplicate root `jobs:` key hiding an entire second job map from
/// [`WfDoc::parse`].
///
/// # Coverage correction (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-MED-001)
///
/// The paragraph above was true in INTENT from the moment this function
/// was added, but NOT in fact until this pass: fix-burst-3 wired only TWO
/// call sites through this function ([`WfDoc::parse`]'s `jobs:` lookup and
/// [`descend_as_mappings`]'s per-segment walk) while leaving TWELVE other
/// `entries.iter().find(|e| e.key == ...)` sites in this same module doing
/// the exact silent-first-match thing this function exists to refuse —
/// including, concretely, the (since-deleted, S-CIGATE-3 fix-burst-7 —
/// zero remaining callers; see `step_mapping_child_value_for_step`'s doc
/// comment for what replaced it) `first_step_mapping_child_value`'s
/// `with:`/`env:` lookups, which meant a duplicated `toolchain:` or
/// `RUSTUP_TOOLCHAIN:` key inside the `msrv` job's own YAML could resolve
/// to its FIRST occurrence with no panic at all — verbatim the S-626-1
/// MSRV false-green this whole module exists to prevent, reopened one
/// layer down. Every mapping-child lookup in this module — at the time,
/// [`extract_steps`], [`build_step`], [`job_level_value_span`],
/// [`step_mapping_child_keys`], [`step_mapping_child_value`],
/// [`job_level_nested_value`], [`job_level_nested_sequence_items`], and
/// `first_step_mapping_child_value` (since deleted), in addition to the
/// original two — now routes through this function; grep for `.find(|e| e.key` (or
/// `.find(|e| e\.key` as a regex) in this file returns zero matches as of
/// this pass. If that grep ever returns a hit again, this doc comment's
/// coverage claim is false again and the offending site needs the same fix
/// applied here, not a doc-comment correction alone.
///
/// # Panics
///
/// Panics if `entries` contains more than one entry with key text `key`,
/// naming the duplicated key and the caller.
fn find_unique_entry<'a>(entries: &'a [MapEntry], key: &str, caller: &str) -> Option<&'a MapEntry> {
    let matches: Vec<&MapEntry> = entries.iter().filter(|e| e.key == key).collect();
    assert!(
        matches.len() <= 1,
        "wf.rs: {caller}: mapping has {} occurrences of the `{key}:` key at \
         this level — this is invalid YAML (a duplicate mapping key) that \
         GitHub Actions and actionlint both reject at parse time, but this \
         checker refuses to silently pick a winner rather than rely on \
         that external validation. Remove the duplicate `{key}:` key.",
        matches.len(),
    );
    matches.into_iter().next()
}

/// Assert that `entries` — a mapping's own complete direct entries, as
/// returned by [`read_mapping`] or [`descend_as_mappings`] — contain no two
/// entries with the same key text. Panics naming the duplicated key
/// otherwise.
///
/// Sibling of [`find_unique_entry`] (which checks for a duplicate of ONE
/// named key while navigating toward it): this checks an entire resolved
/// mapping level at once, for callers (like [`root_level_nested_keys`])
/// that hand the FULL key set of a resolved mapping back to their own
/// caller — a duplicate INSIDE that final level (e.g. two `CARGO_TERM_COLOR:`
/// children of one `env:` block) would otherwise silently survive into a
/// caller's "complete key set" with no indication of the problem.
///
/// # Panics
///
/// Panics on the first duplicate key found, naming it and `caller`.
fn assert_no_duplicate_keys(entries: &[MapEntry], caller: &str) {
    let mut seen: Vec<&str> = Vec::with_capacity(entries.len());
    for entry in entries {
        let key = entry.key.as_str();
        assert!(
            !seen.contains(&key),
            "wf.rs: {caller}: mapping has more than one `{key}:` key at \
             this level — this is invalid YAML (a duplicate mapping key) \
             that GitHub Actions and actionlint both reject at parse \
             time, but this checker refuses to silently pick a winner \
             rather than rely on that external validation. Remove the \
             duplicate `{key}:` key."
        );
        seen.push(key);
    }
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
        Event::Scalar(text, style, anchor_id, tag) => {
            let span = events[value_start].1;
            Value::Scalar {
                text: text.to_string(),
                style: *style,
                tag: tag.as_ref().map(|t| format!("{}{}", t.handle, t.suffix)),
                has_anchor: *anchor_id != 0,
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
    let Some(steps_entry) = find_unique_entry(job_body_entries, "steps", "extract_steps") else {
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
///
/// # Diagnostic-panic byte lookup (S-CIGATE-3 fix-burst-3, ADV-SC3-P1-INFO-001)
///
/// `Step::span`'s byte bounds are resolved through a local `byte_of`
/// closure — the same pattern [`WfDoc::parse`] uses for `Job::span` — rather
/// than raw `table[...]` indexing. **Correction (S-CIGATE-3 fix-burst-6,
/// ADV-SC3-P4-MED-001):** `Step::span` is no longer without a consumer —
/// [`step_mapping_child_value_for_step`] uses it as its sole identity key
/// for re-locating a caller-resolved `&Step` inside a freshly re-parsed
/// `job_block` (see that function's own doc comment, and [`Step::span`]'s
/// field doc, for the full rationale and why this field must NOT be
/// line-snapped the way `Job::span` is). This closure's diagnostic
/// specificity — naming the module, the offending index, and the table's
/// size on an out-of-range char index, rather than a bare `table[...]`
/// index panic's generic Rust "index out of bounds" — matters more now
/// that a real caller depends on this field's correctness, not less.
fn build_step(
    events: &[(Event<'_>, Span)],
    item_start: usize,
    item_end: usize,
    table: &[usize],
) -> Step {
    let byte_of = |char_idx: usize| -> usize {
        *table.get(char_idx).unwrap_or_else(|| {
            panic!(
                "wf.rs: build_step: char index {char_idx} out of range for \
                 a table of {} entries — this indicates a bug in this \
                 module's event-index bookkeeping, not malformed input",
                table.len().saturating_sub(1)
            )
        })
    };
    let start_byte = byte_of(events[item_start].1.start.index());
    let end_byte = byte_of(events[item_end - 1].1.end.index());
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
    let name = find_unique_entry(&entries, "name", "build_step").and_then(|e| {
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
/// see is a node property attached to a KEY that is ALREADY a legitimate,
/// expected member of the pinned set — e.g. `&x run: some-other-command`:
/// the key set stays exactly `{"env","name","run"}`, textually identical to
/// the pin, while the *key* scalar `run` has silently gained an anchor a
/// later alias elsewhere in the same document (GitHub shipped anchor/alias
/// support to production Actions 2025-09-18) could reference. This
/// function closes that residual gap by scanning for the node property
/// itself, independent of whether the key's SET membership also happens to
/// be correct.
///
/// # Scope correction (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-LOW-004; CLOSED
/// by a later fix — see the paragraph after next): KEY anchors and tags
/// only, not VALUE anchors
///
/// In `&x run: some-other-command`, `&x` attaches to the KEY node `run`
/// (YAML node properties bind to the node immediately following them —
/// here that's the key scalar, not the value scalar), which is exactly
/// what [`MapEntry::key_has_anchor`]/`key_tag` capture and this function
/// scans for. A DIFFERENT construct — `run: &x some-other-command`, an
/// anchor on the VALUE — was NOT covered by this function, nor (at the
/// time this section was first written) by anything else in this module:
/// [`resolve_value`] deliberately captured a value scalar's `tag` (see
/// [`Value::Scalar::tag`]) but discarded its `anchor_id` entirely, so a
/// VALUE-side anchor on an already-pinned scalar (e.g. `run: &x cargo
/// check --all-features --locked`) was invisible to every byte-pin
/// assertion in `tests/ci_gate_completeness.rs` that reads a
/// `Value::Scalar`'s `text`/`style`/`tag`.
///
/// **This gap is now CLOSED (S-CIGATE-3, PR #680 review finding B-1 /
/// VALUE-SIDE-ANCHOR-GAP-UNCLOSED) for the five pins that need it.**
/// [`Value::Scalar`] gained a `has_anchor: bool` field, [`resolve_value`]
/// now captures `anchor_id != 0` into it instead of discarding it, and
/// [`job_level_value_span`] (the composite-value sibling used for
/// `needs:`'s flow-sequence form, which has no single `Event::Scalar` to
/// route through `resolve_value` at all) now inspects the value node's own
/// `anchor_id`/`tag` directly and returns [`ValueSpanOutcome::NodeProperty`]
/// instead of a slice-able span when either is present. Every one of the
/// five byte-for-byte pins in `tests/ci_gate_completeness.rs`
/// (`extract_and_normalize_if_expr`, `extract_and_normalize_sole_run_line`,
/// `extract_and_normalize_step_run_line_by_name`,
/// `extract_and_normalize_sole_needs_json_line`, and
/// `extract_and_normalize_sole_needs_line`) now rejects a value-side
/// anchor the same way it already rejected a value-side tag — closing the
/// asymmetry this section used to describe between the KEY-anchor
/// guarantee above and what the VALUE side actually had. A later `*x`
/// alias appearing in a pinned slot remains independently hard-rejected
/// as `Value::Alias` by every pin function's own match arms, unchanged by
/// this fix.
///
/// # Additional accepted residuals (adversarial pass 5, S-CIGATE-3, LOW)
///
/// Two further LOW-severity gaps were found and accepted (documented, not
/// fixed) in the same review pass that produced the last-job-span fix
/// (`WfDoc::parse`'s `end_byte` computation, S-CIGATE-3 ADV pass-5
/// MEDIUM):
///
/// - **LOW-1 (aliased `steps:`/whole-job values):** an aliased `steps:`
///   value (`steps: *common`) or an aliased whole-job value (`build:
///   *tmpl`) is not resolved by this module (see the module docs' own
///   "Aliases are not resolved" note) — [`extract_steps`] and
///   [`WfDoc::parse`]'s job-body branch both require an
///   `Event::MappingStart`/`Event::SequenceStart` to build anything, so
///   an `Event::Alias` value there yields an empty step/key list rather
///   than the aliased content. This FAILS CLOSED (a false-RED: the
///   affected pin simply can't locate the step/key it expects and errors
///   loudly), not open, and is not modeled further — same acceptance
///   basis as the VALUE-side-anchor residual just above: no workflow file
///   in this repo uses an anchor/alias today, and GitHub only shipped
///   Actions anchor/alias support to production on 2025-09-18.
/// - **LOW-3 (gate-step anchoring is first-match, not ambiguity-checked):**
///   `tests/ci_gate_completeness.rs::extract_gate_env_key_set` and
///   `::extract_and_normalize_sole_needs_json_line` anchor the gate step
///   via [`step_mapping_child_keys`]'s first-match-by-`step_anchor_key`
///   semantics (the first step whose own keys include `"run"`), rather
///   than an ambiguity-checked "exactly one such step" lookup. This is
///   backstopped, not exploitable: a decoy earlier `run:`+`env:` step
///   changes the `ci-gate` job's step-KEY-SET arity, which trips
///   `PINNED_GATE_STEP_KEY_SETS`/`PINNED_GATE_JOB_KEYS` independently of
///   whether this anchoring picks the "right" step. Recorded as an
///   accepted consistency residual to avoid churning gate-critical
///   accessors that are already covered by an orthogonal pin, not fixed
///   here.
///
/// # Panics
///
/// Same malformed-YAML-panics contract as [`WfDoc::parse`].
#[must_use]
pub fn find_key_node_properties(yaml: &str) -> Vec<KeyNodeProperty> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(yaml)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| panic!("wf.rs: failed to parse workflow YAML as valid YAML 1.2: {e}"));
    assert_single_document(&events, "find_key_node_properties");

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
        assert_single_document(&events, "parse_single_job");
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

/// Outcome of [`job_level_value_span`]'s lookup for a composite (mapping or
/// sequence) job-level value.
///
/// Added by S-CIGATE-3 fix (B-1 / VALUE-SIDE-ANCHOR-GAP-UNCLOSED): before
/// this, `job_level_value_span` returned a bare `Option<Range<usize>>` whose
/// span started at the value node's CONTENT — not at a preceding node
/// property token — so `needs: &x [fmt, clippy, ...]` sliced out exactly
/// `"[fmt, clippy, ...]"`, byte-identical to the pin, with the anchor
/// silently dropped. This mirrors [`Value::Scalar`]'s `has_anchor`/`tag`
/// fields, just for a composite (non-scalar) value node, which
/// `resolve_value` never captured node-property info for either (it
/// collapses every `MappingStart`/`SequenceStart` to `Value::Other`
/// regardless of anchor/tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueSpanOutcome {
    /// The value node's byte span, safe to slice verbatim for a byte-pin
    /// comparison — the node carries no anchor or tag directly on itself.
    Span(Range<usize>),
    /// The value node itself carries a YAML anchor and/or tag directly on
    /// it (e.g. `needs: &x [fmt, clippy]` or `needs: !!seq [fmt, clippy]`).
    /// The caller MUST reject this rather than slice past the node
    /// property — mirrors [`find_key_node_properties`]'s KEY-side
    /// rejection, applied here to the VALUE side.
    NodeProperty {
        /// Whether the value node carries a YAML anchor (`&x`).
        has_anchor: bool,
        /// The value node's tag (e.g. `!!seq`), if any, as `handle` +
        /// `suffix` concatenated.
        tag: Option<String>,
    },
}

/// Resolve the byte SPAN (into `job_block`) of the job-level value of key
/// `key`, PROVIDED that value is itself a mapping or sequence node (for a
/// scalar/alias value, use [`Job::value_of`] instead — this function exists
/// specifically for a composite value like `needs: [a, b, c]`, which has no
/// single `Event::Scalar` to source byte-for-byte pinned text from).
///
/// Returns `None` if `job_block`'s job has no `key` at its own (job) level,
/// or `key`'s value there is a scalar or alias rather than a mapping/
/// sequence. Returns `Some(ValueSpanOutcome::NodeProperty { .. })`, instead
/// of a span, if the value node itself carries an anchor and/or tag — see
/// [`ValueSpanOutcome`]'s doc comment.
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
pub fn job_level_value_span(job_block: &str, key: &str) -> Option<ValueSpanOutcome> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!(
                "wf.rs: job_level_value_span: failed to parse job block YAML as valid YAML 1.2: {e}"
            )
        });
    assert_single_document(&events, "job_level_value_span");
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

    let target = find_unique_entry(&body_entries, key, "job_level_value_span")?;
    match &events[target.value_start].0 {
        Event::MappingStart(anchor_id, tag) | Event::SequenceStart(anchor_id, tag) => {
            if *anchor_id != 0 || tag.is_some() {
                return Some(ValueSpanOutcome::NodeProperty {
                    has_anchor: *anchor_id != 0,
                    tag: tag.as_ref().map(|t| format!("{}{}", t.handle, t.suffix)),
                });
            }
            let value_end = skip_node(&events, target.value_start);
            let start_byte = table[events[target.value_start].1.start.index()];
            let end_byte = table[events[value_end - 1].1.end.index()];
            Some(ValueSpanOutcome::Span(start_byte..end_byte))
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
    assert_single_document(&events, "step_mapping_child_keys");

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let steps_entry = find_unique_entry(&body_entries, "steps", "step_mapping_child_keys")?;
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
        let target = find_unique_entry(&step_entries, key, "step_mapping_child_keys")?;
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

/// Resolve the sorted, complete key set of the mapping VALUE of key `key`,
/// within the SOLE step whose OWN `name:` value is EXACTLY `step_name`,
/// inside `job_block`. Sibling of [`step_mapping_child_keys`], but selects
/// the step by NAME (tree-unique — `Err` on zero or more than one match)
/// rather than by "the first step that happens to carry `step_anchor_key`".
///
/// # S-CIGATE-3 fix-burst-5 (ADV-SC3-P3-MED-001)
///
/// Added for `ci_gate_completeness.rs`'s `extract_test_guard_env_key_set`,
/// which used to call [`step_mapping_child_keys`] with `step_anchor_key =
/// "run"` — that anchors on "the FIRST step in the job's `steps:` sequence
/// that carries a `run:` key", not specifically the `test` job's named
/// POL-11 guard step. A DECOY step inserted before the real one — sharing
/// the real step's `name:` value, and carrying its own `run:` AND `env:`
/// keys (so it satisfies `PINNED_TEST_GUARD_ENV_KEYS` too) — silently wins
/// that anchor-based lookup while the real step (disabled via `if: false`,
/// or simply the second of two identically-named steps) goes unexamined.
/// This function instead requires the step to be identified UNAMBIGUOUSLY
/// by its own `name:` value first — mirroring
/// `ci_gate_completeness.rs::find_sole_step_by_name`'s ambiguity contract
/// (`Err` on zero or more than one match) — before ever looking at `key`'s
/// children, so a same-named decoy is flagged as ambiguity rather than
/// silently outranking (or being silently outranked by) the real step.
///
/// Returns `Err` if the job has no `steps:` sequence, or zero or more than
/// one step has this `name:` value. Returns `Ok(Vec::new())` if the
/// (uniquely resolved) step has no `key`, or `key`'s value there is not
/// itself a mapping — mirrors `step_mapping_child_keys`'s `None`-becomes-
/// empty convention already relied on at existing call sites via
/// `.unwrap_or_default()`, since "the step exists but has no `env:`
/// block" is a legitimate (if perhaps unexpected) state, not a structural
/// parse failure the caller should be forced to handle separately from
/// "the block is empty".
pub fn step_mapping_child_keys_by_step_name(
    job_block: &str,
    step_name: &str,
    key: &str,
) -> Result<Vec<String>, String> {
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!(
                "wf.rs: step_mapping_child_keys_by_step_name: failed to parse \
                 job block YAML as valid YAML 1.2: {e}"
            )
        });
    assert_single_document(&events, "step_mapping_child_keys_by_step_name");

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
        .ok_or_else(|| "job block has no top-level mapping".to_string())?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries
        .first()
        .ok_or_else(|| "job block's top-level mapping is empty".to_string())?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return Err("job block's single entry's value is not a mapping".to_string());
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let Some(steps_entry) = find_unique_entry(
        &body_entries,
        "steps",
        "step_mapping_child_keys_by_step_name",
    ) else {
        return Err("job has no `steps:` key at all".to_string());
    };
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return Err("job's `steps:` value is not a sequence".to_string());
    }
    let (items, _) = read_sequence(&events, steps_entry.value_start);

    // Collect every step whose OWN `name:` value resolves to exactly
    // `step_name` — not a first-match `.find()`.
    let mut matched_step_entries: Vec<Vec<MapEntry>> = Vec::new();
    for (item_start, _item_end) in &items {
        if !matches!(events[*item_start].0, Event::MappingStart(..)) {
            continue;
        }
        let (step_entries, _) = read_mapping(&events, *item_start);
        let is_match = find_unique_entry(
            &step_entries,
            "name",
            "step_mapping_child_keys_by_step_name",
        )
        .is_some_and(|name_entry| {
            matches!(
                &events[name_entry.value_start].0,
                Event::Scalar(text, ..) if text.as_ref() == step_name
            )
        });
        if is_match {
            matched_step_entries.push(step_entries);
        }
    }

    let step_entries = match matched_step_entries.len() {
        0 => {
            return Err(format!(
                "has no step with `name: {step_name}` at all (checked every \
                 step's own `name:` value, wherever it appears in that \
                 step's mapping, via the parsed event-stream model — tree \
                 membership, not a line-position or first-`{key}`-carrying-\
                 step scan)."
            ));
        }
        1 => &matched_step_entries[0],
        n => {
            return Err(format!(
                "has {n} steps named `name: {step_name}` — this checker \
                 requires exactly one so a single pinned literal \
                 unambiguously covers it. A decoy step sharing the real \
                 step's `name:` VALUE (and, potentially, its own `{key}:` \
                 children) would otherwise silently win or lose a \
                 first-match lookup instead of being flagged as ambiguous."
            ));
        }
    };

    let Some(target) = find_unique_entry(step_entries, key, "step_mapping_child_keys_by_step_name")
    else {
        return Ok(Vec::new());
    };
    if !matches!(events[target.value_start].0, Event::MappingStart(..)) {
        return Ok(Vec::new());
    }
    let (child_entries, _) = read_mapping(&events, target.value_start);
    let mut keys: Vec<String> = child_entries.iter().map(|e| e.key.clone()).collect();
    keys.sort();
    Ok(keys)
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
    assert_single_document(&events, "step_mapping_child_value");

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entry = root_entries.first()?;
    if !matches!(events[entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (body_entries, _) = read_mapping(&events, entry.value_start);
    let steps_entry = find_unique_entry(&body_entries, "steps", "step_mapping_child_value")?;
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
        let mapping_entry =
            find_unique_entry(&step_entries, mapping_key, "step_mapping_child_value")?;
        if !matches!(events[mapping_entry.value_start].0, Event::MappingStart(..)) {
            return None;
        }
        let (child_entries, _) = read_mapping(&events, mapping_entry.value_start);
        let child = find_unique_entry(&child_entries, child_key, "step_mapping_child_value")?;
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
// item) can reach.
//
// **DELETED (S-CIGATE-3, ADV-SC3-P5-MED-001):** this pass originally also
// added a step accessor, `first_step_mapping_child_value`, that did NOT
// require a `step_anchor_key` disambiguator — added because the `msrv` job
// has FOUR steps sharing a `uses:` key, only one of which has the
// `with.toolchain` child pass B's anchored `step_mapping_child_value` would
// need to disambiguate correctly. "No disambiguator needed" turned out to
// be exactly the ambiguity gap fix-burst-7 closed: a decoy step sharing the
// real step's anchor key silently won this function's own unchecked
// first-match, the same shape `find_unique_entry`'s coverage-correction
// paragraph (above) documents for the mapping-child case. Replaced by
// [`step_mapping_child_value_for_step`] plus `ci_gate_completeness.rs`'s
// `find_sole_step_by` — the caller must now resolve the target step
// unambiguously (`Err` on zero or more than one match) BEFORE this module
// ever looks at its children, rather than this module silently picking a
// winner on the caller's behalf.
// ---------------------------------------------------------------------------

/// Shared first step for every job-level-path accessor below: parse
/// `job_block` into its raw event stream, panicking (same contract as
/// [`WfDoc::parse`]) if it is not well-formed YAML 1.2.
fn parse_job_block_events<'a>(job_block: &'a str, caller: &str) -> Vec<(Event<'a>, Span)> {
    let events = Parser::new_from_str(job_block)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!("wf.rs: {caller}: failed to parse job block YAML as valid YAML 1.2: {e}")
        });
    assert_single_document(&events, caller);
    events
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
        let target = find_unique_entry(&entries, segment, "descend_as_mappings")?;
        if !matches!(events[target.value_start].0, Event::MappingStart(..)) {
            return None;
        }
        let (next_entries, _) = read_mapping(events, target.value_start);
        entries = next_entries;
    }
    // The FINAL resolved level's own key set is what several callers
    // (`root_level_nested_keys`, `job_level_nested_keys`, and the trailing
    // `.find(last[0])` in `job_level_nested_value`/
    // `job_level_nested_sequence_items`) hand back as a caller's "complete
    // key set" or use for one more lookup — a duplicate key WITHIN this
    // final level (not merely along the navigation path checked above)
    // must be caught here too, or it would silently survive into that
    // "complete" set. See `assert_no_duplicate_keys`'s doc comment.
    assert_no_duplicate_keys(&entries, "descend_as_mappings");
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
    let target = find_unique_entry(&entries, last[0], "job_level_nested_value")?;
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
    let target = find_unique_entry(&entries, last[0], "job_level_nested_sequence_items")?;
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
/// value of `mapping_key`, within the EXACT step `step` — identified by its
/// own byte [`Step::span`], not by re-scanning `job_block` for "the first
/// step carrying some other key". Sibling of [`step_mapping_child_value`]
/// (anchored by a same-step OTHER key's mere PRESENCE, first-match) — this
/// is the stricter of the two: the caller must have ALREADY unambiguously
/// identified `step` through an ambiguity-checked selection (e.g. an
/// `Err`-on-zero-or-more-than-one helper such as `ci_gate_completeness.rs`'s
/// `find_sole_step_by`/`find_sole_step_by_name` — NOT a raw, unchecked
/// `job.steps.iter().find(...)`, which silently prefers whichever step
/// happens to come first on a decoy match; see `find_sole_step_by`'s own
/// doc comment, S-CIGATE-3 fix-burst-7/ADV-SC3-P5-MED-001, for the concrete
/// bypass an unchecked outer `.find()` left open even with THIS function
/// correctly anchoring on the resolved step's byte span), and this function
/// guarantees the lookup happens on THAT step, not merely a step sharing
/// one of its keys.
///
/// # Why this exists (S-CIGATE-3 fix-burst-6, ADV-SC3-P4-MED-001 / F-02)
///
/// `step_mapping_child_value`'s `step_anchor_key` design silently assumes
/// "exactly one step in this job has `step_anchor_key` at all" — a
/// precondition nothing enforced. For the `msrv` job's `cargo check` step,
/// anchoring on `step_anchor_key = "run"` matches the FIRST step with ANY
/// `run:` key, not specifically the step whose `run:` VALUE is `cargo check
/// --all-features --locked`. A decoy step inserted earlier in `steps:` with
/// its own `run:` + `env: {RUSTUP_TOOLCHAIN: "1.85.0"}` — while the REAL
/// `cargo check` step's `env:` block is deleted entirely — satisfies the old
/// lookup and leaves `test_verify_msrv_job_pins_toolchain_and_rustup_
/// toolchain_env` green while `cargo check` silently runs under
/// `rust-toolchain.toml`'s `channel = "stable"`, verbatim the S-626-1
/// false-green this test exists to prevent. This function closes the gap by
/// re-locating, inside `steps:`, the step whose OWN byte span equals
/// `step.span` — spans are unique source-text positions, so this can only
/// ever match the step the caller already resolved.
///
/// Returns `None` if no step's span matches `step.span` (a caller error —
/// `step` did not come from parsing this same `job_block` text), that step
/// has no `mapping_key`, `mapping_key`'s value there is not itself a
/// mapping, or that mapping has no `child_key`.
///
/// # Panics
///
/// Same malformed-YAML-panics contract as [`WfDoc::parse`]. Additionally
/// panics if MORE THAN ONE step in `job_block` shares `step.span` — byte
/// spans are unique positions in one source text, so this should be
/// structurally impossible unless `job_block` is not the same text `step`
/// was parsed from (a caller error worth surfacing loudly rather than
/// silently picking one).
#[must_use]
pub fn step_mapping_child_value_for_step(
    job_block: &str,
    step: &Step,
    mapping_key: &str,
    child_key: &str,
) -> Option<Value> {
    let table = char_byte_table(job_block);
    let events = parse_job_block_events(job_block, "step_mapping_child_value_for_step");
    let body = job_body_entries(&events)?;
    let steps_entry = find_unique_entry(&body, "steps", "step_mapping_child_value_for_step")?;
    if !matches!(events[steps_entry.value_start].0, Event::SequenceStart(..)) {
        return None;
    }
    let (items, _) = read_sequence(&events, steps_entry.value_start);

    let byte_of = |char_idx: usize| -> usize {
        *table.get(char_idx).unwrap_or_else(|| {
            panic!(
                "wf.rs: step_mapping_child_value_for_step: char index \
                 {char_idx} out of range for a table of {} entries — this \
                 indicates a bug in this module's event-index bookkeeping, \
                 not malformed input",
                table.len().saturating_sub(1)
            )
        })
    };

    let mut matched: Vec<usize> = Vec::new();
    for (item_start, item_end) in items {
        let start_byte = byte_of(events[item_start].1.start.index());
        let end_byte = byte_of(events[item_end - 1].1.end.index());
        if (start_byte..end_byte) == step.span {
            matched.push(item_start);
        }
    }

    let item_start = match matched.len() {
        0 => return None,
        1 => matched[0],
        n => panic!(
            "wf.rs: step_mapping_child_value_for_step: {n} steps in this \
             job block share byte span {:?} — `step` most likely did not \
             come from parsing this same `job_block` text.",
            step.span
        ),
    };

    if !matches!(events[item_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (step_entries, _) = read_mapping(&events, item_start);
    let mapping_entry = find_unique_entry(
        &step_entries,
        mapping_key,
        "step_mapping_child_value_for_step",
    )?;
    if !matches!(events[mapping_entry.value_start].0, Event::MappingStart(..)) {
        return None;
    }
    let (child_entries, _) = read_mapping(&events, mapping_entry.value_start);
    let child = find_unique_entry(
        &child_entries,
        child_key,
        "step_mapping_child_value_for_step",
    )?;
    Some(resolve_value(&events, child.value_start))
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 pass F additions (FINAL migration pass)
//
// Everything below was added for the WORKFLOW-ROOT-scoped guard cluster in
// `tests/ci_gate_completeness.rs` (`test_ci_yml_has_no_workflow_level_shell_
// override`, `test_ci_yml_workflow_level_env_key_set_is_pinned`) — the only
// two guards in that file that read `.github/workflows/ci.yml` at the
// DOCUMENT ROOT rather than inside any job block, because the constructs
// they guard (a top-level `defaults:` override, the workflow's own
// top-level `env:` block) are siblings of `jobs:` itself and therefore
// invisible to every job-scoped accessor above by construction — see
// [`WfDoc::root_keys`]'s own doc comment for the full rationale.
//
// This is the FINAL S-CIGATE-3 pass. After this pass, `tests/ci_gate_
// completeness.rs`'s `extract_key_name_at_indent` and
// `collect_mapping_key_set` — the two line-based primitives at the root of
// the entire round-13/14/16 "lexer disagrees with a real parser" defect
// class this story exists to close — have no remaining callers and are
// deleted from that file.
// ---------------------------------------------------------------------------

/// Resolve the complete key set (source order, not deduplicated) of the
/// mapping found by walking a DOCUMENT-ROOT-level nested mapping PATH —
/// every segment, including the last, is resolved as a mapping.
///
/// Sibling of [`job_level_nested_keys`] (which walks a path from a JOB's
/// own level, one level lower): this one walks from the document ROOT,
/// exactly the shape [`WfDoc::root_keys`] itself is built from but for a
/// NESTED path rather than the root's own direct keys — needed because
/// `root_keys` alone can confirm the workflow declares an `env:` key, but
/// says nothing about that key's own CHILDREN (the actual env-var names a
/// `BASH_ENV` smuggling attempt would add — see
/// `PINNED_WORKFLOW_ENV_KEYS`'s doc comment in `ci_gate_completeness.rs`).
///
/// Motivating caller:
/// `tests/ci_gate_completeness.rs::extract_workflow_env_key_set` — the
/// workflow's own top-level `env:` block's key set (a single-segment path,
/// `&["env"]`).
///
/// Returns `None` if any segment (including the last) is missing, or its
/// value is not itself a mapping — mirrors [`job_level_nested_keys`]'s own
/// contract exactly, one level higher (document root instead of a job's
/// body).
///
/// # Panics
///
/// Panics if `path` is empty (caller error — there is no key to resolve),
/// or if `yaml` is not well-formed YAML 1.2 (same contract as
/// [`WfDoc::parse`]).
#[must_use]
pub fn root_level_nested_keys(yaml: &str, path: &[&str]) -> Option<Vec<String>> {
    assert!(
        !path.is_empty(),
        "wf.rs: root_level_nested_keys: path must not be empty"
    );
    let events: Vec<(Event<'_>, Span)> = Parser::new_from_str(yaml)
        .collect::<Result<Vec<_>, ScanError>>()
        .unwrap_or_else(|e| {
            panic!(
                "wf.rs: root_level_nested_keys: failed to parse workflow YAML \
                 as valid YAML 1.2: {e}"
            )
        });
    assert_single_document(&events, "root_level_nested_keys");

    let root_start = events
        .iter()
        .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))?;
    let (root_entries, _) = read_mapping(&events, root_start);
    let entries = descend_as_mappings(&events, root_entries, path)?;
    Some(entries.iter().map(|e| e.key.clone()).collect())
}

// ---------------------------------------------------------------------------
// S-CIGATE-3 fix-burst-3 additions
//
// Unit tests for this module, added in response to a fresh-context
// adversarial review (ADV-SC3-P1-MED-001) that found this file — the single
// choke point every structural guard in `tests/ci_gate_completeness.rs`
// parses through — had zero direct test coverage of its own. These are
// PERMANENT regression tests, not throwaway RED-proof scratch work (that
// scratch harness, used to confirm the ADV-SC3-P1-MED-004 and
// ADV-SC3-P1-LOW-001 findings were real bugs before fixing them, was
// deleted after use — see this pass's commit message for the RED/GREEN
// transcript).
//
// Where these tests run: `tests/common/` is compiled as a submodule into
// every one of the ~59 integration test binaries under `tests/` that
// declare `mod common;` (each such binary is its OWN separate crate, built
// with the test harness, so `cfg(test)` is active for the WHOLE crate, not
// just the root file). A `#[cfg(test)] mod tests` living inside
// `tests/common/wf.rs` is therefore compiled into, and its `#[test]`
// functions run as part of, EVERY one of those ~59 binaries — confirmed
// empirically (see this pass's completion report for the exact command and
// observed pass count across a sample of binaries, plus the full-suite
// aggregate). This is unusual compared to a typical Rust crate (where a
// shared test-only module usually lives behind a single binary), but not a
// bug: `tests/common/wf.rs` has private (`char_byte_table`,
// `line_start_char_idx`, `read_mapping`, `read_sequence`, `skip_node`,
// `find_unique_entry`, `assert_no_duplicate_keys`,
// `assert_single_document`) items this suite must exercise directly, and
// Rust's privacy rules mean only a `#[cfg(test)] mod tests` NESTED inside
// this same file (as a child module, which can see its ancestor's private
// items) can reach them — a separate `tests/wf_model.rs` integration test
// could only reach this module's `pub` surface, not these private
// primitives, so that fallback (mentioned in this pass's task brief) was
// not needed here.
#[cfg(test)]
mod tests {
    use super::*;
    use saphyr_parser::Marker;

    fn events_for(yaml: &str) -> Vec<(Event<'_>, Span)> {
        Parser::new_from_str(yaml)
            .collect::<Result<Vec<_>, ScanError>>()
            .expect("test fixture must be well-formed YAML 1.2")
    }

    // -----------------------------------------------------------------
    // char_byte_table / line_start_char_idx — multi-byte characters
    // -----------------------------------------------------------------

    #[test]
    fn test_char_byte_table_ascii_only_is_identity() {
        let table = char_byte_table("abc");
        assert_eq!(table, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_char_byte_table_multibyte_characters_diverge_from_char_index() {
        // 'é' = 2 bytes, '€' = 3 bytes, 'x' = 1 byte.
        let s = "é€x";
        let table = char_byte_table(s);
        assert_eq!(table, vec![0, 2, 5, 6]);
        // From char 1 onward, char index and byte index have diverged —
        // exactly the trap `Marker::index()` (a CHAR index) sets for any
        // caller that forgets to route it through this table before
        // slicing a `&str` (which is BYTE-indexed).
        assert_ne!(table[1], 1);
        assert_ne!(table[2], 2);
        assert_eq!(s.len(), 6);
    }

    #[test]
    fn test_line_start_char_idx_recovers_via_col_not_rescan() {
        // A marker at char index 10, column 4 (0-indexed — 4 chars into
        // its physical line) implies the line itself starts at char
        // index 6. `line_start_char_idx` must compute this via
        // `index() - col()`, not by re-scanning for a preceding `\n`.
        let span = Span::new(Marker::new(10, 3, 4), Marker::new(13, 3, 7));
        assert_eq!(line_start_char_idx(&span), 6);
    }

    /// Positions a multi-byte run of characters (café/☕/§/≥/→, 2-3 bytes
    /// each) BEFORE a job boundary AND inside a job block's step name, so
    /// this test only passes if `char_byte_table`/`line_start_char_idx`
    /// correctly resolve BYTE offsets (not char offsets) at both
    /// positions — a wrong offset would either panic (non-char-boundary
    /// `&str` slice) or silently return the wrong text.
    #[test]
    fn test_wfdoc_parse_job_spans_correct_across_multibyte_content() {
        let yaml = concat!(
            "name: CI\n",
            "jobs:\n",
            "  alpha:\n",
            "    name: \"café ☕ § ≥ → multi-byte\"\n",
            "    steps:\n",
            "      - name: emit ★ symbol\n",
            "        run: echo ok\n",
            "  beta:\n",
            "    name: second job\n",
        );
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 2);

        let alpha = &doc.jobs[0];
        let beta = &doc.jobs[1];

        // "at a span boundary": beta's span START is the byte offset
        // immediately following ALL of alpha's accumulated multi-byte
        // content — this is where char/byte divergence has compounded
        // the most.
        assert_eq!(&yaml[beta.span.clone()], "  beta:\n    name: second job\n");
        assert_eq!(
            &yaml[alpha.span.clone()],
            concat!(
                "  alpha:\n",
                "    name: \"café ☕ § ≥ → multi-byte\"\n",
                "    steps:\n",
                "      - name: emit ★ symbol\n",
                "        run: echo ok\n",
            )
        );

        // "inside a job block": the step name itself carries multi-byte
        // content and must round-trip exactly. `Step::span` is NOT
        // line-snapped the way `Job::span` is (see `Step::span`'s own doc
        // comment) — it runs from the step mapping's first event's span
        // start (which, for a block-sequence item that is itself a
        // mapping, lands right after the `- ` marker, at the first key) to
        // the last event's span end (which, per this parser's own Span
        // convention, extends through trailing whitespace up to the start
        // of the next sibling's content). The byte-correctness property
        // under test is that this multi-byte content round-trips exactly,
        // char-boundary-safe, at all — not any particular trimming.
        assert_eq!(alpha.steps.len(), 1);
        assert_eq!(alpha.steps[0].name.as_deref(), Some("emit ★ symbol"));
        assert_eq!(
            &yaml[alpha.steps[0].span.clone()],
            "name: emit ★ symbol\n        run: echo ok\n  "
        );
    }

    // -----------------------------------------------------------------
    // Last-job span bounding (adversarial pass 5, MEDIUM, S-CIGATE-3):
    // `jobs:` need not be the FINAL top-level key of a workflow file —
    // `concurrency:`, `defaults:`, `env:`, `permissions:`, `on:` are all
    // legal AFTER `jobs:`. Before the fix, the LAST job under `jobs:` was
    // always assigned a span running to `yaml.len()`, silently swallowing
    // any trailing root key into that job's own block — which then broke
    // `extract_job_block`/`parse_single_job` on that job with a confusing
    // "malformed job block" diagnostic pointing nowhere near the real
    // cause.
    // -----------------------------------------------------------------

    #[test]
    fn test_wfdoc_parse_last_job_span_excludes_trailing_root_key() {
        let yaml = concat!(
            "name: CI\n",
            "jobs:\n",
            "  alpha:\n",
            "    runs-on: ubuntu-latest\n",
            "  beta:\n",
            "    runs-on: ubuntu-latest\n",
            "    steps:\n",
            "      - run: echo hi\n",
            "concurrency:\n",
            "  group: ci-${{ github.ref }}\n",
            "  cancel-in-progress: true\n",
        );
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 2);
        let beta = &doc.jobs[1];
        assert_eq!(beta.id, "beta");

        let block = &yaml[beta.span.clone()];
        assert!(
            !block.contains("concurrency"),
            "beta's block over-captured the trailing `concurrency:` root \
             key, which is not part of the `jobs:` mapping at all: {block:?}"
        );

        // The over-capture doesn't just leave stray text in the slice — a
        // trailing root key one level further out is a column-0 dedent
        // below beta's own root mapping, so re-parsing the (buggy)
        // over-captured block as a standalone job block fails outright.
        // A correctly-bounded block must parse cleanly and expose exactly
        // beta's own keys.
        let single = WfDoc::parse_single_job(block);
        assert_eq!(single.id, "beta");
        assert_eq!(
            single.keys,
            vec!["runs-on".to_string(), "steps".to_string()]
        );
    }

    #[test]
    fn test_wfdoc_parse_last_job_span_includes_full_trailing_block_scalar_when_jobs_is_last_root_key()
     {
        // Guard against a regression in the OTHER direction (this is why
        // `yaml.len()` was chosen originally — see `Job::span`'s own doc
        // comment): when `jobs:` genuinely IS the workflow's last root
        // key, the last job's span must still capture its own trailing
        // content in FULL, including a multi-line block scalar with no
        // trailing newline at end-of-file — the real shape
        // `sync-upstream.yml`'s last job has today. Must pass both before
        // and after the last-job-span fix.
        let yaml = concat!(
            "name: CI\n",
            "jobs:\n",
            "  alpha:\n",
            "    runs-on: ubuntu-latest\n",
            "  beta:\n",
            "    runs-on: ubuntu-latest\n",
            "    steps:\n",
            "      - name: Sync tags\n",
            "        run: |\n",
            "          git push origin --tags\n",
            "          echo done",
        );
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 2);
        let beta = &doc.jobs[1];
        assert_eq!(beta.id, "beta");

        let block = &yaml[beta.span.clone()];
        assert!(
            block.contains("git push origin --tags") && block.contains("echo done"),
            "beta's block under-captured its own trailing block scalar: {block:?}"
        );

        let single = WfDoc::parse_single_job(block);
        assert_eq!(single.id, "beta");
        assert_eq!(single.steps.len(), 1);
        let run_value = single.steps[0]
            .value_of("run")
            .expect("Sync tags step has a `run:` key");
        match run_value {
            Value::Scalar { text, .. } => {
                assert_eq!(text, "git push origin --tags\necho done\n");
            }
            other => panic!("expected a Scalar value for `run:`, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------
    // read_mapping / read_sequence / skip_node on nested structures
    // -----------------------------------------------------------------

    #[test]
    fn test_read_mapping_and_read_sequence_on_nested_structures() {
        // a:
        //   b:
        //     - 1
        //     - c: 2
        //       d: [3, 4]
        //   e: 5
        let yaml = "a:\n  b:\n    - 1\n    - c: 2\n      d: [3, 4]\n  e: 5\n";
        let events = events_for(yaml);
        let root_start = events
            .iter()
            .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
            .unwrap();

        let (root_entries, _root_end) = read_mapping(&events, root_start);
        assert_eq!(root_entries.len(), 1);
        assert_eq!(root_entries[0].key, "a");

        assert!(matches!(
            events[root_entries[0].value_start].0,
            Event::MappingStart(..)
        ));
        let (a_entries, _a_end) = read_mapping(&events, root_entries[0].value_start);
        assert_eq!(a_entries.len(), 2);
        assert_eq!(a_entries[0].key, "b");
        assert_eq!(a_entries[1].key, "e");

        // "b"'s value is a block sequence with 2 items: a bare scalar,
        // then a nested mapping.
        assert!(matches!(
            events[a_entries[0].value_start].0,
            Event::SequenceStart(..)
        ));
        let (b_items, b_seq_end) = read_sequence(&events, a_entries[0].value_start);
        assert_eq!(b_items.len(), 2);

        let (item0_start, item0_end) = b_items[0];
        assert!(matches!(events[item0_start].0, Event::Scalar(..)));
        // skip_node's Scalar branch must advance by exactly one event.
        assert_eq!(item0_end, item0_start + 1);

        let (item1_start, item1_end) = b_items[1];
        assert!(matches!(events[item1_start].0, Event::MappingStart(..)));
        let (c_entries, c_end) = read_mapping(&events, item1_start);
        assert_eq!(c_entries.len(), 2);
        assert_eq!(c_entries[0].key, "c");
        assert_eq!(c_entries[1].key, "d");
        // read_mapping's own reported end must agree with skip_node's
        // (used internally by read_sequence to advance past this item).
        assert_eq!(c_end, item1_end);

        // "d"'s value is a nested FLOW sequence — skip_node must recurse
        // through it correctly (not stop early) for item1_end to be
        // correct.
        assert!(matches!(
            events[c_entries[1].value_start].0,
            Event::SequenceStart(..)
        ));
        let (d_items, d_end) = read_sequence(&events, c_entries[1].value_start);
        assert_eq!(d_items.len(), 2);
        // `d_end` is one event BEFORE `item1_end`: "d" is the LAST key of
        // item1's own mapping, so after `skip_node` advances past "d"'s
        // value (this nested flow sequence), `read_mapping`'s loop still
        // has to consume item1's own `MappingEnd` event before returning
        // `item1_end`.
        assert_eq!(d_end, item1_end - 1);

        // `b_seq_end` is one event PAST `item1_end`: item1 is the LAST
        // item of "b"'s own sequence, so after `skip_node` advances past
        // item1's own closing event, `read_sequence`'s loop still has to
        // consume "b"'s own `SequenceEnd` event before returning.
        assert_eq!(b_seq_end, item1_end + 1);
    }

    #[test]
    fn test_skip_node_advances_past_alias() {
        let yaml = "anchor: &x foo\nuses_it: *x\n";
        let events = events_for(yaml);
        let root_start = events
            .iter()
            .position(|(ev, _)| matches!(ev, Event::MappingStart(..)))
            .unwrap();
        let (entries, _end) = read_mapping(&events, root_start);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].key, "uses_it");
        assert!(matches!(events[entries[1].value_start].0, Event::Alias(_)));
        // skip_node's Alias branch must advance by exactly one event, the
        // same as its Scalar branch.
        assert_eq!(
            skip_node(&events, entries[1].value_start),
            entries[1].value_start + 1
        );
    }

    // -----------------------------------------------------------------
    // WfDoc::parse vs WfDoc::parse_single_job scoping equivalence
    // -----------------------------------------------------------------

    #[test]
    fn test_parse_single_job_matches_whole_document_parse_for_same_job() {
        let yaml = concat!(
            "jobs:\n",
            "  build:\n",
            "    runs-on: ubuntu-latest\n",
            "    needs: [a, b]\n",
            "    steps:\n",
            "      - name: one\n",
            "        run: echo 1\n",
            "      - name: two\n",
            "        run: echo 2\n",
        );
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 1);
        let job_from_doc = &doc.jobs[0];
        let job_block = &yaml[job_from_doc.span.clone()];

        let job_single = WfDoc::parse_single_job(job_block);

        assert_eq!(job_from_doc.id, job_single.id);
        assert_eq!(job_from_doc.keys, job_single.keys);
        // `Value::Scalar`'s `start_line`/`end_line` are ABSOLUTE line
        // numbers within whatever string was actually parsed —
        // deliberately so (see the field's own doc comment). Parsing the
        // WHOLE document vs. just this job's sliced-out `job_block` gives
        // the SAME scalar a DIFFERENT absolute line number by construction
        // (the job starts on line 1 of `job_block` but not of `yaml`), so
        // line numbers are excluded from this equivalence check — only
        // `text`/`style`/`tag` (the parts that are actually supposed to
        // be scope-independent) are compared.
        assert_eq!(
            job_from_doc
                .values
                .iter()
                .map(value_text_style_tag)
                .collect::<Vec<_>>(),
            job_single
                .values
                .iter()
                .map(value_text_style_tag)
                .collect::<Vec<_>>(),
        );
        assert_eq!(job_from_doc.steps.len(), job_single.steps.len());
        for (from_doc, single) in job_from_doc.steps.iter().zip(job_single.steps.iter()) {
            assert_eq!(from_doc.name, single.name);
            assert_eq!(from_doc.keys, single.keys);
            assert_eq!(
                from_doc
                    .values
                    .iter()
                    .map(value_text_style_tag)
                    .collect::<Vec<_>>(),
                single
                    .values
                    .iter()
                    .map(value_text_style_tag)
                    .collect::<Vec<_>>(),
            );
        }
    }

    /// Reduce a [`Value`] to the parts that are scope-independent (i.e. the
    /// same regardless of whether it was parsed as part of the whole
    /// document or a sliced-out single job block) — everything except
    /// `Value::Scalar`'s `start_line`/`end_line`. See
    /// `test_parse_single_job_matches_whole_document_parse_for_same_job`'s
    /// own comment for why line numbers are deliberately excluded here.
    fn value_text_style_tag(v: &Value) -> (Option<&str>, Option<ScalarStyle>, Option<&str>) {
        match v {
            Value::Scalar {
                text, style, tag, ..
            } => (Some(text.as_str()), Some(*style), tag.as_deref()),
            Value::Alias => (None, None, Some("<alias>")),
            Value::Other => (None, None, Some("<other>")),
        }
    }

    #[test]
    #[should_panic(expected = "expected exactly one root entry")]
    fn test_parse_single_job_panics_on_multi_entry_root() {
        let yaml = "a:\n  runs-on: ubuntu-latest\nb:\n  runs-on: ubuntu-latest\n";
        let _ = WfDoc::parse_single_job(yaml);
    }

    // -----------------------------------------------------------------
    // Duplicate-key behavior (ADV-SC3-P1-MED-004)
    // -----------------------------------------------------------------

    #[test]
    #[should_panic(expected = "refuses to silently pick a winner")]
    fn test_wfdoc_parse_panics_on_duplicate_root_jobs_key() {
        // The exact malicious fixture from ADV-SC3-P1-MED-004: a second
        // root-level `jobs:` map smuggling in an extra job. Before the
        // fix, `WfDoc::parse` silently saw only the FIRST `jobs:` map.
        let yaml =
            "jobs:\n  x:\n    runs-on: ubuntu-latest\njobs:\n  evil:\n    runs-on: ubuntu-latest\n";
        let _ = WfDoc::parse(yaml);
    }

    #[test]
    #[should_panic(expected = "refuses to silently pick a winner")]
    fn test_root_level_nested_keys_panics_on_duplicate_root_env_block() {
        // The exact malicious fixture from ADV-SC3-P1-MED-004: a second
        // root-level `env:` block smuggling in `BASH_ENV`. Before the
        // fix, `root_level_nested_keys` silently returned only the FIRST
        // `env:` block's keys.
        let yaml = "env:\n  CARGO_TERM_COLOR: always\njobs:\n  x:\n    runs-on: ubuntu-latest\nenv:\n  BASH_ENV: /tmp/shim.sh\n";
        let _ = root_level_nested_keys(yaml, &["env"]);
    }

    #[test]
    #[should_panic(expected = "refuses to silently pick a winner")]
    fn test_descend_as_mappings_panics_on_duplicate_key_within_final_level() {
        // A duplicate key WITHIN the final resolved level itself (not
        // along the navigation path) — two `CARGO_TERM_COLOR:` children
        // of one `env:` block.
        let yaml = "env:\n  CARGO_TERM_COLOR: always\n  CARGO_TERM_COLOR: never\n";
        let _ = root_level_nested_keys(yaml, &["env"]);
    }

    #[test]
    fn test_root_level_nested_keys_succeeds_with_no_duplicates() {
        let yaml = "env:\n  CARGO_TERM_COLOR: always\njobs:\n  x:\n    runs-on: ubuntu-latest\n";
        let keys = root_level_nested_keys(yaml, &["env"]).unwrap();
        assert_eq!(keys, vec!["CARGO_TERM_COLOR".to_string()]);
    }

    // -----------------------------------------------------------------
    // Single-document behavior (ADV-SC3-P1-LOW-001)
    // -----------------------------------------------------------------

    #[test]
    #[should_panic(expected = "expected at most one YAML document")]
    fn test_wfdoc_parse_panics_on_second_yaml_document() {
        // The exact malicious fixture from ADV-SC3-P1-LOW-001: a second
        // `---`-separated document smuggling in a workflow-level
        // `defaults: run: shell:` override. Before the fix, `WfDoc::parse`
        // silently discarded this second document entirely.
        let yaml =
            "jobs:\n  x:\n    runs-on: ubuntu-latest\n---\ndefaults:\n  run:\n    shell: cat {0}\n";
        let _ = WfDoc::parse(yaml);
    }

    #[test]
    fn test_wfdoc_parse_accepts_single_explicit_document_marker() {
        // A single leading `---` (explicit document start) is ONE
        // document, not two — must not be mistaken for a multi-document
        // stream.
        let yaml = "---\njobs:\n  x:\n    runs-on: ubuntu-latest\n";
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 1);
        assert_eq!(doc.jobs[0].id, "x");
    }

    #[test]
    fn test_wfdoc_parse_accepts_single_implicit_document() {
        let yaml = "jobs:\n  x:\n    runs-on: ubuntu-latest\n";
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.jobs.len(), 1);
    }

    // -----------------------------------------------------------------
    // Zero-document (comment-only / whitespace-only) streams are NOT a
    // multi-document violation (ADV-SC3-P2-LOW-001). Before the fix,
    // `assert_single_document`'s `doc_start_count == 1` equality panicked
    // on a comment-only stream (doc_start_count == 0) with a message
    // blaming a `---`-separated multi-document stream — the opposite of
    // what actually happened, and a direct contradiction of
    // `WfDoc::parse`'s own documented "not malformed, just empty" contract
    // for a missing top-level mapping.
    // -----------------------------------------------------------------

    #[test]
    fn test_wfdoc_parse_accepts_comment_only_stream_as_empty_result() {
        let yaml = "# just a comment, no content at all\n";
        let doc = WfDoc::parse(yaml);
        assert_eq!(doc.root_keys, Vec::<String>::new());
        assert_eq!(doc.jobs.len(), 0);
    }

    #[test]
    fn test_wfdoc_parse_accepts_whitespace_only_stream_as_empty_result() {
        let doc = WfDoc::parse("   \n\n  \n");
        assert_eq!(doc.root_keys, Vec::<String>::new());
        assert_eq!(doc.jobs.len(), 0);
    }

    #[test]
    fn test_wfdoc_parse_accepts_empty_string_as_empty_result() {
        let doc = WfDoc::parse("");
        assert_eq!(doc.root_keys, Vec::<String>::new());
        assert_eq!(doc.jobs.len(), 0);
    }

    // -----------------------------------------------------------------
    // find_key_node_properties single-document guard (shares the
    // assert_single_document plumbing with WfDoc::parse; regression-pin
    // that the guard fires on this entry point too, not just WfDoc::parse)
    // -----------------------------------------------------------------

    #[test]
    #[should_panic(expected = "expected at most one YAML document")]
    fn test_find_key_node_properties_panics_on_multi_document_stream() {
        let yaml = "jobs:\n  x:\n    runs-on: ubuntu-latest\n---\nother: 1\n";
        let _ = find_key_node_properties(yaml);
    }

    // -----------------------------------------------------------------
    // find_key_node_properties DETECTION path (M-1, adversarial finding,
    // S-CIGATE-3 close-out). Before this pass, this function's ONLY
    // consumer was `tests/ci_gate_completeness.rs`'s M2-q assertion
    // (`assert!(find_key_node_properties(gate_block).is_empty(), …)`)
    // against the real `ci-gate` job block, which has no node properties
    // — that assertion passes VACUOUSLY and says nothing about whether
    // the function can actually detect one. The only test above touching
    // this function exercises its multi-document PANIC path, never its
    // detection path. A mutation that made `key_has_anchor` always
    // `false`, made `key_tag` always `None`, or made
    // `collect_key_node_properties`/`recurse_into_value_for_node_
    // properties` stop recursing would leave every existing test green
    // while silently reopening the round-16 anchor/tag bypass this
    // function exists to close. These tests call the function directly
    // and assert it DETECTS node properties on a key — positive
    // regression coverage, not merely absence-on-clean-input coverage.
    // -----------------------------------------------------------------

    #[test]
    fn test_find_key_node_properties_detects_anchor_on_mapping_key() {
        // `&x` attaches to the KEY node `b`, not the value `c` — exactly
        // the round-16 `&x shell: cat {0}` shape, one level of nesting
        // down from the document root.
        let yaml = "a:\n  &x b: c\n";
        let found = find_key_node_properties(yaml);
        assert_eq!(
            found,
            vec![KeyNodeProperty {
                key: "b".to_string(),
                has_anchor: true,
                tag: None,
            }],
            "expected a single detected node property on key \"b\", got {found:?}"
        );
    }

    #[test]
    fn test_find_key_node_properties_detects_tag_on_mapping_key() {
        // `!!str` attaches to the KEY node `b` — the round-16
        // `!!str shell: cat {0}` shape.
        let yaml = "a:\n  !!str b: c\n";
        let found = find_key_node_properties(yaml);
        assert_eq!(
            found,
            vec![KeyNodeProperty {
                key: "b".to_string(),
                has_anchor: false,
                tag: Some("tag:yaml.org,2002:str".to_string()),
            }],
            "expected a single detected node property on key \"b\", got {found:?}"
        );
    }

    #[test]
    fn test_find_key_node_properties_detects_anchor_nested_two_levels_deep() {
        // Proves `collect_key_node_properties` /
        // `recurse_into_value_for_node_properties` actually DESCEND: the
        // node-propertied key here is two mapping levels below the
        // document root (`a` -> `b` -> `&x c`), not directly under the
        // root the way the two tests above are. A mutation that stopped
        // recursion after the first level (or dropped the sequence-descent
        // arm) would still catch a top-level node property but miss this
        // one — this test is what distinguishes "recurses" from "checks
        // only the mapping it started at".
        let yaml = "a:\n  b:\n    &x c: d\n";
        let found = find_key_node_properties(yaml);
        assert_eq!(
            found,
            vec![KeyNodeProperty {
                key: "c".to_string(),
                has_anchor: true,
                tag: None,
            }],
            "expected the nested node property on key \"c\" to be found by \
             recursing into the value, got {found:?}"
        );
    }

    #[test]
    fn test_find_key_node_properties_clean_mapping_returns_empty() {
        // Control: a mapping with NO node properties anywhere in its tree
        // must return an empty vec. This guards against a mutation that
        // makes the function report EVERY key (or every key past some
        // depth) as node-propertied, which the three detection tests
        // above alone would not catch — they only assert on the presence
        // of the one deliberately-propertied key, not on the absence of
        // spurious entries for the rest of the document.
        let yaml = "a:\n  b: c\n  d:\n    e: f\n    g:\n      - h\n      - i: j\n";
        let found = find_key_node_properties(yaml);
        assert_eq!(
            found,
            Vec::<KeyNodeProperty>::new(),
            "expected no node properties on a clean mapping, got {found:?}"
        );
    }

    // -----------------------------------------------------------------
    // Fixed-denominator self-check (S-CIGATE-3 fix-burst-4,
    // ADV-SC3-P2-LOW-005; enforcement assertions ported fix-burst-6,
    // ADV-SC3-P4-LOW-001)
    //
    // `tests/ci_gate_completeness.rs::EXPECTED_GUARD_TEST_COUNT` (see that
    // constant's own doc comment) exists because POL-11's `test` job
    // canary only requires a NON-ZERO passed count from a binary — it
    // trips on nothing if some number of `#[test]` fns are silently
    // deleted from a file, as long as at least one remains. Fix burst 3
    // added 16 tests directly to THIS file (`tests/common/wf.rs`) — the
    // ONLY regression coverage for `find_unique_entry`,
    // `assert_no_duplicate_keys`, and `assert_single_document` — and
    // explicitly exempted them from that counter, since
    // `include_str!("ci_gate_completeness.rs")` there does not see this
    // file's own source at all. Because `tests/common/wf.rs` is now the
    // single choke point every structural guard in
    // `tests/ci_gate_completeness.rs` parses through, a composite edit
    // that both weakens a `.find`/assert in this file AND deletes the
    // test(s) that would have caught it moved NO counter anywhere prior
    // to this pass — every other file in this story's documented review
    // scope had a tripwire; this one, despite being the most
    // security-relevant of the four, did not.
    //
    // UPDATE THIS CONSTANT in the SAME change whenever a `#[test]` fn is
    // added to or removed from THIS file. A mismatch is a signal to look
    // at what changed, not something to silence by "fixing" the number
    // without checking why it moved.
    //
    // CORRECTION (fix-burst-6, ADV-SC3-P4-LOW-001): this function
    // previously stopped at the `assert_eq!` below and deferred porting
    // its sibling's `#[ignore]`/`#[cfg(...)]` enforcement assertions
    // (`tests/ci_gate_completeness.rs::test_this_file_test_count_
    // matches_expected_denominator`, ADV-P60-HIGH-001) as "out of scope
    // for this pass, which only needed to close the 'no counter moves at
    // all' gap". That rationale was WRONG, not merely incomplete: under
    // `#[ignore]` (or a never-true `#[cfg(...)]`), the `#[test]` line
    // stays present in source either way, so `actual` above never moves
    // regardless — there was never a scenario where silencing a test this
    // way would have moved `EXPECTED_WF_TEST_COUNT` and gone undetected
    // by a counter this file does not have; the gap was always in
    // ENFORCEMENT, exactly as it was for the sibling constant. Checked
    // which of this file's invariants would actually become exploitable
    // if their sole covering test were silenced this way: every guard
    // built on [`find_unique_entry`] is independently caught elsewhere —
    // a weakened duplicate-key tolerance still lengthens the relevant
    // non-deduplicated key vector and fails a `PINNED_*` key-set
    // assertion in `tests/ci_gate_completeness.rs` — EXCEPT
    // [`assert_single_document`], whose four `#[test]`s
    // (`test_wfdoc_parse_panics_on_second_yaml_document`,
    // `test_find_key_node_properties_panics_on_multi_document_stream`,
    // and the two `should_panic` siblings covering `WfDoc::parse`'s and
    // `find_key_node_properties`'s call sites) are its ONLY regression
    // coverage in this codebase; silencing them via `#[ignore]` or a
    // never-true `#[cfg(...)]` has no independent catcher. Both
    // assertions below are ported verbatim (adapted for `source =
    // include_str!("wf.rs")` and this file's own, currently EMPTY,
    // `#[cfg(...)]` allowlist — STRICTER than the sibling's
    // `#[cfg(unix)]` allowance, since this file has no platform-gated
    // test today; see `ALLOWED_TEST_CFG_GATES` below) from
    // `tests/ci_gate_completeness.rs`'s ADV-P60-HIGH-001 fix.
    const EXPECTED_WF_TEST_COUNT: usize = 26;

    /// Collect the line indices (0-based, into `lines`) of every
    /// `#[cfg(...)]` attribute in the CONTIGUOUS attribute/doc block
    /// surrounding a `#[test]` line at `lines[test_idx]` — both the block
    /// immediately BEFORE it and the block immediately AFTER it, up to
    /// (not including) the `fn` line.
    ///
    /// Mirrors `tests/ci_gate_completeness.rs`'s identically-named helper,
    /// added there for the same reason (S-CIGATE-3, ADV-SC3-P6-LOW-002):
    /// this file's own `bad_cfg_gates` scan below shared the sibling
    /// guard's exact position assumption — inspecting only `lines[test_idx
    /// - 1]`, the single line immediately above `#[test]` — before this
    /// fix, even though `ALLOWED_TEST_CFG_GATES` here is currently empty
    /// (no `#[test]` fn in this file is platform-gated today).
    ///
    /// Kept as a separate copy rather than a shared function, matching
    /// this codebase's existing reject-don't-parse duplication convention
    /// (the same relationship `extract_and_normalize_if_expr` has to
    /// `extract_and_normalize_sole_run_line`). `tests/common/wf.rs` and
    /// `tests/ci_gate_completeness.rs` are compiled into separate crates,
    /// and this particular copy is private to its own local test module.
    fn attribute_window_around_test_line(lines: &[&str], test_idx: usize) -> Vec<usize> {
        let mut window = Vec::new();

        let mut i = test_idx;
        while i > 0 {
            let prev = lines[i - 1].trim();
            if prev.starts_with("#[") || prev.starts_with("///") || prev.starts_with("//") {
                window.push(i - 1);
                i -= 1;
            } else {
                break;
            }
        }

        let mut j = test_idx + 1;
        while j < lines.len() {
            let cur = lines[j].trim();
            if cur.starts_with("#[") {
                window.push(j);
                j += 1;
            } else {
                break;
            }
        }

        window
    }

    #[test]
    fn test_wf_rs_test_count_matches_expected_denominator() {
        let source = include_str!("wf.rs");
        let lines: Vec<&str> = source.lines().collect();
        let actual = lines
            .iter()
            .filter(|l| l.trim().starts_with("#[test]"))
            .count();
        assert_eq!(
            actual, EXPECTED_WF_TEST_COUNT,
            "FAIL (S-CIGATE-3 fix-burst-4, ADV-SC3-P2-LOW-005): \
             tests/common/wf.rs contains {actual} `#[test]` functions, but \
             EXPECTED_WF_TEST_COUNT pins {EXPECTED_WF_TEST_COUNT}. This \
             file is the single choke point every structural guard in \
             tests/ci_gate_completeness.rs parses through, and POL-11's \
             zero-test-floor canary in `ci.yml :: test` only requires a \
             NON-ZERO passed count across the whole binary it is compiled \
             into — it does not know how many tests THIS file is supposed \
             to contain, so silently deleting tests from here (alongside a \
             weakened `.find`/assert in the same edit) trips nothing there. \
             If this mismatch is from a deliberate, reviewed addition or \
             removal of a `#[test]` fn in this file, update \
             EXPECTED_WF_TEST_COUNT in the SAME change. If it is not \
             deliberate, some `#[test]` fn was lost — find out which one \
             before changing this constant."
        );

        // ADV-SC3-P4-LOW-001 (ported from ADV-P60-HIGH-001): the
        // assertion above pins the TEXTUAL count of `#[test]` attributes
        // — it says nothing about whether every one of those attributes
        // still actually RUNS. `#[ignore]` leaves the `#[test]` line (and
        // therefore `actual` above) untouched while `cargo test` still
        // reports the binary's overall result as `ok`, satisfied by the
        // remaining tests.
        let ignore_lines: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim().starts_with("#[ignore"))
            .map(|(i, _)| i + 1) // 1-based line numbers for the diagnostic.
            .collect();
        assert!(
            ignore_lines.is_empty(),
            "FAIL (ADV-SC3-P4-LOW-001, ported from ADV-P60-HIGH-001): \
             found `#[ignore]` attribute(s) at line(s) {ignore_lines:?} in \
             tests/common/wf.rs. The `actual == EXPECTED_WF_TEST_COUNT` \
             assertion above counts `#[test]` ATTRIBUTES textually — an \
             `#[ignore]`d test still carries that attribute, so the count \
             does not move, but the test never runs. `cargo test` still \
             reports an overall `ok` result for this binary, and \
             `ci.yml`'s POL-11 zero-test-floor canary only requires a \
             NON-ZERO passed count — satisfied by the remaining tests. \
             Remove the `#[ignore]` attribute, or if a test genuinely must \
             be disabled, that decision needs its own explicit review, not \
             a silent addition that leaves this guard green."
        );

        // ADV-SC3-P4-LOW-001, second gap (ported from ADV-P60-HIGH-001):
        // a `#[cfg(...)]` predicate false on every platform this repo
        // builds for removes the gated `#[test]` fn from the compiled
        // binary entirely — invisible to both the count above and the
        // `#[ignore]` scan above. This file's allowlist is EMPTY: unlike
        // `tests/ci_gate_completeness.rs` (which legitimately gates four
        // bash-shelling-out tests behind `#[cfg(unix)]`), no `#[test]` fn
        // in `tests/common/wf.rs` is platform-gated today — see the grep
        // pin above this test's own doc comment. Reject-don't-parse, same
        // discipline as `extract_and_normalize_if_expr`.
        //
        // CORRECTED (S-CIGATE-3, ADV-SC3-P6-LOW-002; ported from the
        // sibling fix in `tests/ci_gate_completeness.rs`): this scan
        // previously inspected ONLY `lines[i - 1]`, the single line
        // immediately above `#[test]`. Rust's outer attributes are
        // order-independent, so a `#[cfg(...)]` placed AFTER `#[test]`
        // (or separated from it by a doc comment) evaded that
        // single-line-lookback check while leaving this file's own
        // 26/26-green suite unaffected. See
        // `attribute_window_around_test_line`'s own doc comment for the
        // full rationale and both reproductions.
        const ALLOWED_TEST_CFG_GATES: &[&str] = &[];
        let bad_cfg_gates: Vec<(usize, String)> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim().starts_with("#[test]"))
            .flat_map(|(i, _)| {
                attribute_window_around_test_line(&lines, i)
                    .into_iter()
                    .filter_map(|idx| {
                        let candidate = lines[idx].trim();
                        if candidate.starts_with("#[cfg(")
                            && !ALLOWED_TEST_CFG_GATES.contains(&candidate)
                        {
                            Some((idx + 1, candidate.to_string())) // 1-based line number.
                        } else {
                            None
                        }
                    })
            })
            .collect();
        assert!(
            bad_cfg_gates.is_empty(),
            "FAIL (S-CIGATE-3, ADV-SC3-P6-LOW-002; supersedes \
             ADV-SC3-P4-LOW-001/ADV-P60-HIGH-001): found `#[test]` fn(s) \
             in tests/common/wf.rs with a `#[cfg(...)]` attribute \
             somewhere in their surrounding attribute/doc block (before \
             OR after `#[test]`, not merely the single line immediately \
             above it) that is NOT in the (currently EMPTY) allowlist \
             {ALLOWED_TEST_CFG_GATES:?}: \
             {bad_cfg_gates:?} (line number, offending attribute). A \
             `#[cfg(...)]` predicate false on every platform this repo \
             builds for removes the gated `#[test]` fn from the compiled \
             binary ENTIRELY on every one of those platforms — the \
             textual `#[test]` count above does not move, and there is no \
             `#[ignore]` attribute for the scan above to catch either. If \
             this is a deliberate, reviewed new platform-gated test, add \
             its exact `#[cfg(...)]` spelling to ALLOWED_TEST_CFG_GATES in \
             the SAME change — and confirm any helper that test alone \
             uses is gated the same way (PR #671 review round 15's \
             \"gating a test also orphans its helpers\" lesson)."
        );
    }
}
