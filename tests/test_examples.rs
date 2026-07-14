//! The classics harness: every examples/<name>/main.lisp must run to
//! completion with no error, forever. Each program self-checks and
//! errors on failure, so "loads clean" means "still correct".
//!
//! Gated on the default feature set: typed-numerics asserts a COMPILED
//! tier (jit) and sandbox-fuel spawns a thread (concurrency).
//!
//! One test PER example (via the `example_tests!` manifest below) rather
//! than one loop over all of them: each program builds its own stdlib
//! environment (cheap since the world-fork, #370), so splitting lets the
//! test harness run them in parallel — the whole suite's tall pole drops
//! from "sum of all 51 programs" to "the single slowest program". This
//! matters most under `cargo nextest` (process-per-test scheduling) and
//! helps plain `cargo test` too (libtest runs test fns on parallel
//! threads). The manifest is the source of truth; the
//! `examples_manifest_is_complete` guard fails loudly if a program is added
//! to examples/ without a matching row.

mod test_helpers;

use test_helpers::env_with_stdlib;

/// Run one example program to completion; panic (fail the test) on any error.
/// Deep-recursion examples need the interpreter's large-stack entry point
/// (the CLI gets this for free; libtest threads do not).
#[cfg(all(feature = "jit", feature = "concurrency"))]
fn run_example(name: &str) {
    // `with_large_stack` runs the body on a fresh 512 MiB thread, so the
    // closure must be 'static: move owned data (the path and name) into it.
    let main = format!("examples/{name}/main.lisp");
    let name = name.to_string();
    lamedh::with_large_stack(move || {
        let env = env_with_stdlib();
        // Parity with the documented run commands: file-reading examples
        // are run with --capability READ-FS, and scripts always see *ARGV*.
        env.enable_feature("READ-FS");
        lamedh::eval_line("(def *ARGV* ())", &env);
        if let Err(e) = lamedh::load_file(&main, &env) {
            panic!("example {name} failed: {e:?}");
        }
    });
}

/// One `#[test]` per example, expanding to a call into `run_example`.
macro_rules! example_tests {
    ($($fn:ident => $name:literal),* $(,)?) => {
        /// The set of example names the manifest covers (for the drift guard).
        #[cfg(all(feature = "jit", feature = "concurrency"))]
        const MANIFEST_EXAMPLES: &[&str] = &[$($name),*];

        $(
            #[test]
            #[cfg(all(feature = "jit", feature = "concurrency"))]
            fn $fn() {
                run_example($name);
            }
        )*
    };
}

example_tests! {
    anagram_groups => "anagram-groups",
    bank_conditions => "bank-conditions",
    base_conversion => "base-conversion",
    bfs_maze => "bfs-maze",
    binary_search => "binary-search",
    binary_search_tree => "binary-search-tree",
    brainfuck => "brainfuck",
    caesar_cipher => "caesar-cipher",
    church_numerals => "church-numerals",
    coin_change => "coin-change",
    collatz => "collatz",
    dijkstra => "dijkstra",
    factorial => "factorial",
    fibonacci => "fibonacci",
    fizzbuzz => "fizzbuzz",
    game_of_life => "game-of-life",
    huffman => "huffman",
    knapsack => "knapsack",
    lazy_streams => "lazy-streams",
    levenshtein => "levenshtein",
    lisp_in_lisp => "lisp-in-lisp",
    longest_common_subsequence => "longest-common-subsequence",
    lru_cache => "lru-cache",
    macro_control => "macro-control",
    mandelbrot => "mandelbrot",
    matrix_algebra => "matrix-algebra",
    mergesort => "mergesort",
    monte_carlo_pi => "monte-carlo-pi",
    newton_sqrt => "newton-sqrt",
    ninety_nine_bottles => "ninety-nine-bottles",
    n_queens => "n-queens",
    option_pipeline => "option-pipeline",
    palindromes => "palindromes",
    pretty_printer => "pretty-printer",
    primes_sieve => "primes-sieve",
    priority_queue => "priority-queue",
    quicksort => "quicksort",
    roman_numerals => "roman-numerals",
    rpn_calculator => "rpn-calculator",
    run_length_encoding => "run-length-encoding",
    sandbox_fuel => "sandbox-fuel",
    shapes => "shapes",
    state_machine => "state-machine",
    symbolic_diff => "symbolic-diff",
    text_stats => "text-stats",
    topological_sort => "topological-sort",
    towers_of_hanoi => "towers-of-hanoi",
    trie => "trie",
    typed_numerics => "typed-numerics",
    union_find => "union-find",
    wordcount => "wordcount",
}

/// Guard: the manifest above must cover exactly the set of example programs
/// on disk. Adding `examples/<new>/main.lisp` without a manifest row (or
/// removing one) fails here loudly, so no program silently loses coverage.
#[test]
#[cfg(all(feature = "jit", feature = "concurrency"))]
fn examples_manifest_is_complete() {
    use std::collections::BTreeSet;
    let on_disk: BTreeSet<String> = std::fs::read_dir("examples")
        .expect("examples/ exists")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("main.lisp").exists())
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    let in_manifest: BTreeSet<String> = MANIFEST_EXAMPLES.iter().map(|s| s.to_string()).collect();

    let missing_rows: Vec<_> = on_disk.difference(&in_manifest).collect();
    let stale_rows: Vec<_> = in_manifest.difference(&on_disk).collect();
    assert!(
        missing_rows.is_empty() && stale_rows.is_empty(),
        "example manifest out of sync — add per-example test rows for {missing_rows:?}, \
         remove stale rows for {stale_rows:?}"
    );
    assert!(
        on_disk.len() >= 50,
        "expected the 50-program classics suite, found {}",
        on_disk.len()
    );
}
