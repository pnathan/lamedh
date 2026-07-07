//! S-expression parser built with [nom](https://docs.rs/nom) combinator library.
//!
//! The public entry points are [`read`] (one form) and [`read_all`] (zero or
//! more forms).  Both require an [`Environment`] reference so that symbol names
//! can be interned into the global [`crate::environment::SymbolTable`] during
//! parsing — this ensures that two occurrences of the same name share one `Rc`
//! allocation and that `EQ` comparisons are pointer-equality tests.
//!
//! ## Syntax summary
//!
//! | Input | Parsed as |
//! |-------|-----------|
//! | `123`, `-456` | `LispVal::Number(i64)` |
//! | `3.14`, `-1e5` | `LispVal::Float(f64)` |
//! | `177Q` | Octal literal (`177₈ = 127₁₀`) — Lisp 1.5 notation |
//! | `FFh` | Hex literal (`FF₁₆ = 255₁₀`) — assembly-style `H` suffix |
//! | `'c'` | Character literal → `LispVal::Char` (byte 0–255; escapes `\n \t \r \\ \' \0`) |
//! | `"hi\n"` | `LispVal::String` (supports `\n \t \r \\ \"`) |
//! | `FOO`, `+`, `*x*`, `:key` | `LispVal::Symbol` (uppercased, interned) |
//! | `(a b c)` | Proper list (cons chain ending in Nil) |
//! | `(a . b)` | Dotted pair |
//! | `'e` | `(QUOTE e)` |
//! | `` `e `` | `(QUASIQUOTE e)` |
//! | `,e` | `(UNQUOTE e)` |
//! | `,@e` | `(UNQUOTE-SPLICING e)` |
//! | `#'f` | `(FUNCTION f)` |
//! | `#x1F`, `#b101`, `#o17` | Radix literals (hex, binary, octal) |
//! | `; comment` | Ignored to end of line |
//! | `#\| comment \|#` | Block comment (nests) |
//! | `#!...` (first line) | Shebang line, ignored |
//!
//! Parse errors are reported with 1-based line/column positions (issue #238).
//!
//! Symbols are **always** uppercased during interning, so `foo`, `FOO`, and
//! `Foo` all resolve to the same interned `Symbol` named `"FOO"`.

use crate::LispVal;
use crate::Shared;
use crate::environment::Environment;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_not, tag, take_while, take_while1},
    character::complete::{alpha1, alphanumeric1, char, digit1, hex_digit1, multispace1, one_of},
    combinator::{cut, map, map_res, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
};

type ParseResult<'a> = IResult<&'a str, LispVal>;

/// Default maximum nesting depth `parse_expr` will recurse through — bounds
/// the parser's native call-stack usage so that pathological input like tens
/// of thousands of nested `(` or a long `'''...` chain produces a parse error
/// instead of a native stack overflow (issue #270).
///
/// Chosen empirically (see the issue #270 PR for the measurement methodology):
/// on this platform, unbounded recursion on nested-paren input crashes an
/// 8 MiB thread at a depth of about 1300-1320 in a debug build, i.e. roughly
/// 6.4 KB of stack per nesting level. 512 therefore leaves ~2.5x headroom on
/// any stack of 8 MiB or more (a typical main thread).
///
/// **What this default does and does not protect:** callers of the plain
/// [`read`] / [`read_all`] / [`read_next`] entry points are protected as long
/// as they run on a stack of at least ~4 MiB (512 levels x ~6.4 KB ≈ 3.3 MiB
/// worst case). On *smaller* stacks — e.g. a 2 MiB spawned thread, whose
/// capacity is only ~330 levels — this default is **not** low enough; such
/// callers must use [`read_with_depth_limit`] (or the other `_with_depth_limit`
/// variants) with a limit sized to their stack. Conversely, callers on the
/// 512 MiB [`crate::with_large_stack`] thread (the CLI, `load_file`,
/// `read-from-string`) can afford a far higher limit — the evaluator-facing
/// entry points read it from
/// [`crate::environment::Environment::reader_depth_limit`], which
/// [`crate::environment::Environment::with_stdlib`] raises to 50,000
/// (~320 MB worst case, still under the 512 MiB stack; the empirical crash
/// boundary there is ~84,000-90,000 levels).
pub const DEFAULT_READER_DEPTH: usize = 512;

/// `nom::error::ErrorKind` used as a sentinel to distinguish the "nesting too
/// deep" failure from ordinary parse errors so [`read_next_with_depth_limit`]
/// can render a dedicated message. Not otherwise produced by this module.
const TOO_DEEP_KIND: nom::error::ErrorKind = nom::error::ErrorKind::TooLarge;

fn too_deep_error(input: &str) -> nom::Err<nom::error::Error<&str>> {
    // A hard `Failure` (not `Error`): once the depth limit is hit we want to
    // unwind immediately rather than have `alt`/`many0`/`opt` backtrack and
    // retry other productions at the same (already too-deep) position.
    nom::Err::Failure(nom::error::Error::new(input, TOO_DEEP_KIND))
}

