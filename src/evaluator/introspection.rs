use super::*;
// Introspection operations: describe, see-source, disassemble.
#[inline(never)]
pub(super) fn apply_introspection(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    use std::io::{self, Write};
    match op {
        BuiltinFunc::Describe => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "describe requires exactly one argument".to_string(),
                ));
            }
            print!("{}", describe_text(&args[0], env));
            let _ = io::stdout().flush();
            Ok(LispVal::Symbol(env.intern_symbol("T")))
        }
        BuiltinFunc::SeeSource => {
            if args.is_empty() || args.len() > 2 {
                return Err(LispError::Generic(
                    "see-source takes one or two arguments".to_string(),
                ));
            }
            let form = see_source_form(&args[0], env)?;
            let as_tree = args.len() == 2 && args[1] != LispVal::Nil;
            if as_tree {
                let mut s = String::new();
                render_form_tree(&form, 0, &mut s);
                print!("{s}");
                let _ = io::stdout().flush();
                Ok(LispVal::Symbol(env.intern_symbol("T")))
            } else {
                Ok(form)
            }
        }
        BuiltinFunc::SeeType => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "see-type requires exactly one argument".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::Symbol(s) => s.borrow().name.clone(),
                other => {
                    return Err(LispError::Generic(format!(
                        "see-type requires a symbol, got {other:?}"
                    )));
                }
            };
            Ok(see_type_form(&name, env))
        }
        BuiltinFunc::ExplainCompile => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "explain-compile requires exactly one argument".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::Symbol(s) => s.borrow().name.clone(),
                other => {
                    return Err(LispError::Generic(format!(
                        "explain-compile requires a symbol, got {other:?}"
                    )));
                }
            };
            Ok(explain_compile_form(&name, env))
        }
        BuiltinFunc::ReadString => {
            // (read-string "text") — parse TEXT and return the list of forms it
            // contains. Pure (no I/O): the inverse of PRINC-TO-STRING for code.
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "read-string requires exactly one argument".to_string(),
                ));
            }
            let text = match &args[0] {
                LispVal::String(s) => s.clone(),
                other => {
                    return Err(LispError::Generic(format!(
                        "read-string requires a string, got {other:?}"
                    )));
                }
            };
            let forms = crate::reader::read_all(&text, env)
                .map_err(|e| LispError::Generic(format!("read-string: {e}")))?;
            Ok(vec_to_list(forms))
        }
        BuiltinFunc::DeclareType => {
            // (declare-type! 'name '(forall (r) (-> (...) ...))) — register a
            // declared scheme (experimental rows) for NAME. The declaration is
            // an axiom the checker will trust at call sites; the caller is
            // responsible for keeping the implementation in lockstep.
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "declare-type! requires a symbol and a type form".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::Symbol(s) => s.borrow().name.clone(),
                other => {
                    return Err(LispError::Generic(format!(
                        "declare-type! requires a symbol, got {other:?}"
                    )));
                }
            };
            match env.jit_declare_scheme(&name, &args[1]) {
                Ok(rendered) => {
                    let form =
                        crate::reader::read(&rendered, env).unwrap_or(LispVal::String(rendered));
                    Ok(form)
                }
                Err(e) => Err(LispError::Generic(format!("declare-type!: {e}"))),
            }
        }
        BuiltinFunc::Disassemble => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "disassemble requires exactly one argument".to_string(),
                ));
            }
            let name = match &args[0] {
                LispVal::Symbol(s) => s.borrow().name.clone(),
                other => {
                    return Err(LispError::Generic(format!(
                        "disassemble requires a symbol, got {other:?}"
                    )));
                }
            };
            match env.jit_disassemble(&name) {
                Some(text) => print!("{text}"),
                None => {
                    println!("{name} has no typed (JIT) edition — nothing to disassemble.");
                    println!(
                        "  (define one with (defun-typed (name ret) ((arg ty)...) body...) to jot it.)"
                    );
                }
            }
            let _ = io::stdout().flush();
            Ok(LispVal::Symbol(env.intern_symbol("T")))
        }
        _ => Err(LispError::Generic(
            "Not an introspection operation".to_string(),
        )),
    }
}

/// Format a `&REST`-aware parameter list as `(p1 p2 &REST r)`.
pub(super) fn arity_string(params: &[String], rest: Option<&String>) -> String {
    let mut parts: Vec<String> = params.to_vec();
    if let Some(r) = rest {
        parts.push("&REST".to_string());
        parts.push(r.clone());
    }
    format!("({})", parts.join(" "))
}

