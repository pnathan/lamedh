use crate::{BuiltinFunc, LispError, LispVal, environment::Environment};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Helper function to convert a Lisp list (Cons chain) to a Rust Vec.
fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    let mut vec = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(*car.clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(
            "list_to_vec: not a proper list".to_string(),
        ));
    }
    Ok(vec)
}

// Helper function to convert a Rust Vec to a Lisp list.
fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        })
}

fn is_truthy(val: &LispVal) -> bool {
    match val {
        LispVal::Nil => false,
        _ => true,
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
                return Err(LispError::Generic(
                    "- requires at least one argument".to_string(),
                ));
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
                return Err(LispError::Generic(
                    "/ requires exactly two arguments".to_string(),
                ));
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
                return Err(LispError::Generic(
                    "car requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { car, .. } => Ok(*car.clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("car requires a list".to_string())),
            }
        }
        BuiltinFunc::Cdr => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "cdr requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Cons { cdr, .. } => Ok(*cdr.clone()),
                LispVal::Nil => Ok(LispVal::Nil),
                _ => Err(LispError::Generic("cdr requires a list".to_string())),
            }
        }
        BuiltinFunc::Cons => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "cons requires exactly two arguments".to_string(),
                ));
            }
            Ok(LispVal::Cons {
                car: Box::new(args[0].clone()),
                cdr: Box::new(args[1].clone()),
            })
        }
        BuiltinFunc::Member => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "member requires exactly two arguments".to_string(),
                ));
            }
            let item = &args[0];
            let mut list = &args[1];
            while let LispVal::Cons { car, cdr } = list {
                if &**car == item {
                    return Ok(list.clone());
                }
                list = cdr;
            }
            Ok(LispVal::Nil)
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
                return Err(LispError::Generic(
                    "index requires exactly two arguments".to_string(),
                ));
            }
            let s = if let LispVal::String(s) = &args[0] {
                s
            } else {
                return Err(LispError::Generic(
                    "index requires a string as its first argument".to_string(),
                ));
            };
            let i = if let LispVal::Number(n) = &args[1] {
                *n as usize
            } else {
                return Err(LispError::Generic(
                    "index requires a number as its second argument".to_string(),
                ));
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
                return Err(LispError::Generic(
                    "eq requires exactly two arguments".to_string(),
                ));
            }
            if args[0] == args[1] {
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Not => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "not requires exactly one argument".to_string(),
                ));
            }
            if is_truthy(&args[0]) {
                Ok(LispVal::Nil)
            } else {
                Ok(LispVal::Symbol("t".to_string()))
            }
        }
        BuiltinFunc::NumericEquals => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "= requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(a), LispVal::Number(b)) = (&args[0], &args[1]) {
                if a == b {
                    Ok(LispVal::Symbol("t".to_string()))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "= requires numeric arguments".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a logical operation".to_string())),
    }
}

fn apply_hashtable_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &mut Environment,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::MakeHashTable => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "make-hash-table takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(HashMap::new()))))
        }
        BuiltinFunc::Set => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "set! takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = args[1].clone();
                let val = args[2].clone();
                h.borrow_mut().insert(key, val);
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Err(LispError::Generic(
                    "set! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::Get => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                if let Some(val) = h.borrow().get(key) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(
                    "get requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::DeleteKey => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "delete-key! takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let key = &args[1];
                h.borrow_mut().remove(key);
                Ok(LispVal::Symbol("t".to_string()))
            } else {
                Err(LispError::Generic(
                    "delete-key! requires a hash table as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::CurrentEnvironment => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "current-environment takes no arguments".to_string(),
                ));
            }
            let bindings = env.all_bindings();
            let mut hash_map = HashMap::new();
            for (k, v) in bindings {
                hash_map.insert(LispVal::Symbol(k), v);
            }
            Ok(LispVal::HashTable(Rc::new(RefCell::new(hash_map))))
        }
        BuiltinFunc::Keys => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "keys requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::HashTable(h) = &args[0] {
                let keys = h.borrow().keys().cloned().collect();
                Ok(vec_to_list(keys))
            } else {
                Err(LispError::Generic(
                    "keys requires a hash table as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a hash table operation".to_string())),
    }
}

