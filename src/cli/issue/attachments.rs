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
use futures::StreamExt;
use serde_json::Value;
use sha1::Digest;
use std::collections::BTreeMap;
use tokio::io::AsyncWriteExt;

use crate::api::client::JiraClient;
use crate::api::jira::attachments::AttachmentObject;
use crate::api::jsm::servicedesks::get_or_fetch_project_meta;
use crate::cache;
use crate::cli::{AttachmentSubcommand, OutputFormat};
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
        AttachmentFilter::Mime(pattern) => attachment
            .mime_type
            .as_deref()
            .is_some_and(|m| glob_match(pattern, m)),
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
                .copied()
                .map(serialize_attachment_curated)
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
                            a.mime_type.as_deref().unwrap_or("-").to_string(),
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
        attachment
            .mime_type
            .as_ref()
            .map_or(Value::Null, |m| Value::String(m.clone())),
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
struct AttachmentDownloadEntry {
    filename: String,
    id: String,
    path: String,
    size: u64,
}

/// JSON wrapper for the download manifest (`{"downloaded": [...]}`).
#[derive(serde::Serialize)]
struct DownloadManifest {
    downloaded: Vec<AttachmentDownloadEntry>,
}

// ---------------------------------------------------------------------------
// S-576-2: private pure helpers
// ---------------------------------------------------------------------------

/// Returns `true` when the stem of `name` (portion before the first `.`) is a
/// Windows reserved device name (case-insensitive): CON, NUL, PRN, AUX, COM1–COM9,
/// LPT1–LPT9.  COM0/LPT0 are NOT Windows device names and are NOT escaped.
/// Single-id call site only (SEC-576-001, BC-2.7.011, AC-016).
fn is_windows_device_name_basename(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    match upper.as_str() {
        "CON" | "NUL" | "PRN" | "AUX" => true,
        s if s.len() == 4 && (s.starts_with("COM") || s.starts_with("LPT")) => {
            // COM1–COM9 / LPT1–LPT9 only; COM0/LPT0 are NOT Windows device names.
            matches!(s.as_bytes()[3], b'1'..=b'9')
        }
        _ => false,
    }
}

/// Compute the 40-character lowercase hex SHA-1 of a string.
/// Used for batch output path uniqueness (BC-2.7.010 ADV-010) — NOT security.
fn sha1_hex(input: &str) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(input.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Return the largest index `pos ≤ limit` such that `&s[..pos]` is a valid UTF-8 slice.
fn floor_char_boundary_at(s: &str, limit: usize) -> usize {
    if s.len() <= limit {
        return s.len();
    }
    let mut pos = limit;
    while !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

// ---------------------------------------------------------------------------
// S-576-2: pure helpers (public)
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
/// # Algorithm (BC-2.7.011)
/// 1. Basename extraction via `Path::file_name()` — strips Unix `/` components.
///    Then split on `\` and take the last non-empty segment to handle Windows paths
///    (since `\` is not a separator on Unix and `Path::file_name()` would not strip it).
/// 2. Pseudo-name rejection: `"."`, `".."`, or empty → `None`.
/// 3. NUL byte rejection: name contains `'\0'` → `None` (NOT stripped, REJECTED).
/// 4. Character scrub: replace `/`, `\`, `:` with `_`.
/// 5. Length cap to 214 bytes on a valid UTF-8 char boundary; strip trailing
///    ASCII whitespace and `.` (step 5.5, SEC-576-007).
pub fn sanitize_attachment_filename(name: &str) -> Option<String> {
    // Step 1 (pre): Replace ':' before basename extraction for cross-platform
    // consistency. On Windows, Path::file_name() treats "C:name.txt" as a
    // drive-relative path and strips the "C:" prefix entirely, producing
    // "name.txt" instead of the spec-required "C_name.txt" (BC-2.7.011 step 4
    // mandates REPLACEMENT not stripping). Pre-replacing ensures uniform output
    // on all platforms. Step 4 still scrubs any remaining occurrences.
    let name_pre = name.replace(':', "_");

    // Step 1: Extract basename — strip Unix directory components.
    let after_unix_strip = std::path::Path::new(&name_pre)
        .file_name()
        .and_then(|f| f.to_str())?;

    // Step 1 (continued): Handle Windows backslash separators.
    // On Unix, `\` is not a path separator, so "C:\path\file.txt" would yield
    // the whole string above.  Split on `\` and take the last non-empty segment.
    let basename = after_unix_strip
        .rsplit('\\')
        .find(|s| !s.is_empty())
        .unwrap_or(after_unix_strip);

    // Step 2: Reject pseudo-names.
    if basename.is_empty() || basename == "." || basename == ".." {
        return None;
    }

    // Step 3: Reject NUL bytes (NOT stripped — REJECTED entirely).
    if basename.contains('\0') {
        return None;
    }

    // Step 4: Character scrub — replace '/', '\', ':' with '_'.
    let scrubbed: String = basename
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Step 5: Length cap to 214 bytes on a valid UTF-8 char boundary.
    let end = floor_char_boundary_at(&scrubbed, 214);
    let truncated = &scrubbed[..end];

    // Step 5.5: Strip trailing ASCII whitespace and '.'.
    let stripped = truncated.trim_end_matches(|c: char| c == '.' || c.is_ascii_whitespace());

    if stripped.is_empty() {
        return None;
    }

    Some(stripped.to_string())
}

/// Compute the default output path for a **batch** downloaded attachment (BC-2.7.010).
///
/// **Batch** (`--all` / `--newest`) only:
/// `<base_dir>/<sha1_of_id(40 hex)>_<sanitize_attachment_filename(filename) or id>`.
/// Combined length guaranteed ≤ 255 bytes (41 + 214 = 255 ≤ NAME_MAX; ADV-010).
/// When `sanitize_attachment_filename` returns `None` (degenerate name), uses
/// `<sha1_of_id>_<attachment_id>` as the basename.
///
/// # Single-mode path is NOT owned here
///
/// Single-mode (`--id`) path construction is INLINE in `handle_single_download` and
/// includes the SEC-576-001 Windows device-name escape (`_CON`, `_NUL`, …) — do NOT
/// consolidate it into this fn without moving that escape (F5-R11-001).
///
/// # Arguments
/// - `base_dir`      — directory for the output file.
/// - `attachment_id` — numeric attachment ID from the Jira API (trusted per SEC-576-008).
/// - `filename`      — raw Jira-supplied filename.
fn compute_default_output_path(
    base_dir: &std::path::Path,
    attachment_id: &str,
    filename: &str,
) -> std::path::PathBuf {
    let sanitized =
        sanitize_attachment_filename(filename).unwrap_or_else(|| attachment_id.to_string());

    let hash = sha1_hex(attachment_id);
    base_dir.join(format!("{hash}_{sanitized}"))
}

/// Defense-in-depth containment check for the batch download loop (BC-2.7.011 / F5-R1-001).
///
/// Returns `Ok(true)` when the parent directory of `final_path` (after canonicalization)
/// is equal to or inside `resolved_dir`.  Returns `Ok(false)` when an escape is detected.
/// Returns `Err` when the parent directory cannot be canonicalized (e.g., it does not yet
/// exist on disk); the caller treats this as fail-open and emits a warning.
///
/// `sanitize_attachment_filename` (VP-576-001 proptest) is the primary containment
/// authority.  This function is the secondary layer — it fires only if
/// `compute_default_output_path` ever acquires sub-directory logic that allows a path
/// component to cross outside `base_dir`.
///
/// Extracted as a standalone function to make the rejection branch directly testable.
fn batch_path_is_within_dir(
    final_path: &std::path::Path,
    resolved_dir: &std::path::Path,
) -> std::io::Result<bool> {
    let parent = final_path.parent().unwrap_or(resolved_dir);
    let canonical_parent = parent.canonicalize()?;
    // F5-R3-002: canonicalize resolved_dir before starts_with so that the
    // helper is correct even when called with a non-canonical base (e.g.,
    // containing `..` components or a macOS `/var` → `/private/var` symlink
    // prefix).  Via handle_batch_download, resolved_dir is always produced
    // by base_dir.canonicalize().unwrap_or(base_dir), so a non-canonical
    // value is not reachable there in practice; this guard provides
    // defense-in-depth for hypothetical future callers (e.g., if
    // compute_default_output_path gains sub-directory logic).  If
    // resolved_dir itself cannot be canonicalized, ? propagates an Err so
    // the call site's warn-and-skip path fires (fail-open).
    let canonical_dir = resolved_dir.canonicalize()?;
    Ok(canonical_parent.starts_with(canonical_dir))
}

// ---------------------------------------------------------------------------
// S-576-2: private async helpers
// ---------------------------------------------------------------------------

/// Classify a disk-write `io::Error` into a user-friendly discriminated message.
///
/// **BC-2.7.012 v1.3.104** — used by all four I/O sites in `stream_to_file`
/// (`File::create`, `write_all`, `flush`, `rename`) so that single-mode (propagate → exit 1)
/// and batch-mode (per-file fail-soft warning) paths both benefit from one chokepoint.
///
/// # Branch mapping
/// - `StorageFull | QuotaExceeded` →
///   `"Disk full: not enough space to write <dest>: <os_err>. Free up disk space and try again."`
/// - `PermissionDenied | ReadOnlyFilesystem` →
///   `"Permission denied: cannot write to <dir> (writing <dest>): <os_err>. Check directory permissions and try again."`
/// - `_` (non-exhaustive fallback, **required** because `ErrorKind` is `#[non_exhaustive]`) →
///   `"Failed to write <dest>: <os_err>."`
///
/// # Arguments
/// - `kind`         — `e.kind()` from the raw `std::io::Error` (call BEFORE any anyhow conversion).
/// - `dest_display` — final destination path, filename portion display-sanitized (CWE-116).
/// - `dir_display`  — `final_path.parent()` rendered verbatim (operator-controlled).
/// - `os_err`       — `e.to_string()` from the raw `std::io::Error` (ends in `(os error N)`).
fn classify_write_error(
    kind: std::io::ErrorKind,
    dest_display: &str,
    dir_display: &str,
    os_err: &str,
) -> String {
    match kind {
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => {
            format!(
                "Disk full: not enough space to write {dest_display}: {os_err}. \
                 Free up disk space and try again."
            )
        }
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
            format!(
                "Permission denied: cannot write to {dir_display} (writing {dest_display}): {os_err}. \
                 Check directory permissions and try again."
            )
        }
        _ => {
            format!("Failed to write {dest_display}: {os_err}.")
        }
    }
}

/// Compute the two display strings used in all write-error messages for `final_path`.
///
/// Returns `(dir_display, dest_display)` where:
/// - `dir_display`  — parent directory, rendered verbatim (operator-controlled); falls
///   back to `"."` when the path has no non-empty parent component.
/// - `dest_display` — full destination string for error messages: `"<dir>/<file>"` when
///   a non-empty parent exists, otherwise just the sanitized filename (no leading
///   path-separator prefix).
///
/// The filename portion is run through `display_sanitize_filename` (CWE-116 / BC-2.7.011).
/// The parent portion is rendered verbatim (operator-controlled; no sanitization applied).
///
/// # Mutant-killing contract (FIX-F5-010)
/// - Non-empty parent path: `dir_display` == the parent (NOT `"."`).
/// - Bare filename (no parent / empty parent):
///   - `dir_display` == `"."` (NOT `""`).
///   - `dest_display` == the filename — no leading path-separator character.
fn write_error_display_strings(final_path: &std::path::Path) -> (String, String) {
    let final_fname = final_path
        .file_name()
        .map(|n| display_sanitize_filename(&n.to_string_lossy()))
        .unwrap_or_else(|| display_sanitize_filename(&final_path.to_string_lossy()));
    let final_dir_display = final_path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.display().to_string())
        .unwrap_or_else(|| ".".to_string());
    let final_dest_display = match final_path.parent() {
        Some(d) if !d.as_os_str().is_empty() => {
            format!("{}{}{final_fname}", d.display(), std::path::MAIN_SEPARATOR)
        }
        _ => final_fname,
    };
    (final_dir_display, final_dest_display)
}

