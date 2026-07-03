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
