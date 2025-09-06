use crate::{environment::Environment, BuiltinFunc, LispError, LispVal};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn is_truthy(val: &LispVal) -> bool {
    match val {
        LispVal::List(list) if list.is_empty() => false, // nil is false
        _ => true, // Everything else is truthy
    }
}

fn apply_math_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    let nums: Result<Vec<i64>, LispError> = args
        .iter()
        .map(|arg| match arg {
            LispVal::Number(n) => Ok(*n),
            _ => Err(LispError::Generic(
                "Math functions only accept numbers".to_string(),
            )),
        })
        .collect();
    let nums = nums?;

    match op {
        BuiltinFunc::Plus => Ok(LispVal::Number(nums.iter().sum())),
        BuiltinFunc::Minus => {
            if nums.is_empty() {
                return Err(LispError::Generic("- requires at least one argument".to_string()));
            }
            if nums.len() == 1 {
                Ok(LispVal::Number(-nums[0]))
            } else {
                let mut result = nums[0];
                for &num in &nums[1..] {
                    result -= num;
                }
                Ok(LispVal::Number(result))
            }
        }
        BuiltinFunc::Multiply => Ok(LispVal::Number(nums.iter().product())),
        BuiltinFunc::Divide => {
            if nums.len() != 2 {
                return Err(LispError::Generic("/ requires exactly two arguments".to_string()));
            }
            if nums[1] == 0 {
                return Err(LispError::Generic("Division by zero".to_string()));
            }
            Ok(LispVal::Number(nums[0] / nums[1]))
        }
        _ => Err(LispError::Generic("Not a math operation".to_string())),
    }
}

fn apply_list_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Car => {
            if args.len() != 1 {
                return Err(LispError::Generic("car requires exactly one argument".to_string()));
            }
            if let LispVal::List(list) = &args[0] {
                if list.is_empty() {
                    Ok(LispVal::List(vec![])) // car of empty list is empty list
                } else {
                    Ok(list[0].clone())
                }
            } else {
                Err(LispError::Generic("car requires a list argument".to_string()))
            }
        }
        BuiltinFunc::Cdr => {
            if args.len() != 1 {
                return Err(LispError::Generic("cdr requires exactly one argument".to_string()));
            }
            if let LispVal::List(list) = &args[0] {
                if list.is_empty() {
                    Ok(LispVal::List(vec![])) // cdr of empty list is empty list
                } else {
                    Ok(LispVal::List(list[1..].to_vec()))
                }
            } else {
                Err(LispError::Generic("cdr requires a list argument".to_string()))
            }
        }
        BuiltinFunc::Cons => {
            if args.len() != 2 {
                return Err(LispError::Generic("cons requires exactly two arguments".to_string()));
            }
            if let LispVal::List(list) = &args[1] {
                let mut new_list = vec![args[0].clone()];
                new_list.extend_from_slice(list);
                Ok(LispVal::List(new_list))
            } else {
                Err(LispError::Generic("cons requires a list as its second argument".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

fn apply_string_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Concat => {
            let strs: Result<Vec<String>, LispError> = args
                .iter()
                .map(|arg| match arg {
                    LispVal::String(s) => Ok(s.clone()),
                    _ => Err(LispError::Generic(
                        "concat only accepts strings".to_string(),
                    )),
                })
                .collect();
            Ok(LispVal::String(strs?.concat()))
        }
        BuiltinFunc::Index => {
            if args.len() != 2 {
                return Err(LispError::Generic("index requires exactly two arguments".to_string()));
            }
            let s = if let LispVal::String(s) = &args[0] {
                s
            } else {
                return Err(LispError::Generic("index requires a string as its first argument".to_string()));
            };
            let i = if let LispVal::Number(n) = &args[1] {
                *n as usize
            } else {
                return Err(LispError::Generic("index requires a number as its second argument".to_string()));
            };
            if let Some(ch) = s.chars().nth(i) {
                Ok(LispVal::String(ch.to_string()))
            } else {
                Err(LispError::Generic("index out of bounds".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a string operation".to_string())),
    }
}

fn apply_logical_op(op: &BuiltinFunc, args: &[LispVal]) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Eq => {
            if args.len() != 2 {
                return Err(LispError::Generic("eq requires exactly two arguments".to_string()));
            }
            if args[0] == args[1] {
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Ok(LispVal::List(vec![])) // nil
            }
        }
        BuiltinFunc::Not => {
            if args.len() != 1 {
                return Err(LispError::Generic("not requires exactly one argument".to_string()));
            }
            if is_truthy(&args[0]) {
                Ok(LispVal::List(vec![])) // nil
            } else {
                Ok(LispVal::Symbol("t".to_string()))
            }
        }
        _ => Err(LispError::Generic("Not a logical operation".to_string())),
    }
}

fn apply_hashtable_op(op: &BuiltinFunc, args: &[LispVal], env: &mut Environment) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeHashTable => {
            if !args.is_empty() {
                return Err(LispError::Generic("make-hash-table takes no arguments".to_string()));
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(HashMap::new()))))
        }
        BuiltinFunc::Set => {
            if args.len() != 3 {
                return Err(LispError::Generic("set! takes exactly three arguments".to_string()));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = args[1].clone();
                let val = args[2].clone();
                h.borrow_mut().insert(key, val);
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Err(LispError::Generic("set! requires a hash table as its first argument".to_string()))
            }
        }
        BuiltinFunc::Get => {
            if args.len() != 2 {
                return Err(LispError::Generic("get takes exactly two arguments".to_string()));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                if let Some(val) = h.borrow().get(key) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::List(vec![])) // nil
                }
            } else {
                Err(LispError::Generic("get requires a hash table as its first argument".to_string()))
            }
        }
        BuiltinFunc::DeleteKey => {
            if args.len() != 2 {
                return Err(LispError::Generic("delete-key! takes exactly two arguments".to_string()));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                h.borrow_mut().remove(key);
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Err(LispError::Generic("delete-key! requires a hash table as its first argument".to_string()))
            }
        }
        BuiltinFunc::CurrentEnvironment => {
            if !args.is_empty() {
                return Err(LispError::Generic("current-environment takes no arguments".to_string()));
            }
            let bindings = env.all_bindings();
            let mut hash_map = HashMap::new();
            for (k, v) in bindings {
                hash_map.insert(LispVal::Symbol(k), v);
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(hash_map))))
        }
        _ => Err(LispError::Generic("Not a hash table operation".to_string())),
    }
}


