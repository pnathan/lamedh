#![allow(clippy::mutable_key_type)]
use crate::{environment::Environment, BuiltinFunc, LispError, LispVal};
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

fn apply_apply(args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "APPLY requires exactly two arguments".to_string(),
        ));
    }
    let func_arg = &args[0];
    let arg_list = &args[1];

    let func = match func_arg {
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Function not found: {}", s.borrow().name))),
        _ => Ok(func_arg.clone()),
    }?;

    let unpacked_args = match list_to_vec(arg_list) {
        Ok(vec) => vec,
        Err(_) => {
            return Err(LispError::Generic(
                "APPLY second argument must be a proper list".to_string(),
            ))
        }
    };

    match &func {
        LispVal::Macro(m) => {
            let expanded = expand_macro(m, &unpacked_args, env)?;
            eval(&expanded, env)
        }
        LispVal::Fexpr(f) => {
            if f.params.len() != 1 {
                return Err(LispError::Generic(
                    "APPLY: fexpr must have exactly one parameter for the list of arguments"
                        .to_string(),
                ));
            }
            let new_env = Environment::new_child(&f.env);
            let fexpr_arg_list = vec_to_list(unpacked_args);
            new_env.set(f.params[0].clone(), fexpr_arg_list);
            eval(&f.body, &new_env)
        }
        _ => apply(&func, &unpacked_args, env),
    }
}

fn is_truthy(val: &LispVal) -> bool {
    !matches!(val, LispVal::Nil)
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
        BuiltinFunc::Plus => {
            let result = nums.iter().fold(0i64, |acc, &x| acc.wrapping_add(x));
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Minus => {
            if nums.is_empty() {
                return Err(LispError::Generic(
                    "- requires at least one argument".to_string(),
                ));
            }
            if nums.len() == 1 {
                Ok(LispVal::Number(nums[0].wrapping_neg()))
            } else {
                let mut result = nums[0];
                for &num in &nums[1..] {
                    result = result.wrapping_sub(num);
                }
                Ok(LispVal::Number(result))
            }
        }
        BuiltinFunc::Multiply => {
            let result = nums.iter().fold(1i64, |acc, &x| acc.wrapping_mul(x));
            Ok(LispVal::Number(result))
        }
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

fn apply_logical_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Eq => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "eq requires exactly two arguments".to_string(),
                ));
            }
            if args[0] == args[1] {
                Ok(LispVal::Symbol(env.intern_symbol("T")))
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
                Ok(LispVal::Symbol(env.intern_symbol("T")))
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
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
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
    env: &Rc<Environment>,
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
                Ok(LispVal::Symbol(env.intern_symbol("T")))
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
                Ok(LispVal::Symbol(env.intern_symbol("T")))
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
                hash_map.insert(LispVal::Symbol(env.intern_symbol(&k)), v);
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

fn apply(func: &LispVal, args: &[LispVal], env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match func {
        LispVal::Builtin(builtin) => match builtin {
            BuiltinFunc::Plus
            | BuiltinFunc::Minus
            | BuiltinFunc::Multiply
            | BuiltinFunc::Divide => apply_math_op(builtin, args),
            BuiltinFunc::Car | BuiltinFunc::Cdr | BuiltinFunc::Cons => apply_list_op(builtin, args),
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
                apply_logical_op(builtin, args, env)
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
                    _ => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                }
            }
            BuiltinFunc::Print => {
                for arg in args {
                    print!("{}", crate::printer::print(arg));
                }
                println!();
                Ok(LispVal::Nil)
            }
            BuiltinFunc::GetP | BuiltinFunc::PutP => apply_symbol_op(builtin, args, env),
            BuiltinFunc::Stringp => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "stringp requires exactly one argument".to_string(),
                    ));
                }
                match &args[0] {
                    LispVal::String(_) => Ok(LispVal::Symbol(env.intern_symbol("T"))),
                    _ => Ok(LispVal::Nil),
                }
            }

            BuiltinFunc::Apply => apply_apply(args, env),

            BuiltinFunc::LoadFile => {
                if args.len() != 1 {
                    return Err(LispError::Generic(
                        "load-file requires exactly one argument".to_string(),
                    ));
                }

                let filename = if let LispVal::String(path) = &args[0] {
                    path.clone()
                } else {
                    return Err(LispError::Generic(
                        "load-file requires a string filename".to_string(),
                    ));
                };

                crate::load_file(&filename, env)?;
                Ok(LispVal::Symbol(env.intern_symbol("T")))
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
            let new_env = Environment::new_child(&lambda.env);
            for (param, arg) in lambda.params.iter().zip(args) {
                new_env.set(param.clone(), arg.clone());
            }

            eval(&lambda.body, &new_env)
        }
        _ => Err(LispError::Generic(format!("Not a function: {func:?}"))),
    }
}

