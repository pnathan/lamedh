//! OS integration: typed Linux/POSIX process environment, time, randomness,
//! process spawning, and signals (issue #260, epic #253).
//!
//! Every `OS-*` primitive here is a thin argument-parsing/capability-check
//! wrapper, exactly like `src/evaluator/builtins_net.rs` is for networking
//! and `src/evaluator/builtins_ports.rs` is for binary ports — the actual
//! `std::env`/`std::process`/`std::time`/`std::fs` I/O and the
//! [`crate::ChildObj`] representation live in `src/lib.rs`. The Lisp-facing
//! names live in `lib/41-os.lisp` (the portable `OS` module) and
//! `lib/42-os-linux.lisp` (the `OS-LINUX` module for typed Linux-specific
//! facilities, per the epic's "portable module + Linux-only module" split).
//!
//! **No raw syscall numbers, no bare file descriptors, no bare PIDs as
//! authority.** A spawned child is an opaque [`crate::LispVal::OsChild`]
//! (compares by identity, closes/reaps deterministically, Drop backstop);
//! its stdio pipes (when requested) are ordinary [`crate::LispVal::Port`]s
//! (issue #255), reusing [`crate::LispVal::wrap_reader`]/
//! [`crate::LispVal::wrap_writer`] rather than adding new `PortState`
//! variants. Signals are sent only by typed name (see
//! [`crate::signal_by_name`]) to either an owned child handle or an
//! explicit PID integer — never via a raw signal number typed by hand from
//! a syscall table, and never via a generic FFI/`(syscall ...)` primitive
//! (see `src/lib.rs`'s `kill(2)` FFI doc comment for why that one hard-coded
//! binding is in scope despite the epic's "no raw syscalls" ruling).
//!
//! **Capability model.** Four ambient authorities, checked via the same
//! `require_*`/`cap_mask_allows` machinery as `READ-FS`/`NET-CONNECT`/etc:
//!
//! - `OS-ENV` — reading process identity/environment: `OS-ARGS*`,
//!   `OS-EXECUTABLE-PATH*`, `OS-CWD*`, `OS-ENV-GET*`, `OS-ENV-LIST*`,
//!   `OS-PID*`, `OS-PPID*`, `OS-HOSTNAME*`.
//! - `OS-ENV-WRITE` — mutating it: `OS-CHDIR*`, `OS-ENV-SET*`,
//!   `OS-ENV-UNSET*`.
//! - `OS-PROCESS` — spawning a child (`OS-SPAWN*`). Once a child handle is
//!   returned, `OS-PROCESS-WAIT*`/`TRY-WAIT*`/`KILL*`/`TERMINATE*`/`ID*`/
//!   `OPEN-P*` need no further capability — continued use of an
//!   already-acquired handle is not gated (the epic's "a successfully
//!   returned handle is authority to continue" rule, same as `PORTS`/`NET`).
//! - `OS-SIGNAL` — sending a signal to a PID *not* held as an owned child
//!   handle (`OS-SIGNAL*`).
//!
//! `OS-LINUX-STAT*`/`OS-LINUX-READLINK*` reuse the existing `READ-FS`
//! capability (filesystem metadata reads), per the ticket's "existing
//! filesystem read/create/temp grants" instruction rather than inventing a
//! parallel one. Time (`OS-NOW*`/`OS-MONOTONIC-NANOS*`/`OS-SLEEP*`) and
//! randomness (`OS-PRNG-STEP*`/`OS-RANDOM-BYTES*`) are ungated: they are
//! pure or read-only-entropy operations with no meaningful confidentiality/
//! integrity impact, mirroring how the pre-existing global `RANDOM`
//! primitive (`src/evaluator/builtins_extra.rs`) is likewise ungated. This
//! is a deliberate scope decision flagged in the PR description, not an
//! oversight.
//!
//! In addition to (not instead of) the capability check, `OS-SPAWN*`/
//! `OS-SIGNAL*` also consult the host policy hook
//! ([`crate::environment::Environment::set_os_policy`]), mirroring the net
//! substrate's `set_net_policy`.
//!
//! **Error data.** Every failure is a structured [`crate::LispVal::Error`]
//! whose `data` is an alist with at least `:OPERATION` (a string) and
//! `:CATEGORY` (a keyword symbol: `:NOT-FOUND`, `:PERMISSION-DENIED`,
//! `:ALREADY-EXISTS`, `:INVALID-ARGUMENT`, `:CLOSED`, `:POLICY-DENIED`,
//! `:UNSUPPORTED-PLATFORM`, `:SIGNAL-FAILED`, or `:OTHER`), plus `:OS-ERROR`
//! (the underlying OS error text) where applicable — see [`os_error`].

