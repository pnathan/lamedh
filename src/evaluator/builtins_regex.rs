//! Regex kernel substrate (`lib/44-regex.lisp`).
//!
//! Every `REGEX-*` primitive here is a thin argument-parsing wrapper around
//! the `regex` crate, following the same "representation access is Rust,
//! policy is Lisp" split as the rest of the kernel (see `CLAUDE.md`) --
//! `lib/44-regex.lisp` is expected to wrap these into the Lisp-facing
//! `REGEX` module names, predicates, and any higher-level convenience forms.
//!
//! **Representation.** A compiled pattern is an opaque host value,
//! [`CompiledRegex`], carried as `LispVal::Extension` (`LispVal::ext(...)`)
//! rather than a new `LispVal` variant -- matching regexes are pure,
//! deterministic computation over already-in-memory strings, so unlike
//! ports/network handles/TLS streams there is no host resource to model, no
//! open/close lifecycle, and therefore no need for a first-class kernel
//! representation or capability gate. Every primitive that takes a "regex"
//! argument accepts either a `CompiledRegex` extension or a plain pattern
//! string -- see [`coerce_regex`] -- so callers that only ever match a
//! pattern once don't need to call `REGEX-COMPILE*` first.
//!
//! **Positions are character indices, not byte offsets.** The `regex` crate
//! itself only knows about UTF-8 byte offsets; every primitive below
//! converts to/from character indices at the boundary (`byte_to_char_index`
//! / `char_index_to_byte`) so Lisp-level code never has to reason about
//! UTF-8 byte layout, matching how the rest of the kernel's string API
//! (`lib/30-text.lisp`, `STRING-LENGTH*`, `SUBSTRING*`, ...) is
//! character-indexed.
//!
//! **No capability gate.** Compiling and running a regex touches no host
//! resource (no filesystem, network, shell, or stdin), so -- unlike
//! `builtins_ports.rs`/`builtins_net.rs`/`builtins_tls.rs` -- nothing here
//! calls a `require_*` capability check.

use super::*;
use crate::LispValExtension;
use std::hash::Hasher;

// ── Compiled-regex extension type ─────────────────────────────────────────

/// A compiled `regex::Regex`, wrapped as a [`crate::LispValExtension`] so it
/// can be carried as an opaque, printable `LispVal::Extension` value (see
/// this module's doc comment for why a new `LispVal` variant isn't
/// warranted).
#[derive(Debug)]
pub struct CompiledRegex {
    pub regex: regex::Regex,
}

impl LispValExtension for CompiledRegex {
    fn type_name(&self) -> &str {
        "REGEX"
    }

    fn display(&self) -> String {
        format!("#<REGEX {:?}>", self.regex.as_str())
    }

    // Identity equality: two compiled regexes are equal iff they were
    // compiled from the same source pattern (not e.g. by comparing the
    // compiled automaton), mirroring how ports/net-handles compare by name.
    fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
        other
            .as_any()
            .downcast_ref::<CompiledRegex>()
            .is_some_and(|o| o.regex.as_str() == self.regex.as_str())
    }

    fn hash_ext(&self, state: &mut dyn Hasher) {
        // `Hash::hash` is generic over `H: Hasher` (so `Sized`), which a
        // `&mut dyn Hasher` trait object can't satisfy -- write the pattern
        // bytes directly via the (non-generic, dyn-compatible) `Hasher::write`.
        state.write(self.regex.as_str().as_bytes());
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── re-or-string coercion ─────────────────────────────────────────────────

/// Either a borrowed, already-compiled [`regex::Regex`] (from a
/// `CompiledRegex` extension argument) or one freshly compiled from a
/// pattern string argument. [`coerce_regex`] returns this instead of `&Regex`
/// directly because the string case has nowhere else to own the freshly
/// compiled `Regex` for the duration of the call.
enum CoercedRegex<'a> {
    Borrowed(&'a regex::Regex),
    Owned(regex::Regex),
}

impl std::ops::Deref for CoercedRegex<'_> {
    type Target = regex::Regex;

    fn deref(&self) -> &regex::Regex {
        match self {
            CoercedRegex::Borrowed(re) => re,
            CoercedRegex::Owned(re) => re,
        }
    }
}