/// `remaining` counts *down* from the caller-chosen depth limit: the top-level
/// entry points pass the limit itself, each nesting production passes
/// `remaining - 1`, and hitting zero raises the too-deep failure. Counting
/// down (rather than up against a threaded limit) keeps the recursion to a
/// single extra parameter.
fn parse_expr(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        if remaining == 0 {
            return Err(too_deep_error(input));
        }
        preceded(
            ws,
            alt((
                parse_atom(env.clone()),
                parse_string,
                parse_list(env.clone(), remaining),
                // Char literal 'c' before the quote reader macro: 'a' is a char,
                // 'a (no closing quote) stays (quote a).
                parse_char_literal,
                parse_quoted(env.clone(), remaining),
                parse_quasiquoted(env.clone(), remaining),
                // ,@ before , : `,@e` is splicing, `,e` is plain unquote.
                parse_unquote_spliced(env.clone(), remaining),
                parse_unquoted(env.clone(), remaining),
                parse_function_shorthand(env.clone(), remaining),
            )),
        )(input)
    }
}

// A parser for a comment
fn parse_comment(input: &str) -> IResult<&str, &str> {
    recognize(pair(tag(";"), is_not("\n\r")))(input)
}

// A parser for a (nesting) block comment: #| ... |# (issue #248).
// An unterminated block comment is a hard Failure so the error position
// points at its opening `#|`.
fn parse_block_comment(input: &str) -> IResult<&str, &str> {
    if !input.starts_with("#|") {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    let bytes = input.as_bytes();
    let mut depth = 1usize;
    let mut i = 2usize;
    while i < bytes.len() {
        if bytes[i] == b'#' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'|' && i + 1 < bytes.len() && bytes[i + 1] == b'#' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Ok((&input[i..], &input[..i]));
            }
        } else {
            i += 1;
        }
    }
    Err(nom::Err::Failure(nom::error::Error::new(
        input,
        nom::error::ErrorKind::TakeUntil,
    )))
}

// A parser for whitespace, including line and block comments
fn ws(input: &str) -> IResult<&str, &str> {
    recognize(many0(alt((
        multispace1,
        parse_comment,
        parse_block_comment,
    ))))(input)
}

fn parse_float(input: &str) -> ParseResult<'_> {
    map(
        map_res(
            recognize(tuple((
                opt(tag("-")),
                digit1,
                tag("."),
                digit1,
                opt(tuple((one_of("Ee"), opt(one_of("+-")), digit1))),
            ))),
            |s: &str| s.parse::<f64>(),
        ),
        LispVal::Float,
    )(input)
}

fn parse_octal_integer(input: &str) -> ParseResult<'_> {
    // Lisp 1.5: digits followed by Q means octal, e.g. 177Q = 127
    let (rest, s) = recognize(pair(opt(tag("-")), digit1))(input)?;
    let (rest, _) = tag("Q")(rest)?;
    let negative = s.starts_with('-');
    let digits = if negative { &s[1..] } else { s };
    match i64::from_str_radix(digits, 8) {
        Ok(n) => Ok((rest, LispVal::Number(if negative { -n } else { n }))),
        Err(_) => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Digit,
        ))),
    }
}

fn parse_hex_integer(input: &str) -> ParseResult<'_> {
    // Assembly-style hex: hex digits followed by H, e.g. FFh = 255, 0Ah = 10,
    // 1Ah = 26. Mirrors the Lisp 1.5 octal `Q` suffix. Case-insensitive in both
    // the digits and the marker (`ffh` and `FFH` both work).
    let err = || nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit));
    let (rest, s) = recognize(pair(opt(tag("-")), hex_digit1))(input)?;
    let (rest, _) = one_of("hH")(rest)?;
    // Boundary guard: a following identifier char means this was a symbol, not a
    // hex literal (so `ffhello` stays a symbol rather than 255 + "ello").
    if let Some(c) = rest.chars().next()
        && (c.is_alphanumeric() || c == '-')
    {
        return Err(err());
    }
    let negative = s.starts_with('-');
    let digits = if negative { &s[1..] } else { s };
    match i64::from_str_radix(digits, 16) {
        Ok(n) => Ok((rest, LispVal::Number(if negative { -n } else { n }))),
        Err(_) => Err(err()),
    }
}

fn parse_integer_or_overflow_float(input: &str) -> ParseResult<'_> {
    let (rest, s) = recognize(pair(opt(tag("-")), digit1))(input)?;
    if let Ok(n) = s.parse::<i64>() {
        Ok((rest, LispVal::Number(n)))
    } else if let Ok(f) = s.parse::<f64>() {
        Ok((rest, LispVal::Float(f)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Digit,
        )))
    }
}

