use super::*;
use smallvec::SmallVec;
/// Default maximum recursion depth (number of nested `eval` frames) before a
/// recoverable error is returned instead of overflowing the native stack.
///
/// The tree-walking interpreter uses large stack frames, so this is calibrated
/// to fit comfortably within the large stack provided by [`crate::with_large_stack`]
/// (~512 MiB). Hosts that run the interpreter on a smaller stack should lower it
/// via [`set_eval_depth_limit`]. See issues #61 (this guard) and #62 (TCO).
pub const DEFAULT_EVAL_DEPTH_LIMIT: usize = 10_000;

thread_local! {
    static EVAL_DEPTH: Cell<usize> = const { Cell::new(0) };
    static EVAL_DEPTH_LIMIT: Cell<usize> = const { Cell::new(DEFAULT_EVAL_DEPTH_LIMIT) };
}

thread_local! {
    /// Kernel fuel (issue #284 Phase 2): a per-thread step budget charged
    /// once per trampoline iteration in `run_trampoline` — i.e. once per
    /// `eval`/`exec` entry *and* once per TCO tail step, in both the
    /// tree-walking and compiled-code worlds. `-1` means unarmed (the
    /// default; one predictable load+branch on the hot path). This is the
    /// step-granular backstop under `lib/22-guard.lisp`'s WITH-FUEL, closing
    /// the Phase-1 leaks (closures defined outside the fence, user macros
    /// expanding to loops, quasiquote bodies): the Lisp fence arms it at
    /// `budget * $guard-kernel-step-multiplier` and restores it on exit.
    static KERNEL_FUEL: Cell<i64> = const { Cell::new(-1) };
}

// ---------------------------------------------------------------------------
// Capability mask (#320): DYNAMIC-EXTENT attenuation. `None` = unmasked
// (host grants only). WITH-CAPABILITIES (a kernel special form) arms the
// mask around its body with RAII restore, and every kernel capability check
// consults it — so attenuation follows the call, not the lexical fence
// body: helpers called from inside a fence are fenced too, and there is no
// Lisp-callable way to widen it.
// ---------------------------------------------------------------------------

thread_local! {
    static CAP_MASK: std::cell::RefCell<Option<Vec<String>>> =
        const { std::cell::RefCell::new(None) };
}

