//! Acceptance tests for `lamedh check` (the static verifier).
//!
//! The centrepiece is the **false-positive regression net**: running the
//! checker over every stdlib, example, and benchmark `.lisp` file in the repo
//! must produce ZERO findings. The checker's whole value proposition is that it
//! never cries wolf, so a single spurious finding here is a bug.

use lamedh::check::{self, FindingKind, Severity};
use lamedh::environment::Environment;

/// Collect `*.lisp` files under `dir` (recursively), sorted for determinism.
fn lisp_files(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect(std::path::Path::new(dir), &mut out);
    out.sort();
    out
}

fn collect(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lisp") {
            out.push(path.to_string_lossy().into_owned());
        }
    }
}

/// The manifest root — this crate's directory.
fn root() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

/// A checkable **program group**: a set of `.lisp` files that are loaded
/// together and are self-contained (they define everything they call, or call
/// only the standard library). The checker is a *static* tool: it cannot know
/// about host-registered Rust natives, and independent programs that happen to
/// reuse a name (`run`, `transition`, …) must not be pooled — so we group by
/// self-contained unit rather than checking the whole tree at once.
fn program_groups() -> Vec<(String, Vec<String>)> {
    let base = root();
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();

    // The standard library is one cohesive corpus (all loaded together).
    groups.push(("lib".to_string(), lisp_files(&format!("{base}/lib"))));

    // Each immediate child of examples/ is a self-contained program: a single
    // top-level script file, or a directory of files loaded together.
    //
    // `game/` is excluded: examples/game/ai.lisp is deliberately NOT
    // self-contained — it is driven by examples/game_demo.rs, which registers
    // `entity-x`, `move-entity!`, `game-log`, … as host natives from Rust. A
    // static checker cannot see host bindings, so those references are
    // (correctly) unbound to it. This is a documented limitation, not a bug.
    let examples = format!("{base}/examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        let mut children: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for path in children {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "game" {
                continue;
            }
            if path.is_dir() {
                let files = lisp_files(&path.to_string_lossy());
                if !files.is_empty() {
                    groups.push((format!("examples/{name}"), files));
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("lisp") {
                groups.push((
                    format!("examples/{name}"),
                    vec![path.to_string_lossy().into_owned()],
                ));
            }
        }
    }

    // Benchmarks: each benchmark's Lamedh sources, grouped per benchmark
    // directory. Files under an `sbcl/` directory are Common Lisp comparison
    // implementations (they use LOOP, VALUES, SB-EXT:*, …) — not Lamedh — and
    // are excluded.
    for f in lisp_files(&format!("{base}/benchmarks")) {
        if f.contains("/sbcl/") {
            continue;
        }
        groups.push((f.clone(), vec![f]));
    }

    groups
}

#[test]
fn corpus_produces_zero_findings() {
    lamedh::with_large_stack(|| {
        let groups = program_groups();
        assert!(!groups.is_empty(), "no corpus groups found");
        // One stdlib world, reused across every group: building it is the
        // expensive step, and reuse is sound because reading (not evaluating) a
        // file only interns symbols — it never adds bindings that could leak
        // between independent programs.
        let env = Environment::with_stdlib_fresh();
        let mut total_files = 0usize;
        let mut report = String::new();
        let mut n = 0usize;
        for (label, files) in &groups {
            total_files += files.len();
            let mut sources = Vec::new();
            for path in files {
                let src = std::fs::read_to_string(path).expect("read corpus file");
                sources.push((path.clone(), src));
            }
            let findings = check::check_sources_in(&env, &sources);
            for f in &findings {
                // Builtins that exist only in feature-gated builds: a
                // no-default-features interpreter genuinely lacks them, so
                // the stdlib's references are correctly reported unbound
                // there — real lint behavior, not a checker false positive.
                #[cfg(not(feature = "concurrency"))]
                if matches!(f.symbol.as_deref(), Some("SPAWN-THREAD" | "CHANNEL-RECV")) {
                    continue;
                }
                report.push_str(&format!("[{label}] {}\n", f.to_human()));
                n += 1;
            }
        }
        assert!(
            report.is_empty(),
            "expected ZERO findings across {} groups ({} files), got {}:\n{}",
            groups.len(),
            total_files,
            n,
            report
        );
    });
}

#[test]
fn clean_file_is_clean() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "clean.lisp".to_string(),
            "(defun square (x) (* x x))\n(square 5)\n".to_string(),
        )]);
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(check::exit_code(&f), 0);
    });
}

#[test]
fn misspelled_call_flagged_with_did_you_mean() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "typo.lisp".to_string(),
            "(defun f (xs) (revrse xs))\n".to_string(),
        )]);
        let uf = f
            .iter()
            .find(|f| f.kind == FindingKind::UnboundFunction)
            .expect("unbound function finding");
        assert_eq!(uf.symbol.as_deref(), Some("REVRSE"));
        assert!(uf.message.contains("did you mean"), "{}", uf.message);
        assert!(uf.message.contains("REVERSE"), "{}", uf.message);
        assert_eq!(check::exit_code(&f), 1);
    });
}

#[test]
fn cl_ism_flagged_with_guidance() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "clism.lisp".to_string(),
            "(defun f (n) (defstruct point x y) n)\n".to_string(),
        )]);
        let ff = f
            .iter()
            .find(|f| f.symbol.as_deref() == Some("DEFSTRUCT"))
            .expect("DEFSTRUCT should be flagged");
        assert!(ff.message.contains("DEFRECORD"), "{}", ff.message);
    });
}