// CL-style radix literals: #x1F / #X1f (hex), #b101 (binary), #o17 (octal),
// with an optional sign after the marker (issue #248).
fn parse_radix_literal(input: &str) -> ParseResult<'_> {
    let err = || nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit));
    let (rest, _) = tag("#")(input)?;
    let (rest, marker) = one_of("xXbBoO")(rest)?;
    let radix = match marker {
        'x' | 'X' => 16,
        'b' | 'B' => 2,
        _ => 8,
    };
    let (rest, neg) = opt(tag("-"))(rest)?;
    let (rest, digits) = take_while1(|c: char| c.is_digit(radix))(rest)?;
    // Boundary guard: a trailing identifier char means this was malformed
    // (e.g. #b102 or #xFG) — fail rather than half-consume.
    if let Some(c) = rest.chars().next()
        && (c.is_alphanumeric() || c == '-')
    {
        return Err(err());
    }
    match i64::from_str_radix(digits, radix) {
        Ok(n) => Ok((rest, LispVal::Number(if neg.is_some() { -n } else { n }))),
        Err(_) => Err(err()),
    }
}

fn parse_number(input: &str) -> ParseResult<'_> {
    alt((
        parse_float,
        parse_radix_literal,
        parse_hex_integer,
        parse_octal_integer,
        parse_integer_or_overflow_float,
    ))(input)
}

/// Character literal: `'c'` denotes the character `c`, read as its integer code
/// point (a `LispVal::Number`). lamedh has no char value type, so a character is
/// its code point — the same number `char-code` yields and `code-char` consumes,
/// and the byte the typed-JIT `char` carries (issue #136).
///
/// One character between single quotes, with C-style escapes `\n \t \r \\ \' \0`.
/// This is tried before the quote reader macro: `'a'` is the char a, while `'a`
/// (no closing quote) remains `(quote a)`. Use `'\''` for the quote character.
fn parse_char_literal(input: &str) -> ParseResult<'_> {
    let err = || nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char));
    let (rest, _) = tag("'")(input)?;
    let mut it = rest.char_indices();
    let (_, c0) = it.next().ok_or_else(err)?;
    let (code, consumed) = if c0 == '\\' {
        let (_, c1) = it.next().ok_or_else(err)?;
        let decoded = match c1 {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '\\' => '\\',
            '\'' => '\'',
            '0' => '\0',
            other => other,
        };
        (decoded as i64, 1 + c1.len_utf8())
    } else if c0 == '\'' {
        // An empty '' is not a character literal; let other parsers try.
        return Err(err());
    } else {
        (c0 as i64, c0.len_utf8())
    };
    let (rest2, _) = tag("'")(&rest[consumed..])?;
    if code > 255 {
        return Err(err());
    }
    Ok((rest2, LispVal::Char(code as u8)))
}

fn parse_one_plus_minus(env: Shared<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        let (rest, sym) = alt((tag("1+"), tag("1-")))(input)?;
        Ok((rest, LispVal::Symbol(env.intern_symbol(sym))))
    }
}

fn parse_keyword_symbol(env: Shared<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        map(
            recognize(pair(
                tag(":"),
                pair(
                    alt((alpha1, tag("&"), tag("$"))),
                    many0(alt((
                        alphanumeric1,
                        tag("-"),
                        tag("*"),
                        tag("?"),
                        tag("!"),
                        tag("+"),
                        tag("="),
                        tag("<"),
                        tag(">"),
                        // `_` as a constituent supports the `?_` match
                        // wildcard (lib/23-match.lisp); previously a parse
                        // error here, so no existing program changes meaning.
                        tag("_"),
                    ))),
                ),
            )),
            |s: &str| LispVal::Symbol(env.intern_symbol(&s.to_uppercase())),
        )(input)
    }
}

fn is_operator_char(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/' | '=' | '<' | '>' | '!' | '~')
}

fn parse_atom(env: Shared<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        alt((
            // Parse special numeric symbols like 1+ and 1- BEFORE numbers
            parse_one_plus_minus(env.clone()),
            parse_number,
            // Parse earmuff symbols (*name*) - dynamic variable naming convention
            // Must come before regular symbols and operators
            parse_earmuff_symbol(env.clone()),
            parse_keyword_symbol(env.clone()),
            map(
                recognize(pair(
                    // `?` as a symbol-start supports pattern variables (`?x`,
                    // `??xs` — lib/23-match.lisp). Previously a parse error in
                    // this position, so no existing program changes meaning.
                    alt((alpha1, tag("&"), tag("$"), tag("?"))),
                    many0(alt((
                        alphanumeric1,
                        tag("-"),
                        tag("*"),
                        tag("?"),
                        tag("!"),
                        tag("+"),
                        tag("="),
                        tag("<"),
                        tag(">"),
                        // `_` as a constituent supports the `?_` match
                        // wildcard (lib/23-match.lisp); previously a parse
                        // error here, so no existing program changes meaning.
                        tag("_"),
                    ))),
                )),
                |s: &str| {
                    let s_upper = s.to_uppercase();
                    match s_upper.as_str() {
                        "T" => LispVal::Symbol(env.intern_symbol("T")),
                        "NIL" => LispVal::Nil,
                        _ => LispVal::Symbol(env.intern_symbol(&s_upper)),
                    }
                },
            ),
            // Parse operator symbol sequences (>=, !=, /=, +, -, etc.)
            map(take_while1(is_operator_char), |s: &str| {
                LispVal::Symbol(env.intern_symbol(s))
            }),
        ))(input)
    }
}

