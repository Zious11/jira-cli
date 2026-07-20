//! Attachment command handlers and display helpers.
//!
//! S-576-1: `jr issue attachment list` — table + JSON output + client-side filters
//! (BC-2.7.001..006).
//!
//! S-576-2: `jr issue attachment download` — single/batch/newest + streaming +
//! CWE-22 sanitization (BC-2.7.007..012). Adds `handle_attachment_download`,
//! `sanitize_attachment_filename` (disk-path CWE-22 variant), `compute_default_output_path`,
//! and `AttachmentDownloadEntry`.
//!
//! Handlers + helpers only. `AttachmentSubcommand` enum is defined in
//! `src/cli/mod.rs`, NOT here (per P24-001 / P30-001 corrections).
//!
//! `display_sanitize_filename` is the earliest consumer of the CWE-116
//! display-sanitization helper (SEC-576-011, DEC-184 R3.13). Stories S3 and S4
//! import it from here — do NOT duplicate.

use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::api::client::JiraClient;
use crate::api::jira::attachments::AttachmentObject;
use crate::cli::OutputFormat;
use crate::error::JrError;
use crate::output;

// ---------------------------------------------------------------------------
// Filter types
// ---------------------------------------------------------------------------

enum AttachmentFilter {
    Mime(String),
    Name(String),
    SizeMax(u64),
}

