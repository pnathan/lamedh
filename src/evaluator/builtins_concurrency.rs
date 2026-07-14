/// Concurrency primitives: make-channel, channel-send, channel-recv,
/// channel-recv-timeout, clone-interpreter.
///
/// Values cross thread boundaries via printer serialisation (send side) and
/// reader deserialisation (receive side) so that the `Rc`-based [`LispVal`]
/// type does not need to be `Send`.
///
/// This module is compiled only when the `concurrency` feature is enabled.
use super::*;
use crate::ChannelObj;
use std::sync::{Arc, mpsc};

/// `(make-channel)` — create a new channel and return it as a `LispVal::Channel`.
///
/// Both the sender and receiver are bundled into the same value.  To get two
/// independent endpoints, use `(clone-channel ch)` (future) or pass the same
/// channel object to both parties.
pub(super) fn apply_make_channel(args: &[LispVal]) -> Result<LispVal, LispError> {
    if !args.is_empty() {
        return Err(LispError::Generic(
            "make-channel takes no arguments".to_string(),
        ));
    }
    let (sender, receiver) = mpsc::channel::<String>();
    Ok(LispVal::Channel(Arc::new(ChannelObj {
        sender,
        receiver: std::sync::Mutex::new(receiver),
    })))
}

/// `(channel-send channel value)` — send `value` through `channel`.
///
/// The value is rendered by the printer into a string and sent across the
/// channel.  Returns `T` on success or signals an error if the receiver has
/// been dropped.
pub(super) fn apply_channel_send(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "channel-send requires exactly two arguments: channel value".to_string(),
        ));
    }
    let ch = match &args[0] {
        LispVal::Channel(c) => Arc::clone(c),
        _ => {
            return Err(LispError::Generic(format!(
                "CHANNEL-SEND: first argument must be a channel, got {}",
                err_val(&args[0])
            )));
        }
    };
    let serialised = crate::printer::print(&args[1]);
    ch.sender
        .send(serialised)
        .map_err(|e| LispError::Generic(format!("channel-send: receiver disconnected: {e}")))?;
    Ok(LispVal::Symbol(env.intern_symbol("T")))
}

/// `(channel-recv channel)` — block until a value is available on `channel`.
///
/// Deserialises the received string back into a [`LispVal`] using the reader.
/// Returns `NIL` if the sender has been dropped and the channel is empty.
pub(super) fn apply_channel_recv(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 1 {
        return Err(LispError::Generic(
            "channel-recv requires exactly one argument: channel".to_string(),
        ));
    }
    let ch = match &args[0] {
        LispVal::Channel(c) => Arc::clone(c),
        _ => {
            return Err(LispError::Generic(format!(
                "CHANNEL-RECV: argument must be a channel, got {}",
                err_val(&args[0])
            )));
        }
    };
    let guard = ch
        .receiver
        .lock()
        .map_err(|e| LispError::Generic(format!("channel-recv: receiver mutex poisoned: {e}")))?;
    match guard.recv() {
        Ok(s) => deserialise_value(&s, env),
        Err(_) => Ok(LispVal::Nil), // sender dropped, channel empty
    }
}

/// `(channel-recv-timeout channel milliseconds)` — receive with a timeout.
///
/// Returns the received value on success, or `NIL` if the timeout expires
/// before a value arrives (or if the sender has been dropped).
pub(super) fn apply_channel_recv_timeout(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 2 {
        return Err(LispError::Generic(
            "channel-recv-timeout requires exactly two arguments: channel milliseconds".to_string(),
        ));
    }
    let ch = match &args[0] {
        LispVal::Channel(c) => Arc::clone(c),
        _ => {
            return Err(LispError::Generic(format!(
                "CHANNEL-RECV-TIMEOUT: first argument must be a channel, got {}",
                err_val(&args[0])
            )));
        }
    };
    let ms = match &args[1] {
        LispVal::Number(n) if *n >= 0 => *n as u64,
        _ => {
            return Err(LispError::Generic(format!(
                "CHANNEL-RECV-TIMEOUT: second argument must be a non-negative integer (ms), got {}",
                err_val(&args[1])
            )));
        }
    };
    let timeout = std::time::Duration::from_millis(ms);
    let guard = ch.receiver.lock().map_err(|e| {
        LispError::Generic(format!(
            "channel-recv-timeout: receiver mutex poisoned: {e}"
        ))
    })?;
    match guard.recv_timeout(timeout) {
        Ok(s) => deserialise_value(&s, env),
        Err(_) => Ok(LispVal::Nil), // timeout or sender dropped
    }
}