/// Build the human-readable summary printed by `describe`.
///
/// A symbol is described by its current binding; a typed (jotted) function
/// takes precedence over its membrane `Native` binding as the more informative
/// view. Any other value is described directly.
pub(super) fn describe_text(arg: &LispVal, env: &Shared<Environment>) -> String {
    let mut out = String::new();
    if let LispVal::Symbol(s) = arg {
        let name = s.borrow().name.clone();
        // A typed function shadows the membrane Native binding as the richer view.
        if let Some((ptys, ret)) = env.jit_signature(&name) {
            let sig = ptys
                .iter()
                .map(crate::jit::ty_name)
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!("{name} is a typed (JIT) function.\n"));
            out.push_str(&format!(
                "  Signature: ({sig}) -> {}\n",
                crate::jit::ty_name(&ret)
            ));
            let compiled = matches!(env.jit_is_compiled(&name), Some(true));
            out.push_str(&format!(
                "  Compiled:  {}\n",
                if compiled { "yes" } else { "no (interpreted)" }
            ));
            push_docstring(&mut out, s);
            return out;
        }
        match env.get(&name) {
            None => out.push_str(&format!("{name} is unbound.\n")),
            Some(val) => {
                out.push_str(&format!("{name} is {}.\n", describe_kind(&val)));
                push_value_detail(&mut out, &val);
                push_docstring(&mut out, s);
            }
        }
    } else {
        out.push_str(&format!("This is {}.\n", describe_kind(arg)));
        push_value_detail(&mut out, arg);
    }
    out
}

/// A short noun phrase naming what kind of thing `val` is.
pub(super) fn describe_kind(val: &LispVal) -> &'static str {
    match val {
        LispVal::Builtin(_) => "a built-in function",
        LispVal::Lambda(_) => "a lambda (function)",
        LispVal::Fexpr(_) => "a fexpr (unevaluated-argument operative)",
        LispVal::Macro(_) => "a macro",
        LispVal::Vau(_) => "a vau operative",
        LispVal::Native(_) => "a host-native function",
        LispVal::Number(_) => "bound to an integer",
        LispVal::Float(_) => "bound to a float",
        LispVal::Char(_) => "bound to a character",
        LispVal::String(_) => "bound to a string",
        LispVal::Symbol(_) => "bound to a symbol",
        LispVal::Cons { .. } => "bound to a list",
        LispVal::Nil => "bound to NIL (the empty list / false)",
        LispVal::HashTable(_) => "bound to a hash table",
        LispVal::Array(_) => "bound to an array",
        LispVal::Struct(_) => "bound to a typed struct",
        LispVal::Environment(_) => "bound to an environment",
        LispVal::Error(_) => "bound to an error/condition object",
        LispVal::Extension(_) => "bound to a host extension value",
        LispVal::Port(_) => "bound to a port",
        LispVal::NetHandle(_) => "bound to a network handle",
        LispVal::OsChild(_) => "bound to a child-process handle",
        #[cfg(feature = "concurrency")]
        LispVal::Channel(_) => "bound to a channel",
    }
}

/// Append type-specific detail lines (parameters, value, etc.) for `describe`.
pub(super) fn push_value_detail(out: &mut String, val: &LispVal) {
    match val {
        LispVal::Lambda(l) => out.push_str(&format!(
            "  Parameters: {}\n",
            arity_string(&l.params, l.rest_param.as_ref())
        )),
        LispVal::Fexpr(f) => {
            let argname = f.params.first().map(String::as_str).unwrap_or("?");
            out.push_str(&format!("  Unevaluated arg list bound to: {argname}\n"));
        }
        LispVal::Macro(m) => out.push_str(&format!(
            "  Parameters: {}\n",
            arity_string(&m.params, m.rest_param.as_ref())
        )),
        LispVal::Vau(v) => out.push_str(&format!(
            "  Operands: {}, Environment: {}\n",
            v.operands_param, v.env_param
        )),
        LispVal::Error(e) => out.push_str(&format!("  Message: {}\n", e.message)),
        LispVal::Array(a) => out.push_str(&format!("  Length: {}\n", a.borrow().len())),
        LispVal::Struct(s) => out.push_str(&format!(
            "  Type: {}\n  Fields: {}\n",
            s.type_name,
            s.fields.len()
        )),
        // Self-representing scalars/aggregates: show the value itself.
        LispVal::Number(_)
        | LispVal::Float(_)
        | LispVal::Char(_)
        | LispVal::String(_)
        | LispVal::Symbol(_)
        | LispVal::Cons { .. } => {
            out.push_str(&format!("  Value: {}\n", crate::printer::print(val)));
        }
        _ => {}
    }
}

/// Append a `Doc:` line if the symbol carries a `"docstring"` plist entry.
pub(super) fn push_docstring(out: &mut String, s: &Shared<SharedCell<crate::Symbol>>) {
    if let Some(LispVal::String(doc)) = s.borrow().plist.get("docstring") {
        out.push_str(&format!("  Doc: {doc}\n"));
    }
}

