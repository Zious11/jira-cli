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
fn compute_default_output_path(
    base_dir: &std::path::Path,
    attachment_id: &str,
    filename: &str,
    is_batch: bool,
) -> std::path::PathBuf {
    let sanitized =
        sanitize_attachment_filename(filename).unwrap_or_else(|| attachment_id.to_string());

    if is_batch {
        let hash = sha1_hex(attachment_id);
        base_dir.join(format!("{hash}_{sanitized}"))
    } else {
        base_dir.join(sanitized)
    }
}

// ---------------------------------------------------------------------------
// S-576-2: private async helpers
// ---------------------------------------------------------------------------

/// Stream `response` body to `final_path` using an atomic temp-file + rename pattern.
///
/// Temp file: `tmp_<16 random hex digits>` in the SAME directory as `final_path`
/// (same device → `rename` is atomic on POSIX; BC-2.7.007 ~749).
///
/// Returns the number of bytes actually written (P31-002: used in the manifest).
///
/// On ANY error after temp-file creation, the temp file is deleted before returning
/// the error (cleanup guarantee for BC-2.7.007 EC-2.7.007-4).
async fn stream_to_file(
    response: reqwest::Response,
    final_path: &std::path::Path,
) -> anyhow::Result<u64> {
    let parent = final_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output path has no parent directory"))?;

    let token: u64 = rand::random();
    let tmp_path = parent.join(format!("tmp_{token:016x}"));

    let result: anyhow::Result<u64> = async {
        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            anyhow::anyhow!("failed to create temp file {}: {e}", tmp_path.display())
        })?;
        let mut bytes_written: u64 = 0;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("stream error: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| anyhow::anyhow!("write error to {}: {e}", tmp_path.display()))?;
            bytes_written += chunk.len() as u64;
        }

        file.flush().await?;
        drop(file);

        tokio::fs::rename(&tmp_path, final_path)
            .await
            .map_err(|e| {
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
                anyhow::anyhow!("failed to rename temp to {display}: {e}")
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
    let metadata = client.get_attachment_metadata(id_str).await?;
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
            let parse_dt =
                |s: &str| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3f%z").ok();
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

        let final_path = compute_default_output_path(&base_dir, &att.id, &att.filename, true);

        // BC-2.7.011 defense-in-depth: unreachable via API-supplied filenames after sanitization steps 1-5.
        // Two-step containment check: canonicalize base_dir, then assert joined path starts_with it.
        if let Some(fname) = final_path.file_name() {
            if !resolved_dir.join(fname).starts_with(&resolved_dir) {
                eprintln!(
                    "warning: skipping attachment {} — path escape detected after sanitization.",
                    att.id
                );
                continue;
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
}
