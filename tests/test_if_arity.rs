//! `IF` takes exactly three operands on every evaluator path (#506).
//!
//! KERNEL.md: a two-operand `(if test then)` or a four-operand form is an
//! error, not an implicit `NIL` else. The tree-walker already enforced this;
//! the closure compiler used for function bodies accepted a one-armed `if`,
//! and both optimizers completed it into a value.
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

const ARITY_ERROR: &str = "if takes exactly three arguments";

fn assert_arity_error(src: &str, env: &lamedh::Shared<lamedh::environment::Environment>) {
    let out = eval_line(src, env);
    assert!(
        out.contains(ARITY_ERROR),
        "expected IF arity error for {src}, got {out}"
    );
}

#[test]
fn two_operand_if_rejected_on_every_path() {
    let env = env_with_stdlib();
    // Top level and LET body: tree-walker.
    assert_arity_error("(if t 1)", &env);
    assert_arity_error("(let () (if t 1))", &env);
    // Function bodies: compiled closure path. Previously returned NIL / 1.
    eval_line("(defun if506-f (x) (if x 1))", &env);
    assert_arity_error("(if506-f nil)", &env);
    assert_arity_error("(if506-f t)", &env);
    assert_arity_error("((lambda (x) (if x 1)) nil)", &env);
}

#[test]
fn four_operand_if_rejected_in_function_body() {
    let env = env_with_stdlib();
    eval_line("(defun if506-g (x) (if x 1 2 3))", &env);
    assert_arity_error("(if506-g t)", &env);
    assert_arity_error("(if t 1 2 3)", &env);
}

#[test]
fn three_operand_if_unchanged_in_function_body() {
    let env = env_with_stdlib();
    eval_line("(defun if506-h (x) (if x 1 2))", &env);
    assert_eq!(eval_line("(if506-h t)", &env), "1");
    assert_eq!(eval_line("(if506-h nil)", &env), "2");
    // The one-armed idiom is WHEN.
    eval_line("(defun if506-w (x) (when x 1))", &env);
    assert_eq!(eval_line("(if506-w nil)", &env), "()");
}

#[test]
fn optimizers_do_not_complete_one_armed_if() {
    let env = env_with_stdlib();
    // The kernel optimizer folds only the three-operand form.
    assert_eq!(eval_line("(optimize '(if nil 1))", &env), "(IF () 1)");
    assert_eq!(eval_line("(optimize '(if t 1))", &env), "(IF T 1)");
    assert_eq!(eval_line("(optimize '(if nil 1 2))", &env), "2");
    // The Lisp optimizer does not add an implicit NIL else.
    assert_eq!(eval_line("(optimize-form '(if nil 1))", &env), "(IF () 1)");
    assert_eq!(eval_line("(optimize-form '(if t 7 0))", &env), "7");
    // An optimized one-armed IF still errors when evaluated.
    assert_arity_error("(eval (optimize-form '(if nil 1)))", &env);
}

/// Every `(if ...)` in the embedded stdlib has exactly three operands, so no
/// stdlib body depends on the one-armed form this change rejects. Quoted data
/// is skipped: it is never evaluated as an `IF`.
#[test]
fn stdlib_sources_use_only_three_operand_if() {
    use lamedh::LispVal;
    fn walk(v: &LispVal, file: &str, bad: &mut Vec<String>) {
        let LispVal::Cons { car, cdr } = v else {
            return;
        };
        let head = lamedh::printer::print(car);
        if head == "QUOTE" {
            return;
        }
        let mut items = vec![(**car).clone()];
        let mut rest = (**cdr).clone();
        while let LispVal::Cons { car, cdr } = rest {
            items.push((*car).clone());
            rest = (*cdr).clone();
        }
        if head == "IF" && items.len() != 4 {
            bad.push(format!("{file}: {}", lamedh::printer::print(v)));
        }
        for item in &items {
            walk(item, file, bad);
        }
    }
    let env = env_with_stdlib();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lib");
    let mut bad = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "lisp"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for path in files {
        let src = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for form in lamedh::reader::read_all(&src, &env).unwrap() {
            walk(&form, &name, &mut bad);
        }
    }
    assert!(
        bad.is_empty(),
        "non-three-operand IF in stdlib:\n{}",
        bad.join("\n")
    );
}
