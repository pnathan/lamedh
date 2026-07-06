use super::*;
#[inline(never)]
pub(super) fn apply_symbol_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::GetP => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "get-p takes exactly two arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(format!(
                        "GET-P: expected a symbol or string as its second argument, got {}",
                        err_val(&args[1])
                    )));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                if let Some(val) = s.borrow().plist.get(&prop) {
                    Ok(val.clone())
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(format!(
                    "GET-P: expected a symbol as its first argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::PutP => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "put-p takes exactly three arguments".to_string(),
                ));
            }
            let prop = match &args[1] {
                LispVal::String(s) => s.clone(),
                LispVal::Symbol(s) => s.borrow().name.clone(),
                _ => {
                    return Err(LispError::Generic(format!(
                        "PUT-P: expected a symbol or string as its second argument, got {}",
                        err_val(&args[1])
                    )));
                }
            };
            if let LispVal::Symbol(s) = &args[0] {
                let val = args[2].clone();
                s.borrow_mut().plist.insert(prop, val);
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Err(LispError::Generic(format!(
                    "PUT-P: expected a symbol as its first argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        _ => Err(LispError::Generic("Not a symbol operation".to_string())),
    }
}

// I/O operations
#[inline(never)]
pub(super) fn apply_io_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Read => {
            if !env.feature_enabled("IO") {
                return Err(LispError::Generic(
                    "IO capability is not enabled (grant it via --capability IO or the host API)"
                        .to_string(),
                ));
            }
            if !args.is_empty() {
                return Err(LispError::Generic("read takes no arguments".to_string()));
            }
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(|e| LispError::Generic(format!("Failed to read input: {}", e)))?;
            crate::reader::read_with_depth_limit(&line, env, env.reader_depth_limit())
                .map_err(|e| LispError::Generic(format!("Failed to parse input: {}", e)))
        }
        BuiltinFunc::Prin1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "prin1 requires exactly one argument".to_string(),
                ));
            }
            print!("{}", crate::printer::print(&args[0]));
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Princ => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "princ requires exactly one argument".to_string(),
                ));
            }
            let output = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            print!("{}", output);
            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| LispError::Generic(format!("Failed to flush output: {}", e)))?;
            Ok(args[0].clone())
        }
        BuiltinFunc::Terpri => {
            if !args.is_empty() {
                return Err(LispError::Generic("terpri takes no arguments".to_string()));
            }
            println!();
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Spaces => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "spaces takes exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                let n = (*n).max(0) as usize;
                print!("{}", " ".repeat(n));
                use std::io::Write;
                let _ = std::io::stdout().flush();
                Ok(LispVal::Nil)
            } else {
                Err(LispError::Generic(format!(
                    "SPACES: expected a number, got {}",
                    err_val(&args[0])
                )))
            }
        }
        _ => Err(LispError::Generic("Not an I/O operation".to_string())),
    }
}

// Error handling operations
#[inline(never)]
pub(super) fn apply_error_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Error => {
            // (error)                       -> signal a generic error
            // (error existing-error-value)  -> re-signal it unchanged
            // (error message irritants...)  -> signal (make-error message irritants)
            if args.is_empty() {
                return Err(LispError::Signaled(Box::new(LispVal::Error(Shared::new(
                    crate::ErrorObj {
                        message: "Error".to_string(),
                        data: LispVal::Nil,
                    },
                )))));
            }
            if let LispVal::Error(_) = &args[0] {
                return Err(LispError::Signaled(Box::new(args[0].clone())));
            }
            let message = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => crate::printer::print(other),
            };
            // (error message [data]) — mirrors make-error: an optional single
            // data payload (a cons or any value), defaulting to NIL.
            let data = args.get(1).cloned().unwrap_or(LispVal::Nil);
            Err(LispError::Signaled(Box::new(LispVal::Error(Shared::new(
                crate::ErrorObj { message, data },
            )))))
        }
        BuiltinFunc::Errorset => {
            if args.len() != 1 && args.len() != 2 {
                return Err(LispError::Generic(
                    "errorset requires one or two arguments".to_string(),
                ));
            }
            let form = &args[0];
            match eval(form, env) {
                Ok(result) => Ok(vec_to_list(vec![result])),
                // Trap ordinary errors and signalled conditions only; let
                // non-local control flow (RETURN/GO/THROW/RETURN-FROM) pass
                // through unchanged.
                Err(LispError::Generic(_)) | Err(LispError::Signaled(_)) => Ok(LispVal::Nil),
                Err(other) => Err(other),
            }
        }
        _ => Err(LispError::Generic(
            "Not an error handling operation".to_string(),
        )),
    }
}

