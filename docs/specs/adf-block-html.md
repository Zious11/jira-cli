# `markdown_to_adf` — Block-level HTML → ADF literal text with `hardBreak` interior newlines

**Issues:** #489 (preserve vs drop), #492 (hardBreak interior newlines — Algorithm B)
**Module:** `src/adf.rs`
**Behavioral contract:** BC-7.2.011 (`.factory/specs/prd/bc-7-output-render.md`)

## Purpose

ADF has no raw-HTML node. Block-level HTML in a markdown description
(`<div>…</div>`, `<!-- comment -->`, etc.) must not be silently dropped — that
is data loss symmetric with how inline HTML is handled. Issue #489 changed
the `NodeKind::HtmlBlock` end-handler from `_ => Sink` (drop) to an active
handler that preserves the block as a `paragraph` of literal text.

Issue #492 tightened the representation: ADF text nodes must never contain a
raw `\n` character (Jira rejects them). Interior source newlines are now
represented as `hardBreak` nodes via Algorithm B, documented in BC-7.2.011.

## Algorithm B (canonical step order)

Implemented in `src/adf.rs::AdfBuilder::end` — `NodeKind::HtmlBlock` arm.

**Step 1 — Concatenate.** pulldown-cmark emits `Event::Html` once per source
line, each appended as a child text node during the `HtmlBlock` span. The
handler concatenates all child `text` fields into one string.

