use super::*;
// ---------------------------------------------------------------------------
// The function cell.
// ---------------------------------------------------------------------------

/// A typed function: its signature (the ABI), the typed core (reference
/// interpreter), and an optional hot-swappable compiled edition.
pub struct TypedFn {
    pub name: String,
    pub(super) params: RefCell<Vec<(String, Ty)>>,
    pub(super) ret: RefCell<Ty>,
    pub(super) core: RefCell<Option<Core>>,
    pub(super) slots: Cell<usize>,
    pub(super) compiled: RefCell<Option<Compiled>>,
    /// Native (Cranelift) edition. Like `compiled`, a call pins (`Rc`-clones) it,
    /// so a redefinition that swaps it out keeps the old code mapped until
    /// in-flight callers return (the `NativeEdition` owns its `JITModule`).
    #[cfg(feature = "jit")]
    pub(super) native: RefCell<Option<Rc<native::NativeEdition>>>,
    /// Stable heap word holding this function's current native entry pointer (or
    /// `0`). Other compiled functions bake this cell's *address* and load it to
    /// make direct calls; it is updated on (re)compile and cleared on deopt. A
    /// heap `Box` so the address is stable across registry `Vec` growth.
    #[cfg(feature = "jit")]
    pub(super) entry: Box<Cell<usize>>,
    pub(super) generation: Cell<u64>,
}

impl std::fmt::Debug for TypedFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedFn")
            .field("name", &self.name)
            .field("params", &self.params.borrow())
            .field("ret", &self.ret.borrow())
            .field("defined", &self.core.borrow().is_some())
            .field("compiled", &self.compiled.borrow().is_some())
            .field("generation", &self.generation.get())
            .finish()
    }
}

impl TypedFn {
    fn placeholder(name: String, params: Vec<(String, Ty)>, ret: Ty) -> TypedFn {
        let slots = params.len();
        TypedFn {
            name,
            params: RefCell::new(params),
            ret: RefCell::new(ret),
            core: RefCell::new(None),
            slots: Cell::new(slots),
            compiled: RefCell::new(None),
            #[cfg(feature = "jit")]
            native: RefCell::new(None),
            #[cfg(feature = "jit")]
            entry: Box::new(Cell::new(0)),
            generation: Cell::new(0),
        }
    }

    /// Address of this function's native-entry cell (stable; baked into other
    /// functions' compiled code for direct calls).
    #[cfg(feature = "jit")]
    fn entry_cell_addr(&self) -> usize {
        &*self.entry as *const Cell<usize> as usize
    }

    pub fn ret(&self) -> Ty {
        self.ret.borrow().clone()
    }
    pub fn params(&self) -> Vec<(String, Ty)> {
        self.params.borrow().clone()
    }
    pub fn is_compiled(&self) -> bool {
        self.compiled.borrow().is_some()
    }
    pub fn is_defined(&self) -> bool {
        self.core.borrow().is_some()
    }
    pub fn generation(&self) -> u64 {
        self.generation.get()
    }
    /// The number of slots in this function's activation frame (params + lets).
    pub fn n_slots(&self) -> usize {
        self.slots.get()
    }
    /// A clone of this function's typed-core IR, for structural inspection/verification.
    pub fn core_clone(&self) -> Option<Core> {
        self.core.borrow().clone()
    }

    #[cfg_attr(not(feature = "jit"), allow(unused_variables))]
    pub(super) fn compile_now(&self, funcs: &[Rc<TypedFn>]) {
        let c = self.core.borrow();
        if let Some(core) = c.as_ref() {
            *self.compiled.borrow_mut() = Some(compile(core));
            // With the `jit` feature, also build a native edition. If Cranelift
            // codegen fails for any reason, fall back to the closure edition
            // rather than failing the definition. The entry cell is updated so
            // other compiled functions call this one's native code directly.
            #[cfg(feature = "jit")]
            {
                let n_params = self.params.borrow().len();
                let cell_addrs: Vec<usize> = funcs.iter().map(|f| f.entry_cell_addr()).collect();
                match native::compile_native(core, n_params, self.slots.get(), &cell_addrs) {
                    Ok(ed) => {
                        self.entry.set(ed.entry_addr());
                        *self.native.borrow_mut() = Some(Rc::new(ed));
                    }
                    Err(_) => {
                        self.entry.set(0);
                        *self.native.borrow_mut() = None;
                    }
                }
            }
            self.generation.set(self.generation.get() + 1);
        }
    }