use super::*;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Argument helpers ───────────────────────────────────────────────────────

fn expect_string(args: &[LispVal], i: usize, who: &str) -> Result<String, LispError> {
    match args.get(i) {
        Some(LispVal::String(s)) => Ok(s.clone()),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a string, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a string argument"
        ))),
    }
}

/// `nil` (absent) or a string.
fn expect_optional_string(
    args: &[LispVal],
    i: usize,
    who: &str,
) -> Result<Option<String>, LispError> {
    match args.get(i) {
        None | Some(LispVal::Nil) => Ok(None),
        Some(LispVal::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected NIL or a string, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
    }
}

/// Any non-NIL value is "true" in Lisp, matching every other boolean-flag
/// argument in this kernel; an absent argument is "false" (NIL).
fn expect_bool(args: &[LispVal], i: usize, _who: &str) -> Result<bool, LispError> {
    Ok(!matches!(args.get(i), None | Some(LispVal::Nil)))
}

fn expect_string_list(args: &[LispVal], i: usize, who: &str) -> Result<Vec<String>, LispError> {
    let list = args.get(i).cloned().unwrap_or(LispVal::Nil);
    let items = list_to_vec(&list)?;
    items
        .into_iter()
        .map(|v| match v {
            LispVal::String(s) => Ok(s),
            other => Err(LispError::Generic(format!(
                "{}: expected a list of strings, got {}",
                who.to_uppercase(),
                err_val(&other)
            ))),
        })
        .collect()
}

/// A list of `(name . value)` string conses, e.g. an environment-variable
/// alist.
fn expect_string_alist(
    args: &[LispVal],
    i: usize,
    who: &str,
) -> Result<Vec<(String, String)>, LispError> {
    let list = args.get(i).cloned().unwrap_or(LispVal::Nil);
    let items = list_to_vec(&list)?;
    items
        .into_iter()
        .map(|v| match v {
            LispVal::Cons { car, cdr } => match (car.as_ref(), cdr.as_ref()) {
                (LispVal::String(k), LispVal::String(v)) => Ok((k.clone(), v.clone())),
                _ => Err(LispError::Generic(format!(
                    "{}: expected an alist of (string . string), got {}",
                    who.to_uppercase(),
                    err_val(&LispVal::Cons { car, cdr })
                ))),
            },
            other => Err(LispError::Generic(format!(
                "{}: expected an alist of (string . string), got {}",
                who.to_uppercase(),
                err_val(&other)
            ))),
        })
        .collect()
}

fn expect_i64(args: &[LispVal], i: usize, who: &str) -> Result<i64, LispError> {
    match args.get(i) {
        Some(LispVal::Number(n)) => Ok(*n),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected an integer, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires an integer argument"
        ))),
    }
}

fn expect_nonneg(args: &[LispVal], i: usize, who: &str) -> Result<usize, LispError> {
    match args.get(i) {
        Some(LispVal::Number(n)) if *n >= 0 => Ok(*n as usize),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a non-negative integer, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a non-negative integer argument"
        ))),
    }
}

/// A "stdio mode" keyword: `:INHERIT`, `:NULL`, or `:PIPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdioMode {
    Inherit,
    Null,
    Pipe,
}

fn expect_stdio_mode(args: &[LispVal], i: usize, who: &str) -> Result<StdioMode, LispError> {
    match args.get(i) {
        Some(LispVal::Symbol(s)) => match s.borrow().name.as_str() {
            "INHERIT" | ":INHERIT" => Ok(StdioMode::Inherit),
            "NULL" | ":NULL" => Ok(StdioMode::Null),
            "PIPE" | ":PIPE" => Ok(StdioMode::Pipe),
            other => Err(LispError::Generic(format!(
                "{}: expected :INHERIT, :NULL, or :PIPE, got {other}",
                who.to_uppercase()
            ))),
        },
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a symbol (:INHERIT/:NULL/:PIPE), got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a stdio-mode argument"
        ))),
    }
}