// List processing operations
#[inline(never)]
pub(super) fn apply_list_processing(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Subst => {
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "subst requires exactly three arguments".to_string(),
                ));
            }
            let new_val = &args[0];
            let old_val = &args[1];
            let tree = &args[2];
            fn subst_helper(new: &LispVal, old: &LispVal, tree: &LispVal) -> LispVal {
                if tree == old {
                    new.clone()
                } else if let LispVal::Cons { car, cdr } = tree {
                    LispVal::Cons {
                        car: Shared::new(subst_helper(new, old, car)),
                        cdr: Shared::new(subst_helper(new, old, cdr)),
                    }
                } else {
                    tree.clone()
                }
            }
            Ok(subst_helper(new_val, old_val, tree))
        }
        BuiltinFunc::Sublis => {
            // SUBLIS: Perform multiple substitutions using an association list
            // (SUBLIS alist tree)
            // Returns tree with all atoms that appear as keys in alist replaced with their values
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "sublis requires exactly two arguments".to_string(),
                ));
            }
            let alist = &args[0];
            let tree = &args[1];

            // Helper function to look up a value in the alist
            fn lookup_in_alist(key: &LispVal, alist: &LispVal) -> Option<LispVal> {
                let mut current = alist;
                while let LispVal::Cons { car, cdr } = current {
                    if let LispVal::Cons {
                        car: pair_key,
                        cdr: pair_val,
                    } = &**car
                        && **pair_key == *key
                    {
                        return Some(pair_val.as_ref().clone());
                    }
                    current = cdr;
                }
                None
            }

            // Recursive substitution helper
            fn sublis_helper(alist: &LispVal, tree: &LispVal) -> LispVal {
                match tree {
                    LispVal::Cons { car, cdr } => {
                        // Recursively process both car and cdr
                        LispVal::Cons {
                            car: Shared::new(sublis_helper(alist, car)),
                            cdr: Shared::new(sublis_helper(alist, cdr)),
                        }
                    }
                    _ => {
                        // For atoms, try to find replacement in alist
                        lookup_in_alist(tree, alist).unwrap_or_else(|| tree.clone())
                    }
                }
            }

            Ok(sublis_helper(alist, tree))
        }
        BuiltinFunc::Assoc => {
            // ASSOC: Search an association list for a key
            // (ASSOC key alist)
            // Returns the first pair (key . value) where the car equals key.
            // NOTE: Malformed alist elements (non-cons) are skipped with a warning.
            // This is intentional to allow graceful degradation with imperfect data.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "assoc requires exactly two arguments".to_string(),
                ));
            }
            let key = &args[0];
            let mut alist = &args[1];
            loop {
                match alist {
                    LispVal::Nil => return Ok(LispVal::Nil),
                    LispVal::Cons { car, cdr } => {
                        if let LispVal::Cons {
                            car: pair_car,
                            cdr: _,
                        } = &**car
                        {
                            if **pair_car == *key {
                                return Ok(car.as_ref().clone());
                            }
                        } else {
                            // Warn about malformed alist element
                            eprintln!("Warning: ASSOC skipping non-cons alist element: {:?}", car);
                        }
                        alist = cdr;
                    }
                    other => {
                        return Err(LispError::Generic(format!(
                            "ASSOC: expected a proper association list, got tail {}",
                            err_val(other)
                        )));
                    }
                }
            }
        }
        BuiltinFunc::Maplist => {
            // Arg order: (maplist fn list) — function first, matching Common Lisp
            // and the rest of the functional toolkit (differs from the Lisp 1.5
            // manual's maplist[x;fn]; alignment is intentional).
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "maplist requires exactly two arguments".to_string(),
                ));
            }
            let func = &args[0];
            let list = &args[1];
            let mut result = Vec::new();
            let mut current = list.clone();
            loop {
                match &current {
                    LispVal::Nil => break,
                    LispVal::Cons { car: _, cdr } => {
                        let applied = apply(func, &[current.clone()], env)?;
                        result.push(applied);
                        current = cdr.as_ref().clone();
                    }
                    other => {
                        return Err(LispError::Generic(format!(
                            "MAPLIST: expected a proper list, got tail {}",
                            err_val(other)
                        )));
                    }
                }
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Mapcar => {
            // Arg order: (mapcar fn list) — function first, matching Common Lisp
            // (and the rest of the functional toolkit). Note this differs from
            // the Lisp 1.5 manual's mapcar[x;fn]; the alignment is intentional.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mapcar requires exactly two arguments".to_string(),
                ));
            }
            let func = &args[0];
            let list = &args[1];
            let mut result = Vec::new();
            let mut current = list;
            loop {
                match current {
                    LispVal::Nil => break,
                    LispVal::Cons { car, cdr } => {
                        let applied = apply(func, &[car.as_ref().clone()], env)?;
                        result.push(applied);
                        current = cdr;
                    }
                    other => {
                        return Err(LispError::Generic(format!(
                            "MAPCAR: expected a proper list, got tail {}",
                            err_val(other)
                        )));
                    }
                }
            }
            Ok(vec_to_list(result))
        }
        BuiltinFunc::Rplaca => {
            // RPLACA: Replace the CAR of a cons cell
            // (RPLACA cons new-car)
            // IMPORTANT: This implementation returns a NEW cons cell rather than
            // modifying the original. This is a FUNCTIONAL approach that prevents
            // circular list creation, avoiding potential infinite loops in list
            // traversal operations. Circular lists are therefore NOT possible in
            // this implementation, which is an intentional safety feature.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplaca requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car: _, cdr } = &args[0] {
                Ok(LispVal::Cons {
                    car: Shared::new(args[1].clone()),
                    cdr: cdr.clone(),
                })
            } else {
                Err(LispError::Generic(format!(
                    "RPLACA: expected a cons cell as its first argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::Rplacd => {
            // RPLACD: Replace the CDR of a cons cell
            // (RPLACD cons new-cdr)
            // IMPORTANT: This implementation returns a NEW cons cell rather than
            // modifying the original. This is a FUNCTIONAL approach that prevents
            // circular list creation, avoiding potential infinite loops in list
            // traversal operations. Circular lists are therefore NOT possible in
            // this implementation, which is an intentional safety feature.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "rplacd requires exactly two arguments".to_string(),
                ));
            }
            if let LispVal::Cons { car, cdr: _ } = &args[0] {
                Ok(LispVal::Cons {
                    car: car.clone(),
                    cdr: Shared::new(args[1].clone()),
                })
            } else {
                Err(LispError::Generic(format!(
                    "RPLACD: expected a cons cell as its first argument, got {}",
                    err_val(&args[0])
                )))
            }
        }
        _ => Err(LispError::Generic(
            "Not a list processing operation".to_string(),
        )),
    }
}