fn apply(func: &LispVal, args: &[LispVal], env: &mut Environment) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus
            | BuiltinFunc::Minus
            | BuiltinFunc::Multiply
            | BuiltinFunc::Divide => apply_math_op(builtin, args),
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons | BuiltinFunc::Member => {
                apply_list_op(builtin, args)
            }
            BuiltinFunc::Concat | BuiltinFunc::Index => apply_string_op(builtin, args),
            BuiltinFunc::Eval => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "eval takes exactly one argument".to_string(),
                    ));
                }
                eval(&args[0], env)
            }
            BuiltinFunc::Eq | BuiltinFunc::Not | BuiltinFunc::NumericEquals => {
                apply_logical_op(builtin, args)
            }
            BuiltinFunc::MakeHashTable
            | BuiltinFunc::Get
            | BuiltinFunc::Set
            | BuiltinFunc::DeleteKey
            | BuiltinFunc::CurrentEnvironment
            | BuiltinFunc::Keys => apply_hashtable_op(builtin, args, env),
            BuiltinFunc::Atom => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "atom requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::Cons { .. } => Ok(LispVal::Nil),
                    _ => Ok(LispVal::Symbol("t".to_string())),
                }
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
        _ => Err(LispError::Generic(format!("Not a function: {func:?}"))),
    }
}

fn make_lambda(params: &LispVal, body: &LispVal, env: &Environment) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
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
}

fn make_fexpr(params: &LispVal, body: &LispVal, env: &Environment) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
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
}