impl StdioMode {
    fn to_stdio(self) -> Stdio {
        match self {
            StdioMode::Inherit => Stdio::inherit(),
            StdioMode::Null => Stdio::null(),
            StdioMode::Pipe => Stdio::piped(),
        }
    }
}

fn expect_os_child<'a>(
    args: &'a [LispVal],
    i: usize,
    who: &str,
) -> Result<&'a Shared<crate::ChildObj>, LispError> {
    match args.get(i) {
        Some(LispVal::OsChild(c)) => Ok(c),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a process handle, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a process-handle argument"
        ))),
    }
}

// ── Structured error / alist construction ──────────────────────────────────

fn cons(car: LispVal, cdr: LispVal) -> LispVal {
    LispVal::Cons {
        car: Shared::new(car),
        cdr: Shared::new(cdr),
    }
}

/// Build a proper alist `((key1 . val1) (key2 . val2) ...)` from `pairs`,
/// interning each key as an (uppercased) keyword-style symbol.
fn alist(env: &Shared<Environment>, pairs: &[(&str, LispVal)]) -> LispVal {
    let mut out = LispVal::Nil;
    for (k, v) in pairs.iter().rev() {
        let key = LispVal::Symbol(env.intern_symbol(k));
        out = cons(cons(key, v.clone()), out);
    }
    out
}

/// Classify an [`std::io::Error`] into one of this module's error
/// categories (mirrors `builtins_net.rs::classify_io_error` but for
/// filesystem/process errors rather than connection errors).
fn classify_os_io_error(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match kind {
        NotFound => "NOT-FOUND",
        PermissionDenied => "PERMISSION-DENIED",
        AlreadyExists => "ALREADY-EXISTS",
        InvalidInput | InvalidData => "INVALID-ARGUMENT",
        _ => "OTHER",
    }
}

/// Build a structured OS error: a `LispVal::Error` whose `data` is an alist
/// `((:operation . "...") (:category . :keyword) (:os-error . "...") ...)`
/// — see this module's doc comment. `extra` appends operation-specific
/// fields (e.g. `:path`, `:pid`).
fn os_error(
    env: &Shared<Environment>,
    operation: &str,
    category: &str,
    detail: &str,
    extra: &[(&str, LispVal)],
) -> LispError {
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let mut pairs = vec![
        (":OPERATION", LispVal::String(operation.to_string())),
        (":CATEGORY", sym(&format!(":{category}"))),
        (":OS-ERROR", LispVal::String(detail.to_string())),
    ];
    pairs.extend_from_slice(extra);
    let data = alist(env, &pairs);
    let message = format!("{operation}: {category}: {detail}");
    LispError::Signaled(Box::new(LispVal::Error(Shared::new(crate::ErrorObj {
        message,
        data,
    }))))
}

fn io_os_error(
    env: &Shared<Environment>,
    operation: &str,
    e: std::io::Error,
    extra: &[(&str, LispVal)],
) -> LispError {
    let category = classify_os_io_error(e.kind());
    os_error(env, operation, category, &e.to_string(), extra)
}

fn closed_child_error(env: &Shared<Environment>, operation: &str, name: &str) -> LispError {
    os_error(
        env,
        operation,
        "CLOSED",
        "process handle is closed (already reaped)",
        &[(":NAME", LispVal::String(name.to_string()))],
    )
}

fn unsupported_platform(env: &Shared<Environment>, operation: &str) -> LispError {
    os_error(
        env,
        operation,
        "UNSUPPORTED-PLATFORM",
        "not available on this platform",
        &[],
    )
}

fn bytes_to_char_array(bytes: Vec<u8>) -> LispVal {
    let items: Vec<LispVal> = bytes.into_iter().map(LispVal::Char).collect();
    LispVal::Array(Shared::new(SharedCell::new(items)))
}

fn strings_to_list(strings: Vec<String>) -> LispVal {
    let mut out = LispVal::Nil;
    for s in strings.into_iter().rev() {
        out = cons(LispVal::String(s), out);
    }
    out
}

/// [`crate::ChildExitStatus`] as a structured alist: `((:exit-code
/// . n-or-nil) (:signal . n-or-nil) (:success . t-or-nil))`.
fn exit_status_alist(env: &Shared<Environment>, status: crate::ChildExitStatus) -> LispVal {
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    let code = status
        .code
        .map(|c| LispVal::Number(c as i64))
        .unwrap_or(LispVal::Nil);
    let signal = status
        .signal
        .map(|s| LispVal::Number(s as i64))
        .unwrap_or(LispVal::Nil);
    alist(
        env,
        &[
            (":EXIT-CODE", code),
            (":SIGNAL", signal),
            (":SUCCESS", if status.success { t() } else { LispVal::Nil }),
        ],
    )
}