/// Reconstruct the source form for `see-source`. A symbol is resolved to its
/// binding first; the binding (or a directly-passed value) must be a
/// user-defined operative (lambda/fexpr/macro/vau).
pub(super) fn see_source_form(
    arg: &LispVal,
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    // defun-typed / defun-typed-opt annotate the symbol's plist with the
    // original defining form; check that first before falling back to closure
    // reconstruction.
    if let LispVal::Symbol(s) = arg {
        if let Some(form) = s.borrow().plist.get("source-form").cloned() {
            return Ok(form);
        }
        let name = s.borrow().name.clone();
        let val = env
            .get(&name)
            .ok_or_else(|| LispError::Generic(format!("see-source: {name} is unbound")))?;
        return reconstruct_source(&val, env).ok_or_else(|| {
            LispError::Generic(
                "see-source: value has no inspectable source (built-in, host-native, typed, \
                 or not a function)"
                    .to_string(),
            )
        });
    }
    reconstruct_source(arg, env).ok_or_else(|| {
        LispError::Generic(
            "see-source: value has no inspectable source (built-in, host-native, typed, \
             or not a function)"
                .to_string(),
        )
    })
}

/// Build a `(p1 p2 &REST r)` parameter list as a `LispVal` of interned symbols.
pub(super) fn param_list_form(
    params: &[String],
    rest: Option<&String>,
    env: &Shared<Environment>,
) -> LispVal {
    let mut syms: Vec<LispVal> = params
        .iter()
        .map(|p| LispVal::Symbol(env.intern_symbol(p)))
        .collect();
    if let Some(r) = rest {
        syms.push(LispVal::Symbol(env.intern_symbol("&REST")));
        syms.push(LispVal::Symbol(env.intern_symbol(r)));
    }
    vec_to_list(syms)
}

/// Splice a closure body into its top-level forms. Multi-form bodies are stored
/// wrapped in `(PROGN ...)`; a single-form body is returned as one element.
pub(super) fn body_forms(body: &LispVal) -> Vec<LispVal> {
    if let LispVal::Cons { car, cdr } = body
        && let LispVal::Symbol(s) = &**car
        && s.borrow().name == "PROGN"
        && let Ok(forms) = list_to_vec(cdr)
    {
        return forms;
    }
    vec![body.clone()]
}

/// Reconstruct an approximate defining form for an operative value. Returns
/// `None` for values with no Lisp-level source (builtins, natives, scalars).
pub(super) fn reconstruct_source(val: &LispVal, env: &Shared<Environment>) -> Option<LispVal> {
    let head = |tag: &str| LispVal::Symbol(env.intern_symbol(tag));
    match val {
        LispVal::Lambda(l) => {
            let mut items = vec![
                head("LAMBDA"),
                param_list_form(&l.params, l.rest_param.as_ref(), env),
            ];
            items.extend(body_forms(&l.body));
            Some(vec_to_list(items))
        }
        LispVal::Fexpr(f) => {
            let mut items = vec![head("FEXPR"), param_list_form(&f.params, None, env)];
            items.extend(body_forms(&f.body));
            Some(vec_to_list(items))
        }
        LispVal::Macro(m) => {
            let mut items = vec![
                head("MACRO"),
                param_list_form(&m.params, m.rest_param.as_ref(), env),
            ];
            items.extend(body_forms(&m.body));
            Some(vec_to_list(items))
        }
        LispVal::Vau(v) => {
            let mut items = vec![
                head("VAU"),
                LispVal::Symbol(env.intern_symbol(&v.operands_param)),
                LispVal::Symbol(env.intern_symbol(&v.env_param)),
            ];
            items.extend(body_forms(&v.body));
            Some(vec_to_list(items))
        }
        _ => None,
    }
}

/// Render `form` as an indented tree of forms. Lists whose elements are all
/// atoms are kept on one line; lists containing sub-lists are expanded.
pub(super) fn render_form_tree(form: &LispVal, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    if let LispVal::Cons { .. } = form {
        match list_to_vec(form) {
            Ok(items) if items.iter().any(|i| matches!(i, LispVal::Cons { .. })) => {
                out.push_str(&format!("{pad}(\n"));
                for it in &items {
                    render_form_tree(it, depth + 1, out);
                }
                out.push_str(&format!("{pad})\n"));
            }
            // Flat list (all atoms) or improper list: print on one line.
            _ => out.push_str(&format!("{pad}{}\n", crate::printer::print(form))),
        }
    } else {
        out.push_str(&format!("{pad}{}\n", crate::printer::print(form)));
    }
}
