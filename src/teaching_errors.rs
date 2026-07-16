//! "Teaching errors": did-you-mean suggestions and Common-Lisp-ism guidance
//! appended to unbound-symbol / undefined-function errors.
//!
//! Lamedh has essentially zero presence in LLM training data, so error
//! messages double as in-context documentation for both humans and models.
//! Two mechanisms live here:
//!
//! 1. **Did-you-mean**: when a symbol lookup fails, scan the *bound* symbols
//!    (not merely interned ones) for near matches by Levenshtein distance and
//!    suggest up to three.
//! 2. **CL-ism guidance**: a small static table of well-known Common Lisp
//!    forms that Lamedh deliberately does not have, each paired with the
//!    idiomatic Lamedh replacement. This takes precedence over did-you-mean
//!    when both would fire, since a targeted redirect is more useful than a
//!    fuzzy-match guess.
//!
//! This module only runs on the (cold) error-construction path, so an O(n)
//! scan over bound symbols is acceptable — see `AGENTS.md`'s note that
//! kernel changes should stay small and Rust-side work should be reserved
//! for things with no Lisp-layer expression; this is a small, self-contained
//! exception because it hangs directly off the error-formatting call sites.

/// Well-known Common Lisp forms that Lamedh deliberately lacks, paired with
/// the idiomatic Lamedh replacement to point users at.
///
/// Every entry here has been checked against `lib/*.lisp` and
/// `src/environment.rs`'s special-form table to confirm both that the CL
/// form is genuinely absent *and* that the suggested replacement genuinely
/// exists. Do not add an entry without doing the same — see the
/// `read-the-owner` guidance in project memory.
const CL_ISMS: &[(&str, &str)] = &[
    ("LOOP", "use DOTIMES, WHILE, or MAP"),
    ("DEFSTRUCT", "removed in 0.3 — use DEFRECORD"),
    ("DEFCLASS", "use DEFPROTOCOL and DEFINSTANCE"),
    ("DEFMETHOD", "use DEFINSTANCE (with DEFPROTOCOL)"),
    ("DEFGENERIC", "use DEFPROTOCOL"),
    (
        "DEFCONSTANT",
        "use DEF (Lamedh has no separate constant-binding form)",
    ),
    (
        "MULTIPLE-VALUE-BIND",
        "Lamedh has no multiple return values — return a LIST and use DESTRUCTURING-BIND",
    ),
    (
        "VALUES",
        "Lamedh has no multiple return values — return a LIST directly",
    ),
    ("WITH-OPEN-FILE", "use WITH-OPEN-PORT"),
];

/// Look up `name` (already uppercased) in the CL-ism table.
fn cl_ism_hint(name: &str) -> Option<&'static str> {
    CL_ISMS
        .iter()
        .find(|(cl_name, _)| *cl_name == name)
        .map(|(_, hint)| *hint)
}

