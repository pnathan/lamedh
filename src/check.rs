//! `lamedh check`: static verification of Lisp source **without executing it**.
//!
//! The goal is to close the edit/run loop for both humans and LLMs writing
//! Lamedh: a typo'd function name, a Common-Lisp-ism, or a call with the wrong
//! number of arguments should be reported by reading the source, not by
//! running it and watching it crash. See `docs/check.md` for the diagnostic
//! schema and the conservativeness contract.
//!
//! # The overriding constraint: zero false positives
//!
//! A checker that cries wolf is worse than none — an LLM that learns to
//! distrust the checker will ignore its real findings. Every heuristic here is
//! biased toward **silence when in doubt**. We would rather miss a real problem
//! (false negative) than invent one (false positive). Concretely:
//!
//! * We build a full stdlib [`Environment`] so every builtin, stdlib function,
//!   macro, and operative is a *known* name — but we never evaluate the user's
//!   file.
//! * A first pass collects every top-level definition across **all** checked
//!   files, so cross-file and forward references never look unbound.
//! * We only recurse into forms whose binding/evaluation semantics we know
//!   exactly (a curated whitelist of special forms plus the `defun`/`lambda`
//!   families). For an operator that is a known **macro** or **operative**
//!   (`vau`/fexpr) but not on that whitelist, we do **not** descend — a macro
//!   body can bind or introduce anything, so peering inside risks false
//!   positives. This loses coverage but never invents a finding.
//! * Records and variants generate families of names (`make-X`, `X-p`,
//!   `X-field`, …). We enumerate the documented ones and additionally suppress
//!   any operator sharing a defined record/variant/constructor prefix.
//!
//! Unbound *variable* (non-operator) reporting is deliberately **not**
//! implemented: dynamic variables, forward globals, and macro-introduced
//! bindings make it too false-positive-prone to satisfy the contract. Only
//! operator-position names are checked for boundness.

use std::collections::{HashMap, HashSet};

use crate::environment::Environment;
use crate::{LispVal, Shared, SpecialForm, reader, teaching_errors};

/// Severity of a [`Finding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The file could not be fully parsed. Emitted for reader errors.
    Error,
    /// A lint observation. The file parsed, but a call looks wrong.
    Warning,
}

impl Severity {
    /// The lowercase token used in both human and sexpr output.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// The category of a [`Finding`]. Rendered as the `kind` field in sexpr output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    /// The reader rejected the source (unbalanced parens, bad token, …).
    ParseError,
    /// A symbol in operator position is bound nowhere.
    UnboundFunction,
    /// A call to a statically-known function cannot match its arity.
    ArityMismatch,
    /// The file could not be read from disk.
    FileError,
}

impl FindingKind {
    /// The hyphenated token used in sexpr output.
    pub fn as_str(self) -> &'static str {
        match self {
            FindingKind::ParseError => "parse-error",
            FindingKind::UnboundFunction => "unbound-function",
            FindingKind::ArityMismatch => "arity-mismatch",
            FindingKind::FileError => "file-error",
        }
    }
}

/// A single diagnostic produced by [`check_paths`].
#[derive(Debug, Clone)]
pub struct Finding {
    /// The file the finding refers to (as passed in by the caller).
    pub file: String,
    /// 1-based line of the relevant top-level form (or the parse error).
    pub line: usize,
    /// 1-based column. `0` when a column is not meaningful.
    pub column: usize,
    /// How serious the finding is.
    pub severity: Severity,
    /// What kind of finding it is.
    pub kind: FindingKind,
    /// The offending symbol, when there is one.
    pub symbol: Option<String>,
    /// A human-readable, self-contained explanation.
    pub message: String,
}

impl Finding {
    /// Render as a single human-oriented line:
    /// `file:LINE: severity: message`.
    pub fn to_human(&self) -> String {
        format!(
            "{}:{}: {}: {}",
            self.file,
            self.line,
            self.severity.as_str(),
            self.message
        )
    }

    /// Render as one readable s-expression. Schema (stable):
    /// `((file . "…") (line . N) (column . N) (severity . S) (kind . K)
    ///   (symbol . SYM) (message . "…"))`. `symbol` is `nil` when absent.
    pub fn to_sexpr(&self) -> String {
        let symbol = match &self.symbol {
            Some(s) => sexpr_string(s),
            None => "nil".to_string(),
        };
        format!(
            "((file . {}) (line . {}) (column . {}) (severity . {}) (kind . {}) (symbol . {}) (message . {}))",
            sexpr_string(&self.file),
            self.line,
            self.column,
            self.severity.as_str(),
            self.kind.as_str(),
            symbol,
            sexpr_string(&self.message),
        )
    }
}

