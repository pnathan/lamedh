use super::*;
// ---------------------------------------------------------------------------
// Elaboration = lowering + monomorphic type checking, in one pass.
// ---------------------------------------------------------------------------

pub(super) type Scope = Vec<(String, Ty)>;

/// Elaboration context: read-only access to the signatures of all registered
/// functions, so call sites can be type-checked (and self/forward references
/// resolved) while a body is being elaborated.
pub(super) struct Cx<'a> {
    pub(super) funcs: &'a [Rc<TypedFn>],
    pub(super) by_name: &'a HashMap<String, usize>,
    pub(super) structs: &'a HashMap<String, Rc<StructDef>>,
    /// The inference state for this definition: fresh variables + substitution
    /// (issue #135). Held behind a `RefCell` so the elaboration methods keep
    /// their `&self` signatures while still threading one shared substitution.
    pub(super) infer: RefCell<Infer>,
    /// Checker mode (#162): when set, elaboration types the *full* checkable
    /// lattice (lists, pairs, symbols, strings) and degrades an unknown/untyped
    /// call to `Any` (the gradual frontier) instead of rejecting it. The produced
    /// [`Core`] is *not* used (the checker never compiles), only the types are.
    /// When unset (the codegen path), behavior is unchanged: every type must be
    /// compileable and unknown calls are errors.
    pub(super) checking: bool,
}