/// `(clone-interpreter)` — deep-clone the current interpreter for use in a
/// new thread or isolated evaluation context.
///
/// Creates a fresh [`Environment`] with the full standard library loaded, then
/// copies all globally visible bindings from the current environment into it.
/// The resulting environment is returned as a `LispVal::Environment`.
///
/// Closures captured in the copied bindings still reference the original
/// environment chain; this is intentional — they share the immutable
/// read-only parts.  When `arc-val` is available this restriction will be
/// lifted.
pub(super) fn apply_clone_interpreter(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if !args.is_empty() {
        return Err(LispError::Generic(
            "clone-interpreter takes no arguments".to_string(),
        ));
    }
    // Build a fresh environment with the standard library.
    let new_env = Environment::with_stdlib();
    // Copy all currently visible bindings on top.
    for (name, val) in env.all_bindings() {
        new_env.set(name, val);
    }
    Ok(LispVal::Environment(new_env))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Deserialise a `String` received from a channel back into a [`LispVal`].
fn deserialise_value(s: &str, env: &Shared<Environment>) -> Result<LispVal, LispError> {
    crate::reader::read(s, env).map_err(|e| {
        LispError::Generic(format!("channel-recv: failed to parse received value: {e}"))
    })
}

/// `(spawn-thread body-string caps-list fuel-or-nil)` — the kernel half of the
/// `spawn` operative (issue #140, capability-process design). Spins a fresh
/// 512 MiB interpreter thread with its own share-nothing environment: parse
/// the serialized BODY-STRING, grant exactly the capabilities named in
/// CAPS-LIST (already intersected with the parent's grants by the Lisp
/// `spawn` operative — this builtin trusts that list), arm the kernel fuel
/// backstop if FUEL is a number, evaluate, and post the result (or error)
/// back over a returned channel as a `(:ok value)` / `(:error "msg")` datum.
/// Everything crossing the thread boundary is a `String`/`Vec<String>`/`u64`,
/// so the `Rc`-based `LispVal` never needs `Send`.
pub(super) fn apply_spawn(
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if args.len() != 3 {
        return Err(LispError::Generic(
            "spawn-thread requires exactly three arguments: body-string caps-list fuel".to_string(),
        ));
    }
    let body_src = match &args[0] {
        LispVal::String(s) => s.clone(),
        other => {
            return Err(LispError::Generic(format!(
                "SPAWN-THREAD: body must be a string, got {}",
                err_val(other)
            )));
        }
    };
    let caps: Vec<String> = list_to_vec(&args[1])?
        .iter()
        .map(|c| match c {
            LispVal::Symbol(s) => Ok(s.borrow().name.clone()),
            LispVal::String(s) => Ok(s.clone()),
            other => Err(LispError::Generic(format!(
                "SPAWN-THREAD: capability must be a symbol or string, got {}",
                err_val(other)
            ))),
        })
        .collect::<Result<_, _>>()?;
    let fuel: Option<u64> = match &args[2] {
        LispVal::Nil => None,
        LispVal::Number(n) if *n >= 0 => Some(*n as u64),
        other => {
            return Err(LispError::Generic(format!(
                "SPAWN-THREAD: fuel must be a non-negative integer or nil, got {}",
                err_val(other)
            )));
        }
    };

    // The result channel: the child holds the sender, the caller the whole
    // ChannelObj to `channel-recv` on.
    let (sender, receiver) = mpsc::channel::<String>();
    let handle = Arc::new(ChannelObj {
        sender: sender.clone(),
        receiver: std::sync::Mutex::new(receiver),
    });

    // Everything below is Send: String body, Vec<String> caps, Option<u64>
    // fuel, and the Sender. The environment is built INSIDE the thread and
    // never crosses the boundary.
    std::thread::Builder::new()
        .stack_size(crate::INTERPRETER_STACK_SIZE)
        .spawn(move || {
            // `_fresh`: this thread builds exactly one environment and then
            // exits, so the per-thread prototype cache behind with_stdlib()
            // would only add a fork pass and a second retained copy.
            let child = Environment::with_stdlib_fresh();
            for cap in &caps {
                child.enable_feature(cap);
            }
            if let Some(f) = fuel {
                crate::evaluator::set_kernel_fuel(Some(f));
            }
            let outcome = match crate::reader::read(&body_src, &child) {
                Ok(form) => match crate::evaluator::eval(&form, &child) {
                    Ok(v) => format!("(:OK {})", crate::printer::print(&v)),
                    Err(e) => format!(
                        "(:ERROR {})",
                        crate::printer::print(&LispVal::String(format!("{e:?}")))
                    ),
                },
                Err(e) => format!("(:ERROR {})", crate::printer::print(&LispVal::String(e))),
            };
            // A disconnected receiver (caller dropped the handle) is fine.
            let _ = sender.send(outcome);
        })
        .map_err(|e| LispError::Generic(format!("spawn-thread: failed to spawn thread: {e}")))?;

    let _ = env;
    Ok(LispVal::Channel(handle))
}