/// Quote a Rust string as a Lisp string literal (escaping `"` and `\`).
fn sexpr_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The exit code convention: `0` clean, `1` any warnings, `2` if any file
/// failed to parse or read (a hard error dominates warnings).
pub fn exit_code(findings: &[Finding]) -> i32 {
    let mut code = 0;
    for f in findings {
        match f.severity {
            Severity::Error => return 2,
            Severity::Warning => code = 1,
        }
    }
    code
}

/// Static arity of a function whose parameter list we can read.
///
/// `max == None` means unbounded (a `&rest`/`&key` tail). A call with `n`
/// positional arguments is *impossible* iff `n < min` or (`max == Some(m)` and
/// `n > m`).
#[derive(Debug, Clone, Copy)]
struct FnArity {
    min: usize,
    max: Option<usize>,
}

/// Everything the linter learned in the collection pass, shared read-only
/// across the lint pass.
struct Definitions {
    /// All top-level definition names (functions, macros, variables, records,
    /// variants, protocols, …) across every checked file. Used so a reference
    /// to something defined elsewhere in the corpus never looks unbound.
    names: HashSet<String>,
    /// File-defined **callable functions** with a statically-known arity.
    functions: HashMap<String, FnArity>,
    /// File-defined names we must treat as opaque operators: macros and
    /// operatives (`vau`/fexpr). Calls to these are known but never descended
    /// into, and never arity-checked.
    opaque: HashSet<String>,
    /// Base names of records/variants/constructors, for prefix suppression of
    /// their generated accessors/predicates/validators/lenses.
    record_like: Vec<String>,
}

impl Definitions {
    fn new() -> Self {
        Definitions {
            names: HashSet::new(),
            functions: HashMap::new(),
            opaque: HashSet::new(),
            record_like: Vec::new(),
        }
    }

