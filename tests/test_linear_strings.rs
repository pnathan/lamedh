//! Issue #510: STRING->LIST, STRING-SPLIT, STRING-JOIN and REMOVE-DUPLICATES
//! were quadratic. SUBSTRING indexes by character, so a Lisp loop over it
//! rescans the UTF-8 string on every step; STRING-JOIN re-copied its growing
//! accumulator on every CONCAT; REMOVE-DUPLICATES scanned the rest of the
//! list for every element.
//!
//! Coverage: each rewritten function agrees with its pre-#510 definition
//! (kept here verbatim under a `$old-` name) across ASCII, multibyte and
//! non-BMP input and the edge cases each docstring promises; and each runs
//! at 100k characters / 20k list items within a bound the quadratic
//! versions missed by orders of magnitude (seconds in a release build for
//! the strings; not finishing within 60 s for REMOVE-DUPLICATES).

use lamedh::Shared;
use lamedh::environment::Environment;
use lamedh::eval_line;
use std::time::{Duration, Instant};

/// The pre-#510 definitions, for differential checks.
const OLD_DEFS: &[&str] = &[
    "(defun $old-string->list-aux (s i acc)
       (if (< i 0)
           acc
           ($old-string->list-aux s (- i 1) (cons (substring s i (+ i 1)) acc))))",
    "(defun $old-string->list (s)
       ($old-string->list-aux s (- (string-length* s) 1) nil))",
    "(defun $old-string-split-aux (s delim acc)
       (let ((idx (string-index-of s delim)))
         (if (or (null idx) (= (string-length* delim) 0))
             (reverse-aux (cons s acc) nil)
             ($old-string-split-aux
              (substring s (+ idx (string-length* delim)) (string-length* s))
              delim
              (cons (substring s 0 idx) acc)))))",
    "(defun $old-string-split (s delim) ($old-string-split-aux s delim nil))",
    "(defun $old-string-join-aux (lst sep acc)
       (if (null lst)
           acc
           ($old-string-join-aux (cdr lst) sep (concat acc sep (car lst)))))",
    "(defun $old-string-join (lst sep)
       (cond ((null lst) \"\")
             ((null (cdr lst)) (car lst))
             (t ($old-string-join-aux (cdr lst) sep (car lst)))))",
    "(defun $old-remove-duplicates-aux (lst acc)
       (cond ((null lst) (reverse-aux acc nil))
             ((member (car lst) (cdr lst))
              ($old-remove-duplicates-aux (remove-all (cdr lst) (car lst))
                                          (cons (car lst) acc)))
             (t ($old-remove-duplicates-aux (cdr lst) (cons (car lst) acc)))))",
    "(defun $old-remove-duplicates (lst) ($old-remove-duplicates-aux lst nil))",
];

fn env_with_old_defs() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    for src in OLD_DEFS {
        let out = eval_line(src, &env);
        assert!(!out.starts_with("Error"), "defining `{src}` failed: {out}");
    }
    env
}

/// Evaluate `expr` and assert it prints `expected`.
fn check(env: &Shared<Environment>, expr: &str, expected: &str) {
    assert_eq!(eval_line(expr, env), expected, "for {expr}");
}

/// Assert the new and old definitions agree on `args` (Lisp source).
fn agrees(env: &Shared<Environment>, new: &str, old: &str, args: &str) {
    let got = eval_line(&format!("({new} {args})"), env);
    let want = eval_line(&format!("({old} {args})"), env);
    assert!(!want.starts_with("Error"), "({old} {args}) failed: {want}");
    assert_eq!(
        got, want,
        "({new} {args}) diverged from the pre-#510 result"
    );
}

const STRINGS: &[&str] = &[
    "",
    "a",
    "hello",
    "café",
    "世界",
    "🎉x🎉",
    "a,b,,c,",
    ",",
    ",,",
    "aaa",
    "α→β→→γ→",
    "→",
    "x→y",
];

#[test]
fn string_to_list_matches_old_definition() {
    let env = env_with_old_defs();
    for s in STRINGS {
        agrees(&env, "string->list", "$old-string->list", &format!("{s:?}"));
    }
    check(&env, "(string->list \"\")", "()");
    check(&env, "(string->list \"é→🎉\")", "(\"é\" \"→\" \"🎉\")");
}

#[test]
fn list_to_string_round_trips_multibyte() {
    let env = Environment::with_stdlib();
    for s in STRINGS {
        check(
            &env,
            &format!("(list->string (string->list {s:?}))"),
            &format!("{s:?}"),
        );
    }
}