fn make_lambda(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                Ok(s.borrow().name.clone())
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

fn make_fexpr(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let params_vec: Result<Vec<String>, _> = p_list
        .iter()
        .map(|p| {
            if let LispVal::Symbol(s) = p {
                Ok(s.borrow().name.clone())
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

fn make_macro(
    params: &LispVal,
    body: &LispVal,
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let p_list = list_to_vec(params)?;
    let mut params_vec = Vec::new();
    let mut rest_param = None;
    let mut iter = p_list.iter();

    while let Some(p) = iter.next() {
        if let LispVal::Symbol(s) = p {
            if s.borrow().name == "&REST" {
                if let Some(LispVal::Symbol(rest_p_sym)) = iter.next() {
                    if iter.next().is_some() {
                        return Err(LispError::Generic(
                            "Only one symbol can follow &rest".to_string(),
                        ));
                    }
                    rest_param = Some(rest_p_sym.borrow().name.clone());
                    break; // No more params after &rest
                } else {
                    return Err(LispError::Generic(
                        "&rest must be followed by a symbol".to_string(),
                    ));
                }
            } else {
                params_vec.push(s.borrow().name.clone());
            }
        } else {
            return Err(LispError::Generic(
                "macro parameters must be symbols".to_string(),
            ));
        }
    }

    Ok(LispVal::Macro(crate::Macro {
        params: params_vec,
        rest_param,
        body: Box::new(body.clone()),
        env: env.clone(),
    }))
}

fn expand_macro(
    m: &crate::Macro,
    args: &[LispVal],
    _env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    let macro_env = Environment::new_child(&m.env);

    if let Some(rest_param_name) = &m.rest_param {
        if args.len() < m.params.len() {
            return Err(LispError::Generic(format!(
                "macro expected at least {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args.iter()) {
            macro_env.set(param.clone(), arg.clone());
        }
        let rest_args = vec_to_list(args[m.params.len()..].to_vec());
        macro_env.set(rest_param_name.clone(), rest_args);
    } else {
        if m.params.len() != args.len() {
            return Err(LispError::Generic(format!(
                "macro expected {} arguments, got {}",
                m.params.len(),
                args.len()
            )));
        }
        for (param, arg) in m.params.iter().zip(args) {
            macro_env.set(param.clone(), arg.clone());
        }
    }

    eval(&m.body, &macro_env)
}

pub fn eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    match val {
        LispVal::Nil => Ok(LispVal::Nil),
        LispVal::Symbol(s) => env
            .get(&s.borrow().name)
            .ok_or_else(|| LispError::Generic(format!("Unbound variable: {}", s.borrow().name))),
        LispVal::Number(_)
        | LispVal::Float(_)
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
                match s.borrow().name.as_str() {
                    "QUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return Ok(*car.clone());
                        }
                        Err(LispError::Generic(
                            "quote takes exactly one argument".to_string(),
                        ))
                    }
                    "QUASIQUOTE" => {
                        if let LispVal::Cons { car, cdr } = &**rest
                            && **cdr == LispVal::Nil
                        {
                            return quasiquote_eval(car, env);
                        }
                        Err(LispError::Generic(
                            "quasiquote takes exactly one argument".to_string(),
                        ))
                    }
                    "COND" => {
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
                    "IF" => {
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
                    "AND" => {
                        let mut last_val = LispVal::Symbol(env.intern_symbol("T"));
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
                    "OR" => {
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
                    "DEF" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 && args.len() != 3 {
                            return Err(LispError::Generic(
                                "def takes two or three arguments".to_string(),
                            ));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            let name = s.borrow().name.clone();
                            let val = eval(&args[1], env)?;
                            if args.len() == 3 {
                                if let LispVal::String(doc) = &args[2] {
                                    s.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                } else {
                                    return Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ));
                                }
                            }
                            env.set(name, val);
                            Ok(LispVal::Symbol(s.clone()))
                        } else {
                            Err(LispError::Generic(
                                "def requires a symbol as its first argument".to_string(),
                            ))
                        }
                    }
                    "LAMBDA" => {
                        if let LispVal::Cons {
                            car: params,
                            cdr: body_list,
                        } = &**rest
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym = LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return make_lambda(params, &final_body, env);
                        }
                        Err(LispError::Generic(
                            "lambda requires params and at least one body expression".to_string(),
                        ))
                    }
                    "FUNCTION" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "FUNCTION takes exactly one argument".to_string(),
                            ));
                        }
                        // The argument must be a LAMBDA expression
                        if let LispVal::Cons {
                            car: lambda_sym,
                            cdr: lambda_body,
                        } = &args[0]
                            && let LispVal::Symbol(s) = &**lambda_sym
                            && s.borrow().name == "LAMBDA"
                            && let LispVal::Cons {
                                car: params,
                                cdr: body_list,
                            } = &**lambda_body
                        {
                            let body_exprs = list_to_vec(body_list)?;
                            let final_body = if body_exprs.len() == 1 {
                                body_exprs[0].clone()
                            } else {
                                let progn_sym =
                                    LispVal::Symbol(env.intern_symbol("PROGN"));
                                vec_to_list([vec![progn_sym], body_exprs].concat())
                            };
                            return make_lambda(params, &final_body, env);
                        }
                        Err(LispError::Generic(
                            "FUNCTION argument must be a LAMBDA expression".to_string(),
                        ))
                    }
                    "LABEL" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 2 {
                            return Err(LispError::Generic(
                                "LABEL requires a name and an expression".to_string(),
                            ));
                        }
                        let name_val = &args[0];
                        let expr_val = &args[1];

                        if let LispVal::Symbol(name_sym) = name_val {
                            let new_env = Environment::new_child(env);
                            let label_expr = LispVal::Cons {
                                car: Box::new(LispVal::Symbol(env.intern_symbol("LABEL"))),
                                cdr: rest.clone(),
                            };
                            new_env.set(name_sym.borrow().name.clone(), label_expr);
                            eval(expr_val, &new_env)
                        } else {
                            Err(LispError::Generic(
                                "LABEL name must be a symbol".to_string(),
                            ))
                        }
                    }
                    "DEFINE" => {
                        let defs = list_to_vec(rest)?;
                        if defs.len() != 1 {
                            return Err(LispError::Generic(
                                "DEFINE takes a list of definitions".to_string(),
                            ));
                        }
                        let def_list = list_to_vec(&defs[0])?;
                        let mut defined_names = vec![];
                        for def in def_list {
                            let def_pair = list_to_vec(&def)?;
                            if def_pair.len() != 2 {
                                return Err(LispError::Generic(
                                    "Each definition must be a pair of name and value".to_string(),
                                ));
                            }
                            if let LispVal::Symbol(s) = &def_pair[0] {
                                let name = s.borrow().name.clone();
                                let val = &def_pair[1];
                                env.set(name, val.clone());
                                defined_names.push(LispVal::Symbol(s.clone()));
                            } else {
                                return Err(LispError::Generic(
                                    "Definition name must be a symbol".to_string(),
                                ));
                            }
                        }
                        Ok(vec_to_list(defined_names))
                    }
                    "DEFEXPR" | "DEFMACRO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() < 3 || args.len() > 4 {
                            return Err(LispError::Generic(
                                format!("{} takes three or four arguments", s.borrow().name)
                                    .to_string(),
                            ));
                        }
                        if let LispVal::Symbol(name_sym) = &args[0] {
                            let params = &args[1];
                            let mut body_idx = 2;
                            if args.len() == 4 {
                                if let LispVal::String(doc) = &args[2] {
                                    name_sym.borrow_mut().plist.insert(
                                        "docstring".to_string(),
                                        LispVal::String(doc.clone()),
                                    );
                                    body_idx = 3;
                                } else {
                                    return Err(LispError::Generic(
                                        "docstring must be a string".to_string(),
                                    ));
                                }
                            }
                            let body = &args[body_idx];
                            let func = if s.borrow().name == "DEFEXPR" {
                                make_fexpr(params, body, env)?
                            } else {
                                make_macro(params, body, env)?
                            };
                            env.set(name_sym.borrow().name.clone(), func);
                            Ok(LispVal::Symbol(name_sym.clone()))
                        } else {
                            Err(LispError::Generic(
                                format!(
                                    "{} requires a symbol as its first argument",
                                    s.borrow().name
                                )
                                .to_string(),
                            ))
                        }
                    }
                    "PROGN" => {
                        let mut last_val = LispVal::Nil;
                        let mut current = &**rest;
                        while let LispVal::Cons { car, cdr } = current {
                            last_val = eval(car, env)?;
                            current = cdr;
                        }
                        Ok(last_val)
                    }
                    "SETQ" => {
                        let args_vec = list_to_vec(rest)?;
                        if args_vec.len() % 2 != 0 {
                            return Err(LispError::Generic(
                                "SETQ requires an even number of arguments".to_string(),
                            ));
                        }
                        let mut last_val = LispVal::Nil;
                        for chunk in args_vec.chunks(2) {
                            let var = &chunk[0];
                            let val_expr = &chunk[1];
                            if let LispVal::Symbol(s) = var {
                                let val = eval(val_expr, env)?;
                                Environment::update(env, &s.borrow().name, val.clone());
                                last_val = val;
                            } else {
                                return Err(LispError::Generic(
                                    "SETQ variable name must be a symbol".to_string(),
                                ));
                            }
                        }
                        Ok(last_val)
                    }
                    "PROG" => {
                        let args = list_to_vec(rest)?;
                        if args.is_empty() {
                            return Err(LispError::Generic(
                                "PROG requires at least a var list".to_string(),
                            ));
                        }

                        let var_list = list_to_vec(&args[0])?;
                        let body = &args[1..];

                        let prog_env = Environment::new_child(env);

                        for var in var_list {
                            if let LispVal::Symbol(s) = var {
                                prog_env.set(s.borrow().name.clone(), LispVal::Nil);
                            } else {
                                return Err(LispError::Generic(
                                    "PROG variable list must contain only symbols".to_string(),
                                ));
                            }
                        }

                        let mut labels = HashMap::new();
                        for (i, item) in body.iter().enumerate() {
                            if let LispVal::Symbol(s) = item {
                                labels.insert(s.borrow().name.clone(), i);
                            }
                        }

                        let mut pc = 0;
                        loop {
                            if pc >= body.len() {
                                break Ok(LispVal::Nil); // Fell off the end
                            }

                            let item = &body[pc];

                            // If it's a label, just skip it.
                            if let LispVal::Symbol(_) = item {
                                pc += 1;
                                continue;
                            }

                            match eval(item, &prog_env) {
                                Ok(_) => {
                                    pc += 1;
                                }
                                Err(LispError::Return(val)) => {
                                    break Ok(*val);
                                }
                                Err(LispError::Go(label)) => {
                                    if let Some(new_pc) = labels.get(&label) {
                                        pc = *new_pc;
                                    } else {
                                        break Err(LispError::Generic(format!(
                                            "GO: label not found in PROG: {label}"
                                        )));
                                    }
                                }
                                Err(e) => {
                                    break Err(e);
                                }
                            }
                        }
                    }
                    "RETURN" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "RETURN takes exactly one argument".to_string(),
                            ));
                        }
                        let retval = eval(&args[0], env)?;
                        Err(LispError::Return(Box::new(retval)))
                    }
                    "GO" => {
                        let args = list_to_vec(rest)?;
                        if args.len() != 1 {
                            return Err(LispError::Generic(
                                "GO takes exactly one argument".to_string(),
                            ));
                        }
                        if let LispVal::Symbol(s) = &args[0] {
                            Err(LispError::Go(s.borrow().name.clone()))
                        } else {
                            Err(LispError::Generic(
                                "GO argument must be a symbol".to_string(),
                            ))
                        }
                    }
                    "LET" => {
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
                            if fexpr.params.len() != 1 {
                                return Err(LispError::Generic("fexpr must have exactly one parameter for the list of arguments".to_string()));
                            }
                            let new_env = Environment::new_child(&fexpr.env);
                            new_env.set(fexpr.params[0].clone(), *rest.clone());
                            return eval(&fexpr.body, &new_env);
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

fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    if let LispVal::Cons { car, cdr } = val {
        if let LispVal::Symbol(s) = &**car
            && s.borrow().name == "UNQUOTE"
        {
            if let LispVal::Cons {
                car: unquoted_val,
                cdr: rest,
            } = &**cdr
                && **rest == LispVal::Nil
            {
                return eval(unquoted_val, env);
            }
            return Err(LispError::Generic(
                "unquote takes exactly one argument".to_string(),
            ));
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

fn apply_symbol_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Rc<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::GetP => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get-p takes exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                if let LispVal::String(prop) = &args[1] {
                    if let Some(val) = s.borrow().plist.get(prop) {
                        Ok(val.clone())
                    } else {
                        Ok(LispVal::Nil)
                    }
                } else {
                    Err(LispError::Generic(
                        "get-p requires a string as its second argument".to_string(),
                    ))
                }
            } else {
                Err(LispError::Generic(
                    "get-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        BuiltinFunc::PutP => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "put-p takes exactly three arguments".to_string(),
                ));
            }
            if let LispVal::Symbol(s) = &args[0] {
                if let LispVal::String(prop) = &args[1] {
                    let val = args[2].clone();
                    s.borrow_mut().plist.insert(prop.clone(), val);
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Err(LispError::Generic(
                        "put-p requires a string as its second argument".to_string(),
                    ))
                }
            } else {
                Err(LispError::Generic(
                    "put-p requires a symbol as its first argument".to_string(),
                ))
            }
        }
        _ => Err(LispError::Generic("Not a symbol operation".to_string())),
    }
}