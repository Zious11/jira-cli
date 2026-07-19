//! Attachment command handlers and display helpers.
//!
//! S-576-1: `jr issue attachment list` — table + JSON output + client-side filters
//! (BC-2.7.001..006).
//!
//! Handlers + helpers only. `AttachmentSubcommand` enum is defined in
//! `src/cli/mod.rs`, NOT here (per P24-001 / P30-001 corrections).
//!
//! `display_sanitize_filename` is the earliest consumer of the CWE-116
//! display-sanitization helper (SEC-576-011, DEC-184 R3.13). Stories S3 and S4
//! import it from here — do NOT duplicate.

use anyhow::Result;
use serde_json::Value;

use crate::api::client::JiraClient;
use crate::api::jira::attachments::AttachmentObject;
use crate::cli::OutputFormat;

/// List attachments on a Jira issue, rendering as table or JSON.
///
/// BC-2.7.001 (table columns), BC-2.7.002 (JSON array shape + contentUrl),
/// BC-2.7.003 (mime= filter), BC-2.7.004 (name= filter),
/// BC-2.7.005 (size-max= filter), BC-2.7.006 (error taxonomy).
pub async fn handle_attachment_list(
    _key: &str,
    _filters: &[String],
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> Result<()> {
    todo!()
}

/// Replace all characters in the display-sanitization set with `?`.
///
/// CWE-116 display-sanitization (BC-2.7.011 v1.3.94, SEC-576-011). Covers:
/// - ASCII control characters: 0x00–0x1F and 0x7F
/// - Unicode bidi controls: U+202A..U+202E, U+2066..U+2069
/// - Line separators: U+2028, U+2029
/// - NEL: U+0085
///
/// Char-based matching; each matching character → one `?`. Always returns a
/// `String` (never `None`). Does NOT strip path separators, truncate to 214
/// bytes, or filter Windows device names — those belong to
/// `sanitize_attachment_filename` (S-576-2, disk-path CWE-22 variant).
pub fn display_sanitize_filename(_name: &str) -> String {
    todo!()
}

/// Serialize an `AttachmentObject` into the curated JSON value.
///
/// Curated shape — BTreeMap-alphabetical key order:
/// `author`, `contentUrl`, `created`, `filename`, `id`, `mimeType`, `size`.
///
/// Invariants (VP-576-004):
/// - `"self"` key is OMITTED from every element.
/// - `"content"` is RENAMED to `"contentUrl"` in every element.
/// - `size` is a raw `u64` integer (bytes), not a human-formatted string.
/// - `author` is `null` when the attachment's author is absent/null.
///
/// `pub` required — `tests/attachment_upload.rs::test_vp_576_004_*` calls this
/// directly for cross-path shape verification (P74-001).
pub fn serialize_attachment_curated(_attachment: &AttachmentObject) -> Value {
    todo!()
}