/// Parse `--filter key=value` strings into typed filter variants.
///
/// Validation fires BEFORE any HTTP call (BC-2.7.003 EC-2.7.003-2, BC-2.7.005 EC-2.7.005-1).
fn parse_filters(raw: &[String]) -> Result<Vec<AttachmentFilter>> {
    let mut out = Vec::with_capacity(raw.len());
    for f in raw {
        let eq_pos = f.find('=').ok_or_else(|| {
            JrError::UserError(format!(
                "Invalid filter '{f}': expected key=value form. \
                 Accepted keys: mime=, name=, size-max=."
            ))
        })?;
        let key = &f[..eq_pos];
        let value = &f[eq_pos + 1..];
        match key {
            "mime" => out.push(AttachmentFilter::Mime(value.to_string())),
            "name" => out.push(AttachmentFilter::Name(value.to_string())),
            "size-max" => {
                let limit = value.parse::<u64>().map_err(|_| {
                    JrError::UserError(format!(
                        "Invalid size-max value '{value}': expected an integer (byte count). \
                         Accepted keys: mime=, name=, size-max=."
                    ))
                })?;
                out.push(AttachmentFilter::SizeMax(limit));
            }
            _ => {
                return Err(JrError::UserError(format!(
                    "Unknown filter key '{key}'. Accepted keys: mime=, name=, size-max=."
                ))
                .into());
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Glob matching: case-insensitive, '*' crosses '/'
// ---------------------------------------------------------------------------

/// Case-insensitive glob match where `*` matches any character sequence
/// (including `/` — crosses subtype boundaries, per BC-2.7.003).
fn glob_match(pattern: &str, input: &str) -> bool {
    let p = pattern.to_lowercase();
    let s = input.to_lowercase();
    glob_inner(&p, &s)
}

fn glob_inner(pattern: &str, input: &str) -> bool {
    let mut p_chars = pattern.chars();
    match p_chars.next() {
        None => input.is_empty(),
        Some('*') => {
            let rest = p_chars.as_str();
            // '*' can match 0 or more characters — try every suffix of input.
            let mut pos = 0;
            loop {
                if glob_inner(rest, &input[pos..]) {
                    return true;
                }
                // Advance one UTF-8 character.
                let mut iter = input[pos..].char_indices();
                match iter.next() {
                    Some((_, c)) => pos += c.len_utf8(),
                    None => break,
                }
            }
            false
        }
        Some('?') => {
            let rest = p_chars.as_str();
            // '?' matches exactly one character (any character).
            let mut i_chars = input.chars();
            match i_chars.next() {
                Some(_) => glob_inner(rest, i_chars.as_str()),
                None => false,
            }
        }
        Some(pc) => {
            let mut i_chars = input.chars();
            match i_chars.next() {
                Some(ic) if pc == ic => glob_inner(p_chars.as_str(), i_chars.as_str()),
                _ => false,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Filter application
// ---------------------------------------------------------------------------

fn apply_filter(attachment: &AttachmentObject, filter: &AttachmentFilter) -> bool {
    match filter {
        AttachmentFilter::Mime(pattern) => glob_match(pattern, &attachment.mime_type),
        AttachmentFilter::Name(pattern) => glob_match(pattern, &attachment.filename),
        AttachmentFilter::SizeMax(limit) => attachment.size <= *limit,
    }
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Human-readable file size. Renders to 1 decimal place for KB/MB/GB.
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

/// Author display: displayName → accountId → "(anonymous)".
fn format_author(author: &Option<serde_json::Value>) -> String {
    let Some(obj) = author else {
        return "(anonymous)".to_string();
    };
    if let Some(dn) = obj.get("displayName").and_then(|v| v.as_str()) {
        if !dn.is_empty() {
            return dn.to_string();
        }
    }
    if let Some(aid) = obj.get("accountId").and_then(|v| v.as_str()) {
        if !aid.is_empty() {
            return aid.to_string();
        }
    }
    "(anonymous)".to_string()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// List attachments on a Jira issue, rendering as table or JSON.
///
/// BC-2.7.001 (table columns), BC-2.7.002 (JSON array shape + contentUrl),
/// BC-2.7.003 (mime= filter), BC-2.7.004 (name= filter),
/// BC-2.7.005 (size-max= filter), BC-2.7.006 (error taxonomy).
pub async fn handle_attachment_list(
    key: &str,
    filters: &[String],
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    // Validate filters BEFORE any HTTP call (EC-2.7.003-2, EC-2.7.005-1).
    let parsed_filters = parse_filters(filters)?;

    // Fetch attachments (error mapping handled in list_attachments).
    let attachments = client.list_attachments(key).await?;
    let total = attachments.len();

    // Apply filters with AND semantics (BC-2.7.004).
    let filtered: Vec<&AttachmentObject> = attachments
        .iter()
        .filter(|a| parsed_filters.iter().all(|f| apply_filter(a, f)))
        .collect();
    let n = filtered.len();

    match output_format {
        OutputFormat::Json => {
            // JSON mode: emit curated array; zero-attachment hint suppressed (EC-2.7.001-1).
            let curated: Vec<Value> = filtered
                .iter()
                .map(|a| serialize_attachment_curated(a))
                .collect();
            println!("{}", output::render_json(&curated)?);
            // Filter-count hint fires AFTER the JSON array write (EC-2.7.001-2).
            // Deliberate asymmetry: zero-attachment hint is suppressed in JSON mode
            // (EC-2.7.001-1) but filter-count hint is not.
            if n < total {
                eprintln!("Showing {n} of {total} attachments.");
            }
        }
        OutputFormat::Table => {
            if total == 0 {
                // EC-2.7.001-1: zero-attachment hint on stderr; stdout empty (pipe-friendly).
                eprintln!("No attachments on {key}.");
            } else if filtered.is_empty() {
                // All filtered out — filter-count hint fires after the (empty) stdout write.
                // stdout empty.
                if n < total {
                    eprintln!("Showing {n} of {total} attachments.");
                }
            } else {
                let headers = &["ID", "Filename", "Type", "Size", "Created", "Author"];
                let rows: Vec<Vec<String>> = filtered
                    .iter()
                    .map(|a| {
                        vec![
                            a.id.clone(),
                            display_sanitize_filename(&a.filename),
                            a.mime_type.clone(),
                            format_size(a.size),
                            a.created.clone(),
                            format_author(&a.author),
                        ]
                    })
                    .collect();
                println!("{}", output::render_table(headers, &rows));
                // Filter-count hint fires AFTER the table write (EC-2.7.001-2).
                if n < total {
                    eprintln!("Showing {n} of {total} attachments.");
                }
            }
        }
    }

    Ok(())
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
pub fn display_sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            let cp = c as u32;
            if cp <= 0x1F
                || cp == 0x7F
                || (0x202A..=0x202E).contains(&cp)
                || (0x2066..=0x2069).contains(&cp)
                || cp == 0x2028
                || cp == 0x2029
                || cp == 0x0085
            {
                '?'
            } else {
                c
            }
        })
        .collect()
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
pub fn serialize_attachment_curated(attachment: &AttachmentObject) -> Value {
    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    map.insert(
        "author".into(),
        match &attachment.author {
            // Absent or explicitly null in API response → top-level null.
            None | Some(Value::Null) => Value::Null,
            // Author object present → curate to EXACTLY {accountId, displayName}.
            // All other Jira author fields (self, avatarUrls, accountType, timeZone, …)
            // are stripped (BC-2.7.002 v1.3.95 P1-002).  BTreeMap ensures
            // accountId < displayName alphabetical ordering (BC-2.7.002 (d)).
            Some(author_val) => {
                let mut author_map: BTreeMap<String, Value> = BTreeMap::new();
                author_map.insert(
                    "accountId".into(),
                    author_val.get("accountId").cloned().unwrap_or(Value::Null),
                );
                author_map.insert(
                    "displayName".into(),
                    author_val
                        .get("displayName")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                serde_json::to_value(author_map)
                    .expect("BTreeMap<String, Value> serialization is infallible")
            }
        },
    );
    map.insert(
        "contentUrl".into(),
        Value::String(attachment.content.clone()),
    );
    map.insert("created".into(), Value::String(attachment.created.clone()));
    map.insert(
        "filename".into(),
        Value::String(attachment.filename.clone()),
    );
    map.insert("id".into(), Value::String(attachment.id.clone()));
    map.insert(
        "mimeType".into(),
        Value::String(attachment.mime_type.clone()),
    );
    map.insert("size".into(), Value::from(attachment.size));
    serde_json::to_value(map).expect("BTreeMap<String, Value> serialization is infallible")
}

// ---------------------------------------------------------------------------
// S-576-2: download manifest entry (BC-2.7.007 EC-2.7.007-7; P27-001/P31-002)
// ---------------------------------------------------------------------------

/// One successfully-downloaded attachment in the manifest JSON.
///
/// Key ordering: BTreeMap-alphabetical (`filename`, `id`, `path`, `size`).
/// - `filename` = RAW Jira-supplied name (pre-sanitization; P27-001 deliberate pairing).
/// - `id`       = attachment AID string.
/// - `path`     = on-disk path as-constructed by `jr` (NOT canonicalized; P18-004).
/// - `size`     = bytes actually written to disk from the streaming write loop (P31-002).
#[derive(serde::Serialize)]
pub struct AttachmentDownloadEntry {
    pub filename: String,
    pub id: String,
    pub path: String,
    pub size: u64,
}

// ---------------------------------------------------------------------------
// S-576-2: pure helpers
// ---------------------------------------------------------------------------

/// CWE-22 disk-path sanitization (BC-2.7.011 5-step algorithm).
///
/// **Distinct from `display_sanitize_filename`** (CWE-116 display variant, S-576-1).
/// Do NOT merge or share logic between the two functions.
///
/// Returns `Some(sanitized_name)` when the name is safe for use as a disk
/// path component, or `None` when the name is degenerate (empty, `"."`, `".."`,
/// NUL-containing, or left empty after sanitization steps).
///
/// **Windows device names** (`CON`, `NUL`, `COM1`, …) are NOT handled here —
/// the function returns `Some("CON")`, `Some("NUL")`, etc.  Device-name escape
/// (`_` prefix) is the CALLER's responsibility per SEC-576-001 / BC-2.7.011.
///
/// **Containment check** (BC-2.7.011 two-step, SEC-576-002): performed at the
/// call site after path construction, not inside this function.
///
/// # Algorithm (BC-2.7.011 lines 902–911 — verbatim)
/// 1. Basename extraction via `Path::file_name()` — strips directory components.
/// 2. Pseudo-name rejection: `"."`, `".."`, or empty → `None`.
/// 3. NUL byte rejection: name contains `'\0'` → `None` (NOT stripped).
/// 4. Character scrub: replace `/`, `\`, `:` with `_`.
/// 5. Length cap to 214 bytes on a valid UTF-8 char boundary; strip trailing
///    ASCII whitespace and `.` (step 5.5, SEC-576-007).
pub fn sanitize_attachment_filename(_name: &str) -> Option<String> {
    todo!("BC-2.7.011 5-step CWE-22 disk-path algorithm — implement in Task 2")
}

/// Compute the default output path for a downloaded attachment (BC-2.7.010).
///
/// **Batch** (`--all` / `--newest`):
/// `<base_dir>/<sha1_of_id(40 hex)>_<sanitize_attachment_filename(filename) or id>`.
/// Combined length guaranteed ≤ 255 bytes (41 + 214 = 255 ≤ NAME_MAX; ADV-010).
/// When `sanitize_attachment_filename` returns `None` (degenerate name), uses
/// `<sha1_of_id>_<attachment_id>` as the basename.
///
/// **Single** (`--id`):
/// Bare `sanitize_attachment_filename(filename)` in `base_dir` (no SHA-1 prefix).
/// When `sanitize_attachment_filename` returns `None`, uses bare `attachment_id`.
///
/// # Arguments
/// - `base_dir`      — directory for the output file.
/// - `attachment_id` — numeric attachment ID from the Jira API (trusted per SEC-576-008).
/// - `filename`      — raw Jira-supplied filename.
/// - `is_batch`      — `true` → batch path with SHA-1 prefix; `false` → single bare path.
pub fn compute_default_output_path(
    _base_dir: &std::path::Path,
    _attachment_id: &str,
    _filename: &str,
    _is_batch: bool,
) -> std::path::PathBuf {
    todo!("BC-2.7.010 default output path computation — implement in Task 4/5")
}

// ---------------------------------------------------------------------------
// S-576-2: effectful handler
// ---------------------------------------------------------------------------

/// Handle `jr issue attachment download <KEY>`.
///
/// Dispatched from `src/cli/issue/mod.rs` for `AttachmentSubcommand::Download`.
///
/// # Selectors (mutually exclusive via clap `selector` ArgGroup)
/// - `id`     → single-file download (BC-2.7.007)
/// - `all`    → batch download all attachments (BC-2.7.008)
/// - `newest` → download top-N by created descending (BC-2.7.009)
///
/// # Output path rules (BC-2.7.010)
/// Batch paths: `<out_dir or cwd>/<sha1_of_id>_<sanitized_filename>`.
/// Single path: `<out or cwd/<sanitized_filename>>`.
///
/// # CWE-22 mitigation (BC-2.7.011)
/// All server-supplied filenames pass through `sanitize_attachment_filename`
/// before disk use. Trusted operator `--out`/`--out-dir` paths are NOT sanitized.
///
/// # Error taxonomy (BC-2.7.012)
/// See AC-009 for full error → exit-code → message table.
#[allow(clippy::too_many_arguments)]
pub async fn handle_attachment_download(
    _key: &str,
    _id: Option<&str>,
    _all: bool,
    _newest: Option<i64>,
    _out: Option<&std::path::Path>,
    _out_dir: Option<&std::path::Path>,
    _filter: &[String],
    _force: bool,
    _output_format: &OutputFormat,
    _client: &JiraClient,
) -> anyhow::Result<()> {
    todo!("attachment download handler: BC-2.7.007..012 — implement in Task 4/5")
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // BC-2.7.011 display-sanitization

    #[test]
    fn test_display_sanitize_filename_passthrough_safe_chars() {
        assert_eq!(
            display_sanitize_filename("safe_file-name.txt"),
            "safe_file-name.txt"
        );
    }

    #[test]
    fn test_display_sanitize_filename_ascii_control_nul() {
        assert_eq!(display_sanitize_filename("\x00"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_ascii_control_range() {
        // All bytes 0x00–0x1F should be replaced
        for b in 0x00u8..=0x1Fu8 {
            let buf = [b];
            if let Ok(s) = std::str::from_utf8(&buf) {
                let result = display_sanitize_filename(s);
                assert_eq!(result, "?", "byte 0x{b:02X} should become '?'");
            }
        }
    }

    #[test]
    fn test_display_sanitize_filename_del_0x7f() {
        assert_eq!(display_sanitize_filename("\x7F"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_bidi_rlo_u202e() {
        assert_eq!(display_sanitize_filename("\u{202E}"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_bidi_range_202a_202e() {
        for cp in 0x202Au32..=0x202Eu32 {
            let c = char::from_u32(cp).unwrap();
            let s: String = std::iter::once(c).collect();
            let result = display_sanitize_filename(&s);
            assert_eq!(result, "?", "U+{cp:04X} should become '?'");
        }
    }

    #[test]
    fn test_display_sanitize_filename_bidi_range_2066_2069() {
        for cp in 0x2066u32..=0x2069u32 {
            let c = char::from_u32(cp).unwrap();
            let s: String = std::iter::once(c).collect();
            let result = display_sanitize_filename(&s);
            assert_eq!(result, "?", "U+{cp:04X} should become '?'");
        }
    }

    #[test]
    fn test_display_sanitize_filename_line_sep_u2028() {
        assert_eq!(display_sanitize_filename("\u{2028}"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_para_sep_u2029() {
        assert_eq!(display_sanitize_filename("\u{2029}"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_nel_u0085() {
        assert_eq!(display_sanitize_filename("\u{0085}"), "?");
    }

    #[test]
    fn test_display_sanitize_filename_mixed_safe_and_dangerous() {
        // U+202E (RLO) + U+2028 (line sep) + \0 (null byte)
        let input = "safe\u{202E}rlo\u{2028}sep\x00nul.txt";
        assert_eq!(display_sanitize_filename(input), "safe?rlo?sep?nul.txt");
    }

    // serialize_attachment_curated

    fn make_test_attachment() -> AttachmentObject {
        AttachmentObject {
            self_url: "https://example.atlassian.net/rest/api/3/attachment/10042".into(),
            id: "10042".into(),
            filename: "screenshot.png".into(),
            author: Some(serde_json::json!({"accountId": "acct-001", "displayName": "Alice"})),
            created: "2026-07-10T14:23:11.000+0000".into(),
            size: 43008,
            mime_type: "image/png".into(),
            content: "https://example.atlassian.net/rest/api/3/attachment/content/10042".into(),
        }
    }

    #[test]
    fn test_serialize_attachment_curated_omits_self() {
        let v = serialize_attachment_curated(&make_test_attachment());
        let obj = v.as_object().unwrap();
        assert!(
            !obj.contains_key("self"),
            "VP-576-004: 'self' must be omitted"
        );
    }

    #[test]
    fn test_serialize_attachment_curated_renames_content_to_content_url() {
        let v = serialize_attachment_curated(&make_test_attachment());
        let obj = v.as_object().unwrap();
        assert!(
            !obj.contains_key("content"),
            "VP-576-004: 'content' key must not exist"
        );
        assert!(
            obj.contains_key("contentUrl"),
            "VP-576-004: 'contentUrl' must be present"
        );
    }

    #[test]
    fn test_serialize_attachment_curated_size_is_integer() {
        let v = serialize_attachment_curated(&make_test_attachment());
        let obj = v.as_object().unwrap();
        assert!(obj["size"].is_number(), "size must be a raw integer");
        assert_eq!(obj["size"].as_u64(), Some(43008));
    }

    #[test]
    fn test_serialize_attachment_curated_null_author_when_none() {
        let mut a = make_test_attachment();
        a.author = None;
        let v = serialize_attachment_curated(&a);
        let obj = v.as_object().unwrap();
        assert_eq!(obj["author"], Value::Null, "author must be null when None");
    }

    #[test]
    fn test_serialize_attachment_curated_exact_keys() {
        let v = serialize_attachment_curated(&make_test_attachment());
        let obj = v.as_object().unwrap();
        let keys: std::collections::BTreeSet<&str> = obj.keys().map(|k| k.as_str()).collect();
        let expected: std::collections::BTreeSet<&str> = [
            "author",
            "contentUrl",
            "created",
            "filename",
            "id",
            "mimeType",
            "size",
        ]
        .iter()
        .copied()
        .collect();
        assert_eq!(keys, expected, "curated shape must have exactly 7 keys");
    }

    // format_size

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn test_format_size_kb_exact() {
        assert_eq!(format_size(43008), "42.0 KB");
    }

    #[test]
    fn test_format_size_kb_1024() {
        assert_eq!(format_size(1024), "1.0 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    // These two tests kill the surviving `*` → `+` mutant on the GB constant
    // (`const GB: u64 = 1024 * MB`).  With the mutation, GB = 1024 + MB = 1_049_600,
    // so 2 MiB (2_097_152) would mis-classify as GB and 1 GiB would render as
    // ~1023.0 GB.  EC-MUTANT-001 (cargo-mutants survivorship, S-576-1).
    #[test]
    fn test_format_size_2mb() {
        // 2 MiB = 2 * 1024^2 = 2_097_152 bytes. Must render as "2.0 MB".
        // With mutation GB = 1024 + MB = 1_049_600: 2_097_152 >= 1_049_600 → "2.0 GB" ≠ "2.0 MB".
        assert_eq!(format_size(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn test_format_size_1gb() {
        // 1 GiB = 1024^3 = 1_073_741_824 bytes. Must render as "1.0 GB".
        // With mutation GB = 1_049_600: 1_073_741_824 / 1_049_600 ≈ 1023.0 → "1023.0 GB" ≠ "1.0 GB".
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    // ---------------------------------------------------------------------------
    // sanitize_attachment_filename — BC-2.7.011 unit tests (S-576-2 stubs)
    // All todo!() — will pass once sanitize_attachment_filename is implemented.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_sanitize_attachment_filename_path_traversal_relative() {
        // BC-2.7.011 step 1: "../../etc/passwd" → Some("passwd")
        assert_eq!(
            sanitize_attachment_filename("../../etc/passwd"),
            Some("passwd".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_path_traversal_absolute() {
        // BC-2.7.011 step 1: "/etc/passwd" → Some("passwd")
        assert_eq!(
            sanitize_attachment_filename("/etc/passwd"),
            Some("passwd".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_path() {
        // BC-2.7.011 step 1: Windows path → Some("calc.exe")
        assert_eq!(
            sanitize_attachment_filename("C:\\Windows\\system32\\calc.exe"),
            Some("calc.exe".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_dot_returns_none() {
        // BC-2.7.011 step 2: "." → None
        assert_eq!(sanitize_attachment_filename("."), None);
    }

    #[test]
    fn test_sanitize_attachment_filename_dotdot_returns_none() {
        // BC-2.7.011 step 2: ".." → None
        assert_eq!(sanitize_attachment_filename(".."), None);
    }

    #[test]
    fn test_sanitize_attachment_filename_empty_returns_none() {
        // BC-2.7.011 step 2: empty string → None
        assert_eq!(sanitize_attachment_filename(""), None);
    }

    #[test]
    fn test_sanitize_attachment_filename_nul_byte_returns_none() {
        // BC-2.7.011 step 3: NUL byte → None (NOT stripped, REJECTED)
        assert_eq!(sanitize_attachment_filename("foo\0bar"), None);
    }

    #[test]
    fn test_sanitize_attachment_filename_colon_replaced_not_stripped() {
        // BC-2.7.011 step 4: ':' → '_' (REPLACE, not strip)
        assert_eq!(
            sanitize_attachment_filename("C:config.txt"),
            Some("C_config.txt".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_device_con_passes_through() {
        // BC-2.7.011 device names: CON → Some("CON") (caller escapes per SEC-576-001)
        assert_eq!(
            sanitize_attachment_filename("CON"),
            Some("CON".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_device_nul_passes_through() {
        // NUL as a FILENAME (no NUL BYTE in string) → Some("NUL") — distinct from \0
        assert_eq!(
            sanitize_attachment_filename("NUL"),
            Some("NUL".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_device_com1_passes_through() {
        assert_eq!(
            sanitize_attachment_filename("COM1"),
            Some("COM1".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_nul_txt_passes_through() {
        assert_eq!(
            sanitize_attachment_filename("nul.txt"),
            Some("nul.txt".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_long_name_truncated() {
        // BC-2.7.011 step 5: name exceeding 255 bytes → ≤ 214 bytes
        let long_name = "a".repeat(300);
        let result = sanitize_attachment_filename(&long_name);
        assert!(result.is_some());
        assert!(result.unwrap().len() <= 214);
    }

    #[test]
    fn test_sanitize_attachment_filename_multibyte_char_at_boundary() {
        // BC-2.7.011 step 5: 214-byte ASCII prefix + 3-byte UTF-8 char → Some(214-byte prefix)
        // 'é' is U+00E9, encoded as [0xC3, 0xA9] in UTF-8 (2 bytes actually).
        // Use a 3-byte char: '€' is U+20AC = [0xE2, 0x82, 0xAC].
        let prefix = "a".repeat(214);
        let input = format!("{prefix}€");
        let result = sanitize_attachment_filename(&input);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.len(), 214);
        assert_eq!(s, prefix);
    }

    #[test]
    fn test_sanitize_attachment_filename_trailing_dot_stripped() {
        // BC-2.7.011 step 5.5: trailing '.' stripped
        assert_eq!(
            sanitize_attachment_filename("foo."),
            Some("foo".to_string())
        );
    }

    #[test]
    fn test_sanitize_attachment_filename_trailing_space_stripped() {
        // BC-2.7.011 step 5.5: trailing space stripped
        assert_eq!(
            sanitize_attachment_filename("foo. "),
            Some("foo".to_string())
        );
    }

    // VP-576-001 proptest — domain includes non-ASCII-printable bytes (NUL, control chars)
    // so steps 3 and 4 are exercised by the random input generator.
    #[cfg(test)]
    mod proptest_sanitize {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_sanitize_attachment_filename_no_path_traversal(s in ".*") {
                // VP-576-001 assertions (BC-2.7.011 line 930):
                // 1. No Some(name) contains '/', '\', ':', or NUL.
                // 2. Some(name) length ≤ 214 bytes.
                // 3. All Some(name) are valid UTF-8.
                // 4. ".", "..", empty, NUL-byte inputs each return None.
                // 5. "../../etc/passwd" → Some("passwd"); "/etc/passwd" → Some("passwd").
                // 6. 214-byte ASCII + 3-byte char → Some(214-byte prefix).
                // 7. "CON" → Some("CON"); "NUL" → Some("NUL") (device names pass through).
                if let Some(name) = sanitize_attachment_filename(&s) {
                    prop_assert!(!name.contains('/'), "result must not contain /");
                    prop_assert!(!name.contains('\\'), "result must not contain \\");
                    prop_assert!(!name.contains(':'), "result must not contain :");
                    prop_assert!(!name.contains('\0'), "result must not contain NUL");
                    prop_assert!(name.len() <= 214, "result must be ≤ 214 bytes");
                    prop_assert!(std::str::from_utf8(name.as_bytes()).is_ok(), "must be valid UTF-8");
                }
            }
        }
    }

    // glob_match

    #[test]
    fn test_glob_match_star_crosses_slash() {
        // "image/*" — '*' after the slash matches only the subtype.
        assert!(glob_match("image/*", "image/png"));
        assert!(glob_match("image/*", "image/jpeg"));
        assert!(!glob_match("image/*", "application/pdf"));
        // "image*" — '*' must cross the '/' separator to match "image/png".
        // This genuinely proves the glob allows '*' to cross '/'.
        assert!(glob_match("image*", "image/png"));
        assert!(glob_match("image*", "image/jpeg"));
        assert!(!glob_match("image*", "application/pdf"));
    }

    #[test]
    fn test_glob_match_case_insensitive() {
        assert!(glob_match("IMAGE/*", "image/png"));
        assert!(glob_match("image/*", "IMAGE/PNG"));
    }

    #[test]
    fn test_glob_match_name_pattern() {
        assert!(glob_match("screenshot*", "screenshot.png"));
        assert!(glob_match("screenshot*", "screenshot.jpg"));
        assert!(!glob_match("screenshot*", "report.pdf"));
    }

    #[test]
    fn test_glob_match_exact_no_wildcard() {
        assert!(glob_match("dupe.txt", "dupe.txt"));
        assert!(!glob_match("dupe.txt", "dupe.csv"));
    }

    #[test]
    fn test_glob_match_question_mark_single_char() {
        // '?' matches exactly one character.
        assert!(glob_match("image/pn?", "image/png"));
        assert!(glob_match("image/pn?", "image/pnx"));
        // '?' does NOT match zero characters.
        assert!(!glob_match("image/pn?", "image/pn"));
        // '?' does NOT match two characters.
        assert!(!glob_match("image/pn?", "image/pngg"));
    }

    #[test]
    fn test_glob_match_question_mark_case_insensitive() {
        assert!(glob_match("IMAGE/PN?", "image/png"));
        assert!(glob_match("image/PN?", "IMAGE/PNG"));
    }

    #[test]
    fn test_glob_match_question_mark_in_filename() {
        // '?' in name pattern: report-?.pdf matches single char between '-' and '.'.
        assert!(glob_match("report-?.pdf", "report-1.pdf"));
        assert!(glob_match("report-?.pdf", "report-a.pdf"));
        // Does NOT match two chars after '-'.
        assert!(!glob_match("report-?.pdf", "report-10.pdf"));
        // Does NOT match zero chars after '-'.
        assert!(!glob_match("report-?.pdf", "report-.pdf"));
    }

    // format_author

    #[test]
    fn test_format_author_none_returns_anonymous() {
        assert_eq!(format_author(&None), "(anonymous)");
    }

    #[test]
    fn test_format_author_display_name_preferred() {
        let a = Some(serde_json::json!({"displayName": "Alice", "accountId": "acct-001"}));
        assert_eq!(format_author(&a), "Alice");
    }

    #[test]
    fn test_format_author_falls_back_to_account_id() {
        let a = Some(serde_json::json!({"displayName": null, "accountId": "acct-no-displayname"}));
        assert_eq!(format_author(&a), "acct-no-displayname");
    }

    // EC-2.7.001-3 (v1.3.96): empty-string displayName falls through to accountId.
    #[test]
    fn test_format_author_empty_display_name_falls_through_to_account_id() {
        let a = Some(serde_json::json!({"displayName": "", "accountId": "acct-empty-dn"}));
        assert_eq!(
            format_author(&a),
            "acct-empty-dn",
            "EC-2.7.001-3: empty displayName must fall through to accountId"
        );
    }

    // EC-2.7.001-3 (v1.3.96): empty displayName AND empty accountId → "(anonymous)".
    #[test]
    fn test_format_author_empty_display_name_and_empty_account_id_returns_anonymous() {
        let a = Some(serde_json::json!({"displayName": "", "accountId": ""}));
        assert_eq!(
            format_author(&a),
            "(anonymous)",
            "EC-2.7.001-3: empty displayName + empty accountId must yield (anonymous)"
        );
    }
}