thread_local! {
    /// Nesting depth of active WITH-FUEL fences: KERNEL-FUEL-SET! is
    /// narrow-only while inside one (widening from Lisp would be a fence
    /// escape) but unrestricted at the host/top level.
    static FUEL_FENCE_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Enter/leave a WITH-FUEL fence extent (special-form RAII only).
pub(crate) fn fuel_fence_enter() {
    FUEL_FENCE_DEPTH.with(|d| d.set(d.get() + 1));
}
pub(crate) fn fuel_fence_leave() {
    FUEL_FENCE_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
}
/// Are we inside an active WITH-FUEL fence?
pub(crate) fn fuel_fenced() -> bool {
    FUEL_FENCE_DEPTH.with(|d| d.get() > 0)
}

/// Does the current mask allow capability `name`? Unmasked = yes.
pub fn cap_mask_allows(name: &str) -> bool {
    CAP_MASK.with(|m| match &*m.borrow() {
        None => true,
        Some(mask) => mask.iter().any(|c| c == name),
    })
}

/// Replace the mask, returning the previous state. `pub(crate)`: only the
/// WITH-CAPABILITIES special form (RAII) may call this — deliberately no
/// Lisp-facing setter exists.
pub(crate) fn cap_mask_set(mask: Option<Vec<String>>) -> Option<Vec<String>> {
    CAP_MASK.with(|m| std::mem::replace(&mut *m.borrow_mut(), mask))
}

/// The current mask (for `capabilities-effective` introspection).
pub(crate) fn cap_mask_get() -> Option<Vec<String>> {
    CAP_MASK.with(|m| m.borrow().clone())
}

/// Arm (Some) or disarm (None) the current thread's kernel fuel budget,
/// returning the previous state so callers can restore it on scope exit.
pub fn set_kernel_fuel(fuel: Option<u64>) -> Option<u64> {
    KERNEL_FUEL.with(|f| {
        let prev = f.get();
        f.set(match fuel {
            Some(n) => n.min(i64::MAX as u64) as i64,
            None => -1,
        });
        if prev < 0 { None } else { Some(prev as u64) }
    })
}

/// The current thread's remaining kernel fuel, or `None` when unarmed.
pub fn kernel_fuel_remaining() -> Option<u64> {
    KERNEL_FUEL.with(|f| {
        let v = f.get();
        if v < 0 { None } else { Some(v as u64) }
    })
}

/// Charge one kernel fuel step; error when the armed budget is spent.
/// Exhaustion **disarms** as it signals: the error itself must be
/// catchable, and HANDLER-CASE handlers and UNWIND-PROTECT cleanups (the
/// fence's own budget-restore among them) run through this same trampoline
/// — with the counter stuck at zero they would re-signal forever and the
/// error could never be handled. The WITH-FUEL fence that armed the budget
/// restores/re-arms the enclosing state on exit.
#[inline]
pub(super) fn charge_kernel_fuel() -> Result<(), LispError> {
    KERNEL_FUEL.with(|f| {
        let v = f.get();
        if v < 0 {
            Ok(())
        } else if v == 0 {
            f.set(-1);
            Err(LispError::Generic(
                "fuel exhausted (kernel step budget)".to_string(),
            ))
        } else {
            f.set(v - 1);
            Ok(())
        }
    })
}

// ---------------------------------------------------------------------------
// Backtraces (pay-mostly-on-error). A thread-local stack of NAMED call
// frames: each `run_trampoline` entry is one potential frame slot (its base
// recorded in BT_BASES); a named application in tail position writes the
// callee's name into the current trampoline's slot (`bt_note_tail` — a tail
// call REPLACES the frame, exactly the TCO semantics). On success the
// trampoline truncates back to its base; on error the frames are left in
// place for the catcher (ERRORSET / HANDLER-CASE / the CLI toplevel) to
// snapshot into LAST_BACKTRACE and then truncate. Control-flow unwinds
// (THROW/RETURN/GO) that skip truncation self-heal at the next enclosing
// successful frame, because every exit truncates to its own saved base.
// ---------------------------------------------------------------------------

thread_local! {
    /// Named frames as symbol IDS (resolved to names only at capture).
    static BT_STACK: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Packed control word: high 32 bits = mirror of BT_STACK.len(), low 32
    /// bits = the CURRENT trampoline's frame base. One thread-local access
    /// per operation on the hot path; the RefCell is touched only when a
    /// named frame actually exists.
    static BT_CTL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static BT_LAST: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[inline]
fn ctl_len(v: u64) -> usize {
    (v >> 32) as usize
}
#[inline]
fn ctl_base(v: u64) -> usize {
    (v & 0xffff_ffff) as usize
}
#[inline]
fn ctl_pack(len: usize, base: usize) -> u64 {
    ((len as u64) << 32) | (base as u64 & 0xffff_ffff)
}

/// Current frame-stack depth (a catcher records this at entry).
pub(crate) fn bt_depth() -> usize {
    BT_CTL.with(|c| ctl_len(c.get()))
}

/// Trampoline entry: open a frame slot. Returns the token to pass to
/// [`bt_exit`] (the enclosing trampoline's base).
#[inline]
pub(super) fn bt_enter() -> usize {
    BT_CTL.with(|c| {
        let v = c.get();
        let len = ctl_len(v);
        c.set(ctl_pack(len, len));
        ctl_base(v)
    })
}

/// Trampoline exit with the token from [`bt_enter`]. On success the slot
/// (and anything a control-flow unwind left behind) is discarded; on error
/// the frames stay for the catcher.
#[inline]
pub(super) fn bt_exit(prev_base: usize, ok: bool) {
    BT_CTL.with(|c| {
        let v = c.get();
        let here = ctl_base(v);
        let len = ctl_len(v);
        if ok && len > here {
            BT_STACK.with(|s| s.borrow_mut().truncate(here));
            c.set(ctl_pack(here, prev_base));
        } else {
            c.set(ctl_pack(len, prev_base));
        }
    });
}

/// A named call in tail position: write the callee's symbol id into the
/// current trampoline's frame slot (replacing a previous tail callee — TCO
/// frames collapse, like Scheme traces).
#[inline]
pub(super) fn bt_note_tail(id: u32) {
    BT_CTL.with(|c| {
        let v = c.get();
        let len = ctl_len(v);
        let base = ctl_base(v);
        BT_STACK.with(|s| {
            let mut s = s.borrow_mut();
            if len > base {
                s[len - 1] = id;
            } else {
                s.push(id);
                c.set(ctl_pack(len + 1, base));
            }
        });
    });
}

/// Snapshot the frames above `base` (innermost first) into LAST_BACKTRACE
/// and truncate the stack back to `base`. Called by catchers; `env`
/// resolves the symbol ids back to names.
pub(crate) fn bt_capture(
    base: usize,
    env: &crate::Shared<crate::environment::Environment>,
) -> Vec<String> {
    let names = BT_STACK.with(|s| {
        let mut s = s.borrow_mut();
        let names: Vec<String> = s[base.min(s.len())..]
            .iter()
            .rev()
            .take(64)
            .filter_map(|id| env.symbol_by_id(*id).map(|sym| sym.borrow().name.clone()))
            .collect();
        s.truncate(base);
        names
    });
    BT_CTL.with(|c| {
        let v = c.get();
        c.set(ctl_pack(base.min(ctl_len(v)), ctl_base(v)));
    });
    BT_LAST.with(|l| *l.borrow_mut() = names.clone());
    names
}

/// The most recently captured backtrace (innermost frame first).
pub(crate) fn bt_last() -> Vec<String> {
    BT_LAST.with(|l| l.borrow().clone())
}

/// Set the maximum `eval` recursion depth for the current thread.
pub fn set_eval_depth_limit(limit: usize) {
    EVAL_DEPTH_LIMIT.with(|l| l.set(limit));
}

/// Get the current thread's maximum `eval` recursion depth.
pub fn eval_depth_limit() -> usize {
    EVAL_DEPTH_LIMIT.with(|l| l.get())
}

/// RAII guard that bumps the recursion depth on entry to `eval` and restores it
/// on every exit path (including `?` early returns and caught errors).
pub(super) struct DepthGuard;

impl DepthGuard {
    pub(super) fn enter() -> Result<DepthGuard, LispError> {
        EVAL_DEPTH.with(|depth| {
            let next = depth.get() + 1;
            let limit = EVAL_DEPTH_LIMIT.with(|l| l.get());
            if next > limit {
                Err(LispError::Generic(format!(
                    "recursion limit exceeded ({limit} eval frames); \
                     rewrite iteratively or raise it with set_eval_depth_limit"
                )))
            } else {
                depth.set(next);
                Ok(DepthGuard)
            }
        })
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        EVAL_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

// Helper function to convert a Lisp list (Cons chain) to a Rust Vec.
pub(super) fn list_to_vec(list: &LispVal) -> Result<Vec<LispVal>, LispError> {
    list_to_vec_ctx(list, "")
}

pub(super) fn list_to_vec_ctx(list: &LispVal, context: &str) -> Result<Vec<LispVal>, LispError> {
    let mut vec = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        vec.push(car.as_ref().clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        let ctx = if context.is_empty() {
            format!("; form: {}", err_val(list))
        } else {
            format!(" (in {context}); form: {}", err_val(list))
        };
        return Err(LispError::Generic(format!(
            "not a proper list: dotted pair ending in {}{ctx}",
            err_val(current)
        )));
    }
    Ok(vec)
}

// Helper function to convert a Rust Vec to a Lisp list.
pub(super) fn vec_to_list(vec: Vec<LispVal>) -> LispVal {
    vec.into_iter()
        .rev()
        .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
            car: Shared::new(car),
            cdr: Shared::new(cdr),
        })
}

pub(super) fn proper_list_len(list: &LispVal) -> Result<usize, LispError> {
    let mut len = 0;
    let mut current = list;
    loop {
        match current {
            LispVal::Nil => return Ok(len),
            LispVal::Cons { cdr, .. } => {
                len += 1;
                current = cdr;
            }
            other => {
                return Err(LispError::Generic(format!(
                    "length: argument must be a proper list, got tail {}",
                    err_val(other)
                )));
            }
        }
    }
}

/// Evaluate a call's operands directly off the cons chain.
///
/// This is the hot path for ordinary function/lambda/builtin application. It
/// replaces the older `list_to_vec(rest)` + `.iter().map(eval).collect()` pair,
/// which allocated **two** vectors per call (one of cloned argument *expressions*,
/// one of evaluated results) and cloned every argument expression. Walking the
/// cons cells and evaluating each `car` in place does the same work with a
/// single allocation and no expression clones while still rejecting malformed
/// dotted argument lists.
pub(super) fn eval_operands(
    rest: &LispVal,
    env: &Shared<Environment>,
) -> Result<SmallVec<[LispVal; 4]>, LispError> {
    let mut out = SmallVec::new();
    let mut cur = rest;
    while let LispVal::Cons { car, cdr } = cur {
        out.push(eval(car, env)?);
        cur = cdr;
    }
    if *cur != LispVal::Nil {
        return Err(LispError::Generic(format!(
            "function call arguments must be a proper list, got tail {}",
            err_val(cur)
        )));
    }
    Ok(out)
}

/// Render a value for inclusion in an error message, truncated so a huge
/// list or string cannot flood the output (issue #247).
pub(super) fn err_val(v: &LispVal) -> String {
    let s = crate::printer::print(v);
    if s.chars().count() > 60 {
        let head: String = s.chars().take(57).collect();
        format!("{head}...")
    } else {
        s
    }
}

/// Reject binding/assignment targets that must stay constant.
///
/// `T` is the canonical truth constant and keywords are self-evaluating;
/// rebinding either silently corrupts truth tests far from the mistake
/// (issue #237). `NIL` needs no check here — it reads as the `Nil` value,
/// not a symbol, so it can never reach a binder as a name.
pub(super) fn check_bindable(name: &str, context: &str) -> Result<(), LispError> {
    if name == "T" {
        Err(LispError::Generic(format!(
            "{context}: cannot rebind the constant T"
        )))
    } else if name.starts_with(':') {
        Err(LispError::Generic(format!(
            "{context}: cannot bind the keyword {name}"
        )))
    } else {
        Ok(())
    }
}

/// Reject unsupported lambda-list keywords (`&OPTIONAL`, `&KEY`, ...) so they
/// fail loudly at definition time instead of silently becoming parameters
/// literally named `&OPTIONAL` (issue #242). Callers handle `&REST` before
/// this check, so any `&`-name reaching here is unsupported.
pub(super) fn check_param_name(name: &str, kind: &str) -> Result<(), LispError> {
    if name.starts_with('&') {
        return Err(LispError::Generic(format!(
            "{name} is not supported in {kind} parameter lists (only &REST is)"
        )));
    }
    check_bindable(name, kind)
}

/// Wrap a body of one or more forms into a single evaluable expression.
///
/// A lone form is returned as-is; several forms are wrapped in `(PROGN ...)`
/// so the existing `PROGN` trampoline sequences them and keeps TCO on the last
/// form. Used by the binding special forms (`LET`/`LET*`) to accept multi-form
/// bodies. `forms` is expected to be non-empty; an empty slice yields `(PROGN)`,
/// which evaluates to `NIL`.
pub(super) fn wrap_body_forms(forms: &[LispVal], env: &Shared<Environment>) -> LispVal {
    if forms.len() == 1 {
        forms[0].clone()
    } else {
        let mut list = Vec::with_capacity(forms.len() + 1);
        list.push(LispVal::Symbol(env.intern_symbol("PROGN")));
        list.extend_from_slice(forms);
        vec_to_list(list)
    }
}
