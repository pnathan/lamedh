mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// SUBST tests
#[test]
fn test_subst_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 'x 'a '(a b c))", &env);
    assert_eq!(output, "(X B C)");
}

#[test]
fn test_subst_multiple_occurrences() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 'x 'a '(a b a c a))", &env);
    assert_eq!(output, "(X B X C X)");
}

#[test]
fn test_subst_nested() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 'x 'a '(a (b a) (c (a d))))", &env);
    assert_eq!(output, "(X (B X) (C (X D)))");
}

#[test]
fn test_subst_no_match() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 'x 'z '(a b c))", &env);
    assert_eq!(output, "(A B C)");
}

#[test]
fn test_subst_with_numbers() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 99 1 '(1 2 1 3))", &env);
    assert_eq!(output, "(99 2 99 3)");
}

#[test]
fn test_subst_entire_tree() {
    let env = env_with_stdlib();
    let output = eval_line("(subst 'new '(a b) '(a b))", &env);
    assert_eq!(output, "NEW");
}

// ASSOC tests
#[test]
fn test_assoc_found() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 'b '((a 1) (b 2) (c 3)))", &env);
    assert_eq!(output, "(B 2)");
}

#[test]
fn test_assoc_not_found() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 'd '((a 1) (b 2) (c 3)))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_assoc_first_match() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 'a '((a 1) (b 2) (a 3)))", &env);
    assert_eq!(output, "(A 1)");
}

#[test]
fn test_assoc_with_numbers() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 2 '((1 'one) (2 'two) (3 'three)))", &env);
    assert_eq!(output, "(2 (QUOTE TWO))");
}

#[test]
fn test_assoc_empty_list() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 'a '())", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_assoc_complex_values() {
    let env = env_with_stdlib();
    let output = eval_line("(assoc 'x '((x (1 2 3)) (y (4 5 6))))", &env);
    assert_eq!(output, "(X (1 2 3))");
}

// MAPCAR tests
#[test]
fn test_mapcar_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '(1 2 3) (lambda (x) (* x 2)))", &env);
    assert_eq!(output, "(2 4 6)");
}

#[test]
fn test_mapcar_with_car() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '((a 1) (b 2) (c 3)) car)", &env);
    assert_eq!(output, "(A B C)");
}

#[test]
fn test_mapcar_empty_list() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '() (lambda (x) x))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_mapcar_identity() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '(1 2 3) (lambda (x) x))", &env);
    assert_eq!(output, "(1 2 3)");
}

#[test]
fn test_mapcar_add_one() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '(5 10 15) (lambda (n) (+ n 1)))", &env);
    assert_eq!(output, "(6 11 16)");
}

#[test]
fn test_mapcar_with_atom_check() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar '(1 (2) 3 (4)) atom)", &env);
    assert_eq!(output, "(T () T ())");
}

// MAPLIST tests
#[test]
fn test_maplist_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(maplist '(1 2 3) (lambda (x) (car x)))", &env);
    assert_eq!(output, "(1 2 3)");
}

#[test]
fn test_maplist_count_elements() {
    let env = env_with_stdlib();
    let output = eval_line("(maplist '(a b c d) length)", &env);
    assert_eq!(output, "(4 3 2 1)");
}

#[test]
fn test_maplist_empty() {
    let env = env_with_stdlib();
    let output = eval_line("(maplist '() car)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_maplist_identity() {
    let env = env_with_stdlib();
    let output = eval_line("(maplist '(a b) (lambda (x) x))", &env);
    assert_eq!(output, "((A B) (B))");
}

// RPLACA tests
#[test]
fn test_rplaca_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(rplaca '(a . b) 'x)", &env);
    assert_eq!(output, "(X . B)");
}

#[test]
fn test_rplaca_list() {
    let env = env_with_stdlib();
    let output = eval_line("(rplaca '(1 2 3) 99)", &env);
    assert_eq!(output, "(99 2 3)");
}

#[test]
fn test_rplaca_with_list_as_car() {
    let env = env_with_stdlib();
    let output = eval_line("(rplaca '(a b c) '(x y))", &env);
    assert_eq!(output, "((X Y) B C)");
}

#[test]
fn test_rplaca_preserves_cdr() {
    let env = env_with_stdlib();
    let output = eval_line("(cdr (rplaca '(1 2 3 4) 'new))", &env);
    assert_eq!(output, "(2 3 4)");
}

// RPLACD tests
#[test]
fn test_rplacd_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(rplacd '(a . b) 'x)", &env);
    assert_eq!(output, "(A . X)");
}

#[test]
fn test_rplacd_list() {
    let env = env_with_stdlib();
    let output = eval_line("(rplacd '(1 2 3) '(4 5))", &env);
    assert_eq!(output, "(1 4 5)");
}

#[test]
fn test_rplacd_to_nil() {
    let env = env_with_stdlib();
    let output = eval_line("(rplacd '(1 2 3) '())", &env);
    assert_eq!(output, "(1)");
}

#[test]
fn test_rplacd_preserves_car() {
    let env = env_with_stdlib();
    let output = eval_line("(car (rplacd '(a b c) '(x y z)))", &env);
    assert_eq!(output, "A");
}

#[test]
fn test_rplacd_create_dotted_pair() {
    let env = env_with_stdlib();
    let output = eval_line("(rplacd '(1 2) 'end)", &env);
    assert_eq!(output, "(1 . END)");
}

// Combined tests
#[test]
fn test_mapcar_with_subst() {
    let env = env_with_stdlib();
    let output = eval_line(
        "(mapcar '((a b) (c a) (a a)) (lambda (lst) (subst 'x 'a lst)))",
        &env,
    );
    assert_eq!(output, "((X B) (C X) (X X))");
}

#[test]
fn test_assoc_with_mapcar() {
    let env = env_with_stdlib();
    eval_line("(def alist '((a 1) (b 2) (c 3)))", &env);
    let output = eval_line("(mapcar '(a c b) (lambda (key) (assoc key alist)))", &env);
    assert_eq!(output, "((A 1) (C 3) (B 2))");
}
