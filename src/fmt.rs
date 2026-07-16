//! `lamedh --fmt` / `--fmt-check`: a **conservative** canonical formatter.
//!
//! This is deliberately *not* a pretty-printer: it never reflows or reorders
//! tokens, never changes what a form looks like once it starts, and never
//! touches anything inside a string literal, a line comment (`;`), or a
//! block comment (`#| … |#`). It only rewrites two things:
//!
//! 1. **Indentation.** Every line's leading whitespace is rewritten to
//!    `2 * depth` spaces, where `depth` is the number of `(` that are open
//!    (not yet closed by a matching `)`) at the *start* of that line, counted
//!    only over source outside strings/comments/character literals. This is
//!    the simplest rule that produces stable, unsurprising indentation for
//!    hand-written Lisp: it does not try to align continuation arguments
//!    under a call's first argument the way an editor mode might.
//! 2. **Whitespace hygiene.** Trailing whitespace is stripped from every
//!    line whose *end* is outside a string/comment (so trailing spaces that
//!    are part of a still-open multi-line string are left alone — they are
//!    string content, not formatting). Runs of 3 or more consecutive blank
//!    lines collapse to 2. Trailing blank lines at end of file are dropped
//!    entirely and the file ends with exactly one newline.
//!
//! A line that **starts** inside a string or block comment (i.e. it is a
//! continuation line of a multi-line string/comment) is reproduced
//! byte-for-byte, including its own leading and trailing whitespace: any
//! part of that whitespace could be meaningful content.
//!
//! A leading `#!` shebang line is preserved untouched at column 0 (issue
//! #248's shebang support) and excluded from all of the above.
//!
//! This scanner mirrors the lexical rules in [`crate::reader`] (string
//! escapes, `'c'`/`'\c'` character literals, nesting `#| |#` block comments,
//! `;` line comments) closely enough to track paren depth and string/comment
//! *regions* correctly, without building a full parse tree — formatting is a
//! best-effort text transform; callers that need a parseability guarantee
//! (e.g. the `--fmt`/`--fmt-check` CLI) should validate with
//! [`crate::reader::read_all`] first.

/// Lexical state of the scanner at a given point in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    /// Ordinary code: parens count toward depth, `"`/`'`/`;`/`#|` are live.
    Normal,
    /// Inside a `"..."` string literal (across however many lines).
    InString,
    /// Inside a (possibly nested) `#| ... |#` block comment. The payload is
    /// the nesting depth (always >= 1 while in this state).
    InBlockComment(u32),
}

/// One physical line of source together with the scanner state at its start
/// and end, and the paren depth in effect at its start.
struct Line {
    text: String,
    start_state: ScanState,
    start_depth: usize,
    end_state: ScanState,
}

/// Reformat `src` into canonical form. See the module docs for the exact
/// rule set. Pure text transform: never fails, never validates that `src`
/// parses (callers that need that guarantee should check first).
pub fn format_source(src: &str) -> String {
    let (shebang, body) = split_shebang(src);

    let lines = scan_lines(body);
    let mut rendered: Vec<String> = lines.iter().map(render_line).collect();
    collapse_blank_runs(&mut rendered, &lines);
    strip_trailing_blank_lines(&mut rendered);

    let mut out = shebang.to_string();
    if !rendered.is_empty() {
        out.push_str(&rendered.join("\n"));
        out.push('\n');
    }
    out
}

/// Split off a leading `#!` shebang line (kept verbatim, including its
/// newline) from the rest of the source. Returns `("", src)` when there is
/// no shebang.
fn split_shebang(src: &str) -> (&str, &str) {
    if src.starts_with("#!") {
        match src.find('\n') {
            Some(nl) => (&src[..=nl], &src[nl + 1..]),
            // The whole file is one unterminated shebang line: nothing else
            // to format.
            None => (src, ""),
        }
    } else {
        ("", src)
    }
}

/// Scan `body` into per-line lexical metadata, carrying scanner state and
/// paren depth across physical lines (so multi-line strings/block comments
/// are tracked correctly).
fn scan_lines(body: &str) -> Vec<Line> {
    let mut state = ScanState::Normal;
    let mut depth: i64 = 0;
    let mut lines = Vec::new();
    for raw in body.split('\n') {
        let start_state = state;
        let start_depth = depth.max(0) as usize;
        state = scan_one_line(raw, state, &mut depth);
        lines.push(Line {
            text: raw.to_string(),
            start_state,
            start_depth,
            end_state: state,
        });
    }
    lines
}

/// Advance `state`/`depth` across one physical line's content (no `\n`
/// included). Returns the state in effect at the end of the line.
fn scan_one_line(line: &str, mut state: ScanState, depth: &mut i64) -> ScanState {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    while i < n {
        match state {
            ScanState::InBlockComment(level) => {
                if chars[i] == '#' && i + 1 < n && chars[i + 1] == '|' {
                    state = ScanState::InBlockComment(level + 1);
                    i += 2;
                } else if chars[i] == '|' && i + 1 < n && chars[i + 1] == '#' {
                    state = if level <= 1 {
                        ScanState::Normal
                    } else {
                        ScanState::InBlockComment(level - 1)
                    };
                    i += 2;
                } else {
                    i += 1;
                }
            }
            ScanState::InString => match chars[i] {
                // A backslash escapes exactly the next char, matching
                // reader::parse_string. If there is no next char on this
                // line, the escape reaches across the newline (the real
                // reader operates on the whole buffer); staying InString
                // and letting the next line continue the scan reproduces
                // that.
                '\\' => i += 2,
                '"' => {
                    state = ScanState::Normal;
                    i += 1;
                }
                _ => i += 1,
            },
            ScanState::Normal => match chars[i] {
                ';' => break, // rest of physical line is a line comment
                '#' if i + 1 < n && chars[i + 1] == '|' => {
                    state = ScanState::InBlockComment(1);
                    i += 2;
                }
                '"' => {
                    state = ScanState::InString;
                    i += 1;
                }
                '\'' => {
                    if let Some(consumed) = char_literal_len(&chars[i..]) {
                        i += consumed;
                    } else {
                        // Not a char literal: a bare quote-reader-macro
                        // character. It carries no lexical state of its own.
                        i += 1;
                    }
                }
                '(' => {
                    *depth += 1;
                    i += 1;
                }
                ')' => {
                    *depth -= 1;
                    i += 1;
                }
                _ => i += 1,
            },
        }
    }
    state
}