/// Parse earmuff symbols: *name* (dynamic variable naming convention)
/// Examples: *debug*, *print-level*, *foo123*
fn parse_earmuff_symbol(env: Shared<Environment>) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        map(
            recognize(tuple((
                tag("*"),
                alpha1,
                many0(alt((alphanumeric1, tag("-")))),
                tag("*"),
            ))),
            |s: &str| LispVal::Symbol(env.intern_symbol(&s.to_uppercase())),
        )(input)
    }
}

fn parse_string(input: &str) -> ParseResult<'_> {
    let (input, _) = char('"')(input)?;
    let mut result = String::new();
    let mut remaining = input;
    loop {
        let (rest, literal) = take_while(|c| c != '"' && c != '\\')(remaining)?;
        result.push_str(literal);
        remaining = rest;
        if remaining.starts_with('\\') {
            let after_backslash = &remaining[1..];
            if after_backslash.is_empty() {
                return Err(nom::Err::Failure(nom::error::Error::new(
                    remaining,
                    nom::error::ErrorKind::Char,
                )));
            }
            let c = after_backslash.chars().next().unwrap();
            remaining = &after_backslash[c.len_utf8()..];
            match c {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                'r' => result.push('\r'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                '0' => result.push('\0'),
                _ => {
                    result.push('\\');
                    result.push(c);
                }
            }
        } else if remaining.starts_with('"') {
            remaining = &remaining[1..];
            break;
        } else {
            return Err(nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Char,
            )));
        }
    }
    Ok((remaining, LispVal::String(result)))
}

fn parse_list_contents(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        let (input, exprs) = many0(preceded(ws, parse_expr(env.clone(), remaining)))(input)?;
        let (input, tail) = opt(preceded(
            preceded(ws, char('.')),
            preceded(ws, parse_expr(env.clone(), remaining)),
        ))(input)?;
        if tail.is_some() && exprs.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Char,
            )));
        }

        let end = tail.unwrap_or(LispVal::Nil);
        Ok((
            input,
            exprs.into_iter().rev().fold(end, |cdr, car| LispVal::Cons {
                car: Shared::new(car),
                cdr: Shared::new(cdr),
            }),
        ))
    }
}

fn parse_list(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    move |input: &str| {
        // `cut` after the opening paren: once `(` is consumed no other parser
        // can apply, so a missing `)` becomes a hard Failure whose position
        // points at the offending spot instead of backtracking to the form
        // start (issue #238).
        delimited(
            char('('),
            parse_list_contents(env.clone(), remaining - 1),
            preceded(ws, cut(char(')'))),
        )(input)
    }
}

fn parse_quoted(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    let quote_symbol = LispVal::Symbol(env.intern_symbol("QUOTE"));
    move |input: &str| {
        map(
            preceded(char('\''), parse_expr(env.clone(), remaining - 1)),
            |expr| LispVal::Cons {
                car: Shared::new(quote_symbol.clone()),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(expr),
                    cdr: Shared::new(LispVal::Nil),
                }),
            },
        )(input)
    }
}

fn parse_quasiquoted(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    let quasiquote_symbol = LispVal::Symbol(env.intern_symbol("QUASIQUOTE"));
    move |input: &str| {
        map(
            preceded(char('`'), parse_expr(env.clone(), remaining - 1)),
            |expr| LispVal::Cons {
                car: Shared::new(quasiquote_symbol.clone()),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(expr),
                    cdr: Shared::new(LispVal::Nil),
                }),
            },
        )(input)
    }
}

fn parse_function_shorthand(
    env: Shared<Environment>,
    remaining: usize,
) -> impl Fn(&str) -> ParseResult {
    let function_symbol = LispVal::Symbol(env.intern_symbol("FUNCTION"));
    move |input: &str| {
        map(
            preceded(tag("#'"), parse_expr(env.clone(), remaining - 1)),
            |expr| LispVal::Cons {
                car: Shared::new(function_symbol.clone()),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(expr),
                    cdr: Shared::new(LispVal::Nil),
                }),
            },
        )(input)
    }
}