/// Accept either a compiled regex (`LispVal::Extension(CompiledRegex)`) or a
/// plain pattern string (`LispVal::String`), compiling the latter on the
/// fly. Every `REGEX-*` primitive that takes a "regex" argument goes through
/// this so `(regex-is-match* "^[0-9]+$" s)` and
/// `(regex-is-match* (regex-compile* "^[0-9]+$") s)` both work.
fn coerce_regex<'a>(v: &'a LispVal, who: &str) -> Result<CoercedRegex<'a>, LispError> {
    match v {
        LispVal::Extension(ext) => {
            let compiled = ext
                .as_any()
                .downcast_ref::<CompiledRegex>()
                .ok_or_else(|| {
                    LispError::Generic(format!(
                        "{}: expected a compiled regex or a pattern string, got extension type {}",
                        who.to_uppercase(),
                        ext.type_name()
                    ))
                })?;
            Ok(CoercedRegex::Borrowed(&compiled.regex))
        }
        LispVal::String(s) => regex::Regex::new(s).map(CoercedRegex::Owned).map_err(|e| {
            LispError::Generic(format!(
                "{}: invalid regex pattern {s:?}: {e}",
                who.to_uppercase()
            ))
        }),
        other => Err(LispError::Generic(format!(
            "{}: expected a compiled regex or a pattern string, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
    }
}

// ── Character-index <-> byte-offset conversion ────────────────────────────

/// Count the characters in `s` up to (but not including) byte offset
/// `byte_idx`. `byte_idx` must land on a UTF-8 char boundary -- true of
/// every offset this module feeds it, since they all come from `regex`
/// match boundaries or from [`char_index_to_byte`] itself.
fn byte_to_char_index(s: &str, byte_idx: usize) -> i64 {
    s[..byte_idx].chars().count() as i64
}

/// The byte offset of character index `char_idx` in `s`, clamped to `s.len()`
/// when `char_idx` is at or past the end of the string. Used to turn a
/// caller-supplied `start-char` argument into a byte position to slice at.
fn char_index_to_byte(s: &str, char_idx: usize) -> usize {
    match s.char_indices().nth(char_idx) {
        Some((byte_idx, _)) => byte_idx,
        None => s.len(),
    }
}

// ── Argument helpers (mirrors builtins_net.rs / builtins_ports.rs) ────────

fn expect_string(args: &[LispVal], i: usize, who: &str) -> Result<String, LispError> {
    match args.get(i) {
        Some(LispVal::String(s)) => Ok(s.clone()),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a string, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a string argument"
        ))),
    }
}

/// `nil`/absent or a non-negative integer; used for `REGEX-FIND*`'s optional
/// `start-char` argument (character index to start searching from, default
/// 0).
fn optional_start_char(args: &[LispVal], i: usize, who: &str) -> Result<usize, LispError> {
    match args.get(i) {
        None | Some(LispVal::Nil) => Ok(0),
        Some(LispVal::Number(n)) if *n >= 0 => Ok(*n as usize),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a non-negative integer, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
    }
}

/// `nil`/absent or a positive integer; used for `REGEX-SPLIT*`'s optional
/// `limit` argument. `nil`/absent means "no limit" (split on every match).
fn optional_limit(args: &[LispVal], i: usize, who: &str) -> Result<Option<usize>, LispError> {
    match args.get(i) {
        None | Some(LispVal::Nil) => Ok(None),
        Some(LispVal::Number(n)) if *n > 0 => Ok(Some(*n as usize)),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected NIL or a positive integer, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
    }
}

/// Build a `(text start end)` triple, the shape every match-returning
/// primitive below (`REGEX-FIND*`, `REGEX-FIND-ALL*`, `REGEX-CAPTURES*`,
/// `REGEX-CAPTURES-NAMED*`) uses for a single match. `start`/`end` are
/// character indices, not byte offsets -- see this module's doc comment.
fn match_triple(text: &str, start_char: i64, end_char: i64) -> LispVal {
    vec_to_list(vec![
        LispVal::String(text.to_string()),
        LispVal::Number(start_char),
        LispVal::Number(end_char),
    ])
}

