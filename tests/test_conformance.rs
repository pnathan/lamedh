// Implementation-neutral conformance runner.
//
// Reads every `conformance/cases/*.lisp` file (sorted by name). Each file is a
// sequence of `(case ID SPEC-REF SOURCE EXPECT)` forms, read with the crate's
// own reader. SOURCE is evaluated in a fresh stdlib environment; the printed
// value of its last form must equal EXPECT. EXPECT "ERROR" asserts only that
// an error is signalled.
//
// Files named `99-*` hold disputed cases (KERNEL.md disagrees with the Rust
// reference). They are parsed, so their ids count toward uniqueness, but they
// are not run. Their count is printed.
//
// Every failure is collected and reported together, not stopped at the first.

mod test_helpers;

use lamedh::{LispVal, eval_all, printer, reader, with_large_stack};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use test_helpers::env_with_stdlib;

const MIN_CASES: usize = 100;

/// A case slower than this fails the run. Cases must stay fast so the suite
/// finishes in seconds; see conformance/README.md.
const CASE_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(1);

struct Case {
    id: String,
    spec: String,
    source: String,
    expect: String,
}

fn string_arg(v: &LispVal, what: &str, file: &str) -> String {
    match v {
        LispVal::String(s) => s.clone(),
        other => panic!(
            "{file}: {what} must be a string literal, got {}",
            printer::print(other)
        ),
    }
}

fn parse_case(form: &LispVal, file: &str) -> Case {
    let items = form
        .as_list_vec()
        .unwrap_or_else(|e| panic!("{file}: top-level form is not a list: {e}"));
    if items.len() != 5 {
        panic!(
            "{file}: case form must have 4 operands, got {}",
            items.len() - 1
        );
    }
    match &items[0] {
        LispVal::Symbol(s) if s.borrow().name == "CASE" => {}
        other => panic!(
            "{file}: top-level form must start with CASE, got {}",
            printer::print(other)
        ),
    }
    Case {
        id: string_arg(&items[1], "ID", file),
        spec: string_arg(&items[2], "SPEC-REF", file),
        source: string_arg(&items[3], "SOURCE", file),
        expect: string_arg(&items[4], "EXPECT", file),
    }
}

/// Evaluate `source` in a fresh stdlib environment and print the last value.
/// An error from reading or evaluation prints as "ERROR".
fn run_source(source: &str) -> String {
    let env = env_with_stdlib();
    match eval_all(source, &env) {
        Ok(values) => values
            .last()
            .map_or_else(|| "NIL".to_string(), printer::print),
        Err(_) => "ERROR".to_string(),
    }
}

fn case_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("cannot read directory entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "lisp"))
        .collect();
    files.sort();
    files
}

fn run_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/cases");
    let parse_env = env_with_stdlib();

    let mut ids: HashSet<String> = HashSet::new();
    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let mut disputed = 0usize;
    let suite_started = std::time::Instant::now();

    for path in case_files(&dir) {
        let name = path
            .file_name()
            .expect("case file has a name")
            .to_string_lossy()
            .into_owned();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {name}: {e}"));
        let forms = reader::read_all(&text, &parse_env)
            .unwrap_or_else(|e| panic!("{name} does not read: {e}"));
        let is_disputed = name.starts_with("99-");

        for form in &forms {
            let case = parse_case(form, &name);
            if !ids.insert(case.id.clone()) {
                failures.push(format!("duplicate case id: {} ({name})", case.id));
            }
            if is_disputed {
                disputed += 1;
                continue;
            }
            ran += 1;
            let started = std::time::Instant::now();
            let got = run_source(&case.source);
            let elapsed = started.elapsed();
            if elapsed > CASE_TIME_LIMIT {
                failures.push(format!(
                    "{}\n  file:     {name}\n  spec:     {}\n  source:   {}\n  slow case: took {elapsed:?}, limit is {CASE_TIME_LIMIT:?}",
                    case.id, case.spec, case.source
                ));
            }
            if got != case.expect {
                failures.push(format!(
                    "{}\n  file:     {name}\n  spec:     {}\n  source:   {}\n  expected: {}\n  got:      {got}",
                    case.id, case.spec, case.source, case.expect
                ));
            }
        }
    }

    println!(
        "conformance: {ran} cases ran, {} failed, {disputed} disputed cases skipped, total wall time {:?}",
        failures.len(),
        suite_started.elapsed()
    );
    assert!(
        ran >= MIN_CASES,
        "conformance corpus ran only {ran} cases, expected at least {MIN_CASES}"
    );
    assert!(
        failures.is_empty(),
        "{} conformance case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn conformance_corpus_passes() {
    with_large_stack(run_corpus);
}