fn apply(func: &LispVal, args: &[LispVal], env: &mut Environment) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus | BuiltinFunc::Minus | BuiltinFunc::Multiply | BuiltinFunc::Divide => {
                apply_math_op(builtin, args)
            }
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons => {
                apply_list_op(builtin, args)
            }
            BuiltinFunc::Concat | BuiltinFunc::Index => {
                apply_string_op(builtin, args)
            }
            BuiltinFunc::Eval => {
                if args.len() != 1 {
                    return Err(LispError::Generic("eval takes exactly one argument".to_string()));
                }
                eval(&args[0], env)
            }
            BuiltinFunc::Eq | BuiltinFunc::Not => {
                apply_logical_op(builtin, args)
            }
            BuiltinFunc::MakeHashTable | BuiltinFunc::Get | BuiltinFunc::Set | BuiltinFunc::DeleteKey | BuiltinFunc::CurrentEnvironment => {
                apply_hashtable_op(builtin, args, env)
            }
        },
        LispVal::Lambda(lambda) => {
            if lambda.params.len() != args.len() {
                return Err(LispError::Generic(format!(
                    "lambda expected {} arguments, got {}",
                    lambda.params.len(),
                    args.len()
                )));
            }

            let mut new_env = lambda.env.clone();
            new_env.push_scope();
            for (param, arg) in lambda.params.iter().zip(args) {
                new_env.set(param.clone(), arg.clone());
            }

            let result = eval(&lambda.body, &mut new_env);
            new_env.pop_scope();
            result
        }
        _ => Err(LispError::Generic(format!("Not a function: {:?}", func))),
    }
}

fn make_lambda(params: &LispVal, body: &LispVal, env: &Environment) -> Result<LispVal, LispError> {
    if let LispVal::List(p_list) = params {
        let params_vec: Result<Vec<String>, _> = p_list
            .iter()
            .map(|p| {
                if let LispVal::Symbol(s) = p {
                    Ok(s.clone())
                } else {
                    Err(LispError::Generic(
                        "lambda parameters must be symbols".to_string(),
                    ))
                }
            })
            .collect();

        Ok(LispVal::Lambda(crate::Lambda {
            params: params_vec?,
            body: Box::new(body.clone()),
            env: env.clone(),
        }))
    } else {
        Err(LispError::Generic(
            "lambda requires a list of parameters".to_string(),
        ))
    }
}