// Bitwise operations
#[inline(never)]
pub(super) fn apply_bitwise_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Logor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result |= n;
                } else {
                    return Err(LispError::Generic(format!(
                        "LOGOR: expected integer arguments, got {}",
                        err_val(arg)
                    )));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logand => {
            if args.is_empty() {
                return Ok(LispVal::Number(-1));
            }
            let mut result = if let LispVal::Number(n) = &args[0] {
                *n
            } else {
                return Err(LispError::Generic(format!(
                    "LOGAND: expected integer arguments, got {}",
                    err_val(&args[0])
                )));
            };
            for arg in &args[1..] {
                if let LispVal::Number(n) = arg {
                    result &= n;
                } else {
                    return Err(LispError::Generic(format!(
                        "LOGAND: expected integer arguments, got {}",
                        err_val(arg)
                    )));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Logxor => {
            let mut result = 0i64;
            for arg in args {
                if let LispVal::Number(n) = arg {
                    result ^= n;
                } else {
                    return Err(LispError::Generic(format!(
                        "LOGXOR: expected integer arguments, got {}",
                        err_val(arg)
                    )));
                }
            }
            Ok(LispVal::Number(result))
        }
        BuiltinFunc::Leftshift => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "leftshift requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(n), LispVal::Number(shift)) = (&args[0], &args[1]) {
                // Validate shift amount to avoid overflow panic
                if *shift >= 64 || *shift <= -64 {
                    env.set_flag("OVERFLOW");
                    // Return 0 or -1 depending on sign for extreme shifts
                    if *shift >= 64 {
                        Ok(LispVal::Number(0))
                    } else {
                        // Right shift by >= 64 is effectively sign extension
                        Ok(LispVal::Number(if *n < 0 { -1 } else { 0 }))
                    }
                } else if *shift < 0 {
                    Ok(LispVal::Number(n >> (-shift)))
                } else {
                    // shift is in [0, 63]; wrapping_shl never panics here
                    Ok(LispVal::Number(n.wrapping_shl(*shift as u32)))
                }
            } else {
                Err(LispError::Generic(format!(
                    "LEFTSHIFT: expected integer arguments, got {} and {}",
                    err_val(&args[0]),
                    err_val(&args[1])
                )))
            }
        }
        _ => Err(LispError::Generic("Not a bitwise operation".to_string())),
    }
}

