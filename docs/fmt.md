# Canonical Formatting (`lamedh --fmt`)

`lamedh --fmt` rewrites `.lisp` files in place into a canonical textual form.
`lamedh --fmt-check` reports which files would change without writing
anything — the shape you want in CI or a pre-commit hook.

It is deliberately a **conservative** formatter, not a pretty-printer: it
never reflows an s-expression, never reorders or rewrites tokens, and never
touches anything inside a string literal, a character literal, a line
comment (`;`), or a block comment (`#| … |#`). It only normalizes
indentation and incidental whitespace.

## Usage

```text
lamedh --fmt file.lisp [file2.lisp ...]
lamedh --fmt-check file.lisp [file2.lisp ...]
```

* `--fmt` rewrites every file that is not already canonical and prints its
  path, one per line.
* `--fmt-check` writes nothing; it prints the path of every file that would
  change and exits `1` if there is at least one, `0` if every file is
  already canonical.
* Either flag requires at least one file.

### Exit codes

| Code | `--fmt`                          | `--fmt-check`                    |
|------|-----------------------------------|-----------------------------------|
| 0    | Success (files rewritten or not). | Every file already canonical.     |
| 1    | —                                  | At least one file would change.   |
| 2    | A file could not be read or parsed. | A file could not be read or parsed. |

A file that fails to parse is left untouched; formatting only ever operates
on source that reads cleanly (checked with the same reader
[`lamedh --check`](check.md) uses).

## The rule set

Two rules, applied uniformly:

1. **Indentation.** Every line's leading whitespace is rewritten to
   `2 * depth` spaces, where `depth` is the number of `(` that are open (not
   yet closed by a matching `)`) *at the start of that line*, counted only
   over source outside strings, character literals, and comments. This is
   the simplest rule that produces stable output for hand-written Lisp — it
   does not attempt to align a call's continuation lines under its first
   argument the way an editor mode typically does, so heavily
   hand-aligned code will be visibly re-indented the first time it is
   formatted. That is expected and is why `lib/*.lisp` is *not* pinned to
   be a `--fmt-check`-clean fixed point (it predates the formatter); the
   guarantee the test suite pins is idempotence and meaning-preservation,
   not stability against the pre-formatter house style.
2. **Whitespace hygiene.**
   * Trailing whitespace is stripped from every line whose end is *outside*
     a string/comment (a line that ends mid-string, e.g. inside a
     multi-line string literal, is left completely alone — its trailing
     spaces are string content).
   * Runs of 3 or more consecutive blank lines collapse to 2.
   * Trailing blank lines at end of file are dropped, and the file ends
     with exactly one newline.

A line that **starts** inside a string or block comment (a continuation
line of a multi-line string/comment) is reproduced byte-for-byte, leading
and trailing whitespace included.

A leading `#!` shebang line (see the reader's shebang support) is preserved
untouched at column 0 and excluded from every rule above.

## What is never touched

* String literal contents (`"..."`), including embedded parens, semicolons,
  and newlines.
* Character literals (`'c'`, `'\n'`, `'('`, `')'`, …) — a paren inside a
  character literal is not counted toward indentation depth.
* Line comment (`;`) and block comment (`#| … |#`, nesting) text.
* Token order and content: `--fmt` only ever edits *runs of whitespace*
  between/around tokens, never a token itself.

## Guarantees

* **Idempotence.** `format(format(x)) == format(x)` for every file — running
  `--fmt` twice is the same as running it once.
* **Meaning preservation.** Reading a file before and after formatting
  yields the same forms (checked by comparing printed reads over the whole
  `lib/` corpus in `tests/test_fmt.rs`).
