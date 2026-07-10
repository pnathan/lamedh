use lamedh::{
    Shared, environment::Environment, eval_depth_limit, eval_str, set_eval_depth_limit,
    with_large_stack,
};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

struct EvalDepthLimitGuard(usize);

impl EvalDepthLimitGuard {
    fn set(limit: usize) -> Self {
        let previous = eval_depth_limit();
        set_eval_depth_limit(limit);
        Self(previous)
    }
}

impl Drop for EvalDepthLimitGuard {
    fn drop(&mut self) {
        set_eval_depth_limit(self.0);
    }
}

fn quoted_number_list(len: usize) -> String {
    let values = (0..len).map(|_| "1").collect::<Vec<_>>().join(" ");
    format!("'({values})")
}

// PROG2

#[test]
fn test_prog2_returns_second() {
    let e = env();
    let v = eval_str("(prog2 1 2 3)", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(2));
}

#[test]
fn test_prog2_evaluates_first() {
    let e = env();
    eval_str("(def counter 0)", &e).unwrap();
    eval_str("(prog2 (setq counter (+ counter 1)) 99)", &e).unwrap();
    let v = eval_str("counter", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(1));
}

#[test]
fn test_prog2_two_args() {
    let e = env();
    let v = eval_str("(prog2 10 20)", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(20));
}

// NCONC

#[test]
fn test_nconc_two_lists() {
    let e = env();
    let v = eval_str("(nconc '(1 2) '(3 4))", &e).unwrap();
    let expected = eval_str("'(1 2 3 4)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_nconc_empty_first() {
    let e = env();
    let v = eval_str("(nconc '() '(1 2))", &e).unwrap();
    let expected = eval_str("'(1 2)", &e).unwrap();
    assert_eq!(v, expected);
}

// COPY

#[test]
fn test_copy_flat_list() {
    let e = env();
    let v = eval_str("(copy '(1 2 3))", &e).unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_copy_nested_list() {
    let e = env();
    let v = eval_str("(copy '(1 (2 3) 4))", &e).unwrap();
    let expected = eval_str("'(1 (2 3) 4)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_copy_atom() {
    let e = env();
    let v = eval_str("(copy 'x)", &e).unwrap();
    let expected = eval_str("'x", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_length_large_list_under_low_eval_depth() {
    with_large_stack(|| {
        let e = env();
        let _guard = EvalDepthLimitGuard::set(64);
        let expr = format!("(length {})", quoted_number_list(128));
        let v = eval_str(&expr, &e).unwrap();
        assert_eq!(v, lamedh::LispVal::Number(128));
    });
}

#[test]
fn test_list_to_array_large_list_under_low_eval_depth() {
    with_large_stack(|| {
        let e = env();
        let _guard = EvalDepthLimitGuard::set(64);
        let expr = format!("(array-length* (list->array {}))", quoted_number_list(128));
        let v = eval_str(&expr, &e).unwrap();
        assert_eq!(v, lamedh::LispVal::Number(128));
    });
}

// SASSOC

#[test]
fn test_sassoc_found() {
    let e = env();
    let v = eval_str("(sassoc 'b '((a . 1) (b . 2)) (lambda () 'notfound))", &e).unwrap();
    let expected = eval_str("'(b . 2)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_sassoc_not_found_calls_fn() {
    let e = env();
    let v = eval_str("(sassoc 'x '((a . 1) (b . 2)) (lambda () 'notfound))", &e).unwrap();
    let expected = eval_str("'notfound", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_sassoc_empty_alist() {
    let e = env();
    let v = eval_str("(sassoc 'x '() (lambda () 42))", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(42));
}

// MAPC

#[test]
fn test_mapc_returns_list() {
    let e = env();
    eval_str("(def result '())", &e).unwrap();
    let v = eval_str(
        "(mapc (lambda (x) (setq result (cons x result))) '(1 2 3))",
        &e,
    )
    .unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_mapc_side_effects() {
    let e = env();
    eval_str("(def acc 0)", &e).unwrap();
    eval_str("(mapc (lambda (x) (setq acc (+ acc x))) '(1 2 3))", &e).unwrap();
    let v = eval_str("acc", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(6));
}

#[test]
fn test_mapc_empty_list() {
    let e = env();
    let v = eval_str("(mapc (lambda (x) x) '())", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Nil);
}

// MAPCON

#[test]
fn test_mapcon_collects_tails() {
    let e = env();
    let v = eval_str("(mapcon (lambda (x) (list (car x))) '(1 2 3))", &e).unwrap();
    let expected = eval_str("'(1 2 3)", &e).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn test_mapcon_empty() {
    let e = env();
    let v = eval_str("(mapcon (lambda (x) x) '())", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Nil);
}

// CSET / CSETQ

#[test]
fn test_cset_defines_var() {
    let e = env();
    eval_str("(cset myvar 99)", &e).unwrap();
    let v = eval_str("myvar", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(99));
}

#[test]
fn test_csetq_alias() {
    let e = env();
    eval_str("(csetq another 77)", &e).unwrap();
    let v = eval_str("another", &e).unwrap();
    assert_eq!(v, lamedh::LispVal::Number(77));
}