**Step 2 — Trim trailing newlines only.**
```
let trimmed = text.trim_end_matches(['\n', '\r']);
```
Only `\r` and `\n` are trimmed from the tail. Spaces and tabs on the trailing
line are preserved verbatim (they reach the ADF text node unchanged; the
reverse path's `trim_end()` strips them at the document level — see EC-10).

**Step 3 — Normalize then split.**
```
let normalized = trimmed.replace("\r\n", "\n").replace('\r', "\n");
let segments: Vec<&str> = normalized.split('\n').collect();
```
`\r\n` is normalized to `\n` first; then lone `\r` is normalized to `\n`.
Only then is the result split on `\n`. This order is mandatory: splitting on
a char-set `['\r', '\n']` would double-count CRLF boundaries, producing
spurious extra `hardBreak` nodes.

**Step 4 — Walk segments.**
For each segment index `i`:
- If the segment is non-empty, push a `{"type":"text","text":segment}` node.
- If `i < len - 1`, push exactly one `{"type":"hardBreak"}` node.

Empty segments (from consecutive newlines) get no `text` node but still get a
`hardBreak` for the boundary — preserving line-structure losslessly.

**Step 5 — Wrap.** Wrap the content Vec in a `{"type":"paragraph","content":…}`.

**Step 5b — Trim leading/trailing hardBreaks.**
Call `trim_leading_trailing_hardbreaks` to remove any leading or trailing
`hardBreak` nodes (these arise when the input's first or last character
is a newline — e.g., a block whose first byte is `\n` produces a leading
`hardBreak` in step 4 that step 5b removes). This is the canonical "step 5b"
referenced in BC-7.2.011, the in-code handler comment, and EC-8.

**Step 6 — Early-return if empty.**
If the content array is empty after trimming, return `EndResult::Empty`
(no paragraph emitted). `paragraph` is excluded from `is_empty_block_container`
so this explicit guard is the operative prune path.

**Step 7 — autolink pass.**
`autolink_bare_urls` runs as a post-`finish()` pass over the full built tree.
No change is needed in the handler; the URL-splitting pass operates on the
`text` nodes emitted in step 4.

## Differences from inline HTML

Three load-bearing asymmetries between block and inline HTML handling:

1. **Own paragraph wrapper.** Block HTML gets its own manufactured `paragraph`
   wrapping the text/hardBreak sequence. Inline HTML (`Event::InlineHtml`)
   flows into the enclosing paragraph directly, via the shared `push_text` arm.

2. **Trailing newline trimmed.** Block HTML: trailing `\r`/`\n` stripped in
   step 2. Inline HTML: no trailing trim (it is embedded mid-paragraph).

3. **No active marks.** The mark stack is always empty when a `HtmlBlock` end
   fires (block HTML never appears inside a span like `**…**`). Inline HTML
   inherits the current mark stack (e.g., inline HTML inside `**bold**` is
   bolded). The handler does NOT route through `push_text` (which applies
   active marks and would be incorrect here even if the stack happened to be
   empty).

## autolink interaction (issue #473)

The post-`finish()` `autolink_bare_urls` pass walks the entire built tree,
including paragraphs produced by the block-HTML handler. Any `text` node
inside a block-HTML paragraph that contains a bare `https://` or `http://`
URL at a valid boundary (preceded by whitespace, `*`, `_`, `~`, `(`, or
start-of-node) receives a `link` mark.

Bare URLs embedded in HTML attribute form (`href="https://…"`) are NOT
autolinked: the `"` character before the URL is not in the valid-boundary set,
so the URL is not recognized as a bare link target. This is deliberate and
matches the `#473` boundary rules (bias toward fewer false positives).

After the autolink pass, a URL-containing text node is split: the pre-URL
text, the URL (with `link` mark), and the post-URL text become three separate
nodes. The `adf_to_text` reverse path renders a `link`-marked node as
`[url](url)`, so the round-trip for URL-bearing content is NOT byte-identical.

## Round-trip via `adf_to_text`

`adf_to_text` renders `hardBreak` nodes as `\n` (the standard ADF hard-break
rendering). A `paragraph` in `adf_to_text` appends its content and a trailing
`\n`; the document-level `AdfRenderer::finish()` calls `.trim_end()` on the
final buffer, stripping trailing whitespace including any trailing `\n`.

A round-trip is **byte-identical to the handler input** only when ALL five
conditions hold (forward-path losses are cases 1–3 and 5; the only reverse-path loss is case 4 (EC-10)):

1. **LF-only** — no `\r`; a `\r\n` or lone `\r` is normalized to `\n` by
   step 3 (EC-1, forward).
2. **No leading newline** — a leading `\n` produces a leading `hardBreak` that
   step 5b trims, so it is lost (EC-8, forward).
3. **No trailing newline(s)** — trailing `\r`/`\n` are stripped by step 2 and
   not reconstructed on round-trip (EC-2, forward).
4. **Final line does not end in non-newline whitespace** — trailing spaces/tabs
   on the last line are preserved in the forward ADF node (step 2 does not trim
   them), but stripped on the reverse path by `AdfRenderer::finish().trim_end()`
   (EC-10, reverse).
5. **No bare URL at an autolink boundary** — such a URL is rewritten to
   `[url](url)` by the `#473` autolink pass and the round-trip renders that
   form, not the original bare URL (EC-4, forward/post-pass).

Example of a byte-identical round-trip (all five conditions met):
```
"<div>\n  <span>a</span>\n</div>"
  → [text("<div>"), hb, text("  <span>a</span>"), hb, text("</div>")]
  → "<div>\n  <span>a</span>\n</div>"
```

## Edge cases (EC-1..EC-10)

| EC | Input characteristic | Result |
|----|----------------------|--------|
| EC-1 | CRLF interior (`<div>\r\n  x\r\n</div>`) | Step 3 normalizes `\r\n`→`\n`; no `\r` survives into any text node; same 3-segment output as LF-only. Round-trip is LF-only (CRLF lost). | Test: `test_block_html_crlf_interior_no_dangling_cr` |
| EC-2 | Trailing newlines | Step 2 trims them; they do not appear in the output and are not reconstructed. | Implicit in all `markdown_to_adf` tests |
| EC-3 | Comment-only block (`<!-- x -->`) | Single line, no interior newlines — single `text` node, no `hardBreak`. Visible literal text in output (no special treatment). DOCUMENT-AS-IS. | Test: `test_block_html_comment_only_behavior` |
| EC-4 | Bare URL at valid boundary (`<div>see https://…</div>`) | Gets `link` mark from autolink pass. Href-attribute form (`href="https://…"`) is NOT autolinked (boundary rule). Round-trip for URL content renders as `[url](url)`. URL on an **interior line** of a multi-line block: autolink splits the middle text node into `[pre, link, post]`; flanking `hardBreak` nodes survive at their original positions (F-P1-002). | Tests: `test_block_html_bare_url_gets_link_mark`, `test_block_html_interior_line_url_split_preserves_hardbreaks` |
| EC-5 | Single-line block (`<div>x</div>`) | One segment after trim+split → one `text` node, no `hardBreak`. | Test: `test_convert_block_html_is_preserved_as_literal_text`, `test_block_html_round_trips_through_adf_to_text` |
| EC-6 | Consecutive blank lines (`<div>\n\na\n</div>`, handler-level) | 4 segments, 3 boundaries → `[text("<div>"), hb, hb, text("a"), hb, text("</div>")]` — double `hardBreak` for the empty-segment boundary. pulldown-cmark type-6 rule terminates an HTML block at a blank line, so this is a handler-level defense-in-depth case. | Test: `test_block_html_consecutive_blank_lines_produce_double_hardbreak` |
| EC-7 | All-empty / empty block | After step 2 trim and step 6 guard, no paragraph is emitted (`EndResult::Empty`). | Test: `test_block_html_all_empty_block_emits_no_paragraph` |
| EC-8 | Leading blank line (`\n<div>x</div>\n`, handler-level) | Step 4 generates a leading `hardBreak` for the empty leading segment; step 5b (`trim_leading_trailing_hardbreaks`) removes it. No `hardBreak` at position 0 in output. | Test: `test_block_html_leading_blank_line_no_leading_hardbreak` |
| EC-9 | Lone `\r` interior (`<div>\rx</div>`, handler-level) | Step 3 normalizes lone `\r`→`\n` before split; exactly ONE `hardBreak` produced (not two). pulldown-cmark normalizes lone `\r` per CommonMark §2.3 before tokenizing, so this is a handler-level defense-in-depth path. | Test: `test_block_html_lone_cr_interior_produces_single_hardbreak` |
| EC-10 | Trailing non-newline whitespace on final line (`<div>x</div>\n   `, handler-level) | Step 2 preserves the trailing spaces. Forward ADF: `[text("<div>x</div>"), hb, text("   ")]`. Reverse path: `AdfRenderer::finish().trim_end()` strips trailing whitespace — round-trip is NOT byte-identical to the handler input. | Test: `test_block_html_trailing_whitespace_final_line_not_byte_identical` |

**Note on EC-6/EC-8/EC-9/EC-10:** These cases are exercised as handler-level
unit tests that construct `AdfBuilder` state directly, bypassing
`markdown_to_adf`. This is intentional: pulldown-cmark's CommonMark type-6
rule terminates an HTML block at a blank line (so consecutive interior blank
lines cannot arrive as a single `HtmlBlock` through the parser), and CommonMark
§2.3 normalizes `\r`/`\r\n` before tokenization. The handler tests exercise
Algorithm B's correctness as a standalone algorithm, independent of
parser-level input normalization.

## Parser note

pulldown-cmark emits block HTML as `Tag::HtmlBlock` wrapping per-line
`Event::Html` events. The `start()` arm in `AdfBuilder` routes
`Tag::HtmlBlock` → `NodeKind::HtmlBlock`. Each `Event::Html` line appends its
string to the current node's child text collection via `push_text`. On
`End(TagEnd::HtmlBlock)`, the `NodeKind::HtmlBlock` arm collects and processes
all accumulated text.

The handler does NOT use `push_text` internally to build the output content
array (which would incorrectly apply `active_marks` and break the direct
content-array construction).

## Test coverage (13 tests in `src/adf.rs::tests`)

| Test | What it pins |
|------|--------------|
| `test_convert_block_html_is_preserved_as_literal_text` | Single-line block HTML preserved as one paragraph / one text node with no marks (issue #489, original) |
| `test_convert_multiline_block_html_preserves_interior_newlines` | Multi-line: 3 segments → 5 content nodes `[text, hb, text, hb, text]`; no raw `\n` in any text node (AC-004, issue #492 RED GATE) |
| `test_block_html_round_trips_through_adf_to_text` | Single-line round-trip byte-identical |
| `test_multiline_block_html_round_trips_through_adf_to_text` | Multi-line round-trip byte-identical (LF-only, non-whitespace final line) |
| `test_block_html_comment_only_behavior` | Comment-only block → single text node, no hardBreak (EC-3, DOCUMENT-AS-IS) |
| `test_block_html_bare_url_gets_link_mark` | URL at valid boundary gets link mark; href-attribute form does not (EC-4) |
| `test_block_html_interior_line_url_split_preserves_hardbreaks` | URL on interior line of multi-line block: autolink splits middle text node into `[pre, link, post]`; flanking `hardBreak` nodes survive at correct positions (EC-4, F-P1-002) |
| `test_block_html_crlf_interior_no_dangling_cr` | CRLF normalized; no `\r` in text nodes (EC-1) |
| `test_block_html_consecutive_blank_lines_produce_double_hardbreak` | Double hardBreak for empty interior segment (EC-6, handler-level) |
| `test_block_html_all_empty_block_emits_no_paragraph` | All-whitespace/newlines-only body → step-6 early-return, `builder.root` empty, no paragraph emitted (EC-7, handler-level) |
| `test_block_html_leading_blank_line_no_leading_hardbreak` | Leading hardBreak trimmed by step 5b (EC-8, handler-level) |
| `test_block_html_lone_cr_interior_produces_single_hardbreak` | Lone `\r` → single hardBreak (EC-9, handler-level) |
| `test_block_html_trailing_whitespace_final_line_not_byte_identical` | Trailing-whitespace line preserved in forward ADF; stripped by `finish().trim_end()` on reverse (EC-10, handler-level) |

## References

- BC-7.2.011 — `.factory/specs/prd/bc-7-output-render.md` (authoritative contract)
- Issue #489 — introduced block HTML preservation (preserve vs drop decision)
- Issue #492 — replaced raw-`\n`-in-text with Algorithm B hardBreak representation
- Issue #473 — bare-URL autolink pass (`autolink_bare_urls`)
- `src/adf.rs::AdfBuilder::end` — `NodeKind::HtmlBlock` arm (implementation)
- `src/adf.rs::trim_leading_trailing_hardbreaks` — shared helper (reused from taskItem path)
- `src/adf.rs::AdfRenderer::finish` — document-level `trim_end()` (reverse-path loss EC-10)
