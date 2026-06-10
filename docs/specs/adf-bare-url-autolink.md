# `markdown_to_adf` — bare-URL autolinking → ADF `link` mark

**Issue:** [#473](https://github.com/Zious11/jira-cli/issues/473)
**Module:** `src/adf.rs`
**Research:** `.factory/research/issue-473-bare-url-autolink-scope.md`

## Problem

A bare URL written in plain text — `https://example.com` — was **not** turned
into an ADF `link` mark by `markdown_to_adf`; it stayed plain text. Angle-bracket
autolinks (`<https://example.com>`) and inline links (`[text](url)`) already
become `link` marks; bare URLs did not, which is the form people most often paste
into a Jira issue description or comment.

pulldown-cmark 0.13 cannot fix this with a flag: there is **no autolink
extension option**, and `ENABLE_GFM` in 0.13 only adds alert blockquotes (it does
**not** fold in the GFM extended/bare-URL autolink extension). So bare-URL
linkification must be implemented in our own code.

## Why it is required (not cosmetic)

When ADF is submitted via the Jira Cloud REST API (`POST /rest/api/3/issue`
description/comment body), Jira does **not** auto-linkify a bare URL sitting as
plain text in a `text` node — an explicit `link` mark is **required** for it to
be clickable. Jira's smart-link auto-unfurl is an interactive browser-editor,
compose-time feature; it does not run on ADF JSON arriving through the API.
(Atlassian-confirmed; see research doc.) Consequently the mark is genuinely
needed, and double-linking risk on the API path is low.

## Scope decision: `http(s)://` explicit-scheme ONLY

In scope:

- `https://…` and `http://…` — the scheme is matched **case-insensitively**
  (`HTTPS://`, `Http://`, `httpS://` all link, per RFC 3986 / GFM). The produced
  `href` preserves the user's original casing (the scheme and path are not
  normalized).

Out of scope (documented limitation):

- `www.`-prefixed hosts (no scheme to read; GFM's legacy default is `http://`)
- bare emails → `mailto:` (GFM never publishes the email regex; highest
  false-positive class)
- naked domains (`example.com`)

Rationale: because an applied mark **permanently** writes a link into the user's
issue, every false positive is a defect that ships. An explicit scheme is an
unambiguous intent signal and eliminates the noisy classes — `e.g.`, version
strings (`0.13`), file paths (`src/adf.rs`), sentence-final domains. "Plain"
Markdown (CommonMark, remark core, markdown-it default, pulldown-cmark) does not
linkify bare URLs at all; the expectation that bare URLs link comes from GFM /
chat apps and centers on `https://…`.

## Implementation

A post-`finish()` pass over the built ADF tree, run from `markdown_to_adf`:

| Function | Responsibility |
|----------|----------------|
| `autolink_bare_urls(&mut Vec<Value>)` | Recursively walk node arrays. Skip `codeBlock` subtrees; for `text` nodes without a `link`/`code` mark, splice in the split result; otherwise descend into `content`. |
| `split_text_node_on_urls(&Value) -> Option<Vec<Value>>` | Split one plain text node into alternating plain / link-marked text nodes, preserving existing inline marks. `None` when no URL is present. |
| `find_bare_url_spans(&str) -> Vec<(usize, usize)>` | Locate `http(s)://` byte-spans with GFM boundary + extent rules. |
| `trim_url_extent(&str) -> usize` | GFM trailing-punctuation trimming + parenthesis balancing. |

### Rules (derived from the GFM autolink extension)

1. **Boundary.** A URL may start only at the beginning of a text node, or after
   whitespace or one of `*`, `_`, `~`, `(`. So `foohttps://example.com` does not
   match (preceding `o` is a word char).
2. **Extent.** From `://`, consume up to the next whitespace or `<`.
3. **Trailing trim.** Strip trailing `?`, `!`, `.`, `,`, `:`, `*`, `_`, `~`.
4. **Parenthesis balance.** Drop a trailing `)` only when the slice has more `)`
   than `(`. So `https://en.wikipedia.org/wiki/Foo_(bar)` keeps `(bar)`, but
   `(https://example.com)` and `https://example.com.` exclude the wrapper / period.
5. **Skips.** Text nodes already carrying a `link` mark (`<url>`, `[t](url)`) or a
   `code` mark, and all `codeBlock` content, are never touched — no double-links,
   no linkification inside code.

### Deviations from GFM

The rules above are *GFM-derived*, not a faithful GFM autolink implementation.
Known, deliberate divergences:

1. **Boundary "before" set is narrower.** GFM (micromark `wwwAutolinkBefore`)
   admits `[` and `]` (plus line/tab boundaries) in addition to whitespace and
   `*_~(`. We admit only text-node-start, whitespace, and `*_~(` — omitting both
   `[` and `]` — to cut false positives (e.g. an unresolved reference-shortcut
   `[https://x]` stays plain text rather than linking the inner URL).
2. **URLs split by inline markup link only the leading run.** Because the pass
   runs over the *already-built* ADF tree, a URL whose interior contains inline
   emphasis/strong/strike (`https://example.com/a*b*c`, where `*b*` parsed as
   emphasis) has already been fragmented into separate text nodes. Only the
   leading plain run (`https://example.com/a`) is linked; the emphasized tail is
   not part of the href. Pinned by `test_bare_url_split_by_emphasis_links_only_leading_run`.
3. **`www.`, bare email, and other schemes (`ftp://`, `mailto:`) are out of
   scope** (see Scope decision above).

These divergences are acceptable for the Jira-description use case and bias
toward *fewer* false positives, which matters because an applied mark is written
permanently into the user's issue.

### Round-trip note

Because a bare URL now becomes a real link, `adf_to_text` renders it back in
markdown link form: `https://example.com` → `[https://example.com](https://example.com)`.
This is semantically correct (it is a link) and is pinned by
`test_bare_url_round_trips_to_markdown_link_text`.

## Tests

`src/adf.rs` (`adf::tests`). Core behavior: `test_bare_https_url_becomes_link_mark`,
`test_bare_http_url_becomes_link_mark`, `test_bare_url_trailing_period_is_trimmed`,
`test_bare_url_wrapping_paren_not_captured`, `test_bare_url_balanced_inner_parens_kept`,
`test_bare_url_with_port_is_preserved`, `test_bare_url_trailing_colon_is_trimmed`,
`test_two_bare_urls_in_one_text_node_both_link`,
`test_bare_url_round_trips_to_markdown_link_text`. Skips / negatives:
`test_url_in_inline_code_not_linkified`, `test_url_in_code_block_not_linkified`,
`test_existing_markdown_link_not_double_linkified`, `test_www_url_stays_plain_text`,
`test_bare_email_stays_plain_text`, `test_url_tight_against_preceding_word_not_matched`,
`test_bare_url_after_open_bracket_stays_plain_text`.
Deviations / container paths: `test_bare_url_split_by_emphasis_links_only_leading_run`,
`test_bare_url_inside_emphasis_keeps_em_and_link`, `test_bare_url_in_panel_is_linkified`,
`test_bare_url_in_table_cell_is_linkified`.