// New list operations
#[inline(never)]
pub(super) fn apply_new_list_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    _env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::List => Ok(vec_to_list(args.to_vec())),
        BuiltinFunc::Last => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "last requires exactly one argument".to_string(),
                ));
            }
            let mut current = &args[0];
            while let LispVal::Cons { car: _, cdr } = current {
                if **cdr == LispVal::Nil {
                    return Ok(current.clone());
                }
                current = cdr;
            }
            Ok(LispVal::Nil)
        }
        BuiltinFunc::Nth => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nth requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(format!(
                    "NTH: expected a number as first argument, got {}",
                    err_val(&args[0])
                )));
            };
            let mut current = &args[1];
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = cdr;
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            if let LispVal::Cons { car, cdr: _ } = current {
                Ok(car.as_ref().clone())
            } else {
                Ok(LispVal::Nil)
            }
        }
        BuiltinFunc::Nthcdr => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "nthcdr requires exactly two arguments".to_string(),
                ));
            }
            let n = if let LispVal::Number(n) = &args[0] {
                *n as usize
            } else {
                return Err(LispError::Generic(format!(
                    "NTHCDR: expected a number as first argument, got {}",
                    err_val(&args[0])
                )));
            };
            let mut current = args[1].clone();
            for _ in 0..n {
                if let LispVal::Cons { car: _, cdr } = current {
                    current = cdr.as_ref().clone();
                } else {
                    return Ok(LispVal::Nil);
                }
            }
            Ok(current)
        }
        BuiltinFunc::Efface => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "efface requires exactly two arguments".to_string(),
                ));
            }
            let item = &args[0];
            let list = &args[1];

            // Build a new list without the first occurrence of item
            let items = list_to_vec(list)?;
            let mut found = false;
            let result: Vec<LispVal> = items
                .into_iter()
                .filter(|x| {
                    if !found && x == item {
                        found = true;
                        false
                    } else {
                        true
                    }
                })
                .collect();
            Ok(vec_to_list(result))
        }
        _ => Err(LispError::Generic("Not a list operation".to_string())),
    }
}

