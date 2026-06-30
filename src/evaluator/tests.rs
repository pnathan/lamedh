use super::*;
#[cfg(test)]
mod evaluator_internal_tests {
    use super::*;

    fn dummy_env() -> Shared<Environment> {
        Environment::new_with_builtins()
    }

    // ---- introspection: describe ----
    #[test]
    fn test_describe_lambda_reports_params_and_doc() {
        let env = Environment::with_stdlib();
        eval_line_internal(&env, "(defun sq (x) \"square\" (* x x))");
        let sym = LispVal::Symbol(env.intern_symbol("SQ"));
        let text = describe_text(&sym, &env);
        assert!(text.contains("SQ is a lambda"), "got: {text}");
        assert!(text.contains("Parameters: (X)"), "got: {text}");
        assert!(text.contains("Doc: square"), "got: {text}");
    }

    #[test]
    fn test_describe_unbound_and_value() {
        let env = dummy_env();
        let sym = LispVal::Symbol(env.intern_symbol("NOPE"));
        assert!(describe_text(&sym, &env).contains("is unbound"));
        let n = describe_text(&LispVal::Number(7), &env);
        assert!(n.contains("integer") && n.contains("Value: 7"), "got: {n}");
    }

    // ---- introspection: see-source ----
    #[test]
    fn test_see_source_reconstructs_lambda() {
        let env = dummy_env();
        eval_line_internal(&env, "(def inc (lambda (n) (+ n 1)))");
        let sym = LispVal::Symbol(env.intern_symbol("INC"));
        let form = see_source_form(&sym, &env).unwrap();
        assert_eq!(crate::printer::print(&form), "(LAMBDA (N) (+ N 1))");
    }

    #[test]
    fn test_see_source_rejects_builtin() {
        let env = dummy_env();
        let sym = LispVal::Symbol(env.intern_symbol("CAR"));
        assert!(see_source_form(&sym, &env).is_err());
    }

    #[test]
    fn test_render_form_tree_expands_nested_lists() {
        let env = dummy_env();
        let form = crate::reader::read("(if (< n 2) n (g n))", &env).unwrap();
        let mut s = String::new();
        render_form_tree(&form, 0, &mut s);
        // Nested lists expand across lines; flat atom-lists stay inline.
        assert!(s.contains("(\n"), "tree should expand: {s}");
        assert!(s.contains("(< N 2)"), "flat sublist should be inline: {s}");
    }

    // ---- introspection: disassemble (typed/jotted function) ----
    #[test]
    fn test_disassemble_typed_function() {
        let env = dummy_env();
        eval_line_internal(&env, "(defun-typed (twice int64) ((n int64)) (* n 2))");
        let text = env
            .jit_disassemble("TWICE")
            .expect("typed fn should disassemble");
        assert!(
            text.contains("typed function TWICE (int64) -> int64"),
            "got: {text}"
        );
        assert!(text.contains("imul"), "expected multiply mnemonic: {text}");
        assert!(text.contains("ret rv"), "expected return: {text}");
        // Non-typed symbols have no edition.
        assert!(env.jit_disassemble("CAR").is_none());
    }

    fn eval_line_internal(env: &Shared<Environment>, src: &str) {
        let expr = crate::reader::read(src, env).unwrap();
        eval(&expr, env).unwrap();
    }

    // ---- apply_math_op fallthrough ----
    // Pass a BuiltinFunc that is not handled by apply_math_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~223.
    #[test]
    fn test_apply_math_op_fallthrough() {
        let env = dummy_env();
        let result = apply_math_op(&BuiltinFunc::Car, &[LispVal::Number(1)], &env);
        assert!(result.is_err(), "apply_math_op with Car should error");
    }

    // ---- apply_list_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_list_op (e.g. Plus)
    // to hit the `_ => Err(...)` arm at line ~264.
    #[test]
    fn test_apply_list_op_fallthrough() {
        let result = apply_list_op(&BuiltinFunc::Plus, &[]);
        assert!(result.is_err(), "apply_list_op with Plus should error");
    }

    // ---- apply_string_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_string_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~308.
    #[test]
    fn test_apply_string_op_fallthrough() {
        let result = apply_string_op(&BuiltinFunc::Car, &[]);
        assert!(result.is_err(), "apply_string_op with Car should error");
    }

    // ---- apply_numeric_primitives fallthrough ----
    // Pass a BuiltinFunc not handled by apply_numeric_primitives (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~401.
    #[test]
    fn test_apply_numeric_primitives_fallthrough() {
        let env = dummy_env();
        let result = apply_numeric_primitives(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_numeric_primitives with Car should error"
        );
    }

    // ---- apply_logical_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_logical_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~453.
    #[test]
    fn test_apply_logical_op_fallthrough() {
        let env = dummy_env();
        let result = apply_logical_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_logical_op with Car should error");
    }

    // ---- apply_hashtable_op fallthrough ----
    // Pass a BuiltinFunc not handled by apply_hashtable_op (e.g. Car)
    // to hit the `_ => Err(...)` arm at line ~551.
    #[test]
    fn test_apply_hashtable_op_fallthrough() {
        let env = dummy_env();
        let result = apply_hashtable_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_hashtable_op with Car should error");
    }

    #[test]
    fn test_apply_symbol_op_fallthrough() {
        let env = dummy_env();
        let result = apply_symbol_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_symbol_op with Car should error");
    }

    #[test]
    fn test_apply_io_op_fallthrough() {
        let env = dummy_env();
        let result = apply_io_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_io_op with Car should error");
    }

    #[test]
    fn test_apply_error_op_fallthrough() {
        let env = dummy_env();
        let result = apply_error_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_error_op with Car should error");
    }

    #[test]
    fn test_apply_list_processing_fallthrough() {
        let env = dummy_env();
        let result = apply_list_processing(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_list_processing with Car should error"
        );
    }

    #[test]
    fn test_apply_bitwise_op_fallthrough() {
        let env = dummy_env();
        let result = apply_bitwise_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_bitwise_op with Car should error");
    }

    #[test]
    fn test_apply_new_list_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_list_ops(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_new_list_ops with Car should error");
    }

    #[test]
    fn test_apply_new_numeric_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_numeric_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_new_numeric_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_type_predicates_fallthrough() {
        let env = dummy_env();
        let result = apply_type_predicates(&BuiltinFunc::Car, &[LispVal::Nil], &env);
        assert!(
            result.is_err(),
            "apply_type_predicates with Car should error"
        );
    }

    #[test]
    fn test_apply_function_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_function_ops(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_function_ops with Car should error");
    }

    #[test]
    fn test_apply_string_symbol_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_string_symbol_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_string_symbol_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_new_bitwise_ops_fallthrough() {
        let env = dummy_env();
        let result = apply_new_bitwise_ops(&BuiltinFunc::Car, &[], &env);
        assert!(
            result.is_err(),
            "apply_new_bitwise_ops with Car should error"
        );
    }

    #[test]
    fn test_apply_plist_op_fallthrough() {
        let env = dummy_env();
        let result = apply_plist_op(&BuiltinFunc::Car, &[], &env);
        assert!(result.is_err(), "apply_plist_op with Car should error");
    }
}