impl Cx<'_> {
    /// A fresh type variable from this definition's inference state.
    fn fresh(&self) -> Ty {
        self.infer.borrow_mut().fresh()
    }
    /// Unify two types, extending the substitution (or report the clash).
    pub(super) fn unify(&self, a: &Ty, b: &Ty) -> Result<(), String> {
        self.infer.borrow_mut().unify(a, b)
    }
    /// Read a type's current representative under the substitution (for
    /// diagnostics; may still be a variable).
    pub(super) fn walk(&self, t: &Ty) -> Ty {
        self.infer.borrow().walk(t)
    }
    /// Resolve a type to a concrete type, erroring if it is still ambiguous.
    pub(super) fn resolve(&self, t: &Ty) -> Result<Ty, String> {
        self.infer.borrow().resolve(t)
    }

    fn elab(
        &self,
        form: &LispVal,
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        match form {
            LispVal::Number(n) => Ok((Core::LitI(*n), Ty::Int64)),
            LispVal::Float(f) => Ok((Core::LitF(*f), Ty::Float64)),
            LispVal::Symbol(s) => {
                let name = &s.borrow().name;
                if name == "TRUE" {
                    return Ok((Core::LitI(1), Ty::Bool));
                }
                if name == "FALSE" {
                    return Ok((Core::LitI(0), Ty::Bool));
                }
                match scope.iter().rposition(|(n, _)| n == name) {
                    Some(slot) => Ok((Core::Var(slot), scope[slot].1.clone())),
                    // A free symbol in checker mode is some global we don't track
                    // — the gradual frontier (`Any`). The codegen path rejects it.
                    None if self.checking => Ok((Core::LitI(0), Ty::Any)),
                    None => Err(format!("unbound variable: {name}")),
                }
            }
            // Checker-only literals (#162): a boxed string is `string`, `nil`/`()`
            // is an empty list of some element type. The codegen path rejects them.
            LispVal::String(_) if self.checking => Ok((Core::LitI(0), Ty::Str)),
            LispVal::Nil if self.checking => Ok((Core::LitI(0), Ty::List(Box::new(self.fresh())))),
            LispVal::Cons { .. } => {
                let items = list_to_vec(form);
                let head = match items.first() {
                    Some(LispVal::Symbol(s)) => s.borrow().name.clone(),
                    _ => return Err("typed core: call head must be a symbol".to_string()),
                };
                let args = &items[1..];
                match head.as_str() {
                    "+" | "-" | "*" | "/" | "MOD" => self.elab_bin(&head, args, scope, max),
                    "<" | ">" | "<=" | ">=" | "=" | "/=" => self.elab_cmp(&head, args, scope, max),
                    "NOT" => self.elab_not(args, scope, max),
                    "AND" | "OR" => self.elab_logic(&head, args, scope, max),
                    "IF" => self.elab_if(args, scope, max),
                    "LET-TYPED" => self.elab_let(args, scope, max),
                    "CHAR-CODE" => self.elab_char_code(args, scope, max),
                    "CODE-CHAR" => self.elab_code_char(args, scope, max),
                    "ARRAY" | "MAKE-ARRAY" => self.elab_array_new(args, scope, max),
                    "FETCH" | "AREF" => self.elab_fetch(args, scope, max),
                    "STORE" | "ASET" => self.elab_store(args, scope, max),
                    "ARRAY-LENGTH" => self.elab_array_len(args, scope, max),
                    // Checker-only forms (#162): list/pair processing + the
                    // untyped `let`/`progn`/`quote` that real `defun` bodies use.
                    "CONS" if self.checking => self.elab_cons(args, scope, max),
                    "CAR" | "FIRST" if self.checking => self.elab_car(args, scope, max),
                    "CDR" | "REST" if self.checking => self.elab_cdr(args, scope, max),
                    "LIST" if self.checking => self.elab_list(args, scope, max),
                    "NULL" | "NULL?" | "ENDP" if self.checking => self.elab_null(args, scope, max),
                    "LET" if self.checking => self.elab_let(args, scope, max),
                    "PROGN" if self.checking => self.elab_body(args, scope, max),
                    "QUOTE" if self.checking => self.elab_quote(args),
                    "COND" if self.checking => self.elab_cond(args, scope, max),
                    "WHEN" | "UNLESS" if self.checking => self.elab_when(args, scope, max),
                    _ => self.elab_call(&head, args, scope, max),
                }
            }
            other if self.checking => {
                // Any other literal the checker doesn't model is gradually `Any`.
                let _ = other;
                Ok((Core::LitI(0), Ty::Any))
            }
            other => Err(format!("typed core: unsupported literal {other:?}")),
        }
    }

    fn elab_bin(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if self.unify(&ta, &tb).is_err() {
            return Err(format!(
                "`{op}` operands disagree: {:?} vs {:?}",
                self.walk(&ta),
                self.walk(&tb)
            ));
        }
        // In checker mode the numeric *kind* is irrelevant (no codegen) and the
        // operand type may not be pinned yet (e.g. across `if` branches), so we
        // defer resolution and just propagate the operand type.
        if self.checking {
            return Ok((Core::LitI(0), self.walk(&ta)));
        }
        let rt = self
            .resolve(&ta)
            .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
        let num = rt
            .as_num()
            .ok_or_else(|| format!("`{op}` expects numeric operands, got {rt:?}"))?;
        let bop = match op {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            _ => BinOp::Mod,
        };
        if matches!(bop, BinOp::Mod) && !matches!(num, NumTy::I) {
            return Err("`mod` is int64-only".to_string());
        }
        Ok((Core::Bin(num.into(), bop, Box::new(a), Box::new(b)), rt))
    }

    fn elab_cmp(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        if self.unify(&ta, &tb).is_err() {
            return Err(format!(
                "`{op}` operands disagree: {:?} vs {:?}",
                self.walk(&ta),
                self.walk(&tb)
            ));
        }
        // Checker mode: comparison is `bool` regardless of the operand kind.
        if self.checking {
            return Ok((Core::LitI(0), Ty::Bool));
        }
        let rt = self
            .resolve(&ta)
            .map_err(|_| format!("`{op}`: cannot infer operand type"))?;
        let num = rt
            .cmp_num()
            .ok_or_else(|| format!("`{op}` expects comparable operands, got {rt:?}"))?;
        let cop = match op {
            "<" => CmpOp::Lt,
            ">" => CmpOp::Gt,
            "<=" => CmpOp::Le,
            ">=" => CmpOp::Ge,
            "=" => CmpOp::Eq,
            _ => CmpOp::Ne,
        };
        Ok((
            Core::Cmp(num.into(), cop, Box::new(a), Box::new(b)),
            Ty::Bool,
        ))
    }

    fn elab_not(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`not` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        // In checker mode `not` follows Lisp truthiness (any operand → bool); the
        // codegen path requires a real `bool`.
        if !self.checking && self.unify(&ta, &Ty::Bool).is_err() {
            return Err(format!("`not` expects bool, got {:?}", self.walk(&ta)));
        }
        Ok((Core::Not(Box::new(a)), Ty::Bool))
    }

    fn elab_logic(
        &self,
        op: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`{op}` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (b, tb) = self.elab(&args[1], scope, max)?;
        // Checker mode follows Lisp truthiness: `and`/`or` take any operands and
        // their value is one of the operands (heterogeneous) → `any`. The codegen
        // path requires real `bool` operands and yields `bool`.
        if self.checking {
            return Ok((Core::LitI(0), Ty::Any));
        }
        if self.unify(&ta, &Ty::Bool).is_err() || self.unify(&tb, &Ty::Bool).is_err() {
            return Err(format!(
                "`{op}` expects bool operands, got {:?} and {:?}",
                self.walk(&ta),
                self.walk(&tb)
            ));
        }
        let node = if op == "AND" {
            Core::And(Box::new(a), Box::new(b))
        } else {
            Core::Or(Box::new(a), Box::new(b))
        };
        Ok((node, Ty::Bool))
    }

    fn elab_if(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 3 {
            return Err(format!(
                "`if` expects (if cond then else), got {} args",
                args.len()
            ));
        }
        let (c, tc) = self.elab(&args[0], scope, max)?;
        // Checker mode follows Lisp truthiness: any condition is allowed. The
        // codegen path requires a real `bool`.
        if !self.checking && self.unify(&tc, &Ty::Bool).is_err() {
            return Err(format!(
                "`if` condition must be bool, got {:?}",
                self.walk(&tc)
            ));
        }
        let saved = scope.len();
        let (t, tt) = self.elab(&args[1], scope, max)?;
        scope.truncate(saved);
        let (e, te) = self.elab(&args[2], scope, max)?;
        scope.truncate(saved);
        if self.unify(&tt, &te).is_err() {
            return Err(format!(
                "`if` branches disagree: {:?} vs {:?}",
                self.walk(&tt),
                self.walk(&te)
            ));
        }
        Ok((
            Core::If(Box::new(c), Box::new(t), Box::new(e)),
            self.walk(&tt),
        ))
    }

    fn elab_let(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let bindings = args
            .first()
            .map(list_to_vec)
            .ok_or_else(|| "`let-typed` needs a binding list".to_string())?;
        let body = &args[1..];
        if body.is_empty() {
            return Err("`let-typed` needs a body".to_string());
        }
        let saved = scope.len();
        let mut writes: Vec<(usize, Core)> = Vec::with_capacity(bindings.len());
        for b in &bindings {
            let parts = list_to_vec(b);
            // Two binding shapes: `(name type init)` pins the type explicitly,
            // `(name init)` leaves it to be inferred from the initializer
            // (issue #135 — the one surface-compatible inferable position).
            let (name, declared, init) = match parts.as_slice() {
                [LispVal::Symbol(n), declared_ty, init] => {
                    let mut infer = self.infer.borrow_mut();
                    let ty = parse_ty(declared_ty, &mut infer, self.structs)
                        .map_err(|e| format!("binding `{}`: {e}", n.borrow().name))?;
                    (n.borrow().name.clone(), Some(ty), init)
                }
                [LispVal::Symbol(n), init] => (n.borrow().name.clone(), None, init),
                _ => {
                    return Err(
                        "`let-typed` binding must be (name type init) or (name init)".to_string(),
                    );
                }
            };
            let (init_core, init_ty) = self.elab(init, scope, max)?;
            // An explicit annotation is a principal-type pin: unify it with the
            // inferred initializer type (must agree). Omitting it makes a fresh
            // variable that the initializer constrains — the inference path.
            let ty = match declared {
                Some(d) => {
                    if self.unify(&d, &init_ty).is_err() {
                        return Err(format!(
                            "binding `{name}` declared {d:?} but init is {:?}",
                            self.walk(&init_ty)
                        ));
                    }
                    d
                }
                None => {
                    // A fresh variable constrained by the initializer. It is NOT
                    // resolved here: an array binding's element type is only fixed
                    // by later `store`/`fetch` in the body, so resolution is
                    // deferred (the var flows via `walk` to every reference).
                    let v = self.fresh();
                    self.unify(&v, &init_ty)
                        .expect("fresh variable always unifies");
                    v
                }
            };
            let slot = scope.len();
            scope.push((name, ty));
            *max = (*max).max(scope.len());
            writes.push((slot, init_core));
        }
        let (body_core, body_ty) = self.elab_body(body, scope, max)?;
        scope.truncate(saved);
        let mut acc = body_core;
        for (slot, init_core) in writes.into_iter().rev() {
            acc = Core::Let(slot, Box::new(init_core), Box::new(acc));
        }
        Ok((acc, body_ty))
    }

    fn elab_call(
        &self,
        name: &str,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let id = match self.by_name.get(name) {
            Some(id) => *id,
            // Gradual frontier (#162): an unknown/untyped callee yields `Any`. We
            // still elaborate the arguments so type errors *inside* them surface,
            // but leave them unconstrained (the callee makes no demand). The
            // codegen path keeps rejecting unknown calls.
            None if self.checking => {
                for a in args {
                    self.elab(a, scope, max)?;
                }
                return Ok((Core::LitI(0), Ty::Any));
            }
            None => return Err(format!("call to unknown function `{name}`")),
        };
        let callee = &self.funcs[id];
        let params = callee.params.borrow().clone();
        let ret = callee.ret.borrow().clone();
        if args.len() != params.len() {
            return Err(format!(
                "`{name}` expects {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut arg_cores = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            let (ac, at) = self.elab(a, scope, max)?;
            if self.unify(&at, &params[i].1).is_err() {
                return Err(format!(
                    "`{name}` arg {i} expects {:?}, got {:?}",
                    params[i].1,
                    self.walk(&at)
                ));
            }
            arg_cores.push(ac);
        }
        Ok((Core::Call(id, arg_cores), ret))
    }

    /// `(char-code c)` : char -> int64. Widening: the char word already holds
    /// the byte value, so the core is reused unchanged, only its type changes.
    fn elab_char_code(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`char-code` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if self.unify(&ta, &Ty::Char).is_err() {
            return Err(format!(
                "`char-code` expects char, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((a, Ty::Int64))
    }

    /// `(code-char n)` : int64 -> char. Narrowing: mask the word to a byte.
    fn elab_code_char(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`code-char` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        if self.unify(&ta, &Ty::Int64).is_err() {
            return Err(format!(
                "`code-char` expects int64, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((Core::ToChar(Box::new(a)), Ty::Char))
    }

    /// `(array n)` / `(make-array n)` : int64 -> (array α). The element type is a
    /// fresh variable, unified at each `fetch`/`store` site and resolved before
    /// codegen (#137/#138 — element types are inferred, never annotated here).
    fn elab_array_new(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`array` expects 1 arg (size), got {}", args.len()));
        }
        let (n, tn) = self.elab(&args[0], scope, max)?;
        if self.unify(&tn, &Ty::Int64).is_err() {
            return Err(format!(
                "`array` size must be int64, got {:?}",
                self.walk(&tn)
            ));
        }
        let elem = self.fresh();
        Ok((Core::ArrayNew(Box::new(n)), Ty::Array(Box::new(elem))))
    }

    /// `(fetch a i)` : (array α) int64 -> α. Bounds-checked at runtime.
    fn elab_fetch(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`fetch` expects 2 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (i, ti) = self.elab(&args[1], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem.clone()))).is_err() {
            return Err(format!(
                "`fetch` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        if self.unify(&ti, &Ty::Int64).is_err() {
            return Err(format!(
                "`fetch` index must be int64, got {:?}",
                self.walk(&ti)
            ));
        }
        let rt = self.walk(&elem);
        Ok((Core::ArrayGet(Box::new(a), Box::new(i)), rt))
    }

    /// `(store a i v)` : (array α) int64 α -> α. Evaluates to the stored value.
    fn elab_store(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 3 {
            return Err(format!("`store` expects 3 args, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let (i, ti) = self.elab(&args[1], scope, max)?;
        let (v, tv) = self.elab(&args[2], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem.clone()))).is_err() {
            return Err(format!(
                "`store` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        if self.unify(&ti, &Ty::Int64).is_err() {
            return Err(format!(
                "`store` index must be int64, got {:?}",
                self.walk(&ti)
            ));
        }
        if self.unify(&tv, &elem).is_err() {
            return Err(format!(
                "`store` value type {:?} does not match element type {:?}",
                self.walk(&tv),
                self.walk(&elem)
            ));
        }
        let rt = self.walk(&elem);
        Ok((Core::ArraySet(Box::new(a), Box::new(i), Box::new(v)), rt))
    }

    /// `(array-length a)` : (array α) -> int64.
    fn elab_array_len(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`array-length` expects 1 arg, got {}", args.len()));
        }
        let (a, ta) = self.elab(&args[0], scope, max)?;
        let elem = self.fresh();
        if self.unify(&ta, &Ty::Array(Box::new(elem))).is_err() {
            return Err(format!(
                "`array-length` expects an array, got {:?}",
                self.walk(&ta)
            ));
        }
        Ok((Core::ArrayLen(Box::new(a)), Ty::Int64))
    }

    // --- checker-only list/pair forms (#162) -------------------------------
    // These run only in `checking` mode; the produced `Core` is a placeholder
    // (`LitI(0)`) since the checker never compiles — only the *types* matter.

    /// `(cons x xs)` : α (list α) -> (list α). The list-cons view (lamedh lists
    /// are nested conses); proper homogeneous lists are the useful case to check.
    fn elab_cons(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 2 {
            return Err(format!("`cons` expects 2 args, got {}", args.len()));
        }
        let (_, tx) = self.elab(&args[0], scope, max)?;
        let (_, txs) = self.elab(&args[1], scope, max)?;
        let lst = Ty::List(Box::new(tx));
        if self.unify(&txs, &lst).is_err() {
            return Err(format!(
                "`cons`: tail {:?} is not a list of the head's type",
                self.walk(&txs)
            ));
        }
        Ok((Core::LitI(0), self.walk(&lst)))
    }

    /// `(car xs)` : (list α) -> α.
    fn elab_car(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`car` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        let elem = self.fresh();
        if self.unify(&txs, &Ty::List(Box::new(elem.clone()))).is_err() {
            return Err(format!("`car` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), self.walk(&elem)))
    }

    /// `(cdr xs)` : (list α) -> (list α).
    fn elab_cdr(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`cdr` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        let lst = Ty::List(Box::new(self.fresh()));
        if self.unify(&txs, &lst).is_err() {
            return Err(format!("`cdr` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), self.walk(&lst)))
    }

    /// `(list e0 e1 …)` : all elements unified to α -> (list α). Empty → (list α).
    fn elab_list(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let elem = self.fresh();
        for (i, a) in args.iter().enumerate() {
            let (_, ta) = self.elab(a, scope, max)?;
            if self.unify(&ta, &elem).is_err() {
                return Err(format!(
                    "`list` element {i} has type {:?}, expected {:?}",
                    self.walk(&ta),
                    self.walk(&elem)
                ));
            }
        }
        Ok((Core::LitI(0), Ty::List(Box::new(self.walk(&elem)))))
    }

    /// `(null xs)` : (list α) -> bool.
    fn elab_null(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.len() != 1 {
            return Err(format!("`null` expects 1 arg, got {}", args.len()));
        }
        let (_, txs) = self.elab(&args[0], scope, max)?;
        if self.unify(&txs, &Ty::List(Box::new(self.fresh()))).is_err() {
            return Err(format!("`null` expects a list, got {:?}", self.walk(&txs)));
        }
        Ok((Core::LitI(0), Ty::Bool))
    }

    /// `(cond (test body…) …)` : every clause body unifies to one result type;
    /// tests follow Lisp truthiness (any type). With no clause, `any`.
    fn elab_cond(
        &self,
        clauses: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        let result = self.fresh();
        let mut had_clause = false;
        for clause in clauses {
            let parts = list_to_vec(clause);
            if parts.is_empty() {
                continue;
            }
            let saved = scope.len();
            let (_, test_ty) = self.elab(&parts[0], scope, max)?;
            // A clause with no body yields the test value; otherwise the body.
            let bt = if parts.len() == 1 {
                test_ty
            } else {
                self.elab_body(&parts[1..], scope, max)?.1
            };
            scope.truncate(saved);
            if self.unify(&bt, &result).is_err() {
                return Err(format!(
                    "`cond` clauses disagree: {:?} vs {:?}",
                    self.walk(&bt),
                    self.walk(&result)
                ));
            }
            had_clause = true;
        }
        if had_clause {
            Ok((Core::LitI(0), self.walk(&result)))
        } else {
            Ok((Core::LitI(0), Ty::Any))
        }
    }

    /// `(when test body…)` / `(unless test body…)`: the test follows truthiness
    /// and the body is checked, but the value is the body *or* `nil`
    /// (heterogeneous), so the result is `any`.
    fn elab_when(
        &self,
        args: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if args.is_empty() {
            return Err("`when`/`unless` need a condition".to_string());
        }
        self.elab(&args[0], scope, max)?;
        if args.len() > 1 {
            self.elab_body(&args[1..], scope, max)?;
        }
        Ok((Core::LitI(0), Ty::Any))
    }

    /// `(quote x)`: a quoted symbol is `symbol`, quoted `()` is a list, any other
    /// quoted datum is `any` (the checker does not model quoted structure).
    fn elab_quote(&self, args: &[LispVal]) -> Result<(Core, Ty), String> {
        let ty = match args.first() {
            Some(LispVal::Symbol(_)) => Ty::Symbol,
            Some(LispVal::Nil) => Ty::List(Box::new(self.fresh())),
            _ => Ty::Any,
        };
        Ok((Core::LitI(0), ty))
    }

    pub(super) fn elab_body(
        &self,
        forms: &[LispVal],
        scope: &mut Scope,
        max: &mut usize,
    ) -> Result<(Core, Ty), String> {
        if forms.is_empty() {
            return Err("empty body".to_string());
        }
        let mut cores = Vec::with_capacity(forms.len());
        let mut last_ty = Ty::Int64;
        for f in forms {
            let (c, t) = self.elab(f, scope, max)?;
            cores.push(c);
            last_ty = t;
        }
        // A single form needs no wrapper; multiple forms sequence (earlier ones
        // run for their side effects, e.g. `store`), yielding the last's type.
        let core = if cores.len() == 1 {
            cores.pop().unwrap()
        } else {
            Core::Seq(cores)
        };
        Ok((core, last_ty))
    }
}
