use super::*;
#[inline(never)]
pub(super) fn quasiquote_eval(val: &LispVal, env: &Rc<Environment>) -> Result<LispVal, LispError> {
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
        // A splicing form `,@e` appearing as a list element: evaluate `e` to a
        // list and graft its elements into the surrounding list, ahead of the
        // result of processing the remaining elements (the cdr).
        if let Some(spliced) = unquote_splicing_arg(car) {
            let spliced_list = eval(spliced, env)?;
            let cdr_eval = quasiquote_eval(cdr, env)?;
            return append_lists(&spliced_list, cdr_eval);
        }
        let car_eval = quasiquote_eval(car, env)?;
        let cdr_eval = quasiquote_eval(cdr, env)?;
        Ok(LispVal::Cons {
            car: Rc::new(car_eval),
            cdr: Rc::new(cdr_eval),
        })
    } else {
        Ok(val.clone())
    }
}

/// If `val` is a well-formed `(UNQUOTE-SPLICING e)` form, return a reference to
/// `e`; otherwise return `None` (including ill-arity forms, which then fall
/// through to ordinary template processing).
pub(super) fn unquote_splicing_arg(val: &LispVal) -> Option<&LispVal> {
    if let LispVal::Cons { car, cdr } = val
        && let LispVal::Symbol(s) = &**car
        && s.borrow().name == "UNQUOTE-SPLICING"
        && let LispVal::Cons {
            car: arg,
            cdr: rest,
        } = &**cdr
        && **rest == LispVal::Nil
    {
        return Some(arg);
    }
    None
}

/// Build a fresh cons chain holding every element of the proper list `front`
/// followed by `tail`. The cons cells of `front` are copied so the original is
/// left untouched; a non-list `front` (or an improper tail) yields an error.
pub(super) fn append_lists(front: &LispVal, tail: LispVal) -> Result<LispVal, LispError> {
    match front {
        LispVal::Nil => Ok(tail),
        LispVal::Cons { car, cdr } => {
            let rest = append_lists(cdr, tail)?;
            Ok(LispVal::Cons {
                car: car.clone(),
                cdr: Rc::new(rest),
            })
        }
        _ => Err(LispError::Generic(
            "unquote-splicing requires a list argument".to_string(),
        )),
    }
}
