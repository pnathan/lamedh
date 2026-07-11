//! The classics harness: every examples/<name>/main.lisp must run to
//! completion with no error, forever. Each program self-checks and
//! errors on failure, so "loads clean" means "still correct".
//!
//! Gated on the default feature set: typed-numerics asserts a COMPILED
//! tier (jit) and sandbox-fuel spawns a thread (concurrency).

mod test_helpers;

use std::fs;
use test_helpers::env_with_stdlib;

#[test]
#[cfg(all(feature = "jit", feature = "concurrency"))]
fn every_example_program_runs_clean() {
    // Deep-recursion examples need the interpreter's large-stack entry
    // point (the CLI gets this for free; libtest threads do not).
    lamedh::with_large_stack(|| {
        let mut dirs: Vec<_> = fs::read_dir("examples")
            .expect("examples/ exists")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.join("main.lisp").exists())
            .collect();
        dirs.sort();
        assert!(
            dirs.len() >= 50,
            "expected the 50-program classics suite, found {}",
            dirs.len()
        );
        for dir in dirs {
            let main = dir.join("main.lisp");
            let env = env_with_stdlib();
            // Parity with the documented run commands: file-reading examples
            // are run with --capability READ-FS, and scripts always see *ARGV*.
            env.enable_feature("READ-FS");
            lamedh::eval_line("(def *ARGV* ())", &env);
            if let Err(e) = lamedh::load_file(main.to_str().unwrap(), &env) {
                panic!("example {} failed: {:?}", dir.display(), e);
            }
        }
    })
}
