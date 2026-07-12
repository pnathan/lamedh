//! Binary ports: kernel substrate (issue #255, epic #253).
//!
//! Every `PORT-*` primitive here is a thin argument-parsing/capability-check
//! wrapper around [`crate::PortObj`]'s methods — the actual I/O and the
//! `PortState` representation live in `src/lib.rs` next to `ErrorObj` and
//! `StructObj`, following the same "representation access is Rust, policy is
//! Lisp" split as the rest of the kernel (see `CLAUDE.md`). The Lisp-facing
//! names, `with-open-port`, and the text convenience wrappers
//! (`read-line!`/`read-string!`/`write-string!`) live in `lib/31-ports.lisp`
//! (the `PORTS` module), mirroring how `lib/30-text.lisp` wraps the UTF-8
//! kernel primitives.
//!
//! Capability model: opening a file port for reading needs `READ-FS`,
//! opening for writing/appending needs `CREATE-FS`, and `(ports:stdin)`
//! needs `IO` — exactly the vocabulary the existing file builtins in
//! `src/evaluator/apply.rs` already use, checked with the same
//! `require_read_fs`/`require_create_fs`/`require_io` helpers so fences
//! (`WITH-CAPABILITIES`) attenuate port construction the same way they
//! attenuate `read-file`/`write-file` (issue #320/#325's dynamic-extent
//! capability mask). Once a port exists, the binary operations on it
//! (read/write/close/...) perform no further capability check — acquisition
//! is gated, continued use of an already-acquired handle is not (the epic's
//! documented "a successfully returned handle is authority to continue"
//! rule).

use super::*;

fn expect_port<'a>(
    args: &'a [LispVal],
    i: usize,
    who: &str,
) -> Result<&'a Shared<PortObj>, LispError> {
    match args.get(i) {
        Some(LispVal::Port(p)) => Ok(p),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a port, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a port argument"
        ))),
    }
}

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