fn parse_unquoted(env: Shared<Environment>, remaining: usize) -> impl Fn(&str) -> ParseResult {
    let unquote_symbol = LispVal::Symbol(env.intern_symbol("UNQUOTE"));
    move |input: &str| {
        map(
            preceded(char(','), parse_expr(env.clone(), remaining - 1)),
            |expr| LispVal::Cons {
                car: Shared::new(unquote_symbol.clone()),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(expr),
                    cdr: Shared::new(LispVal::Nil),
                }),
            },
        )(input)
    }
}

fn parse_unquote_spliced(
    env: Shared<Environment>,
    remaining: usize,
) -> impl Fn(&str) -> ParseResult {
    let splice_symbol = LispVal::Symbol(env.intern_symbol("UNQUOTE-SPLICING"));
    move |input: &str| {
        map(
            preceded(tag(",@"), parse_expr(env.clone(), remaining - 1)),
            |expr| LispVal::Cons {
                car: Shared::new(splice_symbol.clone()),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(expr),
                    cdr: Shared::new(LispVal::Nil),
                }),
            },
        )(input)
    }
}

// ---------------------------------------------------------------------------
// Positions, error rendering, and the incremental read API (issue #238)
// ---------------------------------------------------------------------------

/// Compute the 1-based (line, column) of byte `offset` within `src`.
pub fn position_of(src: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(src.len());
    let prefix = &src[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = prefix
        .rsplit('\n')
        .next()
        .map(|l| l.chars().count())
        .unwrap_or(0)
        + 1;
    (line, col)
}

/// Render a parse error message with line/column context.
pub fn format_parse_error(src: &str, offset: usize, detail: &str) -> String {
    let (line, col) = position_of(src, offset);
    format!("parse error at line {line}, column {col}: {detail}")
}

/// Describe the unparseable text starting at `rest` (one truncated line).
fn error_detail(rest: &str) -> String {
    if rest.trim().is_empty() {
        "unexpected end of input (unclosed '(' or unterminated string?)".to_string()
    } else {
        let line = rest.lines().next().unwrap_or(rest);
        let snippet: String = line.chars().take(40).collect();
        format!("unexpected input near '{snippet}'")
    }
}

/// Skip leading whitespace and comments (line and block).
pub fn skip_ws(input: &str) -> &str {
    match ws(input) {
        Ok((rest, _)) => rest,
        Err(_) => input,
    }
}

/// Drop a leading `#!` shebang line so `.lisp` files can be executable
/// scripts (issue #248).
pub fn strip_shebang(input: &str) -> &str {
    if input.starts_with("#!") {
        // Keep the trailing newline so line numbers in later errors are
        // unaffected by the stripped shebang.
        match input.find('\n') {
            Some(i) => &input[i..],
            None => "",
        }
    } else {
        input
    }
}

/// Parse the next single form from `input` with the default nesting-depth
/// limit ([`DEFAULT_READER_DEPTH`]).
///
/// Returns `Ok(None)` when only whitespace/comments remain, or
/// `Ok(Some((form, rest)))` with the remaining input on success.
/// On failure returns `(byte_offset, detail)` where the offset is relative to
/// `input` — render it with [`format_parse_error`] (or map it into a larger
/// source buffer first, as [`crate::load_file`] does).
#[allow(clippy::type_complexity)]
pub fn read_next<'a>(
    input: &'a str,
    env: &Shared<Environment>,
) -> Result<Option<(LispVal, &'a str)>, (usize, String)> {
    read_next_with_depth_limit(input, env, DEFAULT_READER_DEPTH)
}

/// Like [`read_next`], but with a caller-chosen nesting-depth limit.
///
/// Pick `depth_limit` from the stack the parse runs on, at roughly 6.4 KB of
/// stack per nesting level (measured; debug build — see
/// [`DEFAULT_READER_DEPTH`]) with at least 2x margin. Nesting beyond the limit
/// yields a normal positioned parse error (`nesting too deep (limit N)`).
#[allow(clippy::type_complexity)]
pub fn read_next_with_depth_limit<'a>(
    input: &'a str,
    env: &Shared<Environment>,
    depth_limit: usize,
) -> Result<Option<(LispVal, &'a str)>, (usize, String)> {
    let rest = skip_ws(input);
    if rest.is_empty() {
        return Ok(None);
    }
    match parse_expr(env.clone(), depth_limit)(rest) {
        Ok((rem, val)) => Ok(Some((val, rem))),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            let detail = if e.code == TOO_DEEP_KIND {
                format!("nesting too deep (limit {depth_limit})")
            } else {
                error_detail(e.input)
            };
            Err((input.len() - e.input.len(), detail))
        }
        Err(nom::Err::Incomplete(_)) => Err((input.len(), "incomplete input".to_string())),
    }
}