    /// `true` when `name` matches a generated member of some collected record
    /// or variant family (e.g. `MAKE-POINT`, `POINT-P`, `POINT-X`,
    /// `VALIDATE-POINT`, `POINT->PLIST`, `PLIST->POINT`).
    fn is_record_member(&self, name: &str) -> bool {
        for base in &self.record_like {
            if name == base
                || name.starts_with(&format!("{base}-"))
                || name == format!("MAKE-{base}")
                || name == format!("VALIDATE-{base}")
                || name == format!("PLIST->{base}")
                || name == format!("{base}->PLIST")
            {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Small LispVal helpers
// ---------------------------------------------------------------------------

/// The (uppercased) name of a symbol, or `None` for any other value.
fn sym_name(form: &LispVal) -> Option<String> {
    match form {
        LispVal::Symbol(s) => Some(s.borrow().name.clone()),
        _ => None,
    }
}

/// The special-form tag of an operator symbol, if it has one.
fn special_of(form: &LispVal) -> Option<SpecialForm> {
    match form {
        LispVal::Symbol(s) => s.borrow().special_form,
        _ => None,
    }
}

/// A proper Lisp list as a `Vec`, or `None` for atoms / improper lists.
fn as_vec(form: &LispVal) -> Option<Vec<LispVal>> {
    form.as_list_vec().ok()
}

/// Parse a lambda-list into a [`FnArity`], honouring `&OPTIONAL`, `&KEY`,
/// `&REST`, and `&BODY`. `(name default)` optional entries count as one slot.
fn arity_from_params(params: &[LispVal]) -> FnArity {
    #[derive(PartialEq)]
    enum Mode {
        Required,
        Optional,
        Tail,
    }
    let mut required = 0usize;
    let mut optional = 0usize;
    let mut unbounded = false;
    let mut mode = Mode::Required;
    for p in params {
        if let Some(name) = sym_name(p) {
            match name.as_str() {
                "&OPTIONAL" => mode = Mode::Optional,
                "&KEY" | "&REST" | "&BODY" => {
                    unbounded = true;
                    mode = Mode::Tail;
                }
                _ => match mode {
                    Mode::Required => required += 1,
                    Mode::Optional => optional += 1,
                    Mode::Tail => {}
                },
            }
        } else {
            // A `(name default)` (or destructuring) entry.
            match mode {
                Mode::Required => required += 1,
                Mode::Optional => optional += 1,
                Mode::Tail => {}
            }
        }
    }
    FnArity {
        min: required,
        max: if unbounded {
            None
        } else {
            Some(required + optional)
        },
    }
}

/// Extract just the bound parameter *names* from a lambda-list (skipping the
/// `&`-keywords; taking `car` of `(name default)` / `(name type)` entries).
fn param_names(params: &[LispVal]) -> Vec<String> {
    let mut out = Vec::new();
    for p in params {
        match p {
            LispVal::Symbol(s) => {
                let n = s.borrow().name.clone();
                if !n.starts_with('&') {
                    out.push(n);
                }
            }
            LispVal::Cons { car, .. } => {
                if let Some(n) = sym_name(car) {
                    out.push(n);
                }
            }
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Pass 1: collect definitions
// ---------------------------------------------------------------------------

/// The field name of a record/variant field spec (`FIELD` or `(FIELD type)`).
fn field_name(spec: &LispVal) -> Option<String> {
    match spec {
        LispVal::Symbol(_) => sym_name(spec),
        LispVal::Cons { car, .. } => sym_name(car),
        _ => None,
    }
}

/// `true` for a record section keyword clause like `(:invariant …)`.
fn is_record_section(part: &LispVal) -> bool {
    if let LispVal::Cons { car, .. } = part
        && let Some(head) = sym_name(car)
    {
        return head.starts_with(':');
    }
    false
}

/// Collect the names one top-level `form` introduces into `defs`.
fn collect_defs(form: &LispVal, defs: &mut Definitions) {
    let Some(items) = as_vec(form) else { return };
    if items.is_empty() {
        return;
    }
    let Some(op) = sym_name(&items[0]) else {
        return;
    };

    // Helper: the "defined name" second element, taking car of a list head.
    let head_name = |v: &LispVal| -> Option<String> {
        match v {
            LispVal::Symbol(_) => sym_name(v),
            LispVal::Cons { car, .. } => sym_name(car),
            _ => None,
        }
    };

    match op.as_str() {
        // Function-like definitions with a statically-knowable arity.
        "DEFUN" | "DEFUN*" | "DEFUN-TYPED" => {
            if items.len() >= 3
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name.clone());
                if let Some(params) = as_vec(&items[2]) {
                    defs.functions.insert(name, arity_from_params(&params));
                }
            }
        }
        // Operator-introducing definitions we must never descend into.
        "DEFMACRO" | "DEFEXPR" | "DEFVAU" | "DEFINE-SYNTAX" => {
            if items.len() >= 2
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name.clone());
                defs.opaque.insert(name);
            }
        }
        // Plain value / variable bindings.
        "DEF" | "DEFINE" | "DEFDYNAMIC" | "DEFVAR" | "DEFPARAMETER" | "DEFCONSTANT" | "DEFLAW"
        | "DEFTEST" | "DEFPROTOCOL" | "DEFINSTANCE" | "DEFMODULE" | "DEFRULE" | "EXAMPLE" => {
            if items.len() >= 2
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name);
            }
        }
        // Records: make-N, N-p, validate-N, N-field per field, plus N itself.
        "DEFRECORD" => {
            if items.len() >= 2
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name.clone());
                defs.names.insert(format!("MAKE-{name}"));
                defs.names.insert(format!("{name}-P"));
                defs.names.insert(format!("VALIDATE-{name}"));
                for part in &items[2..] {
                    if is_record_section(part) {
                        continue;
                    }
                    if let Some(f) = field_name(part) {
                        defs.names.insert(format!("{name}-{f}"));
                    }
                }
                defs.record_like.push(name);
            }
        }
        // Variants: the union name plus every bare constructor and its family.
        "DEFVARIANT" => {
            if items.len() >= 2
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name.clone());
                defs.names.insert(format!("{name}-P"));
                defs.record_like.push(name);
                for spec in &items[2..] {
                    let Some(ctor) = head_name(spec) else {
                        continue;
                    };
                    defs.names.insert(ctor.clone());
                    defs.names.insert(format!("{ctor}-P"));
                    if let Some(fields) = as_vec(spec) {
                        for f in &fields[1.min(fields.len())..] {
                            if let Some(fname) = field_name(f) {
                                defs.names.insert(format!("{ctor}-{fname}"));
                            }
                        }
                    }
                    defs.record_like.push(ctor);
                }
            }
        }
        // A literal top-level (progn …) can wrap several definitions.
        "PROGN" => {
            for sub in &items[1..] {
                collect_defs(sub, defs);
            }
        }
        // Catch-all: any other `def…`-shaped head with a symbol name.
        other if other.starts_with("DEF") => {
            if items.len() >= 2
                && let Some(name) = head_name(&items[1])
            {
                defs.names.insert(name);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Pass 2: lint
// ---------------------------------------------------------------------------

/// Operators we treat as no-op declarations: never a call, never descended.
/// `DECLARE` is stripped by the `defun` macro rather than evaluated.
fn is_ignored_operator(name: &str) -> bool {
    matches!(name, "DECLARE" | "DECLAIM" | "THE")
}

/// Known "transparent" macros that evaluate all of their arguments as ordinary
/// expressions and bind nothing. Descending into these is always sound.
fn is_transparent_macro(name: &str) -> bool {
    matches!(name, "WHEN" | "UNLESS")
}

struct Linter<'a> {
    env: &'a Shared<Environment>,
    defs: &'a Definitions,
    /// Candidate names for did-you-mean suggestions (stdlib-bound + defined).
    suggest_pool: Vec<String>,
    file: &'a str,
    findings: Vec<Finding>,
}

impl<'a> Linter<'a> {
    fn push(&mut self, line: usize, kind: FindingKind, symbol: Option<String>, message: String) {
        self.findings.push(Finding {
            file: self.file.to_string(),
            line,
            column: 0,
            severity: Severity::Warning,
            kind,
            symbol,
            message,
        });
    }

    /// Is `name` a known operator (bound, special, defined, or a record member)?
    fn operator_known(&self, name: &str, locals: &HashSet<String>) -> bool {
        locals.contains(name)
            || name == "T"
            || name == "NIL"
            || name.starts_with(':')
            || self.env.is_bound(name)
            || self.defs.names.contains(name)
            || self.defs.is_record_member(name)
    }

    /// Walk one top-level form (`line` anchors every finding it produces).
    fn walk_toplevel(&mut self, form: &LispVal, line: usize) {
        let locals = HashSet::new();
        self.walk_expr(form, &locals, line);
    }

    /// Walk an expression in evaluated position.
    fn walk_expr(&mut self, form: &LispVal, locals: &HashSet<String>, line: usize) {
        let LispVal::Cons { .. } = form else {
            // Atoms (symbols, numbers, strings, …) in argument position are not
            // checked — see the module docs on unbound-variable suppression.
            return;
        };
        let Some(items) = as_vec(form) else {
            return; // improper/dotted list: stay silent.
        };
        if items.is_empty() {
            return; // `()` == nil
        }

        // Operator is not a symbol: e.g. `((lambda …) …)`. Descend everything.
        let Some(op) = sym_name(&items[0]) else {
            for it in &items {
                self.walk_expr(it, locals, line);
            }
            return;
        };

        if is_ignored_operator(&op) {
            return;
        }

        // Definition/binding forms handled by name (some are macros, not
        // special forms, so they must be recognised here explicitly).
        match op.as_str() {
            "DEFUN" => {
                self.walk_defun_like(&items, locals, line, 2);
                return;
            }
            "DEFMACRO" | "DEFEXPR" | "DEFVAU" | "DEFINE-SYNTAX" => {
                // Macro/operative body: never descend (arbitrary bindings).
                return;
            }
            _ if is_transparent_macro(&op) => {
                for it in &items[1..] {
                    self.walk_expr(it, locals, line);
                }
                return;
            }
            _ => {}
        }

        // Special forms with binding semantics we know exactly.
        if let Some(sf) = special_of(&items[0]) {
            self.walk_special(sf, &items, locals, line);
            return;
        }

        // Otherwise this is an operator application (or a macro/operative call).
        // Decide by what `op` denotes.
        let known_function = self.classify_operator(&op, locals);
        match known_function {
            OpClass::Descend(arity) => {
                if let Some(ar) = arity {
                    self.check_arity(&op, ar, items.len() - 1, line);
                }
                for it in &items[1..] {
                    self.walk_expr(it, locals, line);
                }
            }
            OpClass::Opaque => {
                // Known macro/operative not on our whitelist: do not descend.
            }
            OpClass::Unbound => {
                let suffix =
                    teaching_errors::teaching_suffix(&op, self.suggest_pool.iter().cloned());
                self.push(
                    line,
                    FindingKind::UnboundFunction,
                    Some(op.clone()),
                    format!("unbound function {op}{suffix}"),
                );
                // Best-effort: still descend the arguments as expressions.
                for it in &items[1..] {
                    self.walk_expr(it, locals, line);
                }
            }
        }
    }

    /// Classify an operator symbol that is neither an ignored declaration nor a
    /// recognised special/definition form.
    fn classify_operator(&self, op: &str, locals: &HashSet<String>) -> OpClass {
        if locals.contains(op) {
            // A locally-bound value used as an operator: a call, unknown arity.
            return OpClass::Descend(None);
        }
        if op == "T" || op == "NIL" || op.starts_with(':') {
            return OpClass::Descend(None);
        }
        // File-defined names take precedence over stdlib (redefinition).
        if let Some(ar) = self.defs.functions.get(op) {
            return OpClass::Descend(Some(*ar));
        }
        if self.defs.opaque.contains(op) {
            return OpClass::Opaque;
        }
        if self.defs.names.contains(op) || self.defs.is_record_member(op) {
            // Defined by some other form (def/defrecord/defvariant/…): known,
            // treat as a call (constructors/accessors are functions).
            return OpClass::Descend(None);
        }
        match self.env.get(op) {
            Some(LispVal::Macro(_)) | Some(LispVal::Vau(_)) | Some(LispVal::Fexpr(_)) => {
                OpClass::Opaque
            }
            Some(LispVal::Lambda(bx)) => {
                let arity = FnArity {
                    min: bx.params.len(),
                    max: if bx.rest_param.is_some() {
                        None
                    } else {
                        Some(bx.params.len())
                    },
                };
                OpClass::Descend(Some(arity))
            }
            Some(_) => OpClass::Descend(None), // builtin/native/value: no arity
            None => {
                if self.operator_known(op, locals) {
                    OpClass::Descend(None)
                } else {
                    OpClass::Unbound
                }
            }
        }
    }

    fn check_arity(&mut self, op: &str, arity: FnArity, nargs: usize, line: usize) {
        let too_few = nargs < arity.min;
        let too_many = arity.max.is_some_and(|m| nargs > m);
        if !too_few && !too_many {
            return;
        }
        let expectation = match arity.max {
            Some(m) if m == arity.min => format!("exactly {}", arity.min),
            Some(m) => format!("between {} and {}", arity.min, m),
            None => format!("at least {}", arity.min),
        };
        self.push(
            line,
            FindingKind::ArityMismatch,
            Some(op.to_string()),
            format!(
                "{op} expects {expectation} argument{}, but is called with {nargs}",
                if arity.min == 1 && arity.max == Some(1) {
                    ""
                } else {
                    "s"
                }
            ),
        );
    }

    /// `(defun NAME (params) [decls/docstring] body…)` and friends: bind the
    /// parameters, then descend the body. `params_idx` is the index of the
    /// parameter list within `items`.
    fn walk_defun_like(
        &mut self,
        items: &[LispVal],
        locals: &HashSet<String>,
        line: usize,
        params_idx: usize,
    ) {
        if items.len() <= params_idx {
            return;
        }
        let mut inner = locals.clone();
        if let Some(params) = as_vec(&items[params_idx]) {
            for name in param_names(&params) {
                inner.insert(name);
            }
        }
        for form in &items[params_idx + 1..] {
            self.walk_expr(form, &inner, line);
        }
    }

    /// Dispatch a special form we understand. Anything not matched here is
    /// intentionally skipped (no descent) to preserve zero false positives.
    fn walk_special(
        &mut self,
        sf: SpecialForm,
        items: &[LispVal],
        locals: &HashSet<String>,
        line: usize,
    ) {
        match sf {
            // Pure sequencing / conditional: all operands are expressions.
            SpecialForm::If
            | SpecialForm::And
            | SpecialForm::Or
            | SpecialForm::Progn
            | SpecialForm::UnwindProtect
            | SpecialForm::Catch
            | SpecialForm::Throw
            | SpecialForm::Return
            | SpecialForm::While
            | SpecialForm::Setq => {
                for it in &items[1..] {
                    self.walk_expr(it, locals, line);
                }
            }
            // `(cond (test body…) …)`: every clause element is an expression.
            SpecialForm::Cond => {
                for clause in &items[1..] {
                    if let Some(parts) = as_vec(clause) {
                        for p in &parts {
                            self.walk_expr(p, locals, line);
                        }
                    }
                }
            }
            // `(block NAME body…)` / `(return-from NAME val)`: skip the label.
            SpecialForm::Block | SpecialForm::ReturnFrom => {
                for it in &items[2..] {
                    self.walk_expr(it, locals, line);
                }
            }
            // `(def NAME value)` / `(defdynamic NAME value)`: skip the name.
            SpecialForm::Def | SpecialForm::Defdynamic => {
                for it in &items[2..] {
                    self.walk_expr(it, locals, line);
                }
            }
            // `(lambda (params) body…)`.
            SpecialForm::Lambda => {
                if items.len() >= 2 {
                    let mut inner = locals.clone();
                    if let Some(params) = as_vec(&items[1]) {
                        // Descend `&optional (x default)` default exprs too.
                        for p in &params {
                            if let LispVal::Cons { cdr, .. } = p {
                                for d in as_vec(cdr).unwrap_or_default() {
                                    self.walk_expr(&d, &inner, line);
                                }
                            }
                        }
                        for name in param_names(&params) {
                            inner.insert(name);
                        }
                    }
                    for it in &items[2..] {
                        self.walk_expr(it, &inner, line);
                    }
                }
            }
            // Typed function definitions: like defun, params then body.
            SpecialForm::DefunStar | SpecialForm::DefunTyped => {
                self.walk_defun_like(items, locals, line, 2);
            }
            // `(let ((n v) …) body…)` — values in outer scope.
            SpecialForm::Let => {
                let mut inner = locals.clone();
                if items.len() >= 2
                    && let Some(bindings) = as_vec(&items[1])
                {
                    for b in &bindings {
                        match b {
                            LispVal::Symbol(s) => {
                                inner.insert(s.borrow().name.clone());
                            }
                            LispVal::Cons { car, cdr } => {
                                if let Some(v) = as_vec(cdr).and_then(|v| v.into_iter().next()) {
                                    self.walk_expr(&v, locals, line);
                                }
                                if let Some(n) = sym_name(car) {
                                    inner.insert(n);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                for it in items.iter().skip(2) {
                    self.walk_expr(it, &inner, line);
                }
            }
            // `(let* ((n v) …) body…)` — values in sequential scope.
            SpecialForm::LetStar => {
                let mut inner = locals.clone();
                if items.len() >= 2
                    && let Some(bindings) = as_vec(&items[1])
                {
                    for b in &bindings {
                        match b {
                            LispVal::Symbol(s) => {
                                inner.insert(s.borrow().name.clone());
                            }
                            LispVal::Cons { car, cdr } => {
                                if let Some(v) = as_vec(cdr).and_then(|v| v.into_iter().next()) {
                                    self.walk_expr(&v, &inner, line);
                                }
                                if let Some(n) = sym_name(car) {
                                    inner.insert(n);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                for it in items.iter().skip(2) {
                    self.walk_expr(it, &inner, line);
                }
            }
            // `(prog (vars) body…)` — vars bound to nil; body statements/tags.
            SpecialForm::Prog => {
                let mut inner = locals.clone();
                if items.len() >= 2 {
                    for v in as_vec(&items[1]).unwrap_or_default() {
                        if let Some(n) = sym_name(&v) {
                            inner.insert(n);
                        }
                    }
                }
                for it in items.iter().skip(2) {
                    self.walk_expr(it, &inner, line);
                }
            }
            // `(for (var start end [step]) body…)`.
            SpecialForm::For => {
                if items.len() >= 2 {
                    let mut inner = locals.clone();
                    if let Some(spec) = as_vec(&items[1]) {
                        for e in spec.iter().skip(1) {
                            self.walk_expr(e, locals, line);
                        }
                        if let Some(v) = spec.first()
                            && let Some(n) = sym_name(v)
                        {
                            inner.insert(n);
                        }
                    }
                    for it in &items[2..] {
                        self.walk_expr(it, &inner, line);
                    }
                }
            }
            // `(label NAME lambda)`.
            SpecialForm::Label => {
                let mut inner = locals.clone();
                if items.len() >= 2
                    && let Some(n) = sym_name(&items[1])
                {
                    inner.insert(n);
                }
                for it in &items[2..] {
                    self.walk_expr(it, &inner, line);
                }
            }
            // `(handler-case expr (type (var) body…) …)`.
            SpecialForm::HandlerCase => {
                if items.len() >= 2 {
                    self.walk_expr(&items[1], locals, line);
                }
                for clause in items.iter().skip(2) {
                    let Some(parts) = as_vec(clause) else {
                        continue;
                    };
                    let mut inner = locals.clone();
                    if parts.len() >= 2 {
                        for v in as_vec(&parts[1]).unwrap_or_default() {
                            if let Some(n) = sym_name(&v) {
                                inner.insert(n);
                            }
                        }
                    }
                    for b in parts.iter().skip(2) {
                        self.walk_expr(b, &inner, line);
                    }
                }
            }
            // Everything else (quote, quasiquote, function, defmacro, defexpr,
            // macro, fexpr, vau, jit-optimize, check-type, with-fuel, …) is
            // skipped: its operands are not plain expressions, or its body can
            // bind arbitrary names. Silence beats a false positive.
            _ => {}
        }
    }
}

/// What an operator symbol denotes for the purposes of descent.
enum OpClass {
    /// A function application; descend arguments. Arity known when `Some`.
    Descend(Option<FnArity>),
    /// A known macro/operative not on our whitelist; do not descend.
    Opaque,
    /// Bound nowhere; emit an unbound-function finding.
    Unbound,
}

// ---------------------------------------------------------------------------
// Reading & driving
// ---------------------------------------------------------------------------

/// Read every top-level form from `src`, tagging each with its 1-based start
/// line. Returns the forms read so far plus, if the reader failed, a single
/// parse finding (matching the file loader, which stops at the first error).
fn read_forms(
    env: &Shared<Environment>,
    file: &str,
    src: &str,
) -> (Vec<(LispVal, usize)>, Option<Finding>) {
    let stripped = reader::strip_shebang(src);
    let base_shift = src.len() - stripped.len();
    let mut forms = Vec::new();
    let mut current = stripped;
    loop {
        current = reader::skip_ws(current);
        let form_offset = stripped.len() - current.len();
        match reader::read_next(current, env) {
            Ok(None) => return (forms, None),
            Ok(Some((val, rest))) => {
                let (line, _col) = reader::position_of(src, base_shift + form_offset);
                forms.push((val, line));
                current = rest;
            }
            Err((offset, detail)) => {
                let absolute = reader::error_anchor(stripped, form_offset, offset, &detail);
                let (line, col) = reader::position_of(src, base_shift + absolute);
                let finding = Finding {
                    file: file.to_string(),
                    line,
                    column: col,
                    severity: Severity::Error,
                    kind: FindingKind::ParseError,
                    symbol: None,
                    message: format!("parse error: {detail}"),
                };
                return (forms, Some(finding));
            }
        }
    }
}

/// Check already-loaded source buffers. `sources` pairs each file label with
/// its contents. This is the API the tests drive; [`check_paths`] wraps it with
/// filesystem reads. A fresh stdlib environment is built once and shared.
pub fn check_sources(sources: &[(String, String)]) -> Vec<Finding> {
    let env = Environment::with_stdlib_fresh();
    check_sources_in(&env, sources)
}

/// Like [`check_sources`] but against a caller-provided environment (used by
/// tests that want to reuse one stdlib world across many invocations).
pub fn check_sources_in(env: &Shared<Environment>, sources: &[(String, String)]) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Read every file first, collecting parse findings and parsed forms.
    let mut parsed: Vec<(String, Vec<(LispVal, usize)>)> = Vec::new();
    for (file, src) in sources {
        let (forms, parse_err) = read_forms(env, file, src);
        if let Some(f) = parse_err {
            findings.push(f);
        }
        parsed.push((file.clone(), forms));
    }

    // Pass 1: collect all definitions across all files.
    let mut defs = Definitions::new();
    for (_file, forms) in &parsed {
        for (form, _line) in forms {
            collect_defs(form, &mut defs);
        }
    }

    // The did-you-mean candidate pool: stdlib-bound names plus every collected
    // definition (so suggestions can point at file-local helpers too).
    let mut suggest_pool = env.bound_symbol_names();
    suggest_pool.extend(defs.names.iter().cloned());

    // Pass 2: lint each file.
    for (file, forms) in &parsed {
        let mut linter = Linter {
            env,
            defs: &defs,
            suggest_pool: suggest_pool.clone(),
            file,
            findings: Vec::new(),
        };
        for (form, line) in forms {
            linter.walk_toplevel(form, *line);
        }
        findings.extend(linter.findings);
    }

    findings
}

/// Check each path on disk. Files that cannot be read produce a `FileError`
/// finding. Directories are **not** expanded — pass explicit file paths.
pub fn check_paths(paths: &[String]) -> Vec<Finding> {
    let mut sources = Vec::new();
    let mut findings = Vec::new();
    for path in paths {
        match std::fs::read_to_string(path) {
            Ok(src) => sources.push((path.clone(), src)),
            Err(e) => findings.push(Finding {
                file: path.clone(),
                line: 0,
                column: 0,
                severity: Severity::Error,
                kind: FindingKind::FileError,
                symbol: None,
                message: format!("cannot read file: {e}"),
            }),
        }
    }
    findings.extend(check_sources(&sources));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_one(src: &str) -> Vec<Finding> {
        check_sources(&[("test.lisp".to_string(), src.to_string())])
    }

    #[test]
    fn clean_file_has_no_findings() {
        let f = check_one("(defun sq (x) (* x x))\n(sq 3)\n");
        assert!(f.is_empty(), "expected clean, got {f:?}");
    }

    #[test]
    fn misspelled_function_is_flagged_with_suggestion() {
        // LENGT is close to stdlib LENGTH.
        let f = check_one("(defun f (x) (lengt x))\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].kind, FindingKind::UnboundFunction);
        assert_eq!(f[0].symbol.as_deref(), Some("LENGT"));
        assert!(f[0].message.contains("did you mean"), "{}", f[0].message);
        assert!(f[0].message.contains("LENGTH"), "{}", f[0].message);
    }

    #[test]
    fn cl_ism_loop_gets_guidance() {
        let f = check_one("(defun f (x) (loop for i in x collect i))\n");
        // LOOP is flagged as unbound with CL-ism guidance.
        let loop_finding = f.iter().find(|f| f.symbol.as_deref() == Some("LOOP"));
        let lf = loop_finding.expect("LOOP should be flagged");
        assert!(lf.message.contains("Common Lisp"), "{}", lf.message);
    }

    #[test]
    fn arity_too_few_is_flagged() {
        let f = check_one("(defun add (a b) (+ a b))\n(add 1)\n");
        let a = f
            .iter()
            .find(|f| f.kind == FindingKind::ArityMismatch)
            .expect("arity finding");
        assert_eq!(a.symbol.as_deref(), Some("ADD"));
        assert!(a.message.contains("exactly 2"), "{}", a.message);
    }

    #[test]
    fn arity_too_many_is_flagged() {
        let f = check_one("(defun add (a b) (+ a b))\n(add 1 2 3)\n");
        assert!(f.iter().any(|f| f.kind == FindingKind::ArityMismatch));
    }

    #[test]
    fn rest_param_suppresses_upper_arity() {
        let f = check_one("(defun v (a &rest r) a)\n(v 1 2 3 4)\n");
        assert!(
            !f.iter().any(|f| f.kind == FindingKind::ArityMismatch),
            "{f:?}"
        );
    }

    #[test]
    fn parse_error_reports_line() {
        let f = check_one("(defun f (x)\n  (+ x 1)\n(foo\n");
        let pe = f
            .iter()
            .find(|f| f.kind == FindingKind::ParseError)
            .expect("parse error");
        assert_eq!(pe.severity, Severity::Error);
        assert!(pe.line >= 1);
    }

    #[test]
    fn local_bindings_are_respected() {
        // A let-bound operator must not be flagged unbound.
        let f = check_one("(defun f (g) (let ((h g)) (h 1)))\n");
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn forward_reference_across_file_is_ok() {
        // `helper` is defined after its use.
        let f = check_one("(defun a (x) (helper x))\n(defun helper (y) y)\n");
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn quote_body_is_not_walked() {
        let f = check_one("(defun f () '(nonexistent-fn 1 2))\n");
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn macro_call_body_is_not_descended() {
        // `when` is transparent, but an unknown macro's body would be skipped.
        // Here we ensure `when` still finds a genuine typo (transparent).
        let f = check_one("(defun f (x) (when x (lengt x)))\n");
        assert!(f.iter().any(|f| f.symbol.as_deref() == Some("LENGT")));
    }

    #[test]
    fn record_accessors_are_known() {
        let src = "(defrecord point (x int64) (y int64))\n\
                   (defun mag (p) (+ (point-x p) (point-y p)))\n\
                   (defun mk () (make-point 1 2))\n";
        let f = check_one(src);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn sexpr_output_is_wellformed() {
        let f = check_one("(defun f (x) (lengt x))\n");
        let s = f[0].to_sexpr();
        assert!(s.starts_with("((file . \"test.lisp\")"), "{s}");
        assert!(s.contains("(kind . unbound-function)"), "{s}");
        assert!(s.contains("(symbol . \"LENGT\")"), "{s}");
    }

    #[test]
    fn exit_codes() {
        assert_eq!(exit_code(&[]), 0);
        let warn = Finding {
            file: "x".into(),
            line: 1,
            column: 0,
            severity: Severity::Warning,
            kind: FindingKind::UnboundFunction,
            symbol: None,
            message: String::new(),
        };
        assert_eq!(exit_code(std::slice::from_ref(&warn)), 1);
        let err = Finding {
            severity: Severity::Error,
            kind: FindingKind::ParseError,
            ..warn.clone()
        };
        assert_eq!(exit_code(&[warn, err]), 2);
    }
}