#[inline(never)]
pub(super) fn apply_regex_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    match op {
        BuiltinFunc::RegexCompile => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "regex-compile* requires exactly one argument: pattern".to_string(),
                ));
            }
            let pattern = expect_string(args, 0, "regex-compile*")?;
            let re = regex::Regex::new(&pattern).map_err(|e| {
                LispError::Generic(format!(
                    "REGEX-COMPILE*: invalid regex pattern {pattern:?}: {e}"
                ))
            })?;
            Ok(LispVal::ext(CompiledRegex { regex: re }))
        }
        BuiltinFunc::RegexP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "regex-p* requires exactly one argument".to_string(),
                ));
            }
            Ok(match &args[0] {
                LispVal::Extension(ext)
                    if ext.as_any().downcast_ref::<CompiledRegex>().is_some() =>
                {
                    t()
                }
                _ => LispVal::Nil,
            })
        }
        BuiltinFunc::RegexPattern => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "regex-pattern* requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Extension(ext) => {
                    let compiled = ext.as_any().downcast_ref::<CompiledRegex>().ok_or_else(
                        || {
                            LispError::Generic(format!(
                                "REGEX-PATTERN*: expected a compiled regex, got extension type {}",
                                ext.type_name()
                            ))
                        },
                    )?;
                    Ok(LispVal::String(compiled.regex.as_str().to_string()))
                }
                other => Err(LispError::Generic(format!(
                    "REGEX-PATTERN*: expected a compiled regex, got {}",
                    err_val(other)
                ))),
            }
        }
        BuiltinFunc::RegexEscape => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "regex-escape* requires exactly one argument".to_string(),
                ));
            }
            let s = expect_string(args, 0, "regex-escape*")?;
            Ok(LispVal::String(regex::escape(&s)))
        }
        BuiltinFunc::RegexIsMatch => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "regex-is-match* requires exactly two arguments: re s".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-is-match*")?;
            let s = expect_string(args, 1, "regex-is-match*")?;
            Ok(if re.is_match(&s) { t() } else { LispVal::Nil })
        }
        BuiltinFunc::RegexFind => {
            if args.len() < 2 || args.len() > 3 {
                return Err(LispError::Generic(
                    "regex-find* requires two or three arguments: re s [start-char]".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-find*")?;
            let s = expect_string(args, 1, "regex-find*")?;
            let start_char = optional_start_char(args, 2, "regex-find*")?;
            // Slice at the byte position of start-char, search within the
            // slice, then add start_char back onto the returned character
            // positions (the slice starts exactly at a char boundary, so
            // byte_to_char_index on the slice gives an offset relative to
            // start_char, not to the start of the whole string).
            let byte_start = char_index_to_byte(&s, start_char);
            let slice = &s[byte_start..];
            match re.find(slice) {
                Some(m) => {
                    let text = m.as_str();
                    let start = start_char as i64 + byte_to_char_index(slice, m.start());
                    let end = start_char as i64 + byte_to_char_index(slice, m.end());
                    Ok(match_triple(text, start, end))
                }
                None => Ok(LispVal::Nil),
            }
        }
        BuiltinFunc::RegexFindAll => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "regex-find-all* requires exactly two arguments: re s".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-find-all*")?;
            let s = expect_string(args, 1, "regex-find-all*")?;
            let items: Vec<LispVal> = re
                .find_iter(&s)
                .map(|m| {
                    let start = byte_to_char_index(&s, m.start());
                    let end = byte_to_char_index(&s, m.end());
                    match_triple(m.as_str(), start, end)
                })
                .collect();
            Ok(vec_to_list(items))
        }
        BuiltinFunc::RegexCaptures => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "regex-captures* requires exactly two arguments: re s".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-captures*")?;
            let s = expect_string(args, 1, "regex-captures*")?;
            match re.captures(&s) {
                Some(caps) => {
                    let mut items = Vec::with_capacity(caps.len());
                    for i in 0..caps.len() {
                        let item = match caps.get(i) {
                            Some(m) => {
                                let start = byte_to_char_index(&s, m.start());
                                let end = byte_to_char_index(&s, m.end());
                                match_triple(m.as_str(), start, end)
                            }
                            None => LispVal::Nil,
                        };
                        items.push(item);
                    }
                    Ok(vec_to_list(items))
                }
                None => Ok(LispVal::Nil),
            }
        }
        BuiltinFunc::RegexCapturesNamed => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "regex-captures-named* requires exactly two arguments: re s".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-captures-named*")?;
            let s = expect_string(args, 1, "regex-captures-named*")?;
            match re.captures(&s) {
                Some(caps) => {
                    let mut items = Vec::new();
                    for name in re.capture_names().flatten() {
                        let value = match caps.name(name) {
                            Some(m) => {
                                let start = byte_to_char_index(&s, m.start());
                                let end = byte_to_char_index(&s, m.end());
                                match_triple(m.as_str(), start, end)
                            }
                            None => LispVal::Nil,
                        };
                        items.push(LispVal::Cons {
                            car: Shared::new(LispVal::String(name.to_string())),
                            cdr: Shared::new(value),
                        });
                    }
                    Ok(vec_to_list(items))
                }
                None => Ok(LispVal::Nil),
            }
        }
        BuiltinFunc::RegexReplace => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "regex-replace* requires exactly three arguments: re s replacement".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-replace*")?;
            let s = expect_string(args, 1, "regex-replace*")?;
            let template = expect_string(args, 2, "regex-replace*")?;
            let result = re.replacen(&s, 1, template.as_str());
            Ok(LispVal::String(result.into_owned()))
        }
        BuiltinFunc::RegexReplaceAll => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "regex-replace-all* requires exactly three arguments: re s replacement"
                        .to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-replace-all*")?;
            let s = expect_string(args, 1, "regex-replace-all*")?;
            let template = expect_string(args, 2, "regex-replace-all*")?;
            let result = re.replace_all(&s, template.as_str());
            Ok(LispVal::String(result.into_owned()))
        }
        BuiltinFunc::RegexSplit => {
            if args.len() < 2 || args.len() > 3 {
                return Err(LispError::Generic(
                    "regex-split* requires two or three arguments: re s [limit]".to_string(),
                ));
            }
            let re = coerce_regex(&args[0], "regex-split*")?;
            let s = expect_string(args, 1, "regex-split*")?;
            let limit = optional_limit(args, 2, "regex-split*")?;
            let parts: Vec<LispVal> = match limit {
                Some(n) => re
                    .splitn(&s, n)
                    .map(|p| LispVal::String(p.to_string()))
                    .collect(),
                None => re
                    .split(&s)
                    .map(|p| LispVal::String(p.to_string()))
                    .collect(),
            };
            Ok(vec_to_list(parts))
        }
        _ => Err(LispError::Generic("Not a regex operation".to_string())),
    }
}