#[test]
fn arity_mismatch_on_file_function() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "arity.lisp".to_string(),
            "(defun pair (a b) (cons a b))\n(pair 1)\n".to_string(),
        )]);
        let a = f
            .iter()
            .find(|f| f.kind == FindingKind::ArityMismatch)
            .expect("arity finding");
        assert_eq!(a.symbol.as_deref(), Some("PAIR"));
    });
}

#[test]
fn parse_error_reports_position_and_severity() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "broken.lisp".to_string(),
            "(defun ok (x) x)\n(defun bad (y\n".to_string(),
        )]);
        let pe = f
            .iter()
            .find(|f| f.kind == FindingKind::ParseError)
            .expect("parse error finding");
        assert_eq!(pe.severity, Severity::Error);
        assert!(pe.line >= 2, "line was {}", pe.line);
        assert_eq!(check::exit_code(&f), 2);
    });
}

// --- module names (import / with-module) ------------------------------

#[test]
fn import_of_stdlib_module_binds_exported_name() {
    lamedh::with_large_stack(|| {
        // SHELL exports SHELL-OK-P (lib/07-shell.lisp); after `(import shell)`
        // it must be callable unqualified with no finding, even though the
        // checker never evaluates the file — it reads SHELL's live
        // "module.exports" plist entry from the stdlib environment.
        let f = check::check_sources(&[(
            "import.lisp".to_string(),
            "(import shell)\n(defun f (cmd) (shell-ok-p cmd))\n".to_string(),
        )]);
        assert!(f.is_empty(), "{f:?}");
    });
}

#[test]
fn with_module_body_local_call_is_ok() {
    lamedh::with_large_stack(|| {
        // A with-module body calling its own (not-yet-qualified-in-source)
        // definition must not be flagged: with-module rewrites the reference
        // to GEOMETRY:AREA at runtime, so this is a real, bound call.
        let f = check::check_sources(&[(
            "wm.lisp".to_string(),
            "(with-module geometry\n  \
               (defun area (r) (* 3 (* r r)))\n  \
               (defun helper (x) (area x)))\n"
                .to_string(),
        )]);
        assert!(f.is_empty(), "{f:?}");
    });
}

#[test]
fn qualified_call_after_with_module_body_is_ok() {
    lamedh::with_large_stack(|| {
        // GEOMETRY:AREA is always available after the body, per
        // lib/27-modules.lisp's header comment — no import needed.
        let f = check::check_sources(&[(
            "wm.lisp".to_string(),
            "(with-module geometry\n  (defun area (r) (* 3 (* r r))))\n\
             (defun caller () (geometry:area 2))\n"
                .to_string(),
        )]);
        assert!(f.is_empty(), "{f:?}");
    });
}

#[test]
fn unqualified_call_after_with_module_body_without_import_is_unbound() {
    lamedh::with_large_stack(|| {
        // Verified against the real interpreter: `(with-module geometry
        // (defun area ...))` followed by a bare `(area 2)` with no
        // `(import geometry)` genuinely raises "Unbound variable: AREA" at
        // runtime (with-module only binds the qualified name). The checker
        // must agree — this is a real bug, not a false positive to suppress.
        let f = check::check_sources(&[(
            "wm.lisp".to_string(),
            "(with-module geometry\n  (defun area (r) (* 3 (* r r))))\n\
             (defun caller () (area 2))\n"
                .to_string(),
        )]);
        let uf = f
            .iter()
            .find(|f| f.symbol.as_deref() == Some("AREA"))
            .expect("bare AREA should be flagged unbound");
        assert_eq!(uf.kind, FindingKind::UnboundFunction);
    });
}

#[test]
fn import_of_unknown_module_suppresses_conservatively() {
    lamedh::with_large_stack(|| {
        // A module the checker cannot enumerate exports for (not declared
        // anywhere in the checked corpus, not a live stdlib module): we
        // cannot know what names this import binds, so go permissive for the
        // rest of the file rather than risk a false positive on a
        // legitimately-imported name.
        let f = check::check_sources(&[(
            "unknown-import.lisp".to_string(),
            "(import totally-unknown-module-xyz)\n\
             (defun f () (some-name-the-unknown-module-might-export 1))\n"
                .to_string(),
        )]);
        assert!(f.is_empty(), "{f:?}");
    });
}

#[test]
fn typo_inside_with_module_body_is_still_caught() {
    lamedh::with_large_stack(|| {
        // Body-descent is new exposure (with-module bodies used to be
        // opaque), but it must not come at the cost of missing genuine typos
        // inside the body: AERA (misspelled AREA) resolves neither plainly
        // nor as GEOMETRY:AERA.
        let f = check::check_sources(&[(
            "wm-typo.lisp".to_string(),
            "(with-module geometry\n  (defun area (r) (aera r)))\n".to_string(),
        )]);
        let uf = f
            .iter()
            .find(|f| f.symbol.as_deref() == Some("AERA"))
            .expect("AERA typo should be flagged unbound");
        assert_eq!(uf.kind, FindingKind::UnboundFunction);
    });
}

#[test]
fn sexpr_schema_is_stable() {
    lamedh::with_large_stack(|| {
        let f = check::check_sources(&[(
            "s.lisp".to_string(),
            "(defun f (xs) (revrse xs))\n".to_string(),
        )]);
        let s = f[0].to_sexpr();
        for key in [
            "(file . ",
            "(line . ",
            "(column . ",
            "(severity . ",
            "(kind . ",
            "(symbol . ",
            "(message . ",
        ] {
            assert!(s.contains(key), "missing {key} in {s}");
        }
    });
}