fn make_macro(params: &LispVal, body: &LispVal, env: &Environment) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                Ok(s.clone())
            } else {
                Err(LispError::Generic(
                    "macro parameters must be symbols".to_string(),
                ))
            }
        })
        .collect();

    Ok(LispVal::Macro(crate::Macro {
        params: params_vec?,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
}

fn expand_macro(
    m: &crate::Macro,
    args: &[LispVal],
    _env: &mut Environment,
) -> Result<LispVal, LispError> {
    if m.params.len() != args.len() {
        return Err(LispError::Generic(format!(
            "macro expected {} arguments, got {}",
            m.params.len(),
            args.len()
        )));
    }

    let mut macro_env = m.env.clone();
    macro_env.push_scope();
    for (param, arg) in m.params.iter().zip(args) {
        macro_env.set(param.clone(), arg.clone());
    }

    let expanded = eval(&m.body, &mut macro_env);
    macro_env.pop_scope();
    expanded
}

pub fn eval(val: &LispVal, env: &mut Environment) -> Result<LispVal, LispError> {
    match val {
        LispVal::Nil => Ok(LispVal::Nil),
        LispVal::Symbol(s) => env
            .get(s)
            .ok_or_else(|| LispError::Generic(format!("Unbound variable: {s}"))),
        LispVal::Number(_)
        | LispVal::String(_)
        | LispVal::Builtin(_)
        | LispVal::Lambda(_)
        | LispVal::Fexpr(_)
        | LispVal::Macro(_)
        | LispVal::HashTable(_) => Ok(val.clone()),

        LispVal::Cons {
            car: first,
            cdr: rest,
        } => {
            if let LispVal::Symbol(s) = &**first {
                match s.as_str() {
                    "quote" => {
                        if let LispVal::Cons { car, cdr } = &**rest {
                            if **cdr == LispVal::Nil {
                                return Ok(*car.clone());
                            }
                        }
                        Err(LispError::Generic(
                            "quote takes exactly one argument".to_string(),
                        ))
                    }
                    "quasiquote" => {
                        if let LispVal::Cons { car, cdr } = &**rest {
                            if **cdr == LispVal::Nil {
                                return quasiquote_eval(car, env);
                            }
                        }
                        Err(LispError::Generic(
                            "quasiquote takes exactly one argument".to_string(),
                        ))
                    }
                    "cond" => {
                        let mut current_clause = &**rest;
                        while let LispVal::Cons {
                            car: clause,
                            cdr: next_clauses,
                        } = current_clause
                        {
                            if let LispVal::Cons {
                                car: predicate,
                                cdr: expressions,
                            } = &**clause
                            {
                                let predicate_result = eval(predicate, env)?;
                                if is_truthy(&predicate_result) {
                                    if **expressions == LispVal::Nil {
                                        return Ok(predicate_result);
                                    } else {
                                        let mut last_val = LispVal::Nil;
                                        let mut current_expr = &**expressions;
                                        while let LispVal::Cons {
                                            car: expr,
                                            cdr: next_exprs,
                                        } = current_expr
                                        {
                                            last_val = eval(expr, env)?;
                                            current_expr = next_exprs;
                                        }
                                        return Ok(last_val);
                                    }
                                }
                            } else {
                                return Err(LispError::Generic(
                                    "cond clauses must be lists".to_string(),
                                ));
                            }
                            current_clause = next_clauses;
                        }
                        Ok(LispVal::Nil) // No clause was true
                    }
                    "if" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 3 {
                            return Err(LispError::Generic(
                                "if takes exactly three arguments".to_string(),
                            ));
                        }
                        let cond_result = eval(&args[0], env)?;
                        if is_truthy(&cond_result) {
                            eval(&args[1], env)
                        } else {
                            eval(&args[2], env)
                        }
                    }
                    "and" => {
                        let mut last_val = LispVal::Symbol("t".to_string());
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            last_val = eval(car, env)?;
                            if !is_truthy(&last_val) {
                                return Ok(LispVal::Nil);
                            }
                            current = cdr;
                        }
                        Ok(last_val)
                    }
                    "or" => {
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            let val = eval(car, env)?;
                            if is_truthy(&val) {
                                return Ok(val);
                            }
                            current = cdr;
                        }
                        Ok(LispVal::Nil)
                    }
                    "def" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Err(LispError::Generic(
                                "def takes exactly two arguments".to_string(),
                            ));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            // For recursion, first bind the symbol to a dummy value.
                            env.set(s.clone(), LispVal::Nil);
                            // Then, eval the value, which can now capture itself in its closure.
                            let val = eval(&args[1], env)?;
                            // Now set the actual value.
                            env.set(s.clone(), val);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "def requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "lambda" => {
                        if let LispVal::Cons {
                            car: params,
                            cdr: body_list,
                        } = &**rest
                        {
                            if let LispVal::Cons {
                                car: body,
                                cdr: rest_body,
                            } = &**body_list
                            {
                                if **rest_body == LispVal::Nil {
                                    return make_lambda(params, body, env);
                                }
                            }
                        }
                        Err(LispError::Generic(
                            "lambda takes exactly two arguments".to_string(),
                        ))
                    }
                    "defexpr" | "defmacro" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 3 {
                            return Err(LispError::Generic(
                                format!("{s} takes exactly three arguments").to_string(),
                            ));
                        }
                        if let LispVal::Symbol(name_str) = &args[0] {
                            let func = if s == "defexpr" {
                                make_fexpr(&args[1], &args[2], env)?
                            } else {
                                make_macro(&args[1], &args[2], env)?
                            };
                            env.set(name_str.clone(), func);
                            Ok(LispVal::Symbol(name_str.clone()))
                        } else {
                            Err(LispError::Generic(
                                format!("{s} requires a symbol as its first argument")
                                    .to_string(),
                            ))
                        }
                    }
                    "let" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Err(LispError::Generic(
                                "let takes exactly two arguments".to_string(),
                            ));
                        }
                        let bindings_vec = list_to_vec(&args[0])?;
                        let body = &args[1];

                        let mut params = vec![];
                        let mut arg_exprs = vec![];
                        for binding in bindings_vec {
                            let pair = list_to_vec(&binding)?;
                            if pair.len() != 2 {
                                return Err(LispError::Generic(
                                    "let binding must be a pair".to_string(),
                                ));
                            }
                            params.push(pair[0].clone());
                            arg_exprs.push(pair[1].clone());
                        }

                        let lambda = make_lambda(&vec_to_list(params), body, env)?;
                        let mut application = vec![lambda];
                        application.extend(arg_exprs);
                        eval(&vec_to_list(application), env)
                    }
                    _ => {
                        // Function call
                        let func = eval(first, env)?;
                        let args_list = list_to_vec(rest)?;
                        if let LispVal::Macro(m) = &func {
                            let expanded = expand_macro(m, &args_list, env)?;
                            return eval(&expanded, env);
                        }
                        if let LispVal::Fexpr(fexpr) = &func {
                            let mut new_env = fexpr.env.clone();
                            new_env.push_scope();
                            if fexpr.params.len() != 1 {
                                return Err(LispError::Generic("fexpr must have exactly one parameter for the list of arguments".to_string()));
                            }
                            new_env.set(fexpr.params[0].clone(), *rest.clone());
                            let result = eval(&fexpr.body, &mut new_env);
                            new_env.pop_scope();
                            return result;
                        }

                        let eval_args: Result<Vec<LispVal>, LispError> =
                            args_list.iter().map(|arg| eval(arg, env)).collect();
                        apply(&func, &eval_args?, env)
                    }
                }
            } else {
                let func = eval(first, env)?;
                let args_list = list_to_vec(rest)?;
                let eval_args: Result<Vec<LispVal>, LispError> =
                    args_list.iter().map(|arg| eval(arg, env)).collect();
                apply(&func, &eval_args?, env)
            }
        }
    }
}

fn quasiquote_eval(val: &LispVal, env: &mut Environment) -> Result<LispVal, LispError> {
    if let LispVal::Cons { car, cdr } = val {
        if let LispVal::Symbol(s) = &**car {
            if s == "unquote" {
                if let LispVal::Cons {
                    car: unquoted_val,
                    cdr: rest,
                } = &**cdr
                {
                    if **rest == LispVal::Nil {
                        return eval(unquoted_val, env);
                    }
                }
                return Err(LispError::Generic(
                    "unquote takes exactly one argument".to_string(),
                ));
            }
        }
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Box::new(car_eval),
            cdr: Box::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}