// New numeric operations
#[inline(never)]
pub(super) fn apply_new_numeric_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Mod => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "mod requires exactly two arguments".to_string(),
                ));
            }
            if let (LispVal::Number(x), LispVal::Number(y)) = (&args[0], &args[1]) {
                if *y == 0 {
                    return Err(LispError::Generic("Division by zero".to_string()));
                }
                // Use checked_rem_euclid to handle i64::MIN % -1 (overflow)
                Ok(LispVal::Number(x.checked_rem_euclid(*y).unwrap_or(0)))
            } else {
                Err(LispError::Generic(format!(
                    "MOD: expected integer arguments, got {} and {}",
                    err_val(&args[0]),
                    err_val(&args[1])
                )))
            }
        }
        BuiltinFunc::Plusp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "plusp requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => {
                    if *n > 0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                LispVal::Float(f) => {
                    if *f > 0.0 {
                        Ok(LispVal::Symbol(env.intern_symbol("T")))
                    } else {
                        Ok(LispVal::Nil)
                    }
                }
                _ => Err(LispError::Generic(format!(
                    "PLUSP: expected a number, got {}",
                    err_val(&args[0])
                ))),
            }
        }
        BuiltinFunc::Evenp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "evenp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 == 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(format!(
                    "EVENP: expected an integer, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::Oddp => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "oddp requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if n % 2 != 0 {
                    Ok(LispVal::Symbol(env.intern_symbol("T")))
                } else {
                    Ok(LispVal::Nil)
                }
            } else {
                Err(LispError::Generic(format!(
                    "ODDP: expected an integer, got {}",
                    err_val(&args[0])
                )))
            }
        }
        BuiltinFunc::Add1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "add1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n + 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f + 1.0)),
                _ => Err(LispError::Generic(format!(
                    "ADD1: expected a number, got {}",
                    err_val(&args[0])
                ))),
            }
        }
        BuiltinFunc::Sub1 => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "sub1 requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Number(n) => Ok(LispVal::Number(n - 1)),
                LispVal::Float(f) => Ok(LispVal::Float(f - 1.0)),
                _ => Err(LispError::Generic(format!(
                    "SUB1: expected a number, got {}",
                    err_val(&args[0])
                ))),
            }
        }
        BuiltinFunc::Random => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "random requires exactly one argument".to_string(),
                ));
            }
            if let LispVal::Number(n) = &args[0] {
                if *n <= 0 {
                    return Err(LispError::Generic(format!(
                        "RANDOM: expected a positive integer, got {}",
                        err_val(&args[0])
                    )));
                }
                // Simple linear congruential generator using system time as seed
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                let random_val = (seed % (*n as u64)) as i64;
                Ok(LispVal::Number(random_val))
            } else {
                Err(LispError::Generic(format!(
                    "RANDOM: expected an integer, got {}",
                    err_val(&args[0])
                )))
            }
        }
        _ => Err(LispError::Generic("Not a numeric operation".to_string())),
    }
}

// Type predicate operations
#[inline(never)]
pub(super) fn apply_type_predicates(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 1 {
        return Err(LispError::Generic(
            "Type predicate requires exactly one argument".to_string(),
        ));
    }
    let arg = &args[0];
    let result = match op {
        BuiltinFunc::Symbolp => matches!(arg, LispVal::Symbol(_)),
        BuiltinFunc::Boundp => {
            if let LispVal::Symbol(s) = arg {
                env.is_bound(&s.borrow().name)
            } else {
                return Err(LispError::Generic(format!(
                    "BOUNDP: expected a symbol, got {}",
                    err_val(arg)
                )));
            }
        }
        BuiltinFunc::Functionp => matches!(
            arg,
            LispVal::Lambda(_)
                | LispVal::Builtin(_)
                | LispVal::Fexpr(_)
                | LispVal::Native(_)
                | LispVal::Vau(_)
        ),
        BuiltinFunc::Macrop => matches!(arg, LispVal::Macro(_)),
        BuiltinFunc::Arrayp => matches!(arg, LispVal::Array(_)),
        BuiltinFunc::Extensionp => matches!(arg, LispVal::Extension(_)),
        _ => return Err(LispError::Generic("Not a type predicate".to_string())),
    };
    if result {
        Ok(LispVal::Symbol(env.intern_symbol("T")))
    } else {
        Ok(LispVal::Nil)
    }
}

// Function operations
#[inline(never)]
pub(super) fn apply_function_ops(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    match op {
        BuiltinFunc::Funcall => {
            if args.is_empty() {
                return Err(LispError::Generic(
                    "funcall requires at least one argument".to_string(),
                ));
            }
            // If the first arg is a symbol, look it up to get the function
            let func = match &args[0] {
                LispVal::Symbol(s) => env.get(&s.borrow().name).ok_or_else(|| {
                    LispError::Generic(format!("Function not found: {}", s.borrow().name))
                })?,
                other => other.clone(),
            };
            let func_args = &args[1..];
            apply(&func, func_args, env)
        }
        BuiltinFunc::Macroexpand => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "macroexpand requires exactly one argument".to_string(),
                ));
            }
            let form = &args[0];
            if let LispVal::Cons { car, cdr } = form
                && let LispVal::Symbol(s) = &**car
                && let Some(LispVal::Macro(m)) = env.get(&s.borrow().name)
            {
                let macro_args = list_to_vec(cdr)?;
                return expand_macro(&m, &macro_args, env);
            }
            // Not a macro call, return as-is
            Ok(form.clone())
        }
        _ => Err(LispError::Generic("Not a function operation".to_string())),
    }
}
