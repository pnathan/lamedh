//! Integration tests for `lamedh --mcp`: spawn the compiled binary, speak real
//! newline-delimited JSON-RPC 2.0 over stdin/stdout, and assert protocol
//! behaviour end to end (handshake, tool listing, persistent eval state,
//! teaching errors, static checking, fuel-based termination, and the
//! sandbox-by-default capability model).

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

/// Spawn `lamedh --mcp <extra_args>`, feed each request as one JSON line, close
/// stdin, and collect every response line parsed as JSON. Requests without an
/// `id` (notifications) produce no response, so callers should not count on a
/// one-to-one line mapping — match responses by `id`.
fn mcp_session(extra_args: &[&str], requests: &[Value]) -> Vec<Value> {
    let mut args = vec!["--mcp"];
    args.extend_from_slice(extra_args);
    let mut child = Command::new(bin())
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lamedh --mcp");

    {
        let mut stdin = child.stdin.take().unwrap();
        for req in requests {
            writeln!(stdin, "{}", serde_json::to_string(req).unwrap()).unwrap();
        }
        // Dropping stdin closes it, so the server loop reaches EOF and exits.
    }

    let stdout = child.stdout.take().unwrap();
    let mut responses = Vec::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        responses.push(serde_json::from_str::<Value>(&line).expect("response is valid JSON"));
    }
    let _ = child.wait();
    responses
}

/// Find the response with the given id.
fn by_id(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|r| r.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("no response with id {id} in {responses:?}"))
}

/// The text of the first content block of a tools/call result.
fn call_text(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"].as_str().unwrap()
}

fn is_error(resp: &Value) -> bool {
    resp["result"]["isError"].as_bool().unwrap()
}

fn eval_req(id: i64, source: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": "eval", "arguments": { "source": source } }
    })
}

#[test]
fn initialize_handshake() {
    let responses = mcp_session(
        &[],
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        })],
    );
    let init = by_id(&responses, 1);
    assert_eq!(init["jsonrpc"], "2.0");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "lamedh");
    assert!(init["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn notifications_get_no_response_and_ping_replies() {
    let responses = mcp_session(
        &[],
        &[
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }),
        ],
    );
    // The notification produced nothing; only ping replied.
    assert_eq!(responses.len(), 1, "{responses:?}");
    assert_eq!(by_id(&responses, 7)["result"], json!({}));
}

#[test]
fn tools_list_contains_all_six_tools() {
    let responses = mcp_session(
        &[],
        &[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })],
    );
    let tools = by_id(&responses, 1)["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in ["eval", "check", "doc", "apropos", "run-tests", "introspect"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    // Every tool advertises an object inputSchema with properties.
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{t:?}");
        assert!(t["inputSchema"]["properties"].is_object(), "{t:?}");
    }
}

#[test]
fn unknown_method_is_method_not_found() {
    let responses = mcp_session(
        &[],
        &[json!({ "jsonrpc": "2.0", "id": 9, "method": "no/such/method" })],
    );
    assert_eq!(by_id(&responses, 9)["error"]["code"], -32601);
}

#[test]
fn parse_error_yields_negative_32700() {
    // Feed a raw malformed line by going through a manual spawn (mcp_session
    // only takes structured Values).
    let mut child = Command::new(bin())
        .args(["--mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{{not valid json").unwrap();
    }
    let stdout = child.stdout.take().unwrap();
    let line = BufReader::new(stdout).lines().next().unwrap().unwrap();
    let _ = child.wait();
    let resp: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["error"]["code"], -32700);
}

#[test]
fn eval_persists_state_across_calls() {
    let responses = mcp_session(&[], &[eval_req(1, "(setq x 5)"), eval_req(2, "x")]);
    assert_eq!(call_text(by_id(&responses, 1)), "5");
    assert_eq!(call_text(by_id(&responses, 2)), "5");
    assert!(!is_error(by_id(&responses, 2)));
}

#[test]
fn eval_error_carries_did_you_mean() {
    let responses = mcp_session(&[], &[eval_req(1, "(lenght nil)")]);
    let resp = by_id(&responses, 1);
    assert!(is_error(resp));
    let text = call_text(resp);
    assert!(
        text.contains("LENGTH"),
        "expected did-you-mean, got: {text}"
    );
}

#[test]
fn check_reports_a_diagnostic() {
    let responses = mcp_session(
        &[],
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "check", "arguments": { "source": "(defun f (x) (lenght x))" } }
        })],
    );
    let text = call_text(by_id(&responses, 1));
    assert!(text.contains("unbound-function"), "{text}");
    assert!(text.contains("LENGHT"), "{text}");
}

#[test]
fn doc_returns_help_text() {
    let responses = mcp_session(
        &[],
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "doc", "arguments": { "symbol": "car" } }
        })],
    );
    let text = call_text(by_id(&responses, 1));
    assert!(text.contains("CAR"), "{text}");
    assert!(text.to_lowercase().contains("first element"), "{text}");
}

#[test]
fn run_tests_reports_pass_and_fail() {
    let responses = mcp_session(
        &[],
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "run-tests", "arguments": {
                "source": "(deftest a (assert-equal 1 1)) (deftest b (assert-equal 1 2))"
            } }
        })],
    );
    let text = call_text(by_id(&responses, 1));
    assert!(text.contains("1 passed; 1 failed"), "{text}");
    assert!(text.contains("FAIL B"), "{text}");
}

#[test]
fn introspect_reports_signature_fields() {
    let responses = mcp_session(
        &[],
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "introspect", "arguments": { "symbol": "car" } }
        })],
    );
    let text = call_text(by_id(&responses, 1));
    assert!(text.contains("signature"), "{text}");
    assert!(text.contains("compiled-p"), "{text}");
    assert!(text.contains("why-not-typed"), "{text}");
}

#[test]
fn infinite_loop_terminates_with_fuel_error() {
    // A tiny fuel budget must turn a non-terminating program into a clean
    // error, and the server must remain responsive for the next call.
    let responses = mcp_session(
        &["--fuel", "500000"],
        &[
            eval_req(1, "((lambda (f) (f f)) (lambda (f) (f f)))"),
            eval_req(2, "(+ 1 2)"),
        ],
    );
    let first = by_id(&responses, 1);
    assert!(is_error(first), "{first:?}");
    assert!(call_text(first).contains("fuel"), "{}", call_text(first));
    // Still alive afterwards.
    assert_eq!(call_text(by_id(&responses, 2)), "3");
}

#[test]
fn while_loop_terminates_with_fuel_error() {
    let responses = mcp_session(&["--fuel", "500000"], &[eval_req(1, "(while t nil)")]);
    let resp = by_id(&responses, 1);
    assert!(is_error(resp));
    assert!(call_text(resp).contains("fuel"), "{}", call_text(resp));
}

#[test]
fn shell_capability_denied_by_default() {
    // MCP mode is sandboxed by default: SHELL is off, so (shell ...) errors.
    let responses = mcp_session(&[], &[eval_req(1, "(shell \"echo hi\")")]);
    let resp = by_id(&responses, 1);
    assert!(is_error(resp), "{resp:?}");
    let text = call_text(resp).to_uppercase();
    assert!(text.contains("SHELL"), "{text}");
}

#[test]
fn capability_can_be_granted_explicitly() {
    // With --capability SHELL granted, the same call succeeds.
    let responses = mcp_session(
        &["--capability", "SHELL"],
        &[eval_req(1, "(car (shell \"echo hi\"))")],
    );
    let resp = by_id(&responses, 1);
    assert!(!is_error(resp), "expected success, got {resp:?}");
}