/// Stream `response` body to `final_path` using an atomic temp-file + rename pattern.
///
/// Temp file: `tmp_<16 random hex digits>` in the SAME directory as `final_path`
/// (same device → `rename` is atomic on POSIX; BC-2.7.007 ~749).
///
/// Returns the number of bytes actually written (P31-002: used in the manifest).
///
/// On ANY error after temp-file creation, the temp file is deleted before returning
/// the error (cleanup guarantee for BC-2.7.007 EC-2.7.007-4).
///
/// **Write-error classification (BC-2.7.012 v1.3.104):** all four I/O sites
/// (`File::create`, `write_all`, `flush`, `rename`) route through `classify_write_error` to
/// emit discriminated messages (`Disk full: …`, `Permission denied: …`, generic
/// fallback) that name the **final destination** (never the internal `tmp_<hex>` path).
async fn stream_to_file(
    response: reqwest::Response,
    final_path: &std::path::Path,
) -> anyhow::Result<u64> {
    let parent = final_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output path has no parent directory"))?;

    let token: u64 = rand::random();
    let tmp_path = parent.join(format!("tmp_{token:016x}"));

    // BC-2.7.012 v1.3.104: pre-compute display strings shared across all four I/O
    // error sites below. CWE-116 / BC-2.7.011: display-sanitize the server-supplied
    // filename portion; the operator-controlled parent directory is rendered verbatim.
    let (final_dir_display, final_dest_display) = write_error_display_strings(final_path);

    let result: anyhow::Result<u64> = async {
        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            let msg = classify_write_error(
                e.kind(),
                &final_dest_display,
                &final_dir_display,
                &e.to_string(),
            );
            anyhow::anyhow!("{msg}")
        })?;
        let mut bytes_written: u64 = 0;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("stream error: {e}"))?;
            file.write_all(&chunk).await.map_err(|e| {
                let msg = classify_write_error(
                    e.kind(),
                    &final_dest_display,
                    &final_dir_display,
                    &e.to_string(),
                );
                anyhow::anyhow!("{msg}")
            })?;
            bytes_written += chunk.len() as u64;
        }

        file.flush().await.map_err(|e| {
            let msg = classify_write_error(
                e.kind(),
                &final_dest_display,
                &final_dir_display,
                &e.to_string(),
            );
            anyhow::anyhow!("{msg}")
        })?;
        drop(file);

        tokio::fs::rename(&tmp_path, final_path)
            .await
            .map_err(|e| {
                let msg = classify_write_error(
                    e.kind(),
                    &final_dest_display,
                    &final_dir_display,
                    &e.to_string(),
                );
                anyhow::anyhow!("{msg}")
            })?;

        Ok(bytes_written)
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp_path).await;
    }

    result
}

/// Handle a single-attachment download (`--id`).
async fn handle_single_download(
    _key: &str,
    id_str: &str,
    out: Option<&std::path::Path>,
    force: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    // P32-001: ALL pre-flights for --out case fire BEFORE step-1 metadata GET.
    // Order per P32-001: EC-2.7.007-6 (parent-exists) → EC-2.7.007-11 (is-directory)
    // → EC-2.7.007-12 (overwrite-refuse) → metadata GET.
    if let Some(p) = out {
        // EC-2.7.007-6: parent-exists check.
        let parent = p.parent();
        let effective_parent = match parent {
            None => {
                return Err(JrError::UserError(format!(
                    "Output directory does not exist: {}",
                    p.display()
                ))
                .into());
            }
            Some(pp) if pp == std::path::Path::new("") => {
                // Bare filename (no directory component) → treat parent as CWD.
                std::env::current_dir().map_err(|e| {
                    JrError::UserError(format!("Cannot determine current directory: {e}"))
                })?
            }
            Some(pp) => pp.to_path_buf(),
        };
        if !effective_parent.exists() {
            return Err(JrError::UserError(format!(
                "Output directory does not exist: {}",
                effective_parent.display()
            ))
            .into());
        }
        // EC-2.7.007-11: path-is-directory check (P1-003, P32-001).
        if p.is_dir() {
            return Err(
                JrError::UserError(format!("output path is a directory: {}", p.display())).into(),
            );
        }
        // EC-2.7.007-12: overwrite-refuse pre-flight (SEC-576-010, P32-001).
        // Fires before the metadata GET when --out is supplied.
        if p.exists() && !force {
            return Err(JrError::UserError(format!(
                "File already exists: {}. Use --force to overwrite.",
                p.display()
            ))
            .into());
        }
    }

    // BC-2.7.007 step 1: fetch attachment metadata.
    // BC-2.7.012 body-surfacing asymmetry (F5-R3-001): download emits canonical-only;
    // get_attachment_metadata passes 404 through as ApiError so callers choose the format.
    let metadata = client.get_attachment_metadata(id_str).await.map_err(|e| {
        if let Some(JrError::ApiError { status, .. }) = e.downcast_ref::<JrError>() {
            if *status == 404 {
                return JrError::UserError(format!(
                    "Attachment {id_str} not found or not accessible."
                ))
                .into();
            }
        }
        e
    })?;
    let raw_filename = metadata.filename.as_deref().unwrap_or(id_str);

    // Determine final output path.
    let final_path = if let Some(p) = out {
        p.to_path_buf()
    } else {
        // Default: CWD + sanitized_filename, with Windows device-name escape.
        let cwd = std::env::current_dir()
            .map_err(|e| JrError::UserError(format!("Cannot determine current directory: {e}")))?;
        let sanitized = match sanitize_attachment_filename(raw_filename) {
            Some(s) => s,
            None => {
                // Degenerate-name fallback: id-as-filename (BC-2.7.010 R3.10).
                // Emit canonical warning in human mode only (hint, suppressed in JSON mode).
                if matches!(output_format, OutputFormat::Table) {
                    eprintln!(
                        "warning: using id as filename for attachment {} \u{2014} original name '{}' could not be sanitized.",
                        id_str,
                        display_sanitize_filename(raw_filename),
                    );
                }
                id_str.to_string()
            }
        };
        // SEC-576-001: device-name escape at single-id call site only.
        let name = if is_windows_device_name_basename(&sanitized) {
            format!("_{sanitized}")
        } else {
            sanitized
        };
        cwd.join(name)
    };

    // Collision check for default-path case (when --out is not supplied).
    // When --out IS supplied, overwrite-refuse was already checked in the pre-flight above.
    if out.is_none() && final_path.exists() && !force {
        // BC-2.7.011 / CWE-116: final_path was built from a server-derived filename;
        // display-sanitize the filename portion, parent (CWD) is operator-controlled.
        let fname = final_path
            .file_name()
            .map(|n| display_sanitize_filename(&n.to_string_lossy()))
            .unwrap_or_else(|| display_sanitize_filename(&final_path.to_string_lossy()));
        let display = match final_path.parent() {
            Some(d) if !d.as_os_str().is_empty() => {
                format!("{}{}{fname}", d.display(), std::path::MAIN_SEPARATOR)
            }
            _ => fname,
        };
        return Err(JrError::UserError(format!(
            "File already exists: {display}. Use --force to overwrite."
        ))
        .into());
    }

    // BC-2.7.007 step 2: stream attachment content.
    let response = client.get_attachment_content(id_str).await?;
    let bytes_written = stream_to_file(response, &final_path).await?;

    let entry = AttachmentDownloadEntry {
        filename: raw_filename.to_string(), // RAW Jira name (P27-001)
        id: id_str.to_string(),
        path: final_path.to_string_lossy().into_owned(),
        size: bytes_written, // bytes-written, not metadata size (P31-002)
    };

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&DownloadManifest {
                    downloaded: vec![entry]
                })?
            );
        }
        OutputFormat::Table => {
            // BC-2.7.011 / CWE-116: display-sanitize the server-supplied filename
            // portion; parent directory is operator-controlled and rendered verbatim.
            let fname = final_path
                .file_name()
                .map(|n| display_sanitize_filename(&n.to_string_lossy()))
                .unwrap_or_else(|| display_sanitize_filename(&final_path.to_string_lossy()));
            let display = match final_path.parent() {
                Some(d) if !d.as_os_str().is_empty() => {
                    format!("{}{}{fname}", d.display(), std::path::MAIN_SEPARATOR)
                }
                _ => fname,
            };
            eprintln!("Downloaded: {display} ({}).", format_size(bytes_written));
        }
    }

    Ok(())
}