#[inline(never)]
pub(super) fn apply_os_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    match op {
        // ── Process identity / environment (read) ─────────────────────────
        BuiltinFunc::OsArgs => {
            require_os_env(env)?;
            Ok(strings_to_list(std::env::args().collect()))
        }
        BuiltinFunc::OsExecutablePath => {
            require_os_env(env)?;
            let path =
                std::env::current_exe().map_err(|e| io_os_error(env, "executable-path", e, &[]))?;
            Ok(LispVal::String(path.to_string_lossy().into_owned()))
        }
        BuiltinFunc::OsCwd => {
            require_os_env(env)?;
            let path = std::env::current_dir().map_err(|e| io_os_error(env, "cwd", e, &[]))?;
            Ok(LispVal::String(path.to_string_lossy().into_owned()))
        }
        BuiltinFunc::OsEnvGet => {
            require_os_env(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-env-get* requires exactly one argument: name".to_string(),
                ));
            }
            let name = expect_string(args, 0, "os-env-get*")?;
            match std::env::var(&name) {
                Ok(v) => Ok(LispVal::String(v)),
                Err(_) => Ok(LispVal::Nil),
            }
        }
        BuiltinFunc::OsEnvList => {
            require_os_env(env)?;
            let mut out = LispVal::Nil;
            let mut vars: Vec<(String, String)> = std::env::vars().collect();
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, v) in vars.into_iter().rev() {
                out = cons(cons(LispVal::String(k), LispVal::String(v)), out);
            }
            Ok(out)
        }
        BuiltinFunc::OsPid => {
            require_os_env(env)?;
            Ok(LispVal::Number(std::process::id() as i64))
        }
        BuiltinFunc::OsPpid => {
            require_os_env(env)?;
            // std has no portable getppid(); Linux exposes it cheaply and
            // std-only via /proc/self/stat's 4th whitespace-delimited field
            // (after the closing paren of the (possibly space-containing)
            // comm field), avoiding any FFI for this one.
            match std::fs::read_to_string("/proc/self/stat") {
                Ok(contents) => match contents.rfind(')') {
                    Some(idx) => {
                        let rest = contents[idx + 1..].trim_start();
                        let ppid = rest
                            .split_whitespace()
                            .nth(1) // fields after comm: state, ppid
                            .and_then(|s| s.parse::<i64>().ok());
                        match ppid {
                            Some(p) => Ok(LispVal::Number(p)),
                            None => Err(unsupported_platform(env, "ppid")),
                        }
                    }
                    None => Err(unsupported_platform(env, "ppid")),
                },
                Err(_) => Err(unsupported_platform(env, "ppid")),
            }
        }
        BuiltinFunc::OsHostname => {
            require_os_env(env)?;
            // std has no portable gethostname(); Linux exposes it as a
            // plain file read, avoiding any FFI for this one.
            match std::fs::read_to_string("/proc/sys/kernel/hostname") {
                Ok(s) => Ok(LispVal::String(s.trim_end().to_string())),
                Err(_) => Err(unsupported_platform(env, "hostname")),
            }
        }

        // ── Process identity / environment (write) ─────────────────────────
        BuiltinFunc::OsChdir => {
            require_os_env_write(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-chdir* requires exactly one argument: path".to_string(),
                ));
            }
            let path = expect_string(args, 0, "os-chdir*")?;
            std::env::set_current_dir(&path).map_err(|e| {
                io_os_error(env, "chdir", e, &[(":PATH", LispVal::String(path.clone()))])
            })?;
            Ok(t())
        }
        BuiltinFunc::OsEnvSet => {
            require_os_env_write(env)?;
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "os-env-set* requires exactly two arguments: name value".to_string(),
                ));
            }
            let name = expect_string(args, 0, "os-env-set*")?;
            let value = expect_string(args, 1, "os-env-set*")?;
            // SAFETY: `set_var` is only unsound if another thread reads the
            // environment concurrently without synchronization (POSIX
            // setenv is not thread-safe). This kernel does not spawn
            // background threads that read `std::env` behind the caller's
            // back; `OS-SPAWN*` reads the ambient environment only on the
            // calling thread via `std::process::Command`, never
            // concurrently with this call. Flagged in the PR description as
            // a documented limitation for embedders that DO run other
            // threads touching the environment.
            unsafe {
                std::env::set_var(&name, &value);
            }
            Ok(t())
        }
        BuiltinFunc::OsEnvUnset => {
            require_os_env_write(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-env-unset* requires exactly one argument: name".to_string(),
                ));
            }
            let name = expect_string(args, 0, "os-env-unset*")?;
            // SAFETY: see OS-ENV-SET* above.
            unsafe {
                std::env::remove_var(&name);
            }
            Ok(t())
        }

        // ── Time ─────────────────────────────────────────────────────────
        BuiltinFunc::OsNow => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            Ok(cons(
                LispVal::Number(now.as_secs() as i64),
                LispVal::Number(now.subsec_nanos() as i64),
            ))
        }
        BuiltinFunc::OsMonotonicNanos => Ok(LispVal::Number(monotonic_nanos())),
        BuiltinFunc::OsSleep => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-sleep* requires exactly one argument: milliseconds".to_string(),
                ));
            }
            let ms = expect_nonneg(args, 0, "os-sleep*")?;
            std::thread::sleep(Duration::from_millis(ms as u64));
            Ok(t())
        }

        // ── Randomness ───────────────────────────────────────────────────
        BuiltinFunc::OsPrngStep => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-prng-step* requires exactly one argument: state".to_string(),
                ));
            }
            let state = expect_i64(args, 0, "os-prng-step*")? as u64;
            // SplitMix64 step -- deterministic, pure, no host access. Kept
            // entirely in Rust u64 wrapping arithmetic so it never touches
            // Lisp's i64 OVERFLOW-flag semantics (issue #228); the result is
            // bit-reinterpreted back to Lisp's signed i64 representation.
            let next = state.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = next;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^= z >> 31;
            // Mask to 63 bits so the returned value is always representable
            // as a non-negative Lisp integer.
            let value = (z >> 1) as i64;
            Ok(cons(LispVal::Number(next as i64), LispVal::Number(value)))
        }
        BuiltinFunc::OsRandomBytes => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-random-bytes* requires exactly one argument: n".to_string(),
                ));
            }
            let n = expect_nonneg(args, 0, "os-random-bytes*")?;
            let mut buf = vec![0u8; n];
            if n > 0 {
                use std::io::Read;
                let mut f = std::fs::File::open("/dev/urandom")
                    .map_err(|e| io_os_error(env, "random-bytes", e, &[]))?;
                f.read_exact(&mut buf)
                    .map_err(|e| io_os_error(env, "random-bytes", e, &[]))?;
            }
            Ok(bytes_to_char_array(buf))
        }

        // ── Process spawn / control ─────────────────────────────────────────
        BuiltinFunc::OsSpawn => {
            require_os_process(env)?;
            if args.len() != 8 {
                return Err(LispError::Generic(
                    "os-spawn* requires exactly eight arguments: program argv inherit-env-p env-alist cwd stdin-mode stdout-mode stderr-mode"
                        .to_string(),
                ));
            }
            let program = expect_string(args, 0, "os-spawn*")?;
            let argv = expect_string_list(args, 1, "os-spawn*")?;
            let inherit_env = expect_bool(args, 2, "os-spawn*")?;
            let env_overrides = expect_string_alist(args, 3, "os-spawn*")?;
            let cwd = expect_optional_string(args, 4, "os-spawn*")?;
            let stdin_mode = expect_stdio_mode(args, 5, "os-spawn*")?;
            let stdout_mode = expect_stdio_mode(args, 6, "os-spawn*")?;
            let stderr_mode = expect_stdio_mode(args, 7, "os-spawn*")?;

            if !env.check_os_policy(&crate::OsOperation::Spawn {
                program: &program,
                args: &argv,
                cwd: cwd.as_deref(),
            }) {
                return Err(os_error(
                    env,
                    "spawn",
                    "POLICY-DENIED",
                    "denied by host OS policy",
                    &[(":PROGRAM", LispVal::String(program.clone()))],
                ));
            }

            let mut cmd = std::process::Command::new(&program);
            cmd.args(&argv);
            if !inherit_env {
                cmd.env_clear();
            }
            for (k, v) in &env_overrides {
                cmd.env(k, v);
            }
            if let Some(dir) = &cwd {
                cmd.current_dir(dir);
            }
            cmd.stdin(stdin_mode.to_stdio());
            cmd.stdout(stdout_mode.to_stdio());
            cmd.stderr(stderr_mode.to_stdio());

            let mut child = cmd.spawn().map_err(|e| {
                io_os_error(
                    env,
                    "spawn",
                    e,
                    &[(":PROGRAM", LispVal::String(program.clone()))],
                )
            })?;

            let stdin_port = if stdin_mode == StdioMode::Pipe {
                child
                    .stdin
                    .take()
                    .map(|s| LispVal::wrap_writer("<process-stdin>", "CHILD-STDIN", Box::new(s)))
                    .unwrap_or(LispVal::Nil)
            } else {
                LispVal::Nil
            };
            let stdout_port = if stdout_mode == StdioMode::Pipe {
                child
                    .stdout
                    .take()
                    .map(|s| LispVal::wrap_reader("<process-stdout>", "CHILD-STDOUT", Box::new(s)))
                    .unwrap_or(LispVal::Nil)
            } else {
                LispVal::Nil
            };
            let stderr_port = if stderr_mode == StdioMode::Pipe {
                child
                    .stderr
                    .take()
                    .map(|s| LispVal::wrap_reader("<process-stderr>", "CHILD-STDERR", Box::new(s)))
                    .unwrap_or(LispVal::Nil)
            } else {
                LispVal::Nil
            };

            let handle = LispVal::OsChild(crate::ChildObj::new(program, child));
            Ok(cons(
                handle,
                cons(
                    stdin_port,
                    cons(stdout_port, cons(stderr_port, LispVal::Nil)),
                ),
            ))
        }
        BuiltinFunc::OsProcessWait => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-process-wait* requires exactly one argument".to_string(),
                ));
            }
            let child = expect_os_child(args, 0, "os-process-wait*")?;
            let status = child.wait().map_err(|e| io_os_error(env, "wait", e, &[]))?;
            Ok(exit_status_alist(env, status))
        }
        BuiltinFunc::OsProcessTryWait => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-process-try-wait* requires exactly one argument".to_string(),
                ));
            }
            let child = expect_os_child(args, 0, "os-process-try-wait*")?;
            let status = child
                .try_wait()
                .map_err(|e| io_os_error(env, "try-wait", e, &[]))?;
            Ok(match status {
                Some(s) => exit_status_alist(env, s),
                None => LispVal::Nil,
            })
        }
        BuiltinFunc::OsProcessId => {
            let child = expect_os_child(args, 0, "os-process-id*")?;
            Ok(LispVal::Number(child.pid as i64))
        }
        BuiltinFunc::OsProcessKill => {
            let child = expect_os_child(args, 0, "os-process-kill*")?;
            if !child.is_open() {
                return Err(closed_child_error(env, "kill", &child.name));
            }
            child.kill().map_err(|e| io_os_error(env, "kill", e, &[]))?;
            Ok(t())
        }
        BuiltinFunc::OsProcessTerminate => {
            let child = expect_os_child(args, 0, "os-process-terminate*")?;
            if !child.is_open() {
                return Err(closed_child_error(env, "terminate", &child.name));
            }
            child
                .terminate()
                .map_err(|e| io_os_error(env, "terminate", e, &[]))?;
            Ok(t())
        }
        BuiltinFunc::OsProcessOpenP => {
            let child = expect_os_child(args, 0, "os-process-open-p*")?;
            Ok(if child.is_open() { t() } else { LispVal::Nil })
        }
        BuiltinFunc::OsProcessP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-process-p* requires exactly one argument".to_string(),
                ));
            }
            Ok(match &args[0] {
                LispVal::OsChild(_) => t(),
                _ => LispVal::Nil,
            })
        }

        // ── Signals ──────────────────────────────────────────────────────
        BuiltinFunc::OsSignal => {
            require_os_signal(env)?;
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "os-signal* requires exactly two arguments: pid signal-name".to_string(),
                ));
            }
            let pid = expect_i64(args, 0, "os-signal*")?;
            let signal_name = expect_string(args, 1, "os-signal*")?;
            let signal = crate::signal_by_name(&signal_name).ok_or_else(|| {
                os_error(
                    env,
                    "signal",
                    "INVALID-ARGUMENT",
                    &format!("unknown signal name {signal_name:?}"),
                    &[],
                )
            })?;
            if !env.check_os_policy(&crate::OsOperation::Signal { pid, signal }) {
                return Err(os_error(
                    env,
                    "signal",
                    "POLICY-DENIED",
                    "denied by host OS policy",
                    &[(":PID", LispVal::Number(pid))],
                ));
            }
            crate::send_signal(pid as i32, signal).map_err(|e| {
                os_error(
                    env,
                    "signal",
                    "SIGNAL-FAILED",
                    &e.to_string(),
                    &[(":PID", LispVal::Number(pid))],
                )
            })?;
            Ok(t())
        }

        // ── Linux-specific: advanced file metadata / links ─────────────────
        BuiltinFunc::OsLinuxStat => {
            require_read_fs(env)?;
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "os-linux-stat* requires exactly two arguments: path follow-symlinks-p"
                        .to_string(),
                ));
            }
            let path = expect_string(args, 0, "os-linux-stat*")?;
            let follow = expect_bool(args, 1, "os-linux-stat*")?;
            os_linux_stat(env, &path, follow)
        }
        BuiltinFunc::OsLinuxReadlink => {
            require_read_fs(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "os-linux-readlink* requires exactly one argument: path".to_string(),
                ));
            }
            let path = expect_string(args, 0, "os-linux-readlink*")?;
            os_linux_readlink(env, &path)
        }

        _ => Err(LispError::Generic("Not an OS operation".to_string())),
    }
}

