//! Model Context Protocol (MCP) stdio server for the Lamedh interpreter.
//!
//! `lamedh --mcp` speaks JSON-RPC 2.0 (newline-delimited, one JSON object per
//! line) over stdin/stdout against ONE persistent interpreter environment, so
//! an agent converses with a live Lisp image across many tool calls.
//!
//! # Safety model (deliberately different from the interactive CLI)
//!
//! The interactive REPL enables every capability by default — it is a
//! developer tool. `--mcp` does the opposite: it starts FULLY SANDBOXED (all
//! capabilities off) because its whole purpose is to evaluate untrusted,
//! agent-generated code. `--capability X` grants specific ones back;
//! `--sandbox` is a no-op (already the default). Every `eval`/`run-tests`
//! call runs under a per-call [`WITH-FUEL`] fence (see [`fuel`](#fuel)) so a
//! runaway loop terminates with a `fuel exhausted` error instead of hanging
//! the server.
//!
//! # Fuel
//!
//! Each `eval`/`run-tests` tool call arms a fresh kernel-fuel budget via the
//! `WITH-FUEL` special form (issue #284): a narrow-only dynamic fence that
//! untrusted code cannot widen or disarm. `--fuel N` sets the budget; without
//! the flag it defaults to a generous 100_000_000 steps. Arming fuel disables
//! the native JIT for the metered call (a documented no-compile consequence of
//! #284), which is irrelevant to correctness.
//!
//! Known limitation: code that explicitly catches the `fuel exhausted`
//! condition and re-enters a loop can evade the budget (the exhaustion signal
//! disarms the counter so handlers can run). This is the same class of
//! documented hole noted for pre-compiled natives in `lib/22-guard.lisp`.
//! Ordinary runaway code — unbounded recursion, `(while t ...)`, the omega
//! combinator — always terminates.
//!
//! # Protocol surface
//!
//! * `initialize` → capabilities/serverInfo handshake.
//! * `notifications/*` → no response (notifications carry no id).
//! * `ping` → `{}`.
//! * `tools/list` → the six tool definitions with JSON-Schema `inputSchema`.
//! * `tools/call` → `{ "content": [{"type":"text","text":...}], "isError": bool }`.
//! * Unknown method → JSON-RPC error `-32601`; unparseable line → `-32700`.
//!
//! Nothing but protocol JSON is ever written to stdout: on unix, fd 1 is
//! redirected to a scratch file for the interpreter's own `princ`/`terpri`
//! output (captured and returned inside tool results), and responses are
//! written to a preserved duplicate of the original stdout.

use crate::Args;
use lamedh::{Shared, check, environment::Environment, printer};
use serde_json::{Value, json};
use std::io::BufRead;

/// Default per-call fuel budget in MCP mode when `--fuel` is not given.
/// Generous enough that any reasonable program finishes, small enough that a
/// runaway loop terminates in well under a second.
const DEFAULT_MCP_FUEL: u64 = 100_000_000;

/// The protocol version this server implements.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Entry point for `--mcp`. Never returns: runs the read/dispatch loop until
/// stdin reaches EOF, then exits 0.
pub fn run_mcp(args: &Args) -> ! {
    // Sandboxed by default: grant ONLY the explicitly requested capabilities.
    // `--sandbox` is a no-op here (all-off is already the default); the shared
    // DEFAULT_CAPABILITIES / grant_capabilities path is deliberately NOT used.
    let env = Environment::with_stdlib_fresh();
    for cap in &args.capabilities {
        env.enable_feature(&cap.to_uppercase());
    }
    let fuel = args.fuel.unwrap_or(DEFAULT_MCP_FUEL);

    // Preserve the real stdout for protocol writes, then redirect fd 1 so the
    // interpreter's own stdout (princ/terpri/format) never corrupts the wire.
    let mut io = StdoutRedirect::install();

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => dispatch(&msg, &env, fuel, &mut io),
            Err(_) => Some(error_response(
                Value::Null,
                -32700,
                "Parse error: invalid JSON",
            )),
        };
        if let Some(resp) = response {
            io.write_message(&resp);
        }
    }
    std::process::exit(0);
}