fn make_fexpr(params: &LispVal, body: &LispVal, env: &Environment) -> Result<LispVal, LispError> {
    if let LispVal::List(p_list) = params {
        let params_vec: Result<Vec<String>, _> = p_list
            .iter()
            .map(|p| {
                if let LispVal::Symbol(s) = p {
                    Ok(s.clone())
                } else {
                    Err(LispError::Generic(
                        "fexpr parameters must be symbols".to_string(),
                    ))
                }
            })
            .collect();

        Ok(LispVal::Fexpr(crate::Fexpr {
            params: params_vec?,
            body: Box::new(body.clone()),
            env: env.clone(),
        }))
    } else {
        Err(LispError::Generic(
            "fexpr requires a list of parameters".to_string(),
        ))
    }
}

pub fn eval(val: &LispVal, env: &mut Environment) -> Result<LispVal, LispError> {
    match val {
        // Self-evaluating forms
        LispVal::Number(_) => Ok(val.clone()),
        LispVal::String(_) => Ok(val.clone()),
        LispVal::Builtin(_) => Ok(val.clone()),
        LispVal::Lambda(_) => Ok(val.clone()),
        LispVal::Fexpr(_) => Ok(val.clone()),
        LispVal::HashTable(_) => Ok(val.clone()),

        // Symbol: look it up in the environment
        LispVal::Symbol(s) => {
            if let Some(val) = env.get(s) {
                Ok(val)
            } else {
                Err(LispError::Generic(format!("Unbound variable: {s}")))
            }
        }

        // List: this is where function calls and special forms are handled.
        LispVal::List(list) => {
            if list.is_empty() {
                return Ok(LispVal::List(vec![]));
            }

            let first = &list[0];
            let rest = &list[1..];

            if let LispVal::Symbol(s) = first {
                match s.as_str() {
                    "quote" => {
                        if rest.len() != 1 {
                            return Err(LispError::Generic(
                                "quote takes exactly one argument".to_string(),
                            ));
                        }
                        return Ok(rest[0].clone());
                    }
                    "quasiquote" => {
                        if rest.len() != 1 {
                            return Err(LispError::Generic(
                                "quasiquote takes exactly one argument".to_string(),
                            ));
                        }
                        quasiquote_eval(&rest[0], env)
                    }
                    "if" => {
                        if rest.len() != 3 {
                            return Err(LispError::Generic(
                                "if takes exactly three arguments".to_string(),
                            ));
                        }
                        let cond = &rest[0];
                        let then_expr = &rest[1];
                        let else_expr = &rest[2];

                        let cond_result = eval(cond, env)?;
                        if is_truthy(&cond_result) {
                            eval(then_expr, env)
                        } else {
                            eval(else_expr, env)
                        }
                    }
                    "and" => {
                        let mut last_val = LispVal::Symbol("t".to_string());
                        for expr in rest {
                            last_val = eval(expr, env)?;
                            if !is_truthy(&last_val) {
                                return Ok(LispVal::List(vec![])); // nil
                            }
                        }
                        Ok(last_val)
                    }
                    "or" => {
                        for expr in rest {
                            let val = eval(expr, env)?;
                            if is_truthy(&val) {
                                return Ok(val);
                            }
                        }
                        Ok(LispVal::List(vec![])) // nil
                    }
                    "def" => {
                        if rest.len() != 2 {
                            return Err(LispError::Generic(
                                "def takes exactly two arguments".to_string(),
                            ));
                        }
                        let sym = &rest[0];
                        let val_expr = &rest[1];

                        if let LispVal::Symbol(s) = sym {
                            let val = eval(val_expr, env)?;
                            env.set(s.clone(), val);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "def requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "lambda" => {
                        if rest.len() != 2 {
                            return Err(LispError::Generic(
                                "lambda takes exactly two arguments".to_string(),
                            ));
                        }
                        make_lambda(&rest[0], &rest[1], env)
                    }
                    "defun" => {
                        if rest.len() != 3 {
                            return Err(LispError::Generic(
                                "defun takes exactly three arguments".to_string(),
                            ));
                        }
                        let name = &rest[0];
                        let params = &rest[1];
                        let body = &rest[2];

                        if let LispVal::Symbol(s) = name {
                            let lambda = make_lambda(params, body, env)?;
                            env.set(s.clone(), lambda);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "defun requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "defexpr" => {
                        if rest.len() != 3 {
                            return Err(LispError::Generic(
                                "defexpr takes exactly three arguments".to_string(),
                            ));
                        }
                        let name = &rest[0];
                        let params = &rest[1];
                        let body = &rest[2];

                        if let LispVal::Symbol(s) = name {
                            let fexpr = make_fexpr(params, body, env)?;
                            env.set(s.clone(), fexpr);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "defexpr requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "let" => {
                        if rest.len() != 2 {
                            return Err(LispError::Generic(
                                "let takes exactly two arguments".to_string(),
                            ));
                        }
                        let bindings = &rest[0];
                        let body = &rest[1];

                        if let LispVal::List(b_list) = bindings {
                            let mut params = vec![];
                            let mut args = vec![];
                            for binding in b_list {
                                if let LispVal::List(pair) = binding {
                                    if pair.len() != 2 {
                                        return Err(LispError::Generic(
                                            "let binding must be a pair".to_string(),
                                        ));
                                    }
                                    params.push(pair[0].clone());
                                    args.push(pair[1].clone());
                                } else {
                                    return Err(LispError::Generic(
                                        "let bindings must be a list of pairs".to_string(),
                                    ));
                                }
                            }

                            let lambda = make_lambda(&LispVal::List(params), body, env)?;
                            let mut application = vec![lambda];
                            application.extend_from_slice(&args);
                            eval(&LispVal::List(application), env)
                        } else {
                            Err(LispError::Generic(
                                "let requires a list of bindings".to_string(),
                            ))
                        }
                    }
                    _ => {
                        // Function call
                        let func = eval(first, env)?;
                        if let LispVal::Fexpr(fexpr) = &func {
                            let mut new_env = fexpr.env.clone();
                            new_env.push_scope();
                            if fexpr.params.len() != 1 {
                                return Err(LispError::Generic("fexpr must have exactly one parameter for the list of arguments".to_string()));
                            }
                            new_env.set(fexpr.params[0].clone(), LispVal::List(rest.to_vec()));

                            let result = eval(&fexpr.body, &mut new_env);
                            new_env.pop_scope();
                            return result;
                        }
                        let args: Result<Vec<LispVal>, LispError> =
                            rest.iter().map(|arg| eval(arg, env)).collect();
                        apply(&func, &args?, env)
                    }
                }
            } else {
                // The first element is not a symbol, so it must be something that evaluates to a function.
                let func = eval(first, env)?;
                let args: Result<Vec<LispVal>, LispError> =
                    rest.iter().map(|arg| eval(arg, env)).collect();
                apply(&func, &args?, env)
            }
        }
    }
}

fn quasiquote_eval(val: &LispVal, env: &mut Environment) -> Result<LispVal, LispError> {
    if let LispVal::List(list) = val {
        if !list.is_empty() {
            if let LispVal::Symbol(s) = &list[0] {
                if s == "unquote" {
                    if list.len() != 2 {
                        return Err(LispError::Generic("unquote takes exactly one argument".to_string()));
                    }
                    return eval(&list[1], env);
                }
            }
        }
        let new_list: Result<Vec<LispVal>, _> = list.iter().map(|item| quasiquote_eval(item, env)).collect();
        return Ok(LispVal::List(new_list?));
    }
    Ok(val.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{environment::Environment, reader};

    fn eval_from_str(s: &str, env: &mut Environment) -> Result<LispVal, LispError> {
        let val = reader::read(s).unwrap();
        eval(&val, env)
    }

    #[test]
    fn test_eval_number() {
        let mut env = Environment::new();
        let val = LispVal::Number(42);
        assert_eq!(eval(&val, &mut env), Ok(val));
    }

    #[test]
    fn test_eval_string() {
        let mut env = Environment::new();
        let val = LispVal::String("hello".to_string());
        assert_eq!(eval(&val, &mut env), Ok(val));
    }

    #[test]
    fn test_eval_symbol() {
        let mut env = Environment::new();
        env.set("x".to_string(), LispVal::Number(10));
        let val = LispVal::Symbol("x".to_string());
        assert_eq!(eval(&val, &mut env), Ok(LispVal::Number(10)));
    }

    #[test]
    fn test_eval_unbound_symbol() {
        let mut env = Environment::new();
        let val = LispVal::Symbol("y".to_string());
        assert!(eval(&val, &mut env).is_err());
    }

    #[test]
    fn test_eval_quote() {
        let mut env = Environment::new();
        let result = eval_from_str("'(1 2 3)", &mut env);
        let expected = reader::read("(1 2 3)").unwrap();
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_eval_if() {
        let mut env = Environment::new_with_builtins();
        let result_true = eval_from_str("(if t 1 2)", &mut env);
        assert_eq!(result_true, Ok(LispVal::Number(1)));

        let result_false = eval_from_str("(if nil 1 2)", &mut env);
        assert_eq!(result_false, Ok(LispVal::Number(2)));
    }

    #[test]
    fn test_math_ops() {
        let mut env = Environment::new_with_builtins();
        assert_eq!(eval_from_str("(+ 1 2 3)", &mut env), Ok(LispVal::Number(6)));
        assert_eq!(eval_from_str("(- 10 5)", &mut env), Ok(LispVal::Number(5)));
        assert_eq!(eval_from_str("(- 5)", &mut env), Ok(LispVal::Number(-5)));
        assert_eq!(eval_from_str("(* 2 3 4)", &mut env), Ok(LispVal::Number(24)));
        assert_eq!(eval_from_str("(/ 10 2)", &mut env), Ok(LispVal::Number(5)));
    }

    #[test]
    fn test_list_ops() {
        let mut env = Environment::new_with_builtins();
        assert_eq!(eval_from_str("(car '(1 2 3))", &mut env), Ok(LispVal::Number(1)));
        assert_eq!(eval_from_str("(cdr '(1 2 3))", &mut env), Ok(reader::read("(2 3)").unwrap()));
        assert_eq!(eval_from_str("(cons 1 '(2 3))", &mut env), Ok(reader::read("(1 2 3)").unwrap()));
    }

    #[test]
    fn test_lambda_and_def() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(def square (lambda (x) (* x x)))", &mut env).unwrap();
        let result = eval_from_str("(square 5)", &mut env);
        assert_eq!(result, Ok(LispVal::Number(25)));
    }

    #[test]
    fn test_defun() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(defun square (x) (* x x))", &mut env).unwrap();
        let result = eval_from_str("(square 5)", &mut env);
        assert_eq!(result, Ok(LispVal::Number(25)));
    }

    #[test]
    fn test_let() {
        let mut env = Environment::new_with_builtins();
        let result = eval_from_str("(let ((x 1) (y 2)) (+ x y))", &mut env);
        assert_eq!(result, Ok(LispVal::Number(3)));
    }

    #[test]
    fn test_string_ops() {
        let mut env = Environment::new_with_builtins();
        assert_eq!(eval_from_str("(concat \"a\" \"b\" \"c\")", &mut env), Ok(LispVal::String("abc".to_string())));
        assert_eq!(eval_from_str("(++ \"a\" \"b\")", &mut env), Ok(LispVal::String("ab".to_string())));
        assert_eq!(eval_from_str("(index \"hello\" 1)", &mut env), Ok(LispVal::String("e".to_string())));
    }

    #[test]
    fn test_quasiquote() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(def x 10)", &mut env).unwrap();
        let result = eval_from_str("`(1 ,x 3)", &mut env);
        let expected = reader::read("(1 10 3)").unwrap();
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_defexpr() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(defexpr my-if (args) (if (eval (car args)) (eval (car (cdr args))) (eval (car (cdr (cdr args))))))", &mut env).unwrap();
        let result = eval_from_str("(my-if t 1 2)", &mut env);
        assert_eq!(result, Ok(LispVal::Number(1)));
    }

    #[test]
    fn test_logical_ops() {
        let mut env = Environment::new_with_builtins();
        assert_eq!(eval_from_str("(eq 1 1)", &mut env), Ok(LispVal::Symbol("t".to_string())));
        assert_eq!(eval_from_str("(eq 1 2)", &mut env), Ok(LispVal::List(vec![])));
        assert_eq!(eval_from_str("(not t)", &mut env), Ok(LispVal::List(vec![])));
        assert_eq!(eval_from_str("(not nil)", &mut env), Ok(LispVal::Symbol("t".to_string())));
        assert_eq!(eval_from_str("(and t t)", &mut env), Ok(LispVal::Symbol("t".to_string())));
        assert_eq!(eval_from_str("(and t nil)", &mut env), Ok(LispVal::List(vec![])));
        assert_eq!(eval_from_str("(or t nil)", &mut env), Ok(LispVal::Symbol("t".to_string())));
        assert_eq!(eval_from_str("(or nil nil)", &mut env), Ok(LispVal::List(vec![])));
    }

    #[test]
    fn test_hashtable() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(def h (make-hash-table))", &mut env).unwrap();
        eval_from_str("(set! h 'a 1)", &mut env).unwrap();
        assert_eq!(eval_from_str("(get h 'a)", &mut env), Ok(LispVal::Number(1)));
        eval_from_str("(delete-key! h 'a)", &mut env).unwrap();
        assert_eq!(eval_from_str("(get h 'a)", &mut env), Ok(LispVal::List(vec![])));
    }

    #[test]
    fn test_current_environment() {
        let mut env = Environment::new_with_builtins();
        eval_from_str("(def x 10)", &mut env).unwrap();
        let result = eval_from_str("(get (current-environment) 'x)", &mut env);
        assert_eq!(result, Ok(LispVal::Number(10)));
    }
}