/// Levenshtein edit distance between two strings, computed over `char`s.
///
/// Classic single-row DP; `a`/`b` are short symbol names so this is cheap
/// even called once per candidate on the error path.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (la, lb) = (a.len(), b.len());
    if la == 0 {
        return lb;
    }
    if lb == 0 {
        return la;
    }
    let mut prev: Vec<usize> = (0..=lb).collect();
    let mut cur: Vec<usize> = vec![0; lb + 1];
    for i in 1..=la {
        cur[0] = i;
        for j in 1..=lb {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[lb]
}

/// Maximum edit distance to accept as a "close" match, based on the query's
/// length: no suggestions below length 3 (too noisy), distance <= 1 at
/// length 3, distance <= 2 from length 4 up.
fn max_distance_for(len: usize) -> Option<usize> {
    match len {
        0..=2 => None,
        3 => Some(1),
        _ => Some(2),
    }
}

/// Number of suggestions to cap the "did you mean" list at.
const MAX_SUGGESTIONS: usize = 3;

/// Find up to [`MAX_SUGGESTIONS`] bound-symbol names close to `name` by edit
/// distance, sorted by distance then alphabetically for determinism.
///
/// `name` must already be uppercased (interned symbols are case-normalized
/// to uppercase). `bound_names` is expected to be the set of symbol names
/// that actually have a binding (see `Environment::bound_symbol_names`), not
/// merely interned ones — otherwise this suggests garbage left over from
/// gensyms or one-off reader interning.
fn did_you_mean(name: &str, bound_names: impl Iterator<Item = String>) -> Vec<String> {
    let Some(max_dist) = max_distance_for(name.chars().count()) else {
        return Vec::new();
    };
    let mut candidates: Vec<(usize, String)> = bound_names
        .filter(|candidate| candidate != name)
        .filter_map(|candidate| {
            let dist = levenshtein(name, &candidate);
            if dist <= max_dist {
                Some((dist, candidate))
            } else {
                None
            }
        })
        .collect();
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.truncate(MAX_SUGGESTIONS);
    candidates.into_iter().map(|(_, name)| name).collect()
}

/// Format the did-you-mean suggestion list as an error-message suffix, e.g.
/// `" — did you mean LENGTH?"` or `" — did you mean LENGTH, LENGTHP?"`.
/// Returns an empty string when there are no suggestions.
fn format_suggestions(suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        return String::new();
    }
    format!(" — did you mean {}?", suggestions.join(", "))
}

/// Build the full "teaching" suffix to append to an unbound-symbol /
/// undefined-function error message for `name` (already uppercased).
///
/// CL-ism guidance takes precedence: if `name` matches a known Common Lisp
/// form Lamedh deliberately lacks, return that guidance instead of a fuzzy
/// did-you-mean match (a targeted redirect beats a guess). Otherwise, scan
/// `bound_names` for close matches and format up to three as a suggestion
/// suffix. Returns an empty string when neither applies, so callers can
/// unconditionally append the result without special-casing "no hint".
pub fn teaching_suffix(name: &str, bound_names: impl Iterator<Item = String>) -> String {
    if let Some(hint) = cl_ism_hint(name) {
        // The name itself is already in the base error message (e.g.
        // "Unbound variable: LOOP"), so the suffix reads as a continuation
        // rather than repeating it: "...LOOP is Common Lisp, not Lamedh —
        // use DOTIMES, WHILE, or MAP".
        return format!(" is Common Lisp, not Lamedh — {hint}");
    }
    format_suggestions(&did_you_mean(name, bound_names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("LENGTH", "LENGTH"), 0);
        assert_eq!(levenshtein("LENGHT", "LENGTH"), 2);
        assert_eq!(levenshtein("KITTEN", "SITTING"), 3);
    }

    #[test]
    fn short_names_get_no_suggestions() {
        let bound = vec!["ZZ".to_string(), "ZX".to_string()];
        assert!(did_you_mean("ZZ", bound.into_iter()).is_empty());
    }

    #[test]
    fn length_three_uses_distance_one() {
        let bound = vec!["FOO".to_string(), "FOP".to_string(), "BAZ".to_string()];
        let got = did_you_mean("FOX", bound.into_iter());
        assert_eq!(got, vec!["FOO".to_string(), "FOP".to_string()]);
    }

    #[test]
    fn cl_ism_precedes_did_you_mean() {
        // LOOP is both a plausible CL-ism AND, hypothetically, close to some
        // bound symbol -- guidance must win.
        let bound = vec!["LOOT".to_string()];
        let suffix = teaching_suffix("LOOP", bound.into_iter());
        assert!(suffix.contains("Common Lisp"));
        assert!(suffix.contains("DOTIMES"));
    }

    #[test]
    fn no_hint_is_empty_string() {
        let bound = vec!["COMPLETELY-UNRELATED-NAME".to_string()];
        assert_eq!(
            teaching_suffix("SOME-VERY-LONG-NONSENSE-WORD", bound.into_iter()),
            ""
        );
    }
}