#[test]
fn string_split_matches_old_definition() {
    let env = env_with_old_defs();
    let delims = ["", ",", ",,", "a", "aa", "→", "→→", "🎉", "zz"];
    for s in STRINGS {
        for d in delims {
            agrees(
                &env,
                "string-split",
                "$old-string-split",
                &format!("{s:?} {d:?}"),
            );
        }
    }
    // The docstring's own example; an empty DELIM yields (S).
    check(
        &env,
        "(string-split \",a,,b,\" \",\")",
        "(\"\" \"a\" \"\" \"b\" \"\")",
    );
    check(&env, "(string-split \"abc\" \"\")", "(\"abc\")");
    check(&env, "(string-split \"aaa\" \"aa\")", "(\"\" \"a\")");
}

#[test]
fn string_join_matches_old_definition() {
    let env = env_with_old_defs();
    for lst in [
        "nil",
        "(list \"a\")",
        "(list \"a\" \"b\")",
        "(list \"\" \"\" \"\")",
        "(list \"α\" \"世界\" \"🎉\")",
    ] {
        for sep in ["", ",", "→", ", "] {
            agrees(
                &env,
                "string-join",
                "$old-string-join",
                &format!("{lst} {sep:?}"),
            );
        }
    }
    // A single element is returned unchanged, even a non-string.
    check(&env, "(string-join (list 5) \",\")", "5");
    // Two or more non-strings still signal an error, as CONCAT did.
    assert!(eval_line("(string-join (list \"a\" 5) \",\")", &env).starts_with("Error"));
}

#[test]
fn remove_duplicates_matches_old_definition() {
    let env = env_with_old_defs();
    eval_line("(defrecord Pt510 (x integer) (y integer))", &env);
    for lst in [
        "nil",
        "(list 1 1 1)",
        "(list 3 1 2 3 2 1)",
        "(list 1 1.0 -0.0 0.0 1)",
        "(list \"a\" \"b\" \"a\" \"é\" \"é\")",
        "(list 'a 'b 'a nil nil t)",
        "(list '(1 2) '(1 2) '(1 (2)) '(1 (2)) '(1 . 2) '(1 . 2) nil '())",
        "(list (make-Pt510 1 2) 1 (make-Pt510 1 2) '(1 2) (make-Pt510 2 1))",
        "(list (list (make-Pt510 1 2)) (list (make-Pt510 1 2)) (list 1))",
    ] {
        agrees(&env, "remove-duplicates", "$old-remove-duplicates", lst);
    }
    check(
        &env,
        "(remove-duplicates (list 3 1 \"a\" 3 '(1 2) \"a\" '(1 2) 1))",
        "(3 1 \"a\" (1 2))",
    );
    check(
        &env,
        "(length (remove-duplicates (list (make-Pt510 1 2) (make-Pt510 1 2))))",
        "1",
    );
}

/// Median of three runs of `expr` (which must print `expected`).
fn median_time(env: &Shared<Environment>, expr: &str, expected: &str) -> Duration {
    let mut times: Vec<Duration> = (0..3)
        .map(|_| {
            let t = Instant::now();
            let out = eval_line(expr, env);
            let dt = t.elapsed();
            assert_eq!(out, expected, "for {}", &expr[..expr.len().min(80)]);
            dt
        })
        .collect();
    times.sort();
    times[1]
}

/// Each case at 100k characters / 20k items, with a per-case bound. The
/// quadratic definitions took ~10 s (release build) for the strings and did
/// not finish in 60 s for REMOVE-DUPLICATES. The linear ones take tens of
/// milliseconds and about a second respectively in an unoptimized build, so
/// each bound leaves a wide margin for a slow runner.
#[test]
fn linear_at_100k_chars_and_20k_items() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let chars = "é".repeat(100_000);
        let fields = "a,".repeat(50_000);
        eval_line(&format!("(defparameter *chars-510* {chars:?})"), &env);
        eval_line(&format!("(defparameter *fields-510* {fields:?})"), &env);
        eval_line("(defparameter *items-510* (iota 20000))", &env);
        eval_line(
            "(defparameter *dups-510* (append *items-510* (reverse *items-510*)))",
            &env,
        );
        let strings = Duration::from_secs(5);
        let lists = Duration::from_secs(20);
        for (expr, expected, bound) in [
            ("(length (string->list *chars-510*))", "100000", strings),
            (
                "(string-length* (list->string (string->list *chars-510*)))",
                "100000",
                strings,
            ),
            (
                "(length (string-split *fields-510* \",\"))",
                "50001",
                strings,
            ),
            (
                "(string-length* (string-join (string-split *fields-510* \",\") \",\"))",
                "100000",
                strings,
            ),
            ("(length (remove-duplicates *items-510*))", "20000", lists),
            ("(length (remove-duplicates *dups-510*))", "20000", lists),
        ] {
            let dt = median_time(&env, expr, expected);
            assert!(dt < bound, "{expr} took {dt:?} (bound {bound:?})");
        }
    });
}