/// If `chars` (with `chars[0] == '\''`) begins a `reader::parse_char_literal`
/// -shaped literal (`'c'` or `'\c'`), return how many `char`s it spans.
/// Otherwise `None` (a bare quote character, e.g. the reader macro or an
/// empty `''`).
fn char_literal_len(chars: &[char]) -> Option<usize> {
    debug_assert_eq!(chars.first(), Some(&'\''));
    if chars.len() < 3 {
        return None;
    }
    let c0 = chars[1];
    if c0 == '\\' {
        if chars.len() < 4 {
            return None;
        }
        if chars[3] == '\'' { Some(4) } else { None }
    } else if c0 == '\'' {
        None // empty '' is not a char literal (matches parse_char_literal)
    } else if chars[2] == '\'' {
        Some(3)
    } else {
        None
    }
}

/// Render one line per the depth/whitespace rules. Lines that *start* inside
/// a string or block comment are reproduced verbatim.
fn render_line(line: &Line) -> String {
    if line.start_state != ScanState::Normal {
        return line.text.clone();
    }
    let trimmed_start = line.text.trim_start_matches([' ', '\t']);
    if trimmed_start.is_empty() {
        return String::new();
    }
    let indent = "  ".repeat(line.start_depth);
    if line.end_state == ScanState::Normal {
        let trimmed = trimmed_start.trim_end_matches([' ', '\t', '\r']);
        format!("{indent}{trimmed}")
    } else {
        // This line opens a string/block comment that continues past EOL:
        // its tail belongs to that string/comment, so leave it untouched.
        format!("{indent}{trimmed_start}")
    }
}

/// Collapse runs of 3+ consecutive *ordinary* blank lines (start state
/// `Normal`, rendered to `""`) down to 2. A blank-looking line that is really
/// inside a string/comment is never touched and always breaks a run.
fn collapse_blank_runs(rendered: &mut Vec<String>, lines: &[Line]) {
    let mut out = Vec::with_capacity(rendered.len());
    let mut run = 0usize;
    for (text, line) in rendered.iter().zip(lines.iter()) {
        let blank = text.is_empty() && line.start_state == ScanState::Normal;
        if blank {
            run += 1;
            if run <= 2 {
                out.push(text.clone());
            }
        } else {
            run = 0;
            out.push(text.clone());
        }
    }
    *rendered = out;
}

/// Drop trailing blank lines entirely so the file ends with exactly one
/// newline and no dangling blank lines. Safe: a file that parses (the
/// precondition callers are expected to check) cannot end mid-string/comment,
/// so every line at the very end is in `Normal` state.
fn strip_trailing_blank_lines(rendered: &mut Vec<String>) {
    while matches!(rendered.last(), Some(l) if l.is_empty()) {
        rendered.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_on_already_canonical() {
        let src = "(defun sq (x)\n  (* x x))\n";
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn reindents_by_paren_depth() {
        let src = "(defun sq (x)\n(* x x))\n";
        assert_eq!(format_source(src), "(defun sq (x)\n  (* x x))\n");
    }

    #[test]
    fn strips_trailing_whitespace() {
        let src = "(foo)   \n(bar)\t\n";
        assert_eq!(format_source(src), "(foo)\n(bar)\n");
    }

    #[test]
    fn collapses_three_plus_blank_lines_to_two() {
        let src = "(a)\n\n\n\n(b)\n";
        assert_eq!(format_source(src), "(a)\n\n\n(b)\n");
    }

    #[test]
    fn trims_trailing_blank_lines() {
        let src = "(a)\n\n\n\n";
        assert_eq!(format_source(src), "(a)\n");
    }

    #[test]
    fn preserves_shebang_untouched() {
        let src = "#!/usr/bin/env lamedh\n(foo)\n";
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn does_not_reindent_inside_string() {
        let src = "(def x \"line one\n   line two (still string)\")\n";
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn does_not_touch_parens_inside_string_or_char_literal() {
        let src = "(def x \"(unbalanced\")\n(def y '(')\n";
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn preserves_block_comment_body() {
        let src = "(foo)\n#| a comment\n   with (parens) inside |#\n(bar)\n";
        assert_eq!(format_source(src), src);
    }

    #[test]
    fn line_comment_indentation_is_normalized() {
        let src = "(foo\n; a comment\n (bar))\n";
        assert_eq!(format_source(src), "(foo\n  ; a comment\n  (bar))\n");
    }

    #[test]
    fn idempotent_twice() {
        let src = "(foo\n(bar\n(baz)))\n\n\n\n(qux)   \n";
        let once = format_source(src);
        let twice = format_source(&once);
        assert_eq!(once, twice);
    }
}