/// A fixed monotonic reference point, established lazily on first use.
/// [`monotonic_nanos`] returns nanoseconds elapsed since this point --
/// std's `Instant` is not itself a meaningful Lisp value (no epoch), so this
/// keeps the Lisp surface a plain integer, exactly like `OS-MONOTONIC-NANOS*`
/// documents.
fn monotonic_nanos() -> i64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    start.elapsed().as_nanos() as i64
}

#[cfg(unix)]
fn os_linux_stat(
    env: &Shared<Environment>,
    path: &str,
    follow: bool,
) -> Result<LispVal, LispError> {
    use std::os::unix::fs::MetadataExt;
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    let meta = if follow {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    }
    .map_err(|e| {
        io_os_error(
            env,
            "stat",
            e,
            &[(":PATH", LispVal::String(path.to_string()))],
        )
    })?;
    let file_type = meta.file_type();
    Ok(alist(
        env,
        &[
            (":SIZE", LispVal::Number(meta.size() as i64)),
            (":MODE", LispVal::Number((meta.mode() & 0o7777) as i64)),
            (":UID", LispVal::Number(meta.uid() as i64)),
            (":GID", LispVal::Number(meta.gid() as i64)),
            (":NLINK", LispVal::Number(meta.nlink() as i64)),
            (":INO", LispVal::Number(meta.ino() as i64)),
            (":DEV", LispVal::Number(meta.dev() as i64)),
            (":MTIME", LispVal::Number(meta.mtime())),
            (":ATIME", LispVal::Number(meta.atime())),
            (":CTIME", LispVal::Number(meta.ctime())),
            (
                ":IS-DIR",
                if file_type.is_dir() {
                    t()
                } else {
                    LispVal::Nil
                },
            ),
            (
                ":IS-FILE",
                if file_type.is_file() {
                    t()
                } else {
                    LispVal::Nil
                },
            ),
            (
                ":IS-SYMLINK",
                if file_type.is_symlink() {
                    t()
                } else {
                    LispVal::Nil
                },
            ),
        ],
    ))
}

#[cfg(not(unix))]
fn os_linux_stat(
    env: &Shared<Environment>,
    _path: &str,
    _follow: bool,
) -> Result<LispVal, LispError> {
    Err(unsupported_platform(env, "stat"))
}

#[cfg(unix)]
fn os_linux_readlink(env: &Shared<Environment>, path: &str) -> Result<LispVal, LispError> {
    let target = std::fs::read_link(path).map_err(|e| {
        io_os_error(
            env,
            "readlink",
            e,
            &[(":PATH", LispVal::String(path.to_string()))],
        )
    })?;
    Ok(LispVal::String(target.to_string_lossy().into_owned()))
}

#[cfg(not(unix))]
fn os_linux_readlink(env: &Shared<Environment>, _path: &str) -> Result<LispVal, LispError> {
    Err(unsupported_platform(env, "readlink"))
}
