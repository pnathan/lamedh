/// Tests for LispValExtension trait and EVLIS builtin.
use lamedh::{LispVal, LispValExtension, with_large_stack};
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::printer::print;
use std::hash::Hasher;

// ─── Example host type ──────────────────────────────────────────────────────

#[derive(Debug)]
struct Point { x: f64, y: f64 }

impl LispValExtension for Point {
    fn type_name(&self) -> &str { "point" }
    fn display(&self) -> String { format!("#<point {},{}>", self.x, self.y) }
    fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
        other.as_any().downcast_ref::<Point>()
            .map_or(false, |p| p.x == self.x && p.y == self.y)
    }
    fn hash_ext(&self, state: &mut dyn Hasher) {
        state.write_u64(self.x.to_bits());
        state.write_u64(self.y.to_bits());
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ─── Extension trait tests ───────────────────────────────────────────────────

#[test]
fn test_extension_display() {
    let v = LispVal::ext(Point { x: 1.0, y: 2.0 });
    assert_eq!(print(&v), "#<point 1,2>");
}

#[test]
fn test_extension_type_name() {
    let v = LispVal::ext(Point { x: 0.0, y: 0.0 });
    if let LispVal::Extension(e) = &v {
        assert_eq!(e.type_name(), "point");
    } else {
        panic!("expected Extension");
    }
}

#[test]
fn test_extension_eq() {
    let a = LispVal::ext(Point { x: 1.0, y: 2.0 });
    let b = LispVal::ext(Point { x: 1.0, y: 2.0 });
    let c = LispVal::ext(Point { x: 3.0, y: 4.0 });
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_extension_self_evaluating() {
    with_large_stack(|| {
        let env = Environment::with_stdlib();
        let pt = LispVal::ext(Point { x: 5.0, y: 6.0 });
        // Extension values are self-evaluating
        let result = eval(&pt, &env).unwrap();
        assert_eq!(result, pt);
    });
}

#[test]
fn test_extensionp_predicate() {
    with_large_stack(|| {
        let env = Environment::with_stdlib();
        let pt = LispVal::ext(Point { x: 1.0, y: 2.0 });
        env.set("PT".to_string(), pt);
        // eval (extension-p pt)
        use lamedh::reader::read;
        let form = read("(extension-p pt)", &env).unwrap();
        let result = eval(&form, &env).unwrap();
        assert_ne!(result, LispVal::Nil);

        let form2 = read("(extension-p 42)", &env).unwrap();
        let result2 = eval(&form2, &env).unwrap();
        assert_eq!(result2, LispVal::Nil);
    });
}

#[test]
fn test_extension_type_name_builtin() {
    with_large_stack(|| {
        let env = Environment::with_stdlib();
        env.set("PT".to_string(), LispVal::ext(Point { x: 0.0, y: 0.0 }));
        use lamedh::reader::read;
        let form = read("(extension-type pt)", &env).unwrap();
        let result = eval(&form, &env).unwrap();
        assert_eq!(result, LispVal::String("point".to_string()));
    });
}

// ─── EVLIS tests ─────────────────────────────────────────────────────────────

mod test_helpers;
use test_helpers::env_with_stdlib;
use lamedh::eval_line;

#[test]
fn test_evlis_basic() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(evlis '(1 (+ 1 1) 3) (the-environment))", &env);
        assert_eq!(r, "(1 2 3)");
    });
}

#[test]
fn test_evlis_one_arg() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(evlis '((+ 2 3) (* 4 5)))", &env);
        assert_eq!(r, "(5 20)");
    });
}

#[test]
fn test_evlis_empty() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(evlis '())", &env), "()");
    });
}

// ─── let* tests ──────────────────────────────────────────────────────────────

#[test]
fn test_let_star_sequential() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(let* ((x 1) (y (+ x 1))) y)", &env);
        assert_eq!(r, "2");
    });
}

#[test]
fn test_let_star_chain() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let r = eval_line("(let* ((a 2) (b (* a 3)) (c (+ b 1))) c)", &env);
        assert_eq!(r, "7");
    });
}