    pub(super) fn deoptimize(&self) {
        *self.compiled.borrow_mut() = None;
        #[cfg(feature = "jit")]
        {
            *self.native.borrow_mut() = None;
            self.entry.set(0);
        }
    }

    /// Invoke with already-unboxed words. Builds the callee frame, dispatches to
    /// the compiled edition if present (pinning it for the call), else interprets.
    pub(super) fn invoke(&self, args: &[u64], ctx: &Ctx) -> u64 {
        // Native edition first (pinned for the call so a redefinition can't free
        // the code out from under us). `args` are the parameter words directly;
        // the native function builds its own local frame.
        #[cfg(feature = "jit")]
        {
            let native = self.native.borrow().clone();
            if let Some(ed) = native {
                let ctx_ptr = ctx as *const Ctx as *const core::ffi::c_void;
                return unsafe { ed.call(args, ctx_ptr) };
            }
        }
        let mut env = vec![0u64; self.slots.get()];
        env[..args.len()].copy_from_slice(args);
        let edition = self.compiled.borrow().clone();
        match edition {
            Some(f) => f(&mut env, ctx),
            None => {
                let core = self.core.borrow();
                let core = core.as_ref().unwrap_or_else(|| {
                    panic!(
                        "typed function `{}` called before it was defined",
                        self.name
                    )
                });
                eval_core(core, &mut env, ctx)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The registry / front end.
// ---------------------------------------------------------------------------

/// A registry of typed functions, with the define/declare front end. Functions
/// are addressed by a stable id so calls survive redefinition.
#[derive(Default, Debug)]
pub struct Jit {
    pub(super) funcs: Vec<Rc<TypedFn>>,
    pub(super) by_name: HashMap<String, usize>,
    /// Registered typed struct definitions, by (uppercased) name. A struct name
    /// is usable as a type in `defun-typed` signatures, and its accessor
    /// functions (`make-NAME`, `NAME-FIELD`, `set-NAME-FIELD`) are generated as
    /// ordinary typed functions over the [`Core`] struct ops.
    pub(super) structs: HashMap<String, Rc<StructDef>>,
}

impl Jit {
    pub fn new() -> Jit {
        Jit::default()
    }

    fn intern(&mut self, name: &str, params: Vec<(String, Ty)>, ret: Ty) -> usize {
        if let Some(&id) = self.by_name.get(name) {
            let f = &self.funcs[id];
            *f.params.borrow_mut() = params;
            *f.ret.borrow_mut() = ret;
            id
        } else {
            let id = self.funcs.len();
            self.funcs
                .push(Rc::new(TypedFn::placeholder(name.to_string(), params, ret)));
            self.by_name.insert(name.to_string(), id);
            id
        }
    }

    /// Forward-declare a signature so mutually-recursive functions can reference
    /// each other before their bodies exist.
    pub fn declare(&mut self, name: &str, params: &[(&str, Ty)], ret: Ty) -> usize {
        let params = params
            .iter()
            .map(|(n, t)| ((*n).to_string(), t.clone()))
            .collect();
        self.intern(name, params, ret)
    }

    /// Forward-declare from a `(declare-typed (name ret) ((arg ty)...))` form.
    /// Returns the (uppercased) name.
    pub fn declare_form(&mut self, form: &LispVal) -> Result<String, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DECLARE-TYPED" => {}
            _ => return Err("expected a (declare-typed ...) form".to_string()),
        }
        if items.len() != 3 {
            return Err("declare-typed: (declare-typed (name ret) ((arg ty)...))".to_string());
        }
        // A forward declaration has no body to infer from, so its types must be
        // concrete — reject a bare `array` (unpinned element) here.
        let mut infer = Infer::new();
        let (name, ret, params) = parse_signature(&items, &mut infer, &self.structs)?;
        if infer.resolve(&ret).is_err() || params.iter().any(|(_, t)| infer.resolve(t).is_err()) {
            return Err(format!(
                "{name}: a declaration needs concrete types; pin array elements as `(array T)`"
            ));
        }
        self.intern(&name, params, ret);
        Ok(name)
    }

    /// The (parameter types, return type) of a registered function.
    pub fn signature(&self, name: &str) -> Option<(Vec<Ty>, Ty)> {
        let id = self.id(name)?;
        let f = &self.funcs[id];
        let ptys = f.params.borrow().iter().map(|(_, t)| t.clone()).collect();
        Some((ptys, f.ret.borrow().clone()))
    }

    pub fn named_signature(&self, name: &str) -> Option<(Vec<(String, Ty)>, Ty)> {
        let id = self.id(name)?;
        let f = &self.funcs[id];
        Some((f.params.borrow().clone(), f.ret.borrow().clone()))
    }

    /// Type-check and (eagerly) compile a `(defun-typed ...)` form. Returns the
    /// stable function id.
    pub fn define(&mut self, form: &LispVal) -> Result<usize, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DEFUN-TYPED" => {}
            _ => return Err("expected a (defun-typed ...) form".to_string()),
        }
        if items.len() < 4 {
            return Err("defun-typed: (defun-typed (name ret) ((arg ty)...) body...)".to_string());
        }

        // One inference state spans signature parsing and the body, so an array
        // parameter's element type (a fresh variable from `parse_ty`) is unified
        // by `fetch`/`store` in the body and resolved into the stored signature.
        let mut infer = Infer::new();
        let (name, ret, params) = parse_signature(&items, &mut infer, &self.structs)?;
        let mut scope: Scope = params.clone();

        // Register the signature *before* elaborating the body so a function can
        // call itself (and any already-declared peer).
        let id = self.intern(&name, params.clone(), ret.clone());

        let mut max_slots = scope.len();
        let (core, resolved_params, resolved_ret) = {
            let cx = Cx {
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                infer: RefCell::new(infer),
                checking: false,
            };
            let (core, body_ty) = cx.elab_body(&items[3..], &mut scope, &mut max_slots)?;
            // The declared return type is a principal-type pin: the body's
            // inferred type must unify with it.
            if cx.unify(&body_ty, &ret).is_err() {
                return Err(format!(
                    "{name}: declared return {ret:?} but body has type {:?}",
                    cx.walk(&body_ty)
                ));
            }
            // Final resolve: drive the signature (params + return) to concrete
            // types, baking any inferred array element types. A still-ambiguous
            // type (e.g. an array param the body never indexes) is rejected here.
            let resolved_ret = cx
                .resolve(&ret)
                .map_err(|e| format!("{name} return type: {e}"))?;
            let mut resolved_params = Vec::with_capacity(params.len());
            for (pn, pt) in &params {
                let rt = cx
                    .resolve(pt)
                    .map_err(|e| format!("{name} parameter `{pn}`: {e}"))?;
                resolved_params.push((pn.clone(), rt));
            }
            (core, resolved_params, resolved_ret)
        };

        // Bake the resolved (concrete) signature so the membrane and callers see
        // monomorphic types, never variables.
        let f = self.funcs[id].clone();
        *f.params.borrow_mut() = resolved_params;
        *f.ret.borrow_mut() = resolved_ret;
        f.slots.set(max_slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now(&self.funcs);
        Ok(id)
    }

    /// Install a typed function from a *prebuilt* core (no surface elaboration),
    /// used for generated struct accessors. Eagerly compiles like `define`.
    fn install(
        &mut self,
        name: &str,
        params: Vec<(String, Ty)>,
        ret: Ty,
        core: Core,
        slots: usize,
    ) -> usize {
        let id = self.intern(name, params, ret);
        let f = self.funcs[id].clone();
        f.slots.set(slots);
        *f.core.borrow_mut() = Some(core);
        f.compile_now(&self.funcs);
        id
    }

    /// Attempt to type and compile an **un-annotated** function — HM firing
    /// invisibly under `defun` (#134 stretch goal). Every parameter starts as a
    /// fresh type variable; the body is elaborated under inference and the
    /// parameter + return types are resolved. It succeeds *only* if the whole
    /// body is a fully-inferable typed island (scalars/arrays/structs, arithmetic,
    /// and calls to already-typed functions) and every type resolves to a
    /// concrete monomorphic type. Anything outside the island — an untyped call,
    /// a `cons`, an ambiguous (polymorphic) numeric type — makes it fail, and the
    /// caller keeps the dynamic definition.
    ///
    /// On failure the registry is left exactly as it was (the provisional
    /// self-reference registration is rolled back), so this is a safe, silent,
    /// best-effort optimization.
    pub fn infer_untyped(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<usize, String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new();
        let param_tys: Vec<(String, Ty)> =
            params.iter().map(|p| (p.clone(), infer.fresh())).collect();
        let ret_var = infer.fresh();

        // Provisionally register under the name so a self-recursive call resolves
        // during elaboration. A fresh func id is pushed; `prev` lets us roll the
        // name binding back on failure (the orphaned id is simply never reached).
        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);

        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        // (core, resolved params, resolved return) of a successful inference.
        type Inferred = (Core, Vec<(String, Ty)>, Ty);
        let outcome: Result<Inferred, String> = (|| {
            let cx = Cx {
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                infer: RefCell::new(infer),
                checking: false,
            };
            let (core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch".to_string())?;
            let resolved_ret = cx.resolve(&ret_var)?;
            let mut resolved_params = Vec::with_capacity(param_tys.len());
            for (pn, pt) in &param_tys {
                resolved_params.push((pn.clone(), cx.resolve(pt)?));
            }
            Ok((core, resolved_params, resolved_ret))
        })();

        match outcome {
            Ok((core, resolved_params, resolved_ret)) => {
                let f = self.funcs[new_id].clone();
                *f.params.borrow_mut() = resolved_params;
                *f.ret.borrow_mut() = resolved_ret;
                f.slots.set(max_slots);
                *f.core.borrow_mut() = Some(core);
                f.compile_now(&self.funcs);
                Ok(new_id)
            }
            Err(e) => {
                // Roll back the name binding; the pushed func id is orphaned.
                match prev {
                    Some(p) => {
                        self.by_name.insert(name.to_string(), p);
                    }
                    None => {
                        self.by_name.remove(name);
                    }
                }
                Err(e)
            }
        }
    }

    /// **Type-check** an un-annotated function without compiling it (#162) — the
    /// non-compiled checker. Runs full HM inference (checker mode: lists, pairs,
    /// symbols, strings, and a gradual `Any` at the operative/untyped frontier)
    /// and returns the function's generalized type as a printable scheme, or a
    /// type error. Catches software type errors even when the type is *not*
    /// compileable. Installs nothing and leaves the registry untouched (the
    /// provisional self-reference is rolled back).
    pub fn check_untyped(
        &mut self,
        name: &str,
        params: &[String],
        body: &[LispVal],
    ) -> Result<String, String> {
        if body.is_empty() {
            return Err("empty body".to_string());
        }
        let mut infer = Infer::new();
        let param_tys: Vec<(String, Ty)> =
            params.iter().map(|p| (p.clone(), infer.fresh())).collect();
        let ret_var = infer.fresh();

        // Provisional self-reference so recursive calls type against this function
        // (rolled back unconditionally — the checker never installs anything).
        let new_id = self.funcs.len();
        self.funcs.push(Rc::new(TypedFn::placeholder(
            name.to_string(),
            param_tys.clone(),
            ret_var.clone(),
        )));
        let prev = self.by_name.insert(name.to_string(), new_id);

        let mut scope: Scope = param_tys.clone();
        let mut max_slots = scope.len();
        let outcome: Result<String, String> = (|| {
            let cx = Cx {
                funcs: &self.funcs,
                by_name: &self.by_name,
                structs: &self.structs,
                infer: RefCell::new(infer),
                checking: true,
            };
            let (_core, body_ty) = cx.elab_body(body, &mut scope, &mut max_slots)?;
            cx.unify(&body_ty, &ret_var)
                .map_err(|_| "return type mismatch across branches".to_string())?;
            let inf = cx.infer.borrow();
            let arrow = Ty::Fn(
                param_tys.iter().map(|(_, t)| inf.zonk(t)).collect(),
                Box::new(inf.zonk(&ret_var)),
            );
            Ok(infer::scheme_name(&inf.generalize(&arrow)))
        })();

        // Always roll back — checking is side-effect-free.
        match prev {
            Some(p) => {
                self.by_name.insert(name.to_string(), p);
            }
            None => {
                self.by_name.remove(name);
            }
        }
        outcome
    }

    /// One-pass analysis of an un-annotated function (#162 stage 4): run the
    /// **checker** first (so a genuine type error is reported even when nothing
    /// compiles), then gate native codegen on compileability. The two pipelines
    /// are intentionally separate — the checker is permissive (gradual `Any`,
    /// lists), the compiler is strict (compileable monomorphic types only) — so a
    /// function can be *checked* without being *compiled*.
    pub fn analyze_untyped(&mut self, name: &str, params: &[String], body: &[LispVal]) -> Analysis {
        match self.check_untyped(name, params, body) {
            Err(e) => Analysis::TypeError(e),
            Ok(scheme) => {
                if self.infer_untyped(name, params, body).is_ok() {
                    Analysis::Native(scheme)
                } else {
                    Analysis::Checked(scheme)
                }
            }
        }
    }

    /// Define a typed struct from `(defstruct-typed Name (field type)...)`.
    /// Registers the struct type (usable in `defun-typed` signatures) and
    /// generates its accessor functions over the [`Core`] struct ops:
    /// `make-NAME`, `NAME-FIELD` (getter), `set-NAME-FIELD` (setter). Returns the
    /// generated (uppercased) function names so the caller can install membrane
    /// entries. Fields are laid out as a flat one-word-per-field buffer.
    pub fn define_struct(&mut self, form: &LispVal) -> Result<Vec<String>, String> {
        let items = list_to_vec(form);
        match items.first() {
            Some(LispVal::Symbol(s)) if s.borrow().name == "DEFSTRUCT-TYPED" => {}
            _ => return Err("expected a (defstruct-typed ...) form".to_string()),
        }
        if items.len() < 2 {
            return Err("defstruct-typed: (defstruct-typed Name (field type)...)".to_string());
        }
        let name = match &items[1] {
            LispVal::Symbol(s) => s.borrow().name.clone(),
            _ => return Err("defstruct-typed: struct name must be a symbol".to_string()),
        };
        // Field types must be concrete (a struct's layout is fixed); arrays may
        // pin or be elided, but an elided element here has nothing to infer it.
        let mut infer = Infer::new();
        let mut fields: Vec<(String, Ty)> = Vec::new();
        for f in &items[2..] {
            let parts = list_to_vec(f);
            match parts.as_slice() {
                [LispVal::Symbol(fname), fty] => {
                    let ty = parse_ty(fty, &mut infer, &self.structs)?;
                    let ty = infer.resolve(&ty).map_err(|_| {
                        format!(
                            "struct `{name}` field `{}` needs a concrete type",
                            fname.borrow().name
                        )
                    })?;
                    fields.push((fname.borrow().name.clone(), ty));
                }
                _ => return Err("each field must be (field-name type)".to_string()),
            }
        }
        if fields.is_empty() {
            return Err(format!("struct `{name}` must have at least one field"));
        }

        let def = Rc::new(StructDef {
            name: name.clone(),
            fields: fields.clone(),
        });
        self.structs.insert(name.clone(), def.clone());
        let struct_ty = Ty::Struct(def);

        let mut generated = Vec::new();

        // Constructor: make-NAME : (f0 .. fn) -> NAME.
        let ctor = format!("MAKE-{name}");
        let ctor_core = Core::StructNew((0..fields.len()).map(Core::Var).collect());
        self.install(
            &ctor,
            fields.clone(),
            struct_ty.clone(),
            ctor_core,
            fields.len(),
        );
        generated.push(ctor);

        // Per-field getter NAME-FIELD and setter set-NAME-FIELD.
        for (i, (fname, fty)) in fields.iter().enumerate() {
            let getter = format!("{name}-{fname}");
            self.install(
                &getter,
                vec![("SELF".to_string(), struct_ty.clone())],
                fty.clone(),
                Core::FieldGet(Box::new(Core::Var(0)), i),
                1,
            );
            generated.push(getter);

            let setter = format!("SET-{name}-{fname}");
            self.install(
                &setter,
                vec![
                    ("SELF".to_string(), struct_ty.clone()),
                    ("V".to_string(), fty.clone()),
                ],
                fty.clone(),
                Core::FieldSet(Box::new(Core::Var(0)), i, Box::new(Core::Var(1))),
                2,
            );
            generated.push(setter);
        }
        Ok(generated)
    }

    pub fn id(&self, name: &str) -> Option<usize> {
        // Names are case-normalized to uppercase (matching the reader), so
        // callers may use either case.
        self.by_name.get(&name.to_uppercase()).copied()
    }

    /// The (uppercased) name of the function with the given id.
    pub fn name_of(&self, id: usize) -> Option<String> {
        self.funcs.get(id).map(|f| f.name.clone())
    }
    pub fn get(&self, name: &str) -> Option<&Rc<TypedFn>> {
        self.id(name).map(|i| &self.funcs[i])
    }

    pub(super) fn ctx(&self) -> Ctx<'_> {
        Ctx {
            funcs: &self.funcs,
            arena: RefCell::new(Vec::new()),
        }
    }