/// Handle a batch download (`--all` / `--newest N`).
async fn handle_batch_download(
    key: &str,
    newest_n: Option<usize>,
    out_dir: Option<&std::path::Path>,
    parsed_filters: &[AttachmentFilter],
    force: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    // P32-001: pre-flight checks BEFORE any HTTP call.
    if let Some(d) = out_dir {
        if !d.exists() {
            return Err(JrError::UserError(format!(
                "Output directory does not exist: {}",
                d.display()
            ))
            .into());
        }
        if !d.is_dir() {
            return Err(JrError::UserError(format!("Not a directory: {}", d.display())).into());
        }
    }

    let base_dir: std::path::PathBuf = match out_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| JrError::UserError(format!("Cannot determine current directory: {e}")))?,
    };

    // Fetch attachment list for this issue.
    let attachments = client.list_attachments(key).await?;
    let total_unfiltered = attachments.len();

    // Empty issue — no attachments at all.
    if total_unfiltered == 0 {
        match output_format {
            OutputFormat::Table => eprintln!("No attachments on {key}."),
            OutputFormat::Json => println!(
                "{}",
                output::render_json(&DownloadManifest { downloaded: vec![] })?
            ),
        }
        return Ok(());
    }

    // Apply --filter predicates (AND semantics).
    let mut filtered: Vec<&AttachmentObject> = attachments
        .iter()
        .filter(|a| parsed_filters.iter().all(|f| apply_filter(a, f)))
        .collect();

    // Filtered-to-zero — issue has attachments but none matched.
    if filtered.is_empty() {
        match output_format {
            OutputFormat::Table => {
                eprintln!("No attachments matched the filter on {key}.")
            }
            OutputFormat::Json => println!(
                "{}",
                output::render_json(&DownloadManifest { downloaded: vec![] })?
            ),
        }
        return Ok(());
    }

    // --newest N: sort by created descending (chrono instant-based, NOT lexicographic —
    // BC-2.7.009 ~830), then truncate to N.
    if let Some(n) = newest_n {
        filtered.sort_by(|a, b| {
            // RFC 3339 relaxed parser — accepts any fractional-second precision (0, 1, 2, 3,
            // 4+ digits) and both +HH:MM and +HHMM offset forms, same as the --older-than path.
            // The previous %.3f format rejected 1-digit and 4-digit fractional seconds, causing
            // those attachments to sort LAST (None > Some ordering) — F5-R1-002.
            let parse_dt = |s: &str| s.parse::<chrono::DateTime<chrono::FixedOffset>>().ok();
            match (parse_dt(&a.created), parse_dt(&b.created)) {
                (Some(a_dt), Some(b_dt)) => b_dt.cmp(&a_dt), // newest first
                (Some(_), None) => std::cmp::Ordering::Less, // parsed before unparseable
                (None, Some(_)) => std::cmp::Ordering::Greater, // unparseable after parsed
                (None, None) => b.created.cmp(&a.created),   // both fail: lex tiebreak
            }
        });
        filtered.truncate(n);
    }

    let total_to_download = filtered.len();
    let mut entries: Vec<AttachmentDownloadEntry> = Vec::new();
    let mut fail_count: usize = 0;

    // BC-2.7.011 defense-in-depth: pre-canonicalize base_dir once for containment checks.
    let resolved_dir = base_dir
        .canonicalize()
        .unwrap_or_else(|_| base_dir.to_path_buf());

    for att in &filtered {
        // Degenerate-name warning (human mode only; hint, suppressed in JSON mode).
        // Canonical format per BC-2.7.010 ~872: em-dash U+2014, "original name", trailing period.
        if sanitize_attachment_filename(&att.filename).is_none()
            && matches!(output_format, OutputFormat::Table)
        {
            eprintln!(
                "warning: using id as filename for attachment {} \u{2014} original name '{}' could not be sanitized.",
                att.id,
                display_sanitize_filename(&att.filename),
            );
        }

        let final_path = compute_default_output_path(&base_dir, &att.id, &att.filename);

        // BC-2.7.011 defense-in-depth: verify the parent directory of final_path equals
        // (or is inside) resolved_dir. sanitize_attachment_filename (VP-576-001 proptest)
        // is the primary containment authority; this canonicalized-parent check provides
        // an additional layer in case compute_default_output_path ever gains sub-directory
        // logic. The previous check was vacuous: resolved_dir.join(single_component) is
        // always starts_with resolved_dir because a single path component can never contain
        // a traversal (F5-R1-001).
        match batch_path_is_within_dir(&final_path, &resolved_dir) {
            Ok(true) => {} // contained — proceed normally
            Ok(false) => {
                eprintln!(
                    "warning: skipping attachment {} — path escape detected after sanitization.",
                    att.id
                );
                continue;
            }
            Err(e) => {
                // Canonicalization failed (e.g., parent directory does not yet exist).
                // Fail-open: allow the download to proceed, but emit a warning so the
                // operator can observe that the containment check was skipped (SEC-F5-001).
                eprintln!(
                    "warning: containment check skipped for attachment {} \
                     — could not canonicalize path: {e}.",
                    att.id
                );
            }
        }

        // Collision check: skip (NON-ERROR per BC-2.7.008 ~797) if file exists and !force.
        // Collision-skip is a hint — suppressed in JSON mode (P27-003).
        if final_path.exists() && !force {
            if matches!(output_format, OutputFormat::Table) {
                let on_disk_name = final_path
                    .file_name()
                    .map(|n| display_sanitize_filename(&n.to_string_lossy()))
                    .unwrap_or_else(|| display_sanitize_filename(&final_path.to_string_lossy()));
                eprintln!(
                    "Skipping {on_disk_name}: file already exists. Use --force to overwrite."
                );
            }
            continue;
        }

        // Try download: get_attachment_content → stream_to_file.
        let response = match client.get_attachment_content(&att.id).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: failed to download attachment {}: {e}", att.id);
                fail_count += 1;
                continue;
            }
        };

        match stream_to_file(response, &final_path).await {
            Err(e) => {
                eprintln!("warning: failed to download attachment {}: {e}", att.id);
                fail_count += 1;
            }
            Ok(bytes_written) => {
                entries.push(AttachmentDownloadEntry {
                    filename: att.filename.clone(), // RAW Jira name (P27-001)
                    id: att.id.clone(),
                    path: final_path.to_string_lossy().into_owned(),
                    size: bytes_written, // bytes-written (P31-002)
                });
            }
        }
    }

    let success_count = entries.len();

    // Emit manifest (JSON) or summary (human) — BEFORE the error return so output
    // is flushed through the normal stdout path even on partial failure.
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                output::render_json(&DownloadManifest {
                    downloaded: entries
                })?
            );
        }
        OutputFormat::Table => {
            // Trailing period required per BC-2.7.008 ~799.
            eprintln!(
                "Downloaded {} of {} attachments to {}.",
                success_count,
                total_to_download,
                base_dir.display()
            );
        }
    }

    if fail_count > 0 {
        // House pattern for silent exit-1 (P1-007): all per-file warnings and the
        // summary have already been emitted above. Using std::process::exit(1) here
        // avoids routing through main.rs error printing which would emit a spurious
        // "Error: API error (1): ..." line to stderr (BC-2.7.008 fail-soft semantics).
        std::process::exit(1);
    }

    Ok(())
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
/// Takes the full `AttachmentSubcommand::Download` variant so callers avoid
/// exceeding the `clippy::too_many_arguments` threshold, mirroring the
/// `handle_comment_add` / `handle_comment_edit` pattern.
pub async fn handle_attachment_download(
    sub: AttachmentSubcommand,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    let AttachmentSubcommand::Download {
        key,
        id,
        all: _,
        newest,
        out,
        out_dir,
        filter,
        force,
    } = sub
    else {
        unreachable!("handle_attachment_download called with non-Download variant")
    };
    let id = id.as_deref();
    let out = out.as_deref();
    let out_dir = out_dir.as_deref();

    // Handler-level --newest N > 0 guard (clap accepts any i64; EC-2.7.009-1).
    if let Some(n) = newest {
        if n <= 0 {
            return Err(
                JrError::UserError("--newest requires a positive integer.".to_string()).into(),
            );
        }
    }

    if let Some(id_str) = id {
        // Validate: --id must be numeric (EC-2.7.007-5, BC-2.7.012).
        if !id_str.chars().all(|c| c.is_ascii_digit()) || id_str.is_empty() {
            return Err(JrError::UserError(format!(
                "invalid attachment id: '{id_str}' (must be numeric)"
            ))
            .into());
        }
        return handle_single_download(&key, id_str, out, force, output_format, client).await;
    }

    // Batch path (--all or --newest).
    // Parse filter syntax before any HTTP call (P32-001 validation ordering).
    let parsed_filters = parse_filters(&filter)?;
    let newest_n = newest.map(|n| n as usize);

    handle_batch_download(
        &key,
        newest_n,
        out_dir,
        &parsed_filters,
        force,
        output_format,
        client,
    )
    .await
}

// ---------------------------------------------------------------------------
// S-576-3: upload handler + helpers (BC-3.9.001..020)
// ---------------------------------------------------------------------------

/// Render the result of a successful upload as either a JSON array or a
/// 4-column table (Filename / Size / ID / Created).
fn render_upload_result(
    uploaded: &[AttachmentObject],
    output_format: &OutputFormat,
) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Json => {
            let curated: Vec<Value> = uploaded.iter().map(serialize_attachment_curated).collect();
            println!("{}", output::render_json(&curated)?);
        }
        OutputFormat::Table => {
            let headers = &["Filename", "Size", "ID", "Created"];
            let rows: Vec<Vec<String>> = uploaded
                .iter()
                .map(|a| {
                    vec![
                        display_sanitize_filename(&a.filename),
                        format_size(a.size),
                        a.id.clone(),
                        a.created.clone(),
                    ]
                })
                .collect();
            println!("{}", output::render_table(headers, &rows));
        }
    }
    Ok(())
}

/// Upload one or more files as attachments to a Jira issue (BC-3.9.001).
///
/// - Sends a single multipart/form-data POST with `X-Atlassian-Token: no-check` (BC-3.9.001).
/// - `--replace-existing`: calls `replace_existing_attachments` (BC-3.9.017; VP-576-003).
/// - `--dry-run` (requires `--replace-existing` at clap parse time): calls `dry_run_upload`
///   (BC-3.9.020 path-c; EC-3.9.020-6/7/9).
/// - Human output: table of uploaded attachments (BC-3.9.001). JSON: curated array via
///   `serialize_attachment_curated` (BC-3.9.009 / VP-576-004).
pub async fn handle_attachment_upload(
    sub: AttachmentSubcommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> anyhow::Result<()> {
    let AttachmentSubcommand::Upload {
        key,
        file,
        replace_existing,
        yes,
        dry_run,
        public,
        internal,
    } = sub
    else {
        unreachable!("handle_attachment_upload called with non-Upload variant");
    };

    // EC-3.9.001-6: stdin '-' is rejected before any HTTP call.
    for path in &file {
        if path.to_str() == Some("-") {
            return Err(JrError::UserError(
                "stdin upload is not supported; provide a file path.".to_string(),
            )
            .into());
        }
    }

    // EC-3.9.001-4: file existence pre-check fires before HTTP.
    // Two distinct branches: missing → "file not found:"; exists-but-not-regular → "not a regular file:".
    for path in &file {
        if !path.exists() {
            return Err(JrError::UserError(format!("file not found: {}", path.display())).into());
        }
        if !path.is_file() {
            return Err(
                JrError::UserError(format!("not a regular file: {}", path.display())).into(),
            );
        }
    }

    // S-576-5: route --public / --internal through the JSM-aware handler.
    // EC-3.9.003-7: the non-JSM guard fires INSIDE handle_attachment_upload_jsm,
    // AFTER project meta fetch but BEFORE the gate and BEFORE any dry-run preview.
    if public || internal {
        return handle_attachment_upload_jsm(
            &key,
            &file,
            &JsmUploadOpts {
                replace_existing,
                yes,
                dry_run,
                no_input,
                public,
            },
            output_format,
            client,
        )
        .await;
    }

    if dry_run {
        return dry_run_upload(&key, &file, replace_existing, false, output_format, client).await;
    }

    if replace_existing {
        return replace_existing_attachments(&key, &file, yes, no_input, output_format, client)
            .await;
    }

    // Direct upload: single multipart POST (EC-3.9.001-2: one POST regardless of file count).
    let uploaded = client.upload_attachments(&key, &file).await?;
    render_upload_result(&uploaded, output_format)
}

/// Delete all same-filename existing attachments then upload the new files (BC-3.9.017).
///
/// VP-576-003: ALL `delete_attachment` calls MUST complete successfully before the first
/// `upload_attachments` call is issued.
///
/// Dry-run dispatching is handled upstream in `handle_attachment_upload` before this
/// function is called; this function always performs real mutations.
/// When `yes` is `true` or `no_input` is `true`, skips the confirmation gate (BC-3.9.014).
async fn replace_existing_attachments(
    key: &str,
    file_paths: &[std::path::PathBuf],
    yes: bool,
    no_input: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    // Fetch existing attachments to find same-filename matches.
    let existing = client.list_attachments(key).await?;

    let upload_names: std::collections::HashSet<String> = file_paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();

    let would_delete: Vec<&AttachmentObject> = existing
        .iter()
        .filter(|a| upload_names.contains(&a.filename))
        .collect();

    // No same-filename matches → direct upload without any confirmation.
    if would_delete.is_empty() {
        let uploaded = client.upload_attachments(key, file_paths).await?;
        return render_upload_result(&uploaded, output_format);
    }

    // BC-3.9.014: non-interactive guard — exit 64 when --yes is absent.
    if no_input && !yes {
        return Err(JrError::UserError(
            "Use --yes to confirm deletion of existing same-filename attachments.".to_string(),
        )
        .into());
    }

    // Interactive or --yes confirmation gate.
    let proceed = attachment_replace_confirmation_gate(key, &would_delete, yes)?;
    if !proceed {
        eprintln!("Upload cancelled.");
        if let OutputFormat::Json = output_format {
            println!(
                "{}",
                output::render_json(&serde_json::json!({
                    "cancelled": true,
                    "uploaded": false
                }))?
            );
        }
        return Ok(());
    }

    // VP-576-003: ALL DELETEs must complete before the first POST.
    // EC-3.9.017-4: a 404 on DELETE = attachment already deleted by a concurrent actor →
    // benign silent skip; continue to the next DELETE and then to the POST.
    // `delete_attachment` maps HTTP-404 → JrError::UserError("…not found or already deleted.").
    // We detect via downcast_ref rather than modifying delete_attachment (DEC-168: its
    // 404→UserError mapping is correct for the standalone delete command).
    for att in &would_delete {
        match client.delete_attachment(&att.id).await {
            Ok(()) => {}
            Err(e) => {
                let is_benign_404 = e
                    .downcast_ref::<JrError>()
                    .is_some_and(|jr| matches!(jr, JrError::UserError(msg) if msg.contains("not found or already deleted")));
                if !is_benign_404 {
                    return Err(e);
                }
                // 404 → already deleted; silent skip per EC-3.9.017-4.
            }
        }
    }

    let uploaded = client.upload_attachments(key, file_paths).await?;
    render_upload_result(&uploaded, output_format)
}

/// Preview the upload operation without issuing any HTTP mutations (BC-3.9.020 path-c).
///
/// EC-3.9.020-9 three-category taxonomy:
/// - **Category 1 (confirmation gates):** SUPPRESSED — no interactive prompts in dry-run.
/// - **Category 2 (eligibility guards: BC-3.9.005 non-JSM, BC-3.9.017 step-0 validity, flag combos):** NOT suppressed.
/// - **Category 3 (pre-flight file checks: file-not-found, not-a-regular-file, issue-404):** NOT suppressed.
///
/// The read-only list GET still fires to populate the `wouldDelete` preview array
/// (mandatory per AC-008 / BC-3.9.020 path-c; only DELETE and POST are suppressed).
/// EC-3.9.020-7: when `public` is `true`, each `wouldUpload` entry gains
/// `"visibility":"public"` in JSON mode and `[public]` in human mode.
async fn dry_run_upload(
    key: &str,
    file_paths: &[std::path::PathBuf],
    replace_existing: bool,
    public: bool,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    // AC-008: the list GET MUST fire to populate wouldDelete — only DELETE and POST are suppressed.
    let existing = if replace_existing {
        client.list_attachments(key).await?
    } else {
        vec![]
    };

    let upload_names: std::collections::HashSet<String> = file_paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();

    let would_delete: Vec<Value> = existing
        .iter()
        .filter(|a| upload_names.contains(&a.filename))
        .map(|a| serde_json::json!({"id": a.id, "filename": a.filename}))
        .collect();

    // EC-3.9.020-7: add "visibility":"public" to each wouldUpload entry when public=true.
    let would_upload: Vec<Value> = file_paths
        .iter()
        .filter_map(|p| {
            p.file_name().map(|n| {
                if public {
                    serde_json::json!({"filename": n.to_string_lossy(), "visibility": "public"})
                } else {
                    serde_json::json!({"filename": n.to_string_lossy()})
                }
            })
        })
        .collect();

    let preview = serde_json::json!({
        "dryRun": true,
        "wouldDelete": would_delete,
        "wouldUpload": would_upload,
    });

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&preview)?);
        }
        OutputFormat::Table => {
            println!("DRY RUN — no changes will be made.");
            if !would_delete.is_empty() {
                println!(
                    "Would delete {} existing attachment(s).",
                    would_delete.len()
                );
            }
            // EC-3.9.020-7: [public] annotation when public=true.
            if public {
                println!("Would upload {} file(s) [public].", would_upload.len());
            } else {
                println!("Would upload {} file(s).", would_upload.len());
            }
        }
    }
    Ok(())
}