fn expect_byte(args: &[LispVal], i: usize, who: &str) -> Result<u8, LispError> {
    match args.get(i) {
        Some(LispVal::Char(b)) => Ok(*b),
        Some(LispVal::Number(n)) if (0..=255).contains(n) => Ok(*n as u8),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a byte (Char or integer 0-255), got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a byte argument"
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

fn bytes_to_char_array(bytes: Vec<u8>) -> LispVal {
    let items: Vec<LispVal> = bytes.into_iter().map(LispVal::Char).collect();
    LispVal::Array(Shared::new(SharedCell::new(items)))
}

/// Build a structured I/O error: a `LispVal::Error` whose `data` is an alist
/// `((:operation . "...") (:kind . "...") (:name . "...") (:os-error .
/// "..."))`, signalled the same way `(error ...)` does (issue #255: "I/O
/// errors carry operation, resource kind/name, and OS error information in
/// ErrorData rather than only an English string").
fn io_error(
    env: &Shared<Environment>,
    operation: &str,
    kind: &str,
    name: &str,
    detail: &str,
) -> LispError {
    let cons = |car: LispVal, cdr: LispVal| LispVal::Cons {
        car: Shared::new(car),
        cdr: Shared::new(cdr),
    };
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let pair = |k: &str, v: LispVal| cons(sym(k), v);
    let data = cons(
        pair(":OPERATION", LispVal::String(operation.to_string())),
        cons(
            pair(":KIND", LispVal::String(kind.to_string())),
            cons(
                pair(":NAME", LispVal::String(name.to_string())),
                cons(
                    pair(":OS-ERROR", LispVal::String(detail.to_string())),
                    LispVal::Nil,
                ),
            ),
        ),
    );
    let message = format!("{operation}: {kind} {name:?}: {detail}");
    LispError::Signaled(Box::new(LispVal::Error(Shared::new(crate::ErrorObj {
        message,
        data,
    }))))
}

/// Like [`io_error`], but with an extra `:CATEGORY` key (issue #258: a
/// connected TCP stream is an ordinary port, so its `PORTS`-level read/
/// write/flush/close/timeout errors need the same timeout/refused/reset/
/// closed classification as every other networking error --
/// `src/evaluator/builtins_net.rs`'s `classify_io_error`). Only used for
/// `port.kind == "tcp-stream"`, below; every other port kind keeps
/// `io_error`'s original four-key shape unchanged.
fn io_error_categorized(
    env: &Shared<Environment>,
    operation: &str,
    kind: &str,
    name: &str,
    category: &str,
    detail: &str,
) -> LispError {
    match io_error(env, operation, kind, name, detail) {
        LispError::Signaled(v) => {
            if let LispVal::Error(e) = v.as_ref() {
                let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
                let cons = |car: LispVal, cdr: LispVal| LispVal::Cons {
                    car: Shared::new(car),
                    cdr: Shared::new(cdr),
                };
                let category_pair = cons(sym(":CATEGORY"), sym(&format!(":{category}")));
                let data = cons(category_pair, e.data.clone());
                LispError::Signaled(Box::new(LispVal::Error(Shared::new(crate::ErrorObj {
                    message: e.message.clone(),
                    data,
                }))))
            } else {
                LispError::Signaled(v)
            }
        }
        other => other,
    }
}

fn port_io_error(
    env: &Shared<Environment>,
    operation: &str,
    port: &PortObj,
    e: std::io::Error,
) -> LispError {
    if port.kind == "tcp-stream" {
        io_error_categorized(
            env,
            operation,
            port.kind,
            &port.name,
            super::builtins_net::classify_io_error(e.kind()),
            &e.to_string(),
        )
    } else {
        io_error(env, operation, port.kind, &port.name, &e.to_string())
    }
}

#[inline(never)]
pub(super) fn apply_port_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    match op {
        BuiltinFunc::PortOpenInputFile => {
            require_read_fs(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-open-input-file* requires exactly one argument".to_string(),
                ));
            }
            let path = expect_string(args, 0, "port-open-input-file*")?;
            let p = crate::PortObj::open_input_file(&path)
                .map_err(|e| io_error(env, "open-input", "file", &path, &e.to_string()))?;
            Ok(LispVal::Port(p))
        }
        BuiltinFunc::PortOpenOutputFile => {
            require_create_fs(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-open-output-file* requires exactly one argument".to_string(),
                ));
            }
            let path = expect_string(args, 0, "port-open-output-file*")?;
            let p = crate::PortObj::open_output_file(&path, false)
                .map_err(|e| io_error(env, "open-output", "file", &path, &e.to_string()))?;
            Ok(LispVal::Port(p))
        }
        BuiltinFunc::PortOpenAppendFile => {
            require_create_fs(env)?;
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-open-append-file* requires exactly one argument".to_string(),
                ));
            }
            let path = expect_string(args, 0, "port-open-append-file*")?;
            let p = crate::PortObj::open_output_file(&path, true)
                .map_err(|e| io_error(env, "open-append", "file", &path, &e.to_string()))?;
            Ok(LispVal::Port(p))
        }
        BuiltinFunc::PortOpenInputBytes => {
            // No capability: an in-memory buffer touches no host resource.
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-open-input-bytes* requires exactly one argument".to_string(),
                ));
            }
            let bytes = get_char_array_bytes(&args[0], "port-open-input-bytes*")?;
            Ok(LispVal::Port(crate::PortObj::open_memory_input(bytes)))
        }
        BuiltinFunc::PortOpenOutputBytes => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "port-open-output-bytes* takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::Port(crate::PortObj::open_memory_output()))
        }
        BuiltinFunc::PortOutputContents => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-output-contents* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-output-contents*")?;
            let bytes = port.output_contents().map_err(|e| {
                io_error(
                    env,
                    "output-contents",
                    port.kind,
                    &port.name,
                    &e.to_string(),
                )
            })?;
            Ok(bytes_to_char_array(bytes))
        }
        BuiltinFunc::PortStdin => {
            require_io(env)?;
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "port-stdin* takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::Port(crate::PortObj::stdin_port()))
        }
        BuiltinFunc::PortStdout => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "port-stdout* takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::Port(crate::PortObj::stdout_port()))
        }
        BuiltinFunc::PortStderr => {
            if !args.is_empty() {
                return Err(LispError::Generic(
                    "port-stderr* takes no arguments".to_string(),
                ));
            }
            Ok(LispVal::Port(crate::PortObj::stderr_port()))
        }
        BuiltinFunc::PortReadByte => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-read-byte* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-read-byte*")?;
            match port.read_byte() {
                Ok(Some(b)) => Ok(LispVal::Number(b as i64)),
                Ok(None) => Ok(LispVal::Nil),
                Err(e) => Err(port_io_error(env, "read-byte!", port, e)),
            }
        }
        BuiltinFunc::PortReadBytes => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "port-read-bytes* requires exactly two arguments: port n".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-read-bytes*")?;
            let n = expect_nonneg(args, 1, "port-read-bytes*")?;
            match port.read_bytes(n) {
                Ok(bytes) => Ok(bytes_to_char_array(bytes)),
                Err(e) => Err(port_io_error(env, "read-bytes!", port, e)),
            }
        }
        BuiltinFunc::PortWriteByte => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "port-write-byte* requires exactly two arguments: port byte".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-write-byte*")?;
            let b = expect_byte(args, 1, "port-write-byte*")?;
            port.write_bytes(&[b])
                .map_err(|e| port_io_error(env, "write-byte!", port, e))?;
            Ok(t())
        }
        BuiltinFunc::PortWriteBytes => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "port-write-bytes* requires exactly two arguments: port bytes".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-write-bytes*")?;
            let bytes = get_char_array_bytes(&args[1], "port-write-bytes*")?;
            let n = port
                .write_bytes(&bytes)
                .map_err(|e| port_io_error(env, "write-bytes!", port, e))?;
            Ok(LispVal::Number(n as i64))
        }
        BuiltinFunc::PortFlush => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-flush* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-flush*")?;
            port.flush()
                .map_err(|e| port_io_error(env, "flush!", port, e))?;
            Ok(t())
        }
        BuiltinFunc::PortClose => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-close* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-close*")?;
            // Idempotent: closing an already-closed port is a silent no-op.
            port.close();
            Ok(t())
        }
        BuiltinFunc::PortOpenP => {
            let port = expect_port(args, 0, "port-open-p*")?;
            Ok(if port.is_open() { t() } else { LispVal::Nil })
        }
        BuiltinFunc::PortInputP => {
            let port = expect_port(args, 0, "port-input-p*")?;
            Ok(if port.is_readable() {
                t()
            } else {
                LispVal::Nil
            })
        }
        BuiltinFunc::PortOutputP => {
            let port = expect_port(args, 0, "port-output-p*")?;
            Ok(if port.is_writable() {
                t()
            } else {
                LispVal::Nil
            })
        }
        BuiltinFunc::PortSeekableP => {
            let port = expect_port(args, 0, "port-seekable-p*")?;
            Ok(if port.is_seekable() {
                t()
            } else {
                LispVal::Nil
            })
        }
        BuiltinFunc::PortPosition => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-position* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-position*")?;
            let pos = port
                .position()
                .map_err(|e| port_io_error(env, "position", port, e))?;
            Ok(LispVal::Number(pos as i64))
        }
        BuiltinFunc::PortSeek => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "port-seek* requires exactly two arguments: port offset".to_string(),
                ));
            }
            let port = expect_port(args, 0, "port-seek*")?;
            let offset = expect_nonneg(args, 1, "port-seek*")?;
            let pos = port
                .seek_to(offset as u64)
                .map_err(|e| port_io_error(env, "seek!", port, e))?;
            Ok(LispVal::Number(pos as i64))
        }
        BuiltinFunc::PortP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "port-p requires exactly one argument".to_string(),
                ));
            }
            Ok(match &args[0] {
                LispVal::Port(_) => t(),
                _ => LispVal::Nil,
            })
        }
        BuiltinFunc::PortName => {
            let port = expect_port(args, 0, "port-name*")?;
            Ok(LispVal::String(port.name.clone()))
        }
        BuiltinFunc::PortKind => {
            let port = expect_port(args, 0, "port-kind*")?;
            Ok(LispVal::Symbol(
                env.intern_symbol(&port.kind.to_uppercase()),
            ))
        }
        _ => Err(LispError::Generic("Not a port operation".to_string())),
    }
}
