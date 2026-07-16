//! Structured results for `lamedh --test`.
//!
//! Per the project's Lisp-first philosophy, the actual test-running logic
//! (registration, assertion bookkeeping, error trapping, per-test pass/fail
//! classification) lives in Lisp: `lib/10-testing.lisp`'s `deftest`/xUnit
//! machinery, extended with `run-all-tests-detailed` — a thin wrapper around
//! the existing `run-one-test` that also returns one `(name status message)`
//! triple per test. This module is the "thin Rust glue": it evaluates that
//! entry point in a caller-prepared environment and converts the resulting
//! `LispVal` list into a plain Rust type the CLI can render as human text or
//! as sexpr findings (mirroring [`crate::check`]'s output conventions).
//!
//! Building the environment (stdlib + capabilities) and loading the test
//! file(s)/directories is left to the caller — the CLI already has that
//! logic for script/`-i` batch mode and reuses it verbatim.

use crate::environment::Environment;
use crate::{LispError, LispVal, Shared, eval_str, printer};

/// The outcome of one registered `deftest`, as produced by
/// `run-all-tests-detailed` in `lib/10-testing.lisp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    /// The test's name, as passed to `deftest` (upcased, like every symbol).
    pub name: String,
    /// `true` iff every assertion in the test passed and its body did not
    /// raise an error.
    pub passed: bool,
    /// A readable rendering of the recorded failure. `None` iff `passed`.
    pub message: Option<String>,
}

/// Run every test registered (via `deftest`) in `env`, in registration
/// order. `env` must already have the testing module loaded (true of any
/// `with_stdlib`/`with_stdlib_fresh` environment) and any test files loaded
/// into it — this function only triggers the run and reads back the result.
pub fn run_registered_tests(env: &Shared<Environment>) -> Result<Vec<TestOutcome>, LispError> {
    let result = eval_str("(run-all-tests-detailed)", env)?;
    Ok(to_outcomes(&result))
}

/// The (uppercased) name of a symbol, or `None` for any other value.
fn sym_name(v: &LispVal) -> Option<String> {
    match v {
        LispVal::Symbol(s) => Some(s.borrow().name.clone()),
        _ => None,
    }
}

/// Convert the `((name status message) ...)` list `run-all-tests-detailed`
/// returns into [`TestOutcome`]s. Malformed entries (which should not occur
/// against the shipped `lib/10-testing.lisp`) are skipped rather than
/// panicking — this boundary is best-effort, not a parser.
fn to_outcomes(result: &LispVal) -> Vec<TestOutcome> {
    let mut out = Vec::new();
    let Ok(items) = result.as_list_vec() else {
        return out;
    };
    for item in items {
        let Ok(parts) = item.as_list_vec() else {
            continue;
        };
        if parts.len() < 3 {
            continue;
        }
        let Some(name) = sym_name(&parts[0]) else {
            continue;
        };
        let status = sym_name(&parts[1]).unwrap_or_default();
        let passed = status == "PASS";
        let message = if passed || !parts[2].is_truthy() {
            None
        } else {
            Some(printer::print(&parts[2]))
        };
        out.push(TestOutcome {
            name,
            passed,
            message,
        });
    }
    out
}

/// Exit-code convention for `lamedh --test`, mirroring [`crate::check`]:
/// `0` when every test passed (including zero tests registered), `1` when
/// any test failed. Load/parse failures are a separate, earlier `2` decided
/// by the caller before tests ever run.
pub fn exit_code(outcomes: &[TestOutcome]) -> i32 {
    if outcomes.iter().all(|o| o.passed) {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;

    fn env_with(src: &str) -> Shared<Environment> {
        let env = Environment::with_stdlib_fresh();
        crate::eval_all(src, &env).expect("load test source");
        env
    }

    #[test]
    fn all_passing() {
        crate::with_large_stack(|| {
            let env = env_with(
                "(deftest t1 (assert-equal (+ 1 2) 3))\n\
                 (deftest t2 (assert-true t))\n",
            );
            let outcomes = run_registered_tests(&env).expect("run tests");
            assert_eq!(outcomes.len(), 2);
            assert!(outcomes.iter().all(|o| o.passed));
            assert!(outcomes.iter().all(|o| o.message.is_none()));
            assert_eq!(exit_code(&outcomes), 0);
        });
    }

    #[test]
    fn mixed_pass_and_fail() {
        crate::with_large_stack(|| {
            let env = env_with(
                "(deftest ok (assert-equal 1 1))\n\
                 (deftest broken (assert-equal 1 2))\n",
            );
            let outcomes = run_registered_tests(&env).expect("run tests");
            assert_eq!(outcomes.len(), 2);
            let ok = outcomes.iter().find(|o| o.name == "OK").expect("ok test");
            assert!(ok.passed);
            let broken = outcomes
                .iter()
                .find(|o| o.name == "BROKEN")
                .expect("broken test");
            assert!(!broken.passed);
            assert!(broken.message.is_some());
            assert_eq!(exit_code(&outcomes), 1);
        });
    }

    #[test]
    fn error_in_body_counts_as_failure() {
        crate::with_large_stack(|| {
            let env = env_with("(deftest boom (error \"kaboom\"))\n");
            let outcomes = run_registered_tests(&env).expect("run tests");
            assert_eq!(outcomes.len(), 1);
            assert!(!outcomes[0].passed);
            assert!(outcomes[0].message.is_some());
        });
    }

    #[test]
    fn no_tests_registered_is_a_clean_zero() {
        crate::with_large_stack(|| {
            let env = Environment::with_stdlib_fresh();
            let outcomes = run_registered_tests(&env).expect("run tests");
            assert!(outcomes.is_empty());
            assert_eq!(exit_code(&outcomes), 0);
        });
    }
}