/// Interactive `--replace-existing` confirmation gate (BC-3.9.014).
///
/// Uses `eprint!` + `io::stdin().read_line()` only — NOT `dialoguer::Confirm`.
/// This exact mechanism is required by BC-3.9.014 gate mechanics (same pattern
/// as `interactions.rs::handle_delete_comment`).
///
/// Return semantics:
/// - `yes` is `true` → skip prompt, return `Ok(true)`.
/// - User inputs `"y"` / `"yes"` (case-insensitive, trimmed) → `Ok(true)`.
/// - Any other non-empty input → `Ok(false)` (caller exits 0 — cancelled).
/// - EOF or IO error → `Err(JrError::Interrupted)` (exit 130).
///
/// Note: callers on `--no-input` paths MUST exit 64 BEFORE calling this function when
/// `--yes` is absent (BC-3.9.014 non-interactive enforcement in `replace_existing_attachments`).
fn attachment_replace_confirmation_gate(
    key: &str,
    would_delete: &[&AttachmentObject],
    yes: bool,
) -> anyhow::Result<bool> {
    // Caller invariant: `no_input && !yes` has already exited 64 before reaching here,
    // so `no_input=true, yes=false` is unreachable at this point.
    if yes {
        return Ok(true);
    }

    eprintln!("Replace existing attachment(s) on {}:", key);
    for att in would_delete {
        eprintln!(
            "  {} (id: {})",
            display_sanitize_filename(&att.filename),
            att.id
        );
    }
    eprint!("Continue? [y/N] ");
    let _ = std::io::Write::flush(&mut std::io::stderr());

    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => Err(JrError::Interrupted.into()),
        Ok(_) => {
            let trimmed = line.trim();
            Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
        }
    }
}

// ---------------------------------------------------------------------------
// S-576-5: JSM visibility helpers and handler
// ---------------------------------------------------------------------------

/// Interactive `--public` confirmation gate (consumer 1: `--public` only, no replace).
///
/// BC-3.9.014 EC-3.9.014-5 prompt format:
/// - N ≤ 3: `"Upload <f1>, <f2>, <fN> to <KEY> as customer-visible (public)? [y/N] "`
/// - N > 3: `"Upload <N> files to <KEY> as customer-visible (public)? [y/N] "`
///
/// Returns `Ok(true)` → proceed, `Ok(false)` → cancelled,
/// `Err(JrError::Interrupted)` → EOF/IO error (exit 130).
fn jsm_public_gate(file_paths: &[std::path::PathBuf], key: &str) -> anyhow::Result<bool> {
    if file_paths.len() <= 3 {
        let names: Vec<String> = file_paths
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(display_sanitize_filename)
                    .unwrap_or_else(|| "<unknown>".to_string())
            })
            .collect();
        eprint!(
            "Upload {} to {} as customer-visible (public)? [y/N] ",
            names.join(", "),
            key
        );
    } else {
        eprint!(
            "Upload {} files to {} as customer-visible (public)? [y/N] ",
            file_paths.len(),
            key
        );
    }
    let _ = std::io::Write::flush(&mut std::io::stderr());
    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => Err(JrError::Interrupted.into()),
        Ok(_) => {
            let trimmed = line.trim();
            Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
        }
    }
}

/// Interactive combined gate: `--public` + `--replace-existing` with ≥1 filename match (consumer 3).
///
/// BC-3.9.014 prompt format:
/// `"Upload to <KEY> as customer-visible (public) and replace existing attachment(s):\n"`
/// followed by `"  <filename> (id: <AID>)\n"` for each match, then `"Continue? [y/N] "`.
///
/// VP-576-005: ONE prompt only (not one for replace + one for public).
///
/// Returns `Ok(true)` → proceed, `Ok(false)` → cancelled,
/// `Err(JrError::Interrupted)` → EOF/IO error (exit 130).
fn jsm_public_combined_gate(key: &str, would_delete: &[AttachmentObject]) -> anyhow::Result<bool> {
    eprintln!(
        "Upload to {} as customer-visible (public) and replace existing attachment(s):",
        key
    );
    for att in would_delete {
        eprintln!(
            "  {} (id: {})",
            display_sanitize_filename(&att.filename),
            att.id
        );
    }
    eprint!("Continue? [y/N] ");
    let _ = std::io::Write::flush(&mut std::io::stderr());
    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => Err(JrError::Interrupted.into()),
        Ok(_) => {
            let trimmed = line.trim();
            Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
        }
    }
}

/// Boolean-flag bundle for `handle_attachment_upload_jsm` (reduces argument count).
struct JsmUploadOpts {
    replace_existing: bool,
    yes: bool,
    dry_run: bool,
    no_input: bool,
    public: bool,
}