    /// Call a function by name with boxed [`Value`]s; type-checks the arguments
    /// against the signature and re-boxes the result. This is the public membrane.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        if !f.is_defined() {
            return Err(format!("{name}: declared but not defined"));
        }
        let params = f.params.borrow();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        // The arena (in `ctx`) must outlive both arg lowering (which allocates
        // compound buffers into it) and result reading (which copies them out),
        // so it is created up front and dropped only at return.
        let ctx = self.ctx();
        let mut words = Vec::with_capacity(args.len());
        for (a, (_, ty)) in args.iter().zip(params.iter()) {
            words.push(a.to_word(ty, &ctx)?);
        }
        let ret = f.ret.borrow().clone();
        drop(params);
        let w = f.invoke(&words, &ctx);
        Ok(Value::from_word(w, &ret))
    }

    /// Convenience for callers holding `LispVal`s: maps `Number`/`Float` to
    /// [`Value`], calls, and re-boxes to `Number`/`Float`/(`Number 0/1` for bool).
    pub fn call_lisp(&self, name: &str, args: &[LispVal]) -> Result<LispVal, String> {
        let (ptys, ret) = self
            .signature(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        if args.len() != ptys.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                ptys.len(),
                args.len()
            ));
        }
        let mut vals = Vec::with_capacity(args.len());
        for (a, ty) in args.iter().zip(ptys.iter()) {
            vals.push(lispval_to_value(a, ty)?);
        }
        Ok(value_to_lispval(&self.call(name, &vals)?, &ret))
    }

    /// Trace a call through the **reference (typed-core) interpreter**, returning
    /// the boxed result alongside a step-by-step [`TraceStep`] log of the callee's
    /// own body (nested calls run normally and are summarised by a single `call`
    /// step). This is the debug stepper/examiner: it lets a correctness suite
    /// inspect *how* a typed function computed its result, and assert invariants
    /// (determinism, result agreement, slot-bound safety) over the trace.
    ///
    /// Independent of whether a compiled edition exists — it always interprets so
    /// the trace reflects the reference semantics that compiled editions must match.
    pub fn trace_call(
        &self,
        name: &str,
        args: &[Value],
    ) -> Result<(Value, Vec<TraceStep>), String> {
        let id = self
            .id(name)
            .ok_or_else(|| format!("unknown function `{name}`"))?;
        let f = &self.funcs[id];
        let core = f
            .core_clone()
            .ok_or_else(|| format!("{name}: declared but not defined"))?;
        let params = f.params.borrow().clone();
        if args.len() != params.len() {
            return Err(format!(
                "{name}: expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let ctx = self.ctx();
        let mut frame = vec![0u64; f.slots.get()];
        for (i, (a, (_, ty))) in args.iter().zip(params.iter()).enumerate() {
            frame[i] = a.to_word(ty, &ctx)?;
        }
        let mut log = Vec::new();
        let w = eval_core_traced(&core, &mut frame, &ctx, 0, &mut log);
        let ret = f.ret.borrow().clone();
        Ok((Value::from_word(w, &ret), log))
    }

    /// Drop every compiled edition (force the interpreter path). Test/diagnostic.
    pub fn deoptimize_all(&self) {
        for f in &self.funcs {
            f.deoptimize();
        }
    }
    /// (Re)compile every defined function.
    pub fn compile_all(&self) {
        for f in &self.funcs {
            f.compile_now(&self.funcs);
        }
    }

    /// Render the typed-core IR of `name` as a flat pseudo-assembly listing.
    ///
    /// This is the "what is actually being run" view for a jotted (typed)
    /// function: the typed core is the reference program every compiled edition
    /// must match, lowered here to a linear, register-and-label instruction
    /// stream. Returns `None` if no typed function by that name is registered.
    pub fn disassemble(&self, name: &str) -> Option<String> {
        let f = self.get(name)?;
        let params = f.params();
        let ret = f.ret();
        let mut s = String::new();
        let sig = params
            .iter()
            .map(|(_, t)| ty_name(t))
            .collect::<Vec<_>>()
            .join(" ");
        s.push_str(&format!(
            "; typed function {} ({sig}) -> {}\n",
            f.name,
            ty_name(&ret)
        ));
        if params.is_empty() {
            s.push_str("; slots: (none)\n");
        } else {
            s.push_str("; slots:\n");
            for (i, (pname, ty)) in params.iter().enumerate() {
                s.push_str(&format!(";   slot{i} = {pname} : {}\n", ty_name(ty)));
            }
        }
        s.push_str(&format!(
            "; compiled edition: {}\n",
            if f.is_compiled() {
                "yes"
            } else {
                "no (interpreted)"
            }
        ));
        match f.core_clone() {
            None => s.push_str("    ; declared but not yet defined\n"),
            Some(core) => {
                let mut out = Vec::new();
                let mut reg = 0usize;
                let mut lab = 0usize;
                self.dis_emit(&core, "rv", &mut out, &mut reg, &mut lab);
                for line in out {
                    s.push_str(&line);
                    s.push('\n');
                }
                s.push_str("    ret rv\n");
            }
        }
        Some(s)
    }

    /// Linearize `core` into instructions writing their result into register
    /// `dst`, appending textual lines to `out`. `reg`/`lab` are monotonic
    /// counters for fresh temporaries and branch labels.
    fn dis_emit(
        &self,
        core: &Core,
        dst: &str,
        out: &mut Vec<String>,
        reg: &mut usize,
        lab: &mut usize,
    ) {
        let fresh = |reg: &mut usize| {
            let r = format!("r{}", *reg);
            *reg += 1;
            r
        };
        let fresh_lab = |lab: &mut usize, base: &str| {
            let l = format!(".{base}{}", *lab);
            *lab += 1;
            l
        };
        match core {
            Core::LitI(n) => out.push(format!("    {dst} = li   {n}")),
            Core::LitF(x) => out.push(format!("    {dst} = lf   {x}")),
            Core::Var(i) => out.push(format!("    {dst} = ld   slot{i}")),
            Core::ToChar(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = and  {dst}, 0xff        ; code-char"));
            }
            Core::Not(a) => {
                self.dis_emit(a, dst, out, reg, lab);
                out.push(format!("    {dst} = not  {dst}"));
            }
            Core::Bin(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", bin_mnemonic(*k, *op)));
            }
            Core::Cmp(k, op, a, b) => {
                let t1 = fresh(reg);
                let t2 = fresh(reg);
                self.dis_emit(a, &t1, out, reg, lab);
                self.dis_emit(b, &t2, out, reg, lab);
                out.push(format!("    {dst} = {} {t1}, {t2}", cmp_mnemonic(*k, *op)));
            }
            Core::And(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "and_end");
                out.push(format!("    brz  {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::Or(a, b) => {
                self.dis_emit(a, dst, out, reg, lab);
                let end = fresh_lab(lab, "or_end");
                out.push(format!("    brnz {dst}, {end}          ; short-circuit"));
                self.dis_emit(b, dst, out, reg, lab);
                out.push(format!("{end}:"));
            }
            Core::If(c, t, e) => {
                let tc = fresh(reg);
                self.dis_emit(c, &tc, out, reg, lab);
                let l_else = fresh_lab(lab, "else");
                let l_end = fresh_lab(lab, "endif");
                out.push(format!("    brz  {tc}, {l_else}"));
                self.dis_emit(t, dst, out, reg, lab);
                out.push(format!("    br   {l_end}"));
                out.push(format!("{l_else}:"));
                self.dis_emit(e, dst, out, reg, lab);
                out.push(format!("{l_end}:"));
            }
            Core::Let(slot, val, body) => {
                let tv = fresh(reg);
                self.dis_emit(val, &tv, out, reg, lab);
                out.push(format!("    st   slot{slot}, {tv}"));
                self.dis_emit(body, dst, out, reg, lab);
            }
            Core::Call(id, args) => {
                let mut argregs = Vec::with_capacity(args.len());
                for a in args {
                    let t = fresh(reg);
                    self.dis_emit(a, &t, out, reg, lab);
                    argregs.push(t);
                }
                let callee = self.name_of(*id).unwrap_or_else(|| format!("fn#{id}"));
                out.push(format!("    {dst} = call {callee}({})", argregs.join(", ")));
            }
            Core::ArrayNew(n) => {
                let t = fresh(reg);
                self.dis_emit(n, &t, out, reg, lab);
                out.push(format!("    {dst} = alloc {t}        ; array"));
            }
            Core::ArrayGet(a, i) => {
                let (ta, ti) = (fresh(reg), fresh(reg));
                self.dis_emit(a, &ta, out, reg, lab);
                self.dis_emit(i, &ti, out, reg, lab);
                out.push(format!(
                    "    {dst} = ldelem {ta}[{ti}]   ; fetch (bounds-checked)"
                ));
            }
            Core::ArraySet(a, i, v) => {
                let (ta, ti, tv) = (fresh(reg), fresh(reg), fresh(reg));
                self.dis_emit(a, &ta, out, reg, lab);
                self.dis_emit(i, &ti, out, reg, lab);
                self.dis_emit(v, &tv, out, reg, lab);
                out.push(format!(
                    "    stelem {ta}[{ti}], {tv}   ; store (bounds-checked)"
                ));
                out.push(format!("    {dst} = mov {tv}"));
            }
            Core::ArrayLen(a) => {
                let t = fresh(reg);
                self.dis_emit(a, &t, out, reg, lab);
                out.push(format!("    {dst} = ldlen {t}"));
            }
            Core::StructNew(inits) => {
                let mut regs = Vec::with_capacity(inits.len());
                for c in inits {
                    let t = fresh(reg);
                    self.dis_emit(c, &t, out, reg, lab);
                    regs.push(t);
                }
                out.push(format!(
                    "    {dst} = struct {{{}}}      ; make struct",
                    regs.join(", ")
                ));
            }
            Core::FieldGet(s, idx) => {
                let t = fresh(reg);
                self.dis_emit(s, &t, out, reg, lab);
                out.push(format!("    {dst} = ldfld {t}.{idx}"));
            }
            Core::FieldSet(s, idx, v) => {
                let (ts, tv) = (fresh(reg), fresh(reg));
                self.dis_emit(s, &ts, out, reg, lab);
                self.dis_emit(v, &tv, out, reg, lab);
                out.push(format!("    stfld {ts}.{idx}, {tv}"));
                out.push(format!("    {dst} = mov {tv}"));
            }
            Core::Seq(forms) => {
                for f in forms {
                    self.dis_emit(f, dst, out, reg, lab);
                }
            }
        }
    }
}