/// Route one parsed JSON-RPC message. Returns `Some(response)` for requests
/// (anything with an `id`) and `None` for notifications (no `id`, no reply).
fn dispatch(
    msg: &Value,
    env: &Shared<Environment>,
    fuel: u64,
    io: &mut StdoutRedirect,
) -> Option<Value> {
    let id = msg.get("id").cloned();
    let is_request = id.is_some();
    let method = msg.get("method").and_then(Value::as_str);

    let Some(method) = method else {
        // A missing/non-string method is an invalid request — but only worth a
        // reply if the peer supplied an id.
        return id.map(|id| error_response(id, -32600, "Invalid Request: missing method"));
    };

    // Notifications (no id) never get a response, whatever the method.
    if !is_request {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => Some(success_response(id, initialize_result(msg))),
        "ping" => Some(success_response(id, json!({}))),
        "tools/list" => Some(success_response(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => Some(success_response(id, tools_call(msg, env, fuel, io))),
        _ => Some(error_response(
            id,
            -32601,
            &format!("Method not found: {method}"),
        )),
    }
}

/// Build the `initialize` result. Echoes the client's requested
/// `protocolVersion` when it sent a string one (so an older-but-known client
/// keeps its version); otherwise advertises [`PROTOCOL_VERSION`].
fn initialize_result(msg: &Value) -> Value {
    let requested = msg
        .get("params")
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": requested,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "lamedh", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// The six tool definitions returned by `tools/list`.
fn tool_definitions() -> Value {
    let string_arg = |name: &str, desc: &str| {
        json!({
            "type": "object",
            "properties": { name: { "type": "string", "description": desc } },
            "required": [name],
        })
    };
    json!([
        {
            "name": "eval",
            "description": "Evaluate Lisp source in the persistent interpreter \
                environment and return the printed value(s). State (definitions, \
                variables) persists across calls. On error, returns the Lisp \
                error text (which carries did-you-mean / teaching hints) with \
                isError=true.",
            "inputSchema": string_arg("source", "Lisp source to evaluate."),
        },
        {
            "name": "check",
            "description": "Statically check Lisp source WITHOUT executing it: \
                report parse errors, unbound function calls (with did-you-mean), \
                and provable arity mismatches, as readable s-expression \
                diagnostics. Returns '(no findings)' when clean.",
            "inputSchema": string_arg("source", "Lisp source to check."),
        },
        {
            "name": "doc",
            "description": "Look up the built-in help for a symbol and return the \
                same text the REPL's help system prints (syntax, description, \
                arguments, examples, see-also).",
            "inputSchema": string_arg("symbol", "Symbol name to document."),
        },
        {
            "name": "apropos",
            "description": "List bound symbols whose name contains the given \
                substring (case-insensitive), one per line.",
            "inputSchema": string_arg("pattern", "Substring to match against symbol names."),
        },
        {
            "name": "run-tests",
            "description": "Evaluate the given source in a fresh scratch \
                environment (isolated from the persistent one), run every test \
                it registered via deftest, and return the pass/fail summary and \
                any failures.",
            "inputSchema": string_arg("source", "Lisp source defining tests (via deftest)."),
        },
        {
            "name": "introspect",
            "description": "Combined type/compilation report for a symbol: its \
                inferred signature, whether it compiled to native code \
                (compiled-p), and why it did not type-check if it did not \
                (why-not-typed).",
            "inputSchema": string_arg("symbol", "Symbol name to introspect."),
        },
    ])
}

/// Handle a `tools/call`: extract the tool name and arguments, run the tool,
/// and package a `{content, isError}` result. A missing tool name or unknown
/// tool is itself reported as an isError result (not a JSON-RPC error), per the
/// MCP convention that tool-level failures ride in the result.
fn tools_call(msg: &Value, env: &Shared<Environment>, fuel: u64, io: &mut StdoutRedirect) -> Value {
    let params = msg.get("params");
    let name = params.and_then(|p| p.get("name")).and_then(Value::as_str);
    let arguments = params
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let Some(name) = name else {
        return tool_result("tools/call: missing tool name", true);
    };

    let arg_str = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };

    match name {
        "eval" => tool_eval(env, fuel, io, &arg_str("source")),
        "check" => tool_check(&arg_str("source")),
        "doc" => tool_doc(env, io, &arg_str("symbol")),
        "apropos" => tool_apropos(env, &arg_str("pattern")),
        "run-tests" => tool_run_tests(args_capabilities(env), fuel, io, &arg_str("source")),
        "introspect" => tool_introspect(env, &arg_str("symbol")),
        other => tool_result(&format!("unknown tool: {other}"), true),
    }
}

/// The capability names currently enabled on `env`, so a scratch environment
/// (run-tests) can be granted the same authority the server was started with.
fn args_capabilities(env: &Shared<Environment>) -> Vec<String> {
    crate::DEFAULT_CAPABILITIES
        .iter()
        .filter(|c| env.feature_enabled(c))
        .map(|c| c.to_string())
        .collect()
}

// ----------------------------------------------------------------------------
// Individual tools

/// `eval`: evaluate untrusted source in the persistent env under a per-call
/// `WITH-FUEL` fence. Returns any captured stdout followed by the printed
/// value; on error, the teaching-suffixed error text with isError=true.
fn tool_eval(env: &Shared<Environment>, fuel: u64, io: &mut StdoutRedirect, source: &str) -> Value {
    // Wrap in a WITH-FUEL fence (narrow-only: untrusted code cannot disarm it)
    // around a PROGN of the caller's forms. The fence yields a single value —
    // the last form's — which matches REPL-style "print the result".
    let wrapped = format!("(with-fuel {fuel} (progn {source}))");
    io.mark();
    let outcome = lamedh::eval_str(&wrapped, env);
    let captured = io.take();
    match outcome {
        Ok(v) => {
            let printed = printer::print(&v);
            let text = if captured.is_empty() {
                printed
            } else {
                format!("{captured}{printed}")
            };
            tool_result(&text, false)
        }
        Err(e) => {
            let msg = lamedh::format_error_with_backtrace(&e, env);
            let text = if captured.is_empty() {
                msg
            } else {
                format!("{captured}{msg}")
            };
            tool_result(&text, true)
        }
    }
}

/// `check`: run the static checker over the given source text (label
/// `<mcp>`), returning one s-expression diagnostic per line.
fn tool_check(source: &str) -> Value {
    let findings = check::check_sources(&[("<mcp>".to_string(), source.to_string())]);
    if findings.is_empty() {
        return tool_result("(no findings)", false);
    }
    let text = findings
        .iter()
        .map(|f| f.to_sexpr())
        .collect::<Vec<_>>()
        .join("\n");
    // A parse/read failure is a hard error; lint warnings are not.
    let is_error = check::exit_code(&findings) >= 2;
    tool_result(&text, is_error)
}

/// `doc`: capture what the help system prints for the symbol. help-symbol
/// writes via princ/terpri (to the redirected fd 1) and returns T; the text is
/// the captured output.
fn tool_doc(env: &Shared<Environment>, io: &mut StdoutRedirect, symbol: &str) -> Value {
    if symbol.trim().is_empty() {
        return tool_result("doc: missing symbol", true);
    }
    let src = format!("(help-symbol '{symbol})");
    io.mark();
    let outcome = lamedh::eval_str(&src, env);
    let captured = io.take();
    match outcome {
        Ok(_) => {
            let text = captured.trim_matches('\n');
            if text.is_empty() {
                tool_result(&format!("no documentation for {symbol}"), false)
            } else {
                tool_result(text, false)
            }
        }
        Err(e) => tool_result(&lamedh::format_error_with_backtrace(&e, env), true),
    }
}

/// `apropos`: bound symbol names containing `pattern` (case-insensitive),
/// sorted, one per line. Implemented directly against the symbol table so it
/// covers every bound name, not just documented ones.
fn tool_apropos(env: &Shared<Environment>, pattern: &str) -> Value {
    let needle = pattern.to_uppercase();
    let mut names: Vec<String> = env
        .bound_symbol_names()
        .into_iter()
        .filter(|n| n.to_uppercase().contains(&needle))
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        tool_result(&format!("(no bound symbols matching '{pattern}')"), false)
    } else {
        tool_result(&names.join("\n"), false)
    }
}

/// `run-tests`: evaluate the source in a FRESH scratch stdlib world (isolated
/// from the persistent env and from the testing bookkeeping), run every
/// registered test, and return a summary plus one line per failure. Fenced by
/// the same per-call fuel budget.
fn tool_run_tests(
    capabilities: Vec<String>,
    fuel: u64,
    io: &mut StdoutRedirect,
    source: &str,
) -> Value {
    // A cached fork of the per-thread stdlib prototype: full isolation, cheap.
    let scratch = Environment::with_stdlib();
    for cap in &capabilities {
        scratch.enable_feature(cap);
    }
    // Define the tests, then compute a summary string, all inside one fuel
    // fence. run-all-tests-detailed yields (name status message) triples.
    let summary_expr = "(let* ((rs (run-all-tests-detailed)) \
         (fails (filter (lambda (r) (eq (cadr r) 'fail)) rs)) \
         (npass (- (length rs) (length fails)))) \
       (string-join \
         (cons (concat \"test result: \" (princ-to-string npass) \" passed; \" \
                       (princ-to-string (length fails)) \" failed\") \
               (mapcar (lambda (r) (concat \"FAIL \" (princ-to-string (car r)) \": \" \
                                            (princ-to-string (caddr r)))) \
                       fails)) \
         (code-char 10)))";
    let wrapped = format!("(with-fuel {fuel} (progn {source} {summary_expr}))");
    io.mark();
    let outcome = lamedh::eval_str(&wrapped, &scratch);
    let captured = io.take();
    match outcome {
        Ok(v) => {
            let text = match &v {
                lamedh::LispVal::String(s) => s.clone(),
                other => printer::print(other),
            };
            let text = if captured.is_empty() {
                text
            } else {
                format!("{captured}{text}")
            };
            // Any failure line means the tool call surfaces isError=false (the
            // call succeeded); failures are content, matching `--test`.
            tool_result(&text, false)
        }
        Err(e) => tool_result(&lamedh::format_error_with_backtrace(&e, &scratch), true),
    }
}

/// `introspect`: one combined signature / compiled-p / why-not-typed report.
/// Runs unmetered (bounded, trusted introspection) so that arming fuel does not
/// perturb why-not-typed's compile probe.
fn tool_introspect(env: &Shared<Environment>, symbol: &str) -> Value {
    if symbol.trim().is_empty() {
        return tool_result("introspect: missing symbol", true);
    }
    let probe = |form: String| match lamedh::eval_str(&form, env) {
        Ok(v) => printer::print(&v),
        Err(e) => format!("error: {e}"),
    };
    let signature = probe(format!("(signature '{symbol})"));
    let compiled = probe(format!("(compiled-p '{symbol})"));
    let why_not = probe(format!("(why-not-typed '{symbol})"));
    let text = format!(
        "symbol:      {symbol}\nsignature:   {signature}\ncompiled-p:  {compiled}\nwhy-not-typed: {why_not}"
    );
    tool_result(&text, false)
}

// ----------------------------------------------------------------------------
// JSON-RPC helpers

/// A successful JSON-RPC response.
fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// A JSON-RPC error response.
fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A `tools/call` result payload: one text content block plus the isError flag.
fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

// ----------------------------------------------------------------------------
// Stdout redirection
//
// The interpreter writes princ/terpri/format output straight to fd 1. In MCP
// mode fd 1 is the JSON-RPC wire, so we must keep the two apart. On unix we dup
// the real stdout aside for protocol writes and point fd 1 at a scratch file we
// read back per call. On other platforms we degrade gracefully (no capture,
// protocol still on stdout) — the binary's target platform is Linux.

/// Owns the protocol-output handle and (on unix) the captured-stdout scratch
/// file. `mark`/`take` bracket a single evaluation to return what it printed.
struct StdoutRedirect {
    inner: RedirectImpl,
}

impl StdoutRedirect {
    fn install() -> Self {
        StdoutRedirect {
            inner: RedirectImpl::install(),
        }
    }

    /// Write one JSON message followed by a newline to the protocol stream.
    fn write_message(&mut self, value: &Value) {
        self.inner.write_message(value);
    }

    /// Record the current end of captured output; pairs with [`take`].
    fn mark(&mut self) {
        self.inner.mark();
    }

    /// Return everything the interpreter printed since the last [`mark`].
    fn take(&mut self) -> String {
        self.inner.take()
    }
}

#[cfg(unix)]
mod redirect_unix {
    use serde_json::Value;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::FromRawFd;

    unsafe extern "C" {
        fn dup(oldfd: i32) -> i32;
        fn dup2(oldfd: i32, newfd: i32) -> i32;
    }

    pub struct RedirectImpl {
        /// Duplicate of the original fd 1: where protocol JSON goes.
        proto: File,
        /// Scratch file now backing fd 1; the interpreter's stdout lands here.
        scratch: Option<File>,
        /// Byte offset of the current call's output within `scratch`.
        mark: u64,
    }

    impl RedirectImpl {
        pub fn install() -> Self {
            // Duplicate the real stdout for protocol writes.
            let proto_fd = unsafe { dup(1) };
            let proto = unsafe { File::from_raw_fd(proto_fd) };

            // Point fd 1 at a fresh scratch file (unlinked immediately: it
            // survives as long as an fd holds it, then vanishes on exit).
            let scratch = tempfile().and_then(|f| {
                let ok = unsafe { dup2(std::os::fd::AsRawFd::as_raw_fd(&f), 1) } >= 0;
                if ok { Some(f) } else { None }
            });

            RedirectImpl {
                proto,
                scratch,
                mark: 0,
            }
        }

        pub fn write_message(&mut self, value: &Value) {
            let line = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
            let _ = self.proto.write_all(line.as_bytes());
            let _ = self.proto.write_all(b"\n");
            let _ = self.proto.flush();
        }

        pub fn mark(&mut self) {
            // fd 1 and `scratch` share one open file description, so the shared
            // offset already sits at end-of-file here.
            self.mark = self
                .scratch
                .as_mut()
                .and_then(|f| f.stream_position().ok())
                .unwrap_or(0);
        }

        pub fn take(&mut self) -> String {
            // Flush Rust's buffered stdout so all of the call's output has hit
            // the scratch file before we read it.
            let _ = std::io::stdout().flush();
            let Some(f) = self.scratch.as_mut() else {
                return String::new();
            };
            if f.seek(SeekFrom::Start(self.mark)).is_err() {
                return String::new();
            }
            let mut buf = String::new();
            let _ = f.read_to_string(&mut buf);
            buf
        }
    }

    /// A scratch file for captured stdout, unlinked from the filesystem the
    /// moment it is opened so nothing is left behind.
    fn tempfile() -> Option<File> {
        use std::os::fd::AsRawFd;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lamedh-mcp-{}.out", std::process::id()));
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .ok()?;
        // Unlink now; the open fd keeps the inode alive.
        let _ = std::fs::remove_file(&path);
        // Touch as_raw_fd here so a stray import lint never trips.
        let _ = file.as_raw_fd();
        Some(file)
    }
}

#[cfg(not(unix))]
mod redirect_fallback {
    use serde_json::Value;
    use std::io::Write;

    pub struct RedirectImpl;

    impl RedirectImpl {
        pub fn install() -> Self {
            RedirectImpl
        }
        pub fn write_message(&mut self, value: &Value) {
            let line = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            let _ = writeln!(lock, "{line}");
            let _ = lock.flush();
        }
        pub fn mark(&mut self) {}
        pub fn take(&mut self) -> String {
            String::new()
        }
    }
}

#[cfg(not(unix))]
use redirect_fallback::RedirectImpl;
#[cfg(unix)]
use redirect_unix::RedirectImpl;