/// JSM-aware upload handler (S-576-5).
///
/// Called from `handle_attachment_upload` when `--public` or `--internal` is set.
///
/// **EC-3.9.003-7:** the non-JSM project guard fires AFTER project meta fetch but
/// BEFORE the non-interactive gate and BEFORE any dry-run preview (EC-3.9.020-8).
///
/// **OQ-9:** `--internal` on a non-JSM project is a silent no-op — the upload
/// falls through to the platform path with no warning and no servicedeskapi calls.
///
/// **BC-3.9.005:** `--public` on a non-JSM project exits 64.
///
/// **SEC-576-006:** a 404/403 from step-1 (`attachTemporaryFile`) triggers a
/// one-time stale-ID self-heal: invalidate the cache entry, re-resolve sdId, and
/// retry ONCE.
///
/// **VP-576-005:** `--public` + `--replace-existing` (with ≥1 filename match) uses
/// ONE combined prompt, not two separate prompts.
///
/// **VP-576-003:** all DELETEs complete before the first POST.
async fn handle_attachment_upload_jsm(
    key: &str,
    file_paths: &[std::path::PathBuf],
    opts: &JsmUploadOpts,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> anyhow::Result<()> {
    let JsmUploadOpts {
        replace_existing,
        yes,
        dry_run,
        no_input,
        public,
    } = *opts;
    // Derive project key via HTTP (P1-004): GET /rest/api/3/issue/{key}?fields=project.
    // Validates that the issue exists and returns its project key exactly as Jira knows it.
    // 404 → JrError::UserError("Issue {key} not found or not accessible.") → exit 64.
    let project_key = client.get_issue_project_key(key).await?;
    let project_key = project_key.as_str();

    // Fetch (or read from cache) project metadata for JSM determination.
    let meta = get_or_fetch_project_meta(client, project_key).await?;

    // EC-3.9.003-7: non-JSM guard fires BEFORE gate and BEFORE dry-run preview.
    if meta.project_type != "service_desk" {
        if public {
            // BC-3.9.005: --public on non-JSM → exit 64.
            return Err(JrError::UserError(
                "--public is only supported on Jira Service Management (JSM) issues.".to_string(),
            )
            .into());
        }
        // OQ-9: --internal on non-JSM → silent no-op, fall through to platform path.
        if dry_run {
            return dry_run_upload(
                key,
                file_paths,
                replace_existing,
                false,
                output_format,
                client,
            )
            .await;
        }
        if replace_existing {
            return replace_existing_attachments(
                key,
                file_paths,
                yes,
                no_input,
                output_format,
                client,
            )
            .await;
        }
        let uploaded = client.upload_attachments(key, file_paths).await?;
        return render_upload_result(&uploaded, output_format);
    }

    // JSM project: resolve sdId via the canonical resolver (P1-005).
    // `resolve_service_desk_id` re-reads from cache (hit — just populated above) and
    // returns the canonical UserError if service_desk_id is None.
    let sd_id = crate::api::jsm::servicedesks::resolve_service_desk_id(client, project_key).await?;

    // JSM dry-run: preview without step-1/step-2, with visibility annotation.
    // The non-JSM guard already fired above so EC-3.9.020-8 is satisfied.
    if dry_run {
        return dry_run_upload(
            key,
            file_paths,
            replace_existing,
            public,
            output_format,
            client,
        )
        .await;
    }

    // HOISTED: attachment-list fetch runs for BOTH --public and --internal when
    // replace_existing=true (P4-001: previously inside `if public { }`, silently
    // skipping DELETEs on the --internal path).
    //
    // BC-3.9.014 (P1-007): fetch FIRST so both non-interactive hint and interactive
    // gate use actual filename-match data.
    // VP-576-005: ONE combined prompt when --public + --replace-existing + ≥1 match.
    let to_delete: Vec<AttachmentObject> = if replace_existing {
        let existing = client.list_attachments(key).await?;
        let upload_names: std::collections::HashSet<String> = file_paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        existing
            .into_iter()
            .filter(|a| upload_names.contains(&a.filename))
            .collect()
    } else {
        vec![]
    };
    let has_replace_matches = !to_delete.is_empty();

    if public {
        // --public visibility gate: consumer-1 (public-only) or consumer-3 (combined).
        // Non-interactive path: exit 64 with hint; no gate presented; no DELETEs issued.
        // Consumer is determined by actual match count (P1-007 fix):
        // consumer-3 only when replace_existing AND ≥1 match; consumer-1 otherwise.
        if no_input && !yes {
            return if replace_existing && has_replace_matches {
                Err(JrError::UserError(
                    "Use --yes to confirm uploading as customer-visible (public) and \
                     deleting existing same-filename attachments."
                        .to_string(),
                )
                .into())
            } else {
                Err(JrError::UserError(format!(
                    "Use --yes to confirm uploading {} file(s) to {} as \
                     customer-visible, or run interactively.",
                    file_paths.len(),
                    key
                ))
                .into())
            };
        }

        if !yes {
            let proceed = if replace_existing && has_replace_matches {
                jsm_public_combined_gate(key, &to_delete)?
            } else {
                jsm_public_gate(file_paths, key)?
            };
            if !proceed {
                eprintln!("Upload cancelled.");
                if let OutputFormat::Json = output_format {
                    println!(
                        "{}",
                        output::render_json(&serde_json::json!({
                            "cancelled": true,
                            "uploaded": false
                        }))?
                    );
                }
                return Ok(());
            }
        }
    } else if replace_existing && has_replace_matches {
        // --internal + --replace-existing + ≥1 filename match: consumer-2 gate
        // (BC-3.9.017). No gate when zero matches.
        if no_input && !yes {
            return Err(JrError::UserError(
                "Use --yes to confirm deletion of existing same-filename attachments.".to_string(),
            )
            .into());
        }
        if !yes {
            let refs: Vec<&AttachmentObject> = to_delete.iter().collect();
            let proceed = attachment_replace_confirmation_gate(key, &refs, false)?;
            if !proceed {
                eprintln!("Upload cancelled.");
                if let OutputFormat::Json = output_format {
                    println!(
                        "{}",
                        output::render_json(&serde_json::json!({
                            "cancelled": true,
                            "uploaded": false
                        }))?
                    );
                }
                return Ok(());
            }
        }
    }

    // VP-576-003: all DELETEs complete before the first POST.
    // Now covers BOTH --public and --internal when has_replace_matches=true (P4-001).
    if has_replace_matches {
        for att in &to_delete {
            match client.delete_attachment(&att.id).await {
                Ok(()) => {}
                Err(e) => {
                    let is_benign = e.downcast_ref::<JrError>().is_some_and(|jr| {
                        matches!(jr, JrError::UserError(msg) if msg.contains("not found or already deleted"))
                    });
                    if !is_benign {
                        return Err(e);
                    }
                }
            }
        }
    }

    // JSM two-step upload.
    // Step 1: attach each file as a temporary attachment; collect tmpIds.
    // SEC-576-006: on 404/403 from step-1, invalidate the cache and retry ONCE.
    let mut current_sd_id = sd_id;
    let mut stale_healed = false;
    let mut tmp_ids: Vec<String> = Vec::with_capacity(file_paths.len());

    for path in file_paths {
        let result =
            crate::api::jsm::attachments::attach_temporary_file(client, &current_sd_id, path).await;

        let tmp_id = match result {
            Ok(id) => id,
            Err(ref e) => {
                let is_stale = e.downcast_ref::<JrError>().is_some_and(|jr| {
                    matches!(jr, JrError::ApiError { status, .. } if *status == 404 || *status == 403)
                });
                if is_stale && !stale_healed {
                    stale_healed = true;
                    cache::invalidate_project_meta_cache(client.profile_name(), project_key);
                    let fresh_meta = get_or_fetch_project_meta(client, project_key).await?;
                    match fresh_meta.service_desk_id {
                        None => {
                            return Err(JrError::UserError(format!(
                                "Service desk for {project_key} not found after refresh."
                            ))
                            .into());
                        }
                        Some(new_id) => {
                            current_sd_id = new_id;
                            // P1-001: explicit EC-4 mapping on retry — bare .await? would
                            // propagate ApiError{status:404} as exit 1 instead of exit 64.
                            let retry_result = crate::api::jsm::attachments::attach_temporary_file(
                                client,
                                &current_sd_id,
                                path,
                            )
                            .await;
                            match retry_result {
                                Ok(id) => id,
                                Err(e) => {
                                    return match e.downcast::<JrError>() {
                                        Ok(JrError::ApiError { status: 404, .. }) => {
                                            Err(JrError::UserError(format!(
                                                "Service desk for {project_key} not found after refresh."
                                            ))
                                            .into())
                                        }
                                        // Post-retry 401 arrives as JrError::NotAuthenticated (exit 2); falls through to Ok(other).
                                        Ok(other) => Err(anyhow::anyhow!(other)),
                                        Err(other) => Err(other),
                                    };
                                }
                            }
                        }
                    }
                } else {
                    return Err(result.unwrap_err());
                }
            }
        };
        tmp_ids.push(tmp_id);
    }

    // Step 2: publish all tmpIds to the JSM request.
    let uploaded =
        crate::api::jsm::attachments::post_request_attachment(client, key, &tmp_ids, public)
            .await?;
    render_upload_result(&uploaded, output_format)
}

// ---------------------------------------------------------------------------
// S-576-4: `jr issue attachment delete` — single-AID + bulk + --older-than + --dry-run
// ---------------------------------------------------------------------------

/// Handle `jr issue attachment delete` (S-576-4; BC-3.9.008/010/013/015/016/019/020).
///
/// Three forms:
///   (1) Single-AID: AID validation → confirmation gate (BC-3.9.015) → DELETE.
///   (2) Multi-AID bulk: AID validation → `--yes` required (BC-3.9.016) → sequential DELETEs.
///   (3) Issue+age: fetch list → `parse_age_duration` → filter → `--yes` required → DELETEs.
///
/// **DEC-168 (targeted single-AID 404):** exit 64; stderr MUST BEGIN with the canonical
/// prefix `"Attachment <AID> not found or not accessible."` then the Jira error body.
/// **BC-3.9.010 (bulk 404):** BENIGN SKIP — asymmetry from targeted single-AID 404.
/// **EC-3.9.020-3:** single-AID `--dry-run` — guards active, gate suppressed, no DELETE.
/// **EC-3.9.020-1/2:** bulk `--dry-run` — guards active, gate suppressed, no DELETE.
pub async fn handle_attachment_delete(
    sub: AttachmentSubcommand,
    output_format: &OutputFormat,
    client: &JiraClient,
    no_input: bool,
) -> anyhow::Result<()> {
    let is_json = matches!(output_format, OutputFormat::Json);

    let (aids, issue, older_than, yes, dry_run) = match sub {
        AttachmentSubcommand::Delete {
            aids,
            issue,
            older_than,
            yes,
            dry_run,
        } => (aids, issue, older_than, yes, dry_run),
        _ => unreachable!("handle_attachment_delete called with non-Delete subcommand"),
    };

    // -------------------------------------------------------------------
    // Path A: positional AID(s)
    // -------------------------------------------------------------------
    if !aids.is_empty() {
        // Validate all AIDs first (before any I/O or HTTP)
        for aid in &aids {
            if aid.is_empty() || !aid.chars().all(|c| c.is_ascii_digit()) {
                return Err(JrError::UserError(format!(
                    "invalid attachment id: '{aid}' (must be numeric)"
                ))
                .into());
            }
        }

        if aids.len() == 1 {
            let aid = &aids[0];

            // Single-AID dry-run (EC-3.9.020-3): guards NOT suppressed, gate suppressed.
            if dry_run {
                if is_json {
                    let payload = serde_json::json!({
                        "attachments": [{"id": aid}],
                        "dryRun": true,
                        "ids": [aid]
                    });
                    println!("{}", output::render_json(&payload)?);
                } else {
                    eprintln!("--dry-run has no effect on single-ID delete; omit the flag.");
                }
                return Ok(());
            }

            // Non-interactive without --yes (BC-3.9.015 EC-3.9.015-3)
            if no_input && !yes {
                return Err(JrError::UserError(
                    "Use --yes to confirm deletion without a prompt.".to_string(),
                )
                .into());
            }

            // Confirmation gate (BC-3.9.015; VP-576-002; DEC-174)
            if !yes {
                // Fetch metadata to get the filename for the gate prompt.
                // DEC-168 / BC-2.7.012 body-surfacing asymmetry (F5-R3-001): on 404
                // the interactive delete path shows canonical prefix + Jira error body
                // (actionable detail). get_attachment_metadata returns ApiError { 404 }
                // with body intact; we format it here as canonical + "\n{body}".
                let meta = client.get_attachment_metadata(aid).await.map_err(|e| {
                    if let Some(JrError::ApiError { status, message }) = e.downcast_ref::<JrError>()
                    {
                        if *status == 404 {
                            return JrError::UserError(format!(
                                "Attachment {aid} not found or not accessible.\n{message}"
                            ))
                            .into();
                        }
                    }
                    e
                })?;
                let filename = meta.filename.as_deref().unwrap_or(aid.as_str());
                let display_name = display_sanitize_filename(filename);

                let confirmed = attachment_delete_confirmation_gate(&display_name, aid)?;
                if !confirmed {
                    if is_json {
                        let payload = serde_json::json!({
                            "cancelled": true,
                            "deleted": false
                        });
                        println!("{}", output::render_json(&payload)?);
                    } else {
                        eprintln!("Deletion cancelled.");
                    }
                    return Ok(());
                }
            }

            // Issue the targeted DELETE (DEC-168 on 404)
            client.delete_attachment_targeted(aid).await?;

            if is_json {
                let payload = serde_json::json!({"deleted": true, "id": aid});
                println!("{}", output::render_json(&payload)?);
            } else {
                eprintln!("Deleted attachment {aid}.");
            }
            return Ok(());
        }

        // Multi-AID bulk path
        // --yes required (BC-3.9.016 EC-3.9.016-8); dry-run exempts
        if !yes && !dry_run {
            return Err(JrError::UserError(
                "--yes is required to delete multiple attachments without a confirmation prompt."
                    .to_string(),
            )
            .into());
        }

        if dry_run {
            // Bulk dry-run: fan out per-AID metadata GETs to populate filenames
            // (AC-009 P2-002: GET /rest/api/3/attachment/{id} for each AID).
            // Metadata failure → {id}-only fallback row; never aborts (dry-run is read-only).
            let mut attachment_rows: Vec<serde_json::Value> = Vec::new();
            let ids: Vec<&str> = aids.iter().map(|s| s.as_str()).collect();
            // Human table rows [ID, Filename, Size, Created] — built alongside JSON rows so
            // the JSON shape ({filename,id} / {id}-only) remains unchanged (P2-002 pins GREEN).
            let mut human_rows: Vec<Vec<String>> = Vec::new();

            for aid in &aids {
                match client.get_attachment_metadata(aid).await {
                    Ok(meta) => {
                        let filename = meta.filename.unwrap_or_default();
                        let size = meta.size;
                        let created = meta.created.clone().unwrap_or_default();
                        // BTreeMap key order: filename < id (alphabetical); JSON shape unchanged.
                        let mut row = std::collections::BTreeMap::new();
                        row.insert("filename", serde_json::Value::String(filename.clone()));
                        row.insert("id", serde_json::Value::String(aid.clone()));
                        attachment_rows.push(serde_json::to_value(row)?);
                        // Human row: display-sanitized filename (CWE-116), formatted size, created.
                        human_rows.push(vec![
                            aid.clone(),
                            display_sanitize_filename(&filename),
                            size.map(format_size).unwrap_or_else(|| "-".to_string()),
                            created,
                        ]);
                    }
                    Err(_) => {
                        // Metadata unavailable → id-only fallback row (no filename key)
                        let mut row = std::collections::BTreeMap::new();
                        row.insert("id", serde_json::Value::String(aid.clone()));
                        attachment_rows.push(serde_json::to_value(row)?);
                        // Human fallback row (AC-009 per-row "(metadata unavailable)" marker).
                        human_rows.push(vec![
                            aid.clone(),
                            "(metadata unavailable)".to_string(),
                            "-".to_string(),
                            "-".to_string(),
                        ]);
                    }
                }
            }

            if is_json {
                let payload = serde_json::json!({
                    "attachments": attachment_rows,
                    "dryRun": true,
                    "ids": ids
                });
                println!("{}", output::render_json(&payload)?);
            } else {
                // Human mode: AC-009 table [ID, Filename (CWE-116), Size, Created]
                eprintln!(
                    "{}",
                    output::render_table(&["ID", "Filename", "Size", "Created"], &human_rows)
                );
                eprintln!(
                    "{} attachment(s) would be deleted. Run without --dry-run to confirm.",
                    aids.len()
                );
            }
            return Ok(());
        }

        // Bulk sequential deletes — 404 is benign skip (BC-3.9.010)
        let mut deleted_ids: Vec<String> = Vec::new();
        for aid in &aids {
            match client.delete_attachment(aid).await {
                Ok(()) => {
                    deleted_ids.push(aid.clone());
                }
                Err(e) => {
                    // 404 → benign skip (BC-3.9.010)
                    if e.chain()
                        .find_map(|c| c.downcast_ref::<JrError>())
                        .map(|je| matches!(je, JrError::UserError(msg) if msg.contains("not found or already deleted")))
                        .unwrap_or(false)
                    {
                        // benign 404 skip — continue
                    } else {
                        // Non-404 error → abort sequence (BC-3.9.010 EC-3.9.010-4)
                        return Err(e);
                    }
                }
            }
        }

        let count = deleted_ids.len();
        if is_json {
            let payload = serde_json::json!({
                "count": count,
                "deleted": count > 0,
                "ids": deleted_ids
            });
            println!("{}", output::render_json(&payload)?);
        } else {
            if count == 0 {
                eprintln!("No attachments deleted (all were already removed or not found).");
            } else {
                for id in &deleted_ids {
                    eprintln!("Deleted attachment {id}.");
                }
            }
        }
        return Ok(());
    }

    // -------------------------------------------------------------------
    // Path B: --issue KEY --older-than DURATION
    // -------------------------------------------------------------------
    let issue_key = issue.expect("clap ensures issue is Some when aids is empty");
    let age_str = older_than.expect("clap ensures older_than is Some when issue is set");

    // Parse duration (BC-3.9.019 EC-3.9.019-3)
    let duration = parse_age_duration(&age_str)?;

    // --yes required on bulk paths (BC-3.9.016 EC-3.9.016-1); dry-run exempts
    if !yes && !dry_run {
        return Err(JrError::UserError(
            "--older-than requires --yes to confirm bulk deletion.".to_string(),
        )
        .into());
    }

    // Fetch attachment list
    let attachments = client.list_attachments(&issue_key).await?;

    // Apply age filter — belt+braces: checked_sub_signed guards against any future
    // duration magnitude regression that bypasses parse_age_duration's Layer 3 clamp
    // (P6-001: Utc::now() - duration panics when result precedes NaiveDate::MIN).
    let cutoff = chrono::Utc::now()
        .checked_sub_signed(duration)
        .ok_or_else(|| {
            JrError::UserError(format!(
                "invalid duration: '{age_str}'. Use formats like 30m, 2h, 1d, 7d, 2w."
            ))
        })?;
    let selected = filter_attachments_older_than(attachments, cutoff);

    if dry_run {
        // Dry-run: emit would-delete manifest (no DELETEs)
        let ids: Vec<&str> = selected.iter().map(|a| a.id.as_str()).collect();
        let att_objs: Vec<serde_json::Value> = selected
            .iter()
            .map(|a| {
                serde_json::json!({
                    "filename": a.filename,
                    "id": a.id
                })
            })
            .collect();
        if is_json {
            let payload = serde_json::json!({
                "attachments": att_objs,
                "dryRun": true,
                "ids": ids
            });
            println!("{}", output::render_json(&payload)?);
        } else {
            let n = selected.len();
            if n == 0 {
                eprintln!("No attachments older than {age_str} found on {issue_key}.");
            } else {
                // AC-009 human table [ID, Filename (CWE-116), Size, Created]
                let table_rows: Vec<Vec<String>> = selected
                    .iter()
                    .map(|a| {
                        vec![
                            a.id.clone(),
                            display_sanitize_filename(&a.filename),
                            format_size(a.size),
                            a.created.clone(),
                        ]
                    })
                    .collect();
                eprintln!(
                    "{}",
                    output::render_table(&["ID", "Filename", "Size", "Created"], &table_rows)
                );
                eprintln!("{n} attachment(s) would be deleted. Run without --dry-run to confirm.");
            }
        }
        return Ok(());
    }

    if selected.is_empty() {
        if !is_json {
            eprintln!("No attachments older than {age_str} found on {issue_key}.");
        } else {
            let payload = serde_json::json!({"count": 0, "deleted": false, "ids": []});
            println!("{}", output::render_json(&payload)?);
        }
        return Ok(());
    }

    // Human mode: pre-deletion hint
    if !is_json {
        eprintln!(
            "Deleting {} attachment(s) older than {} from {}.",
            selected.len(),
            age_str,
            issue_key
        );
    }

    // Sequential deletes — 404 is benign skip (BC-3.9.010)
    let mut deleted_ids: Vec<String> = Vec::new();
    for att in &selected {
        match client.delete_attachment(&att.id).await {
            Ok(()) => {
                deleted_ids.push(att.id.clone());
            }
            Err(e) => {
                if e.chain()
                    .find_map(|c| c.downcast_ref::<JrError>())
                    .map(|je| matches!(je, JrError::UserError(msg) if msg.contains("not found or already deleted")))
                    .unwrap_or(false)
                {
                    // benign 404 skip
                } else {
                    return Err(e);
                }
            }
        }
    }

    let count = deleted_ids.len();
    if is_json {
        let payload = serde_json::json!({
            "count": count,
            "deleted": count > 0,
            "ids": deleted_ids
        });
        println!("{}", output::render_json(&payload)?);
    } else {
        eprintln!(
            "Deleted {} attachment(s) older than {} from {}.",
            count, age_str, issue_key
        );
    }
    Ok(())
}

/// Single-AID confirmation gate (BC-3.9.015 step 2; VP-576-002; DEC-174).
///
/// Uses `eprint!` (NOT `eprintln!`, NOT `dialoguer`) + `io::stdin().read_line`.
///
/// Three-way branch (EC-3.9.015):
///   - `"y"`/`"yes"` (case-insensitive, after trim) → `Ok(true)` → proceed with DELETE.
///   - Other non-empty text / empty Enter (`Ok(n ≥ 1)`, buffer `"\n"`) → `Ok(false)` →
///     caller emits `"Deletion cancelled."` to stderr + exits 0.
///   - EOF (`read_line` returns `Ok(0)`) / `Err(_)` → `Err(JrError::Interrupted)` → exit 130.
///
/// Pre-conditions (caller responsibility):
///   - AID `^[0-9]+$` validation must fire BEFORE this fn.
///   - `no_input && !yes` must exit 64 BEFORE this fn is called.
///   - `yes == true` must bypass this fn entirely.
///
/// `filename` is already display-sanitized by the caller (SEC-576-011 / CWE-116).
fn attachment_delete_confirmation_gate(filename: &str, aid: &str) -> anyhow::Result<bool> {
    eprint!("Delete attachment {filename} ({aid})? [y/N] ");
    let _ = std::io::Write::flush(&mut std::io::stderr());

    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => Err(JrError::Interrupted.into()),
        Ok(_) => {
            let trimmed = line.trim();
            Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
        }
    }
}