/// Return `true` when `input` looks like a *prefix* of a valid program —
/// an unclosed `(`, an unterminated string, or an open block comment — as
/// opposed to text that is malformed outright.  The REPL uses this to decide
/// between prompting for a continuation line and reporting an error
/// (issue #240).
pub fn is_incomplete(input: &str) -> bool {
    let bytes = input.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut depth: i64 = 0;
    let mut block_depth = 0usize;
    // Byte-wise scan is UTF-8 safe: every byte matched below is ASCII, and
    // UTF-8 continuation bytes can never equal an ASCII byte.
    while i < n {
        if block_depth > 0 {
            if bytes[i] == b'#' && i + 1 < n && bytes[i + 1] == b'|' {
                block_depth += 1;
                i += 2;
            } else if bytes[i] == b'|' && i + 1 < n && bytes[i + 1] == b'#' {
                block_depth -= 1;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b';' => {
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'#' if i + 1 < n && bytes[i + 1] == b'|' => {
                block_depth = 1;
                i += 2;
            }
            b'"' => {
                i += 1;
                let mut closed = false;
                while i < n {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            closed = true;
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                if !closed {
                    return true;
                }
            }
            b'\'' => {
                // Skip a char literal ('c' or '\c') so a quoted paren such as
                // '(' does not skew the depth count.
                let rest = &input[i + 1..];
                let mut chars = rest.chars();
                let consumed = match chars.next() {
                    Some('\\') => match (chars.next(), chars.next()) {
                        (Some(c2), Some('\'')) => 1 + c2.len_utf8() + 1,
                        _ => 0,
                    },
                    Some(c1) if c1 != '\'' => match chars.next() {
                        Some('\'') => c1.len_utf8() + 1,
                        _ => 0,
                    },
                    _ => 0,
                };
                i += 1 + consumed;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    depth > 0 || block_depth > 0
}

/// Parse a single s-expression from `input` with the default nesting-depth
/// limit ([`DEFAULT_READER_DEPTH`]) — safe on any stack of ~4 MiB or more.
/// Callers on smaller stacks should use [`read_with_depth_limit`] with a
/// lower limit; callers on the 512 MiB [`crate::with_large_stack`] thread may
/// use a much higher one.
///
/// Symbols are interned into `env`'s symbol table and uppercased.
/// Returns an error if the input contains trailing non-whitespace text after
/// the first expression — use [`read_all`] to parse multiple forms.
pub fn read(input: &str, env: &Shared<Environment>) -> Result<LispVal, String> {
    read_with_depth_limit(input, env, DEFAULT_READER_DEPTH)
}

/// Like [`read`], but with a caller-chosen nesting-depth limit (see
/// [`read_next_with_depth_limit`] for how to size it).
pub fn read_with_depth_limit(
    input: &str,
    env: &Shared<Environment>,
    depth_limit: usize,
) -> Result<LispVal, String> {
    let src = strip_shebang(input);
    match read_next_with_depth_limit(src, env, depth_limit) {
        Ok(None) => Err("empty input".to_string()),
        Ok(Some((val, rest))) => {
            let rest = skip_ws(rest);
            if rest.is_empty() {
                Ok(val)
            } else {
                Err(format_parse_error(
                    src,
                    src.len() - rest.len(),
                    &format!("unexpected trailing input: {}", rest.trim_end()),
                ))
            }
        }
        Err((offset, detail)) => Err(format_parse_error(src, offset, &detail)),
    }
}

/// Parse zero or more s-expressions from `input` and return them in order,
/// with the default nesting-depth limit ([`DEFAULT_READER_DEPTH`]).
///
/// This is the function used for loading files and multi-expression strings.
/// Stops at EOF; returns an error on the first malformed expression.
pub fn read_all(input: &str, env: &Shared<Environment>) -> Result<Vec<LispVal>, String> {
    read_all_with_depth_limit(input, env, DEFAULT_READER_DEPTH)
}

/// Like [`read_all`], but with a caller-chosen nesting-depth limit (see
/// [`read_next_with_depth_limit`] for how to size it).
pub fn read_all_with_depth_limit(
    input: &str,
    env: &Shared<Environment>,
    depth_limit: usize,
) -> Result<Vec<LispVal>, String> {
    let src = strip_shebang(input);
    let mut results = vec![];
    let mut current = src;
    loop {
        current = skip_ws(current);
        let form_offset = src.len() - current.len();
        match read_next_with_depth_limit(current, env, depth_limit) {
            Ok(None) => return Ok(results),
            Ok(Some((val, rest))) => {
                results.push(val);
                current = rest;
            }
            Err((offset, detail)) => {
                let absolute = error_anchor(src, form_offset, offset, &detail);
                return Err(format_parse_error(src, absolute, &detail));
            }
        }
    }
}

/// Pick the byte offset to report for a parse error: normally where parsing
/// stopped, but for errors at end of input (an unclosed form) the *start* of
/// the offending form — "line 3: unclosed '('" beats "line 47: end of file".
pub fn error_anchor(src: &str, form_offset: usize, error_offset: usize, detail: &str) -> usize {
    let absolute = form_offset + error_offset;
    if detail.contains("end of input") || absolute >= src.trim_end().len() {
        form_offset
    } else {
        absolute
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cons(car: LispVal, cdr: LispVal) -> LispVal {
        LispVal::Cons {
            car: Shared::new(car),
            cdr: Shared::new(cdr),
        }
    }

    fn symbol(s: &str, env: &Shared<Environment>) -> LispVal {
        LispVal::Symbol(env.intern_symbol(s))
    }

    fn number(n: i64) -> LispVal {
        LispVal::Number(n)
    }

    fn float(f: f64) -> LispVal {
        LispVal::Float(f)
    }

    #[test]
    fn test_parse_number() {
        assert_eq!(parse_number("123"), Ok(("", number(123))));
        assert_eq!(parse_number("-456"), Ok(("", number(-456))));
    }

    #[test]
    fn test_parse_octal() {
        // 177Q = 1*64 + 7*8 + 7 = 127 decimal
        assert_eq!(parse_number("177Q"), Ok(("", number(127))));
        // 10Q = 8 decimal
        assert_eq!(parse_number("10Q"), Ok(("", number(8))));
        // 0Q = 0
        assert_eq!(parse_number("0Q"), Ok(("", number(0))));
        // negative octal
        assert_eq!(parse_number("-10Q"), Ok(("", number(-8))));
        // non-octal digits still parse as decimal (no trailing Q)
        assert_eq!(parse_number("177"), Ok(("", number(177))));
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(parse_float("3.25"), Ok(("", float(3.25))));
        assert_eq!(parse_float("-0.5"), Ok(("", float(-0.5))));
    }

    #[test]
    fn test_parse_hex() {
        // Assembly-style H suffix; case-insensitive digits and marker.
        assert_eq!(parse_number("ffh"), Ok(("", number(255))));
        assert_eq!(parse_number("FFH"), Ok(("", number(255))));
        assert_eq!(parse_number("0ffh"), Ok(("", number(255))));
        assert_eq!(parse_number("1Ah"), Ok(("", number(26))));
        assert_eq!(parse_number("10h"), Ok(("", number(16))));
        assert_eq!(parse_number("0ah"), Ok(("", number(10))));
        assert_eq!(parse_number("-ffh"), Ok(("", number(-255))));
        assert_eq!(parse_number("deadh"), Ok(("", number(0xDEAD))));
        // Boundary: a following identifier char means it was a symbol, so the
        // hex parser must reject and leave the input for the symbol parser.
        assert!(parse_hex_integer("ffhello").is_err());
        // No H suffix: not hex (handled by the symbol parser, not parse_number).
        assert!(parse_hex_integer("ff").is_err());
    }

    #[test]
    fn test_parse_char_literal() {
        // 'c' is a Char value carrying the byte code point.
        assert_eq!(parse_char_literal("'A'"), Ok(("", LispVal::Char(65))));
        assert_eq!(parse_char_literal("'0'"), Ok(("", LispVal::Char(48))));
        assert_eq!(parse_char_literal("' '"), Ok(("", LispVal::Char(32))));
        // escapes
        assert_eq!(parse_char_literal("'\\n'"), Ok(("", LispVal::Char(10))));
        assert_eq!(parse_char_literal("'\\''"), Ok(("", LispVal::Char(39))));
        assert_eq!(parse_char_literal("'\\\\'"), Ok(("", LispVal::Char(92))));
        // trailing input is left for the next parser
        assert_eq!(parse_char_literal("'a'b"), Ok(("b", LispVal::Char(97))));
    }

    #[test]
    fn test_char_literal_vs_quote() {
        // 'a (no closing quote) is NOT a char literal; the quote macro handles it.
        assert!(parse_char_literal("'a").is_err());
        assert!(parse_char_literal("'(1 2)").is_err());
        // The empty '' is not a char literal.
        assert!(parse_char_literal("''").is_err());
        // Full reader: 'a' is a Char, 'a is (quote a).
        let env = Shared::new(Environment::new());
        assert_eq!(read("'A'", &env), Ok(LispVal::Char(65)));
        assert_eq!(
            read("'a", &env),
            Ok(cons(
                symbol("QUOTE", &env),
                cons(symbol("A", &env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_parse_atom() {
        let env = Shared::new(Environment::new());
        assert_eq!(
            parse_atom(env.clone())("abc"),
            Ok(("", symbol("ABC", &env)))
        );
        assert_eq!(
            parse_atom(env.clone())("with-hyphen"),
            Ok(("", symbol("WITH-HYPHEN", &env)))
        );
        assert_eq!(
            parse_atom(env.clone())(":op"),
            Ok(("", symbol(":OP", &env)))
        );
        assert_eq!(
            parse_atom(env.clone())(":with-hyphen"),
            Ok(("", symbol(":WITH-HYPHEN", &env)))
        );
    }

    #[test]
    fn test_parse_multichar_symbols() {
        let env = Shared::new(Environment::new());
        // alpha + operator suffix
        assert_eq!(parse_atom(env.clone())("v+"), Ok(("", symbol("V+", &env))));
        assert_eq!(parse_atom(env.clone())("v-"), Ok(("", symbol("V-", &env))));
        // multi-char operator sequences
        assert_eq!(parse_atom(env.clone())(">="), Ok(("", symbol(">=", &env))));
        assert_eq!(parse_atom(env.clone())("<="), Ok(("", symbol("<=", &env))));
        assert_eq!(parse_atom(env.clone())("!="), Ok(("", symbol("!=", &env))));
        assert_eq!(parse_atom(env.clone())("/="), Ok(("", symbol("/=", &env))));
        // single operators still work
        assert_eq!(parse_atom(env.clone())("+"), Ok(("", symbol("+", &env))));
        assert_eq!(parse_atom(env.clone())("-"), Ok(("", symbol("-", &env))));
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(
            parse_string("\"hello world\""),
            Ok(("", LispVal::String("hello world".to_string())))
        );
    }

    #[test]
    fn test_parse_list() {
        let env = Shared::new(Environment::new());
        assert_eq!(
            parse_list(env.clone(), DEFAULT_READER_DEPTH)("(PLUS 1 2)"),
            Ok((
                "",
                cons(
                    symbol("PLUS", &env),
                    cons(number(1), cons(number(2), LispVal::Nil))
                )
            ))
        );
    }

    #[test]
    fn test_read_simple_list() {
        let env = Shared::new(Environment::new());
        let result = read("(PLUS 10 20)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(number(10), cons(number(20), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_nested_list() {
        let env = Shared::new(Environment::new());
        let result = read("(PLUS 10 (TIMES 5 2))", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(
                    number(10),
                    cons(
                        cons(
                            symbol("TIMES", &env),
                            cons(number(5), cons(number(2), LispVal::Nil))
                        ),
                        LispVal::Nil
                    )
                )
            ))
        );
    }

    #[test]
    fn test_read_dotted_list() {
        let env = Shared::new(Environment::new());
        let result = read("(a . b)", &env);
        assert_eq!(result, Ok(cons(symbol("A", &env), symbol("B", &env))));
    }

    #[test]
    fn test_read_rejects_dot_without_leading_element() {
        let env = Shared::new(Environment::new());
        assert!(read("(. 1)", &env).is_err());
    }

    #[test]
    fn test_read_complex_dotted_list() {
        let env = Shared::new(Environment::new());
        let result = read("(a b . c)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("A", &env),
                cons(symbol("B", &env), symbol("C", &env))
            ))
        );
    }

    #[test]
    fn test_comment() {
        let env = Shared::new(Environment::new());
        let result = read(
            "
            ; this is a comment
            (PLUS 1 2) ; another comment
        ",
            &env,
        );
        assert_eq!(
            result,
            Ok(cons(
                symbol("PLUS", &env),
                cons(number(1), cons(number(2), LispVal::Nil))
            ))
        );
    }

    #[test]
    fn test_read_quoted() {
        let env = Shared::new(Environment::new());
        let result = read("'a", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUOTE", &env),
                cons(symbol("A", &env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_read_quasiquote() {
        let env = Shared::new(Environment::new());
        let result = read("`(a ,b)", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("QUASIQUOTE", &env),
                cons(
                    cons(
                        symbol("A", &env),
                        cons(
                            cons(
                                symbol("UNQUOTE", &env),
                                cons(symbol("B", &env), LispVal::Nil)
                            ),
                            LispVal::Nil
                        )
                    ),
                    LispVal::Nil
                )
            ))
        );
    }

    #[test]
    fn test_read_unquote_splicing() {
        let env = Shared::new(Environment::new());
        // ,@xs reads as (UNQUOTE-SPLICING xs)
        let result = read(",@xs", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("UNQUOTE-SPLICING", &env),
                cons(symbol("XS", &env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_read_unquote_vs_splicing() {
        let env = Shared::new(Environment::new());
        // ,x (no @) stays plain UNQUOTE, not splicing
        let result = read(",x", &env);
        assert_eq!(
            result,
            Ok(cons(
                symbol("UNQUOTE", &env),
                cons(symbol("X", &env), LispVal::Nil)
            ))
        );
    }

    #[test]
    fn test_read_nil() {
        let env = Shared::new(Environment::new());
        assert_eq!(read("NIL", &env), Ok(LispVal::Nil));
    }

    #[test]
    fn test_read_t() {
        let env = Shared::new(Environment::new());
        assert_eq!(read("T", &env), Ok(symbol("T", &env)));
    }
}