/// Filter `attachments` to those whose `created` timestamp is older than `cutoff`
/// (BC-3.9.019; EC-3.9.019-8).
///
/// Pure function — no I/O. Unparseable `created` fields are silently skipped with
/// a stderr warning; they do NOT abort the call.
/// `created` is an ISO 8601 string; parsed via `chrono`.
fn filter_attachments_older_than(
    attachments: Vec<AttachmentObject>,
    cutoff: chrono::DateTime<chrono::Utc>,
) -> Vec<AttachmentObject> {
    attachments
        .into_iter()
        .filter(|att| {
            match att.created.parse::<chrono::DateTime<chrono::FixedOffset>>() {
                Ok(created_dt) => {
                    let created_utc: chrono::DateTime<chrono::Utc> = created_dt.into();
                    created_utc < cutoff
                }
                Err(_) => {
                    eprintln!(
                        "warning: could not parse created timestamp {:?} for attachment {}; skipping",
                        att.created, att.id
                    );
                    false
                }
            }
        })
        .collect()
}

/// Parse an age-duration string into a `chrono::Duration` (BC-3.9.019; EC-3.9.019-3/8).
///
/// Supported suffixes:
///   `m` = minutes, `h` = hours, `d` = 24 clock-hours (NOT Jira's 8-hour workday),
///   `w` = 7 × 24 clock-hours.
///
/// **EC-3.9.019-8 BOUNDARY PIN:** `parse_age_duration("1d")` MUST equal
/// `chrono::Duration::hours(24)`. A worklog-style `1d = 8h` is WRONG here.
///
/// Invalid / malformed input → `JrError::UserError` with EC-3.9.019-3 canonical message:
///   `"invalid duration: '<VALUE>'. Use formats like 30m, 2h, 1d, 7d, 2w."`
///
/// MUST NOT import or call `src/duration.rs` arithmetic — that module is read for
/// suffix-convention style only.
fn parse_age_duration(s: &str) -> anyhow::Result<chrono::Duration> {
    let canonical_error = || {
        JrError::UserError(format!(
            "invalid duration: '{s}'. Use formats like 30m, 2h, 1d, 7d, 2w."
        ))
    };

    if s.is_empty() {
        return Err(canonical_error().into());
    }

    // Char-aware split: `split_at(len-1)` panics on multi-byte trailing chars
    // (e.g. "5€" — '€' is 3 UTF-8 bytes). Use `chars().next_back()` instead.
    // `next_back()` is always Some here — the is_empty guard above already returned.
    let Some(last_char) = s.chars().next_back() else {
        return Err(canonical_error().into());
    };
    let suffix = &s[s.len() - last_char.len_utf8()..];
    let digits = &s[..s.len() - last_char.len_utf8()];

    let n: i64 = digits.parse().map_err(|_| canonical_error())?;
    if n <= 0 {
        return Err(canonical_error().into());
    }

    // Three-layer overflow guard (P1-001 + P2-001 + P6-001):
    // Layer 1 (P1-001): checked_mul prevents i64 multiplication overflow on `n * factor`.
    // Layer 2 (P2-001): chrono::Duration::try_seconds handles the TimeDelta "panic band"
    //   ~(i64::MAX/1000, i64::MAX] where Duration::seconds(s) multiplies by MILLIS_PER_SEC
    //   (1000), overflowing i64 and panicking (confirmed chrono 0.4.45 src/lib.rs:717).
    //   try_seconds returns None for out-of-bounds values.
    // Layer 3 (P6-001): MAX_AGE_SECS magnitude clamp rejects durations that pass
    //   try_seconds but would panic at `Utc::now() - duration` because the resulting
    //   DateTime falls before chrono::NaiveDate::MIN (~year -262143, ≈-8.34e12 s from
    //   epoch). 8_000_000_000_000 s (≈253,400 years) leaves a 340e9-second safety margin
    //   and is far below the try_seconds panic band (~9.2e15 s).
    const MAX_AGE_SECS: i64 = 8_000_000_000_000;

    let total_secs: i64 = match suffix {
        "m" => n.checked_mul(60),
        "h" => n.checked_mul(3_600),
        "d" => n.checked_mul(24 * 3_600),
        "w" => n.checked_mul(7 * 24 * 3_600),
        _ => return Err(canonical_error().into()),
    }
    .ok_or_else(canonical_error)?; // Layer 1: checked_mul overflow → canonical error

    if total_secs > MAX_AGE_SECS {
        return Err(canonical_error().into()); // Layer 3: DateTime-subtraction band
    }

    chrono::Duration::try_seconds(total_secs) // Layer 2: TimeDelta panic band
        .ok_or_else(|| canonical_error().into())
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
            mime_type: Some("image/png".into()),
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
    fn test_serialize_attachment_curated_null_mimetype_when_none() {
        // P5-002a: sparse AttachmentObject (mime_type: None) →
        // "mimeType" key MUST be present with value null (not absent).
        // BTreeMap order intact: mimeType sorts between "id" and "size".
        let mut a = make_test_attachment();
        a.mime_type = None;
        let v = serialize_attachment_curated(&a);
        let obj = v.as_object().unwrap();
        assert!(
            obj.contains_key("mimeType"),
            "mimeType key must be present even when None (sparse tolerance)"
        );
        assert_eq!(
            obj["mimeType"],
            Value::Null,
            "mimeType must be null when None, not a string"
        );
        // BTreeMap key-order check: keys are sorted, mimeType between id and size.
        let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        let id_pos = keys.iter().position(|k| *k == "id").unwrap();
        let mime_pos = keys.iter().position(|k| *k == "mimeType").unwrap();
        let size_pos = keys.iter().position(|k| *k == "size").unwrap();
        assert!(
            id_pos < mime_pos && mime_pos < size_pos,
            "BTreeMap key order must be … id … mimeType … size …; got: {keys:?}"
        );
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
    // sanitize_attachment_filename — BC-2.7.011 unit tests (S-576-2)
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
        assert_eq!(sanitize_attachment_filename("CON"), Some("CON".to_string()));
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_device_nul_passes_through() {
        // NUL as a FILENAME (no NUL BYTE in string) → Some("NUL") — distinct from \0
        assert_eq!(sanitize_attachment_filename("NUL"), Some("NUL".to_string()));
    }

    #[test]
    fn test_sanitize_attachment_filename_windows_device_com1_passes_through() {
        assert_eq!(
            sanitize_attachment_filename("COM1"),
            Some("COM1".to_string())
        );
    }

    #[test]
    fn test_is_windows_device_name_basename_com0_not_device_name() {
        // AC-016 / O-3: COM0 is NOT a Windows device name; range is COM1–COM9 only.
        // is_windows_device_name_basename("COM0") must return false so "COM0.txt"
        // is NOT escaped with a leading underscore at the single-id call site.
        assert!(!is_windows_device_name_basename("COM0"));
        assert!(!is_windows_device_name_basename("LPT0"));
        assert!(!is_windows_device_name_basename("COM0.txt"));
        assert!(!is_windows_device_name_basename("LPT0.txt"));
        // Verify COM1–COM9 / LPT1–LPT9 still detected.
        assert!(is_windows_device_name_basename("COM1"));
        assert!(is_windows_device_name_basename("COM9"));
        assert!(is_windows_device_name_basename("LPT1"));
        assert!(is_windows_device_name_basename("LPT9"));
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

    // ---------------------------------------------------------------------------
    // Mutation-kill unit tests (PG-F4-10)
    // ---------------------------------------------------------------------------

    /// Mutant 2 — 397:27 `&&` → `||` in `is_windows_device_name_basename`.
    ///
    /// Under the mutant: `s.len() == 4 || (s.starts_with("COM") || s.starts_with("LPT"))`
    /// A 5-char COM/LPT prefix (e.g. "COM1x") satisfies `starts_with("COM")` even though
    /// `len() != 4`, so the arm fires and `s.as_bytes()[3] = b'1'` passes the `b'1'..=b'9'`
    /// range check → returns `true` (wrong).
    /// Original: `len() == 4 && ...` = `false && true` = `false` → `_ => false`.
    #[test]
    fn test_is_windows_device_name_basename_len5_com_prefix_not_device() {
        // len=5 COM/LPT prefix → NOT a Windows device name.
        assert!(!is_windows_device_name_basename("COM1x.txt"));
        assert!(!is_windows_device_name_basename("LPT1x.txt"));
        // Sanity: len=4 COM1/LPT1 are still device names.
        assert!(is_windows_device_name_basename("COM1"));
        assert!(is_windows_device_name_basename("LPT1"));
    }

    /// Mutants 3+4 — 424:13 `-=` → `/=` and `-=` → `+=` in `floor_char_boundary_at`.
    ///
    /// "a🎉b": "a" (1 byte), 🎉 U+1F389 (4 bytes at indices 1–4), "b" (1 byte) = 6 bytes.
    /// Char boundaries: 0, 1, 5, 6.  Indices 2, 3, 4 are INSIDE the emoji.
    ///
    /// limit=4: pos=4 → NOT a boundary → walk back:
    ///   `-=1` (original): 4→3→2→1; is_char_boundary(1)=true → return 1.
    ///   `/=1` (mutant):   pos stays 4 forever → infinite loop → test times out → killed.
    ///   `+=1` (mutant):   pos=4→5; is_char_boundary(5)=true → return 5 ≠ 1 → assertion fails → killed.
    #[test]
    fn test_floor_char_boundary_at_limit_inside_multibyte_char() {
        // "a" + 🎉 (4 bytes) + "b": limit 4 is the last byte of 🎉 → walk back to start of 🎉 (index 1).
        assert_eq!(floor_char_boundary_at("a\u{1F389}b", 4), 1);
        // Indices 2 and 3 are also inside the emoji → also return 1.
        assert_eq!(floor_char_boundary_at("a\u{1F389}b", 3), 1);
        assert_eq!(floor_char_boundary_at("a\u{1F389}b", 2), 1);
    }

    /// Mutant 7 — 494:38 second `||` → `&&` in `sanitize_attachment_filename` step 4.
    ///
    /// Under the mutant: `c == '/' || (c == '\\' && c == ':')` — a bare `\` is no longer
    /// replaced because `'\\' && ':'` is always false.
    ///
    /// Platform behaviour differs for `"\\\\"` (two backslash chars):
    ///
    /// **Unix** (mutation job platform — kills the mutant):
    ///   Step 1: `Path::file_name()` returns the whole string (no `/` separator on Unix).
    ///   Backslash split: `rsplit('\\')` = `["", "", ""]`; no non-empty segment →
    ///   `unwrap_or("\\\\")`; basename = `"\\\\"`.
    ///   Step 4 (original): each `\` → `_` → `Some("__")`.
    ///   Step 4 (mutant):  `\` alone fails `\\ && :` → stays as `\\` → `Some("\\\\")` ≠ `Some("__")`.
    ///
    /// **Windows** (CORRECT — NOT a bug):
    ///   `\\` is a path separator on Windows; `Path::new("\\\\")` is the UNC-root path.
    ///   `Path::file_name()` returns `None` → `and_then(|f| f.to_str())?` returns `None`
    ///   from the function → `sanitize_attachment_filename("\\\\")` returns `None`.
    ///   Both original and mutant code reach the same `None` result (step 1 short-circuits
    ///   before step 4), so the test assertion is `None` on Windows.
    #[test]
    fn test_sanitize_attachment_filename_pure_backslash_scrubbed_to_underscores() {
        #[cfg(not(windows))]
        assert_eq!(sanitize_attachment_filename("\\\\"), Some("__".to_string()));
        #[cfg(windows)]
        assert_eq!(sanitize_attachment_filename("\\\\"), None);
    }

    // -----------------------------------------------------------------------
    // S-576-4: parse_age_duration unit tests
    // -----------------------------------------------------------------------

    /// EC-3.9.019-8 boundary pin (AC-007 / S-576-4).
    ///
    /// `parse_age_duration("1d")` MUST equal `chrono::Duration::hours(24)`.
    /// A worklog-style `1d = 8h` computation is WRONG for this function.
    ///
    /// Private helper — integration tests cannot call it; unit test lives here.
    #[test]
    fn test_bc_3_9_019_ec_8_parse_age_duration_1d_is_24h() {
        let result = parse_age_duration("1d").expect("parse_age_duration(\"1d\") must return Ok");
        assert_eq!(
            result,
            chrono::Duration::hours(24),
            "EC-3.9.019-8: parse_age_duration(\"1d\") must equal chrono::Duration::hours(24) \
             (24 clock-hours, NOT 8h worklog-day)"
        );
    }

    /// Mutation pre-empt: kills the `7` multiplier mutant and the `24` multiplier mutant.
    /// If either is replaced by 1, this test fails.
    #[test]
    fn test_bc_3_9_019_2w_equals_336_hours() {
        let result = parse_age_duration("2w").expect("parse_age_duration(\"2w\") must return Ok");
        assert_eq!(
            result,
            chrono::Duration::hours(2 * 7 * 24),
            "parse_age_duration(\"2w\") must equal 2*7*24 = 336 clock-hours; \
             kills week→day multiplier mutant and day→hour multiplier mutant"
        );
    }

    /// Mutation pre-empt: `"0d"` must return Err (zero duration is not a useful filter).
    /// Kills the `> 0` → `>= 0` boundary mutant.
    #[test]
    fn test_bc_3_9_019_0d_is_err() {
        let result = parse_age_duration("0d");
        assert!(
            result.is_err(),
            "parse_age_duration(\"0d\") must return Err; \
             kills <=→< boundary mutant on the zero-value guard"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid duration"),
            "parse_age_duration(\"0d\") error must contain 'invalid duration'; got: {msg}"
        );
    }

    /// Mutation pre-empt: `"30m"` must parse as exactly 30 minutes.
    #[test]
    fn test_bc_3_9_019_30m_exact() {
        let result = parse_age_duration("30m").expect("parse_age_duration(\"30m\") must return Ok");
        assert_eq!(
            result,
            chrono::Duration::minutes(30),
            "parse_age_duration(\"30m\") must equal chrono::Duration::minutes(30)"
        );
    }

    /// Mutation pre-empt: `"2h"` must parse as exactly 2 hours.
    #[test]
    fn test_bc_3_9_019_2h_exact() {
        let result = parse_age_duration("2h").expect("parse_age_duration(\"2h\") must return Ok");
        assert_eq!(
            result,
            chrono::Duration::hours(2),
            "parse_age_duration(\"2h\") must equal chrono::Duration::hours(2)"
        );
    }

    /// P2-001 unit pin: n=1e12 days → 8.64e16 seconds, inside the chrono panic band
    /// ~(i64::MAX/1000, i64::MAX]. Duration::hours(n*24) calls Duration::seconds(8.64e16)
    /// which internally multiplies by MILLIS_PER_SEC=1000 → 8.64e19 overflows i64 →
    /// checked_mul panics. Implementation must use try_hours/try_seconds and map None→Err.
    #[test]
    fn test_bc_3_9_019_p2_001_chrono_band_1e12d_is_err() {
        let result = parse_age_duration("1000000000000d");
        assert!(
            result.is_err(),
            "parse_age_duration(\"1000000000000d\") must return Err (chrono out-of-bounds); \
             P2-001: must use try_hours/try_seconds to catch the Duration panic band"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid duration"),
            "P2-001: error must contain 'invalid duration'; got: {msg}"
        );
    }

    /// P6-001 unit pin: n=1e11 days → 8.64e15 seconds.
    /// try_seconds(8.64e15) succeeds (8.64e15 * MILLIS_PER_SEC = 8.64e18 < i64::MAX 9.22e18),
    /// but `Utc::now() - duration` panics because the resulting date (~274M years BC) is
    /// before chrono::NaiveDate::MIN (~year -262144).
    ///
    /// If the fix clamps inside parse_age_duration (additional bound check before
    /// try_seconds), this unit test is the discriminator.
    /// If the fix lands at the subtraction site (checked_sub_signed), this test remains RED
    /// and the integration test (attachment_delete.rs P6-001 sub-case) is the sole pin.
    ///
    /// Either way, pinning this behavior here ensures the full rejection band is covered
    /// at the unit level regardless of the implementation approach chosen.
    #[test]
    fn test_bc_3_9_019_p6_001_datetime_band_1e11d_is_err() {
        let result = parse_age_duration("100000000000d");
        assert!(
            result.is_err(),
            "parse_age_duration(\"100000000000d\") must return Err; \
             P6-001: n=1e11 days → 8.64e15 s passes try_seconds but \
             Utc::now()-duration panics at the DateTime subtraction site"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid duration"),
            "P6-001: error must contain 'invalid duration'; got: {msg}"
        );
    }

    // ---------------------------------------------------------------------------
    // batch_path_is_within_dir — containment-rejection unit tests (F5-R1-001 / N2)
    // ---------------------------------------------------------------------------

    /// Containment check must return `Ok(true)` when the output path is a direct child
    /// of the base directory (the normal, always-true case for current
    /// `compute_default_output_path` output).
    ///
    /// This is the positive (non-rejection) arm — confirms the helper does not
    /// false-positive on safe paths.
    #[test]
    fn test_batch_path_is_within_dir_accepts_child_path() {
        let base = tempfile::tempdir().expect("tempdir");
        let resolved = base.path().canonicalize().expect("canonicalize base");

        // A plain filename inside the base dir — the current code's only output shape.
        let final_path = resolved.join("abc123_attachment.pdf");

        let result = batch_path_is_within_dir(&final_path, &resolved);
        assert!(
            matches!(result, Ok(true)),
            "batch_path_is_within_dir must return Ok(true) for a direct child of base_dir; \
             got: {result:?}"
        );
    }

    /// Containment check must return `Ok(false)` when the parent of `final_path` is
    /// OUTSIDE `resolved_dir`.  This is the rejection branch introduced by F5-R1-001.
    ///
    /// **Red-when-reverted rationale:** if `batch_path_is_within_dir` is removed (or
    /// its body is replaced with `return Ok(true);`), this test will either fail to
    /// compile (missing function) or fail the `Ok(false)` assertion — either outcome
    /// ensures that reverting the F5-R1-001 repair causes a test failure.
    #[test]
    fn test_batch_path_is_within_dir_rejects_path_outside_base() {
        let base = tempfile::tempdir().expect("tempdir");
        let escape = tempfile::tempdir().expect("tempdir for escape target");

        let resolved = base.path().canonicalize().expect("canonicalize base");

        // Construct a path whose parent IS the escape directory, not base_dir.
        // The escape tempdir exists on disk, so canonicalize of its path will succeed.
        let final_path = escape.path().join("should_not_land_here.txt");

        let result = batch_path_is_within_dir(&final_path, &resolved);
        assert!(
            matches!(result, Ok(false)),
            "batch_path_is_within_dir must return Ok(false) when parent is outside base_dir; \
             got: {result:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // F5-R3-002: batch_path_is_within_dir must accept a non-canonical base dir
    // ---------------------------------------------------------------------------

    /// F5-R3-002: `batch_path_is_within_dir` must return `Ok(true)` when
    /// `resolved_dir` is NON-canonical (contains `..` segments) but `final_path`
    /// is genuinely inside the base directory.
    ///
    /// **Reachability note:** the `Ok(false)` mass-rejection scenario (all files
    /// skipped) is UNREACHABLE via `handle_batch_download`.  In that caller,
    /// `resolved_dir` is always `base_dir.canonicalize().unwrap_or(base_dir)` —
    /// a canonical path when the directory exists — and `final_path.parent()` is
    /// always `base_dir`, so both `canonicalize()` calls inside
    /// `batch_path_is_within_dir` succeed and agree.  A non-canonical
    /// `resolved_dir` cannot arise there in practice; if either `canonicalize()`
    /// fails, the helper returns `Err` and the call site's warn-and-skip path
    /// fires (fail-open).
    ///
    /// This test exercises the **standalone helper contract** via direct unit
    /// call — hardening for hypothetical future callers (e.g., if
    /// `compute_default_output_path` gains sub-directory logic) that might
    /// supply a non-canonical base path.
    #[test]
    fn test_f5_r3_002_batch_path_is_within_dir_accepts_non_canonical_base() {
        let base = tempfile::tempdir().expect("create tempdir");
        // Canonicalize to get the true filesystem path (resolves symlinks such
        // as macOS /var → /private/var).
        let canonical_base = base
            .path()
            .canonicalize()
            .expect("canonicalize base tempdir");

        // Build a NON-canonical path that refers to the SAME directory by
        // appending `../<basename>`.  For example:
        //   canonical:     /private/var/folders/…/T/tmp_abc
        //   non-canonical: /private/var/folders/…/T/tmp_abc/../tmp_abc
        //
        // `Path::starts_with` is component-based and does NOT normalize `..`,
        // so `canonical_parent.starts_with(non_canonical)` returns `false`
        // even though the directories are identical — this is the defect.
        let basename = canonical_base
            .file_name()
            .expect("canonical_base has a filename");
        let non_canonical_base = canonical_base.join("..").join(basename);

        // final_path is a genuine child of the base dir.
        let final_path = canonical_base.join("safe_attachment_f5r3002.bin");

        let result = batch_path_is_within_dir(&final_path, &non_canonical_base);

        // The F5-R3-002 fix canonicalizes resolved_dir inside
        // batch_path_is_within_dir before the starts_with check, so
        // canonical_parent (/tmp/…/tmp_abc) correctly starts_with the
        // canonicalized resolved_dir even when the input was the
        // non-canonical path (/tmp/…/tmp_abc/../tmp_abc).
        assert!(
            matches!(result, Ok(true)),
            "F5-R3-002 RED: batch_path_is_within_dir must accept a genuine child \
             even when resolved_dir is non-canonical; got: {result:?}\n\
             final_path:    {}\n\
             resolved_dir:  {}",
            final_path.display(),
            non_canonical_base.display()
        );
    }

    // ===========================================================================
    // BC-2.7.012 v1.3.103+ — classify_write_error pure classifier unit tests
    // GREEN — implemented in FIX-F5-010 / PR #649
    //
    // `classify_write_error` is implemented in this file.  These tests pin the
    // three classifier branches.  The PermissionDenied / ReadOnlyFilesystem
    // branch uses the v1.3.103 shape that adds the `(writing <dest>)` parenthetical
    // after `<dir>` — see inline `// BC-2.7.012 v1.3.103` annotations below.
    //
    // Function contract (BC-2.7.012, research doc f5-r5-001):
    //   fn classify_write_error(
    //       kind: std::io::ErrorKind,
    //       dest_display: &str,   // final destination path (display-safe)
    //       dir_display: &str,    // final_path.parent() verbatim
    //       os_err: &str,         // std::io::Error::Display of the raw error
    //   ) -> String
    //
    // Branch mapping (non-exhaustive `_ =>` arm REQUIRED — ErrorKind is
    // `#[non_exhaustive]` and must NOT be exhaustively matched):
    //   StorageFull | QuotaExceeded  →  "Disk full: not enough space to write <dest>: <os_err>. Free up disk space and try again."
    //   PermissionDenied | ReadOnlyFilesystem  →  "Permission denied: cannot write to <dir> (writing <dest>): <os_err>. Check directory permissions and try again."
    //   _ (fallback)  →  "Failed to write <dest>: <os_err>."
    // ===========================================================================

    #[test]
    fn test_bc_2_7_012_classify_storage_full_disk_full_prefix() {
        use std::io::ErrorKind;
        let os_err = std::io::Error::from(ErrorKind::StorageFull).to_string();
        let msg = classify_write_error(
            ErrorKind::StorageFull,
            "/output/report.pdf",
            "/output",
            &os_err,
        );
        assert!(
            msg.starts_with("Disk full: not enough space to write /output/report.pdf:"),
            "StorageFull must produce \
             'Disk full: not enough space to write <dest>:' prefix; got: {msg}"
        );
        assert!(
            msg.contains("Free up disk space and try again."),
            "StorageFull must include remediation hint; got: {msg}"
        );
        assert!(
            msg.contains(&os_err),
            "StorageFull must include the OS error string '{os_err}'; got: {msg}"
        );
    }

    #[test]
    fn test_bc_2_7_012_classify_quota_exceeded_disk_full_prefix() {
        use std::io::ErrorKind;
        let os_err = std::io::Error::from(ErrorKind::QuotaExceeded).to_string();
        let msg = classify_write_error(
            ErrorKind::QuotaExceeded,
            "/output/report.pdf",
            "/output",
            &os_err,
        );
        assert!(
            msg.starts_with("Disk full: not enough space to write /output/report.pdf:"),
            "QuotaExceeded must produce \
             'Disk full: not enough space to write <dest>:' prefix; got: {msg}"
        );
        assert!(
            msg.contains("Free up disk space and try again."),
            "QuotaExceeded must include remediation hint; got: {msg}"
        );
        assert!(
            msg.contains(&os_err),
            "QuotaExceeded must include the OS error string '{os_err}'; got: {msg}"
        );
    }

    #[test]
    fn test_bc_2_7_012_classify_permission_denied_perm_prefix() {
        use std::io::ErrorKind;
        let os_err = std::io::Error::from(ErrorKind::PermissionDenied).to_string();
        let msg = classify_write_error(
            ErrorKind::PermissionDenied,
            "/output/report.pdf",
            "/output",
            &os_err,
        );
        // BC-2.7.012 v1.3.103: parenthetical `(writing <dest>)` added after <dir>.
        assert!(
            msg.starts_with("Permission denied: cannot write to /output (writing"),
            "PermissionDenied must produce \
             'Permission denied: cannot write to <dir> (writing' prefix; got: {msg}"
        );
        assert!(
            msg.contains("Check directory permissions and try again."),
            "PermissionDenied must include remediation hint; got: {msg}"
        );
        assert!(
            msg.contains(&os_err),
            "PermissionDenied must include the OS error string '{os_err}'; got: {msg}"
        );
        // BC-2.7.012 v1.3.103 / P9-001: dest_display must appear in message.
        assert!(
            msg.contains("/output/report.pdf"),
            "PermissionDenied must include dest_display '/output/report.pdf'; got: {msg}"
        );
    }

    #[test]
    fn test_bc_2_7_012_classify_read_only_filesystem_perm_prefix() {
        use std::io::ErrorKind;
        let os_err = std::io::Error::from(ErrorKind::ReadOnlyFilesystem).to_string();
        let msg = classify_write_error(
            ErrorKind::ReadOnlyFilesystem,
            "/output/report.pdf",
            "/output",
            &os_err,
        );
        // BC-2.7.012 v1.3.103: parenthetical `(writing <dest>)` added after <dir>.
        assert!(
            msg.starts_with("Permission denied: cannot write to /output (writing"),
            "ReadOnlyFilesystem must produce \
             'Permission denied: cannot write to <dir> (writing' prefix; got: {msg}"
        );
        assert!(
            msg.contains("Check directory permissions and try again."),
            "ReadOnlyFilesystem must include remediation hint; got: {msg}"
        );
        assert!(
            msg.contains(&os_err),
            "ReadOnlyFilesystem must include the OS error string '{os_err}'; got: {msg}"
        );
        // BC-2.7.012 v1.3.103 / P9-001: dest_display must appear in message.
        assert!(
            msg.contains("/output/report.pdf"),
            "ReadOnlyFilesystem must include dest_display '/output/report.pdf'; got: {msg}"
        );
    }

    #[test]
    fn test_bc_2_7_012_classify_generic_fallback() {
        use std::io::ErrorKind;
        let os_err = std::io::Error::from(ErrorKind::Other).to_string();
        let msg = classify_write_error(ErrorKind::Other, "/output/report.pdf", "/output", &os_err);
        assert!(
            msg.starts_with("Failed to write /output/report.pdf:"),
            "Generic fallback must produce 'Failed to write <dest>:' prefix; got: {msg}"
        );
        assert!(
            msg.contains(&os_err),
            "Generic fallback must include the OS error string '{os_err}'; got: {msg}"
        );
        assert!(
            msg.ends_with('.'),
            "Generic fallback message must end with '.'; got: {msg}"
        );
        // Generic fallback must NOT include the discriminated remediation hints.
        assert!(
            !msg.contains("Free up disk space"),
            "Generic fallback must NOT include 'Free up disk space'; got: {msg}"
        );
        assert!(
            !msg.contains("Check directory permissions"),
            "Generic fallback must NOT include 'Check directory permissions'; got: {msg}"
        );
    }

    // FIX-F5-010: write_error_display_strings mutant-killing tests.
    //
    // Mutant 1 ("delete ! in stream_to_file", line 668): changes
    //   `.filter(|d| !d.as_os_str().is_empty())`
    //   to `.filter(|d| d.as_os_str().is_empty())`.
    //   - Normal path ("out/dir/file.txt"): non-empty parent is REJECTED by the mutated
    //     filter (is_empty() → false → filter drops it), so unwrap_or falls to "." — wrong.
    //   - Bare filename ("file.txt"): empty parent is ACCEPTED by the mutated filter
    //     (is_empty() → true → filter keeps it), so .map returns "" — wrong.
    //
    // Mutant 2 ("replace match guard !d.as_os_str().is_empty() with true", line 672):
    //   changes `Some(d) if !d.as_os_str().is_empty()` to `Some(d) if true`.
    //   - Bare filename: Some("") now matches first arm, dest becomes "/" + fname — wrong.
    //   - Normal path: arm already matched correctly (non-empty parent), no observable change.
    //
    // The two tests together kill both mutants by pinning exact output for both cases.

    #[test]
    fn test_write_error_display_strings_normal_path_kills_mutant1() {
        // Non-empty parent "out/dir": the filter and guard both pass in correct code.
        // Mutant 1 (! deleted): filter rejects "out/dir" (not empty) → dir falls to "."
        // → assertion `dir == "out/dir"` fails, killing the mutant.
        let path = std::path::Path::new("out/dir/file.txt");
        let (dir, dest) = write_error_display_strings(path);
        assert_eq!(
            dir, "out/dir",
            "non-empty parent must be used verbatim as dir; mutant 1 would yield '.'"
        );
        assert!(
            dest.starts_with("out/dir"),
            "dest must start with the parent directory; got: {dest}"
        );
        assert!(
            dest.contains("file.txt"),
            "dest must contain the filename portion; got: {dest}"
        );
    }

    #[test]
    fn test_write_error_display_strings_bare_filename_kills_both_mutants() {
        // Bare filename "file.txt": Path::parent() → Some("") (empty component).
        // Correct: dir=".", dest="file.txt".
        //
        // Mutant 1 (! deleted): empty parent is ACCEPTED by mutated filter
        //   (d.as_os_str().is_empty() → true) → .map returns "" → dir="" ≠ "." → caught.
        //
        // Mutant 2 (guard replaced with true): Some("") matches first arm →
        //   dest = format!("{}{}file.txt", "", MAIN_SEPARATOR) = "/file.txt" on Unix,
        //   "\file.txt" on Windows — either way dest ≠ "file.txt" → caught.
        let path = std::path::Path::new("file.txt");
        let (dir, dest) = write_error_display_strings(path);
        assert_eq!(
            dir, ".",
            "bare filename must fall back to '.'; mutant 1 (! deleted) would yield ''"
        );
        assert_eq!(
            dest, "file.txt",
            "bare filename dest must be just the filename with no leading separator; \
             mutant 2 (guard=true) would prepend the path separator"
        );
    }
}
