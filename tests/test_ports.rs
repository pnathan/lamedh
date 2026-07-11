//! Binary ports and deterministic ownership for host resources (issue #255,
//! epic #253): the kernel PORT-* primitives (src/evaluator/builtins_ports.rs,
//! PortObj in src/lib.rs) wrapped by lib/31-ports.lisp's PORTS module.
//!
//! Coverage: open/read/write/close round-trip (exact bytes, including
//! invalid-UTF-8 and every byte value), closed-port errors, double-close
//! idempotence, capability denial and fence attenuation, WITH-OPEN-PORT
//! closing on every unwind path, EOF/partial-read/partial-write behavior,
//! seek/position on seekable vs. non-seekable ports, the TEXT-module
//! interplay, the Rust host-embedding API (WRAP_READER/WRAP_WRITER), and the
//! Drop backstop (a port that is never explicitly closed still releases its
//! file descriptor once the last Lisp reference to it is dropped).

use lamedh::environment::Environment;
use lamedh::{LispVal, Shared, eval_line, eval_str};

fn env_with_ports() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import ports)", &env), "PORTS");
    env
}

fn temp_path(name: &str) -> String {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lamedh-port-test-{}-{}-{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p.to_string_lossy().into_owned()
}

// ── Capability gating ───────────────────────────────────────────────────

#[test]
fn open_input_requires_read_fs() {
    let env = env_with_ports();
    let out = eval_line("(open-input \"/etc/hostname\")", &env);
    assert!(
        out.contains("READ-FS capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn open_output_requires_create_fs() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    let out = eval_line(&format!("(open-output {:?})", temp_path("wcap")), &env);
    assert!(
        out.contains("CREATE-FS capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn open_append_requires_create_fs() {
    let env = env_with_ports();
    let out = eval_line(&format!("(open-append {:?})", temp_path("acap")), &env);
    assert!(out.contains("CREATE-FS capability"), "got: {out}");
}

#[test]
fn stdin_requires_io() {
    let env = env_with_ports();
    let out = eval_line("(stdin)", &env);
    assert!(
        out.contains("IO capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn stdout_stderr_need_no_capability() {
    let env = env_with_ports();
    assert_eq!(eval_line("(port-p (stdout))", &env), "T");
    assert_eq!(eval_line("(port-p (stderr))", &env), "T");
}

#[test]
fn memory_ports_need_no_capability() {
    let env = env_with_ports();
    assert_eq!(eval_line("(port-p (open-output-bytes))", &env), "T");
    assert_eq!(
        eval_line(
            "(port-p (open-input-bytes (list->array (list 1 2 3))))",
            &env
        ),
        "T"
    );
}

#[test]
fn capability_grant_allows_file_open() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let path = temp_path("grant");
    let out = eval_line(&format!("(port-p (open-output {path:?}))"), &env);
    assert_eq!(out, "T");
    // Now a read should succeed (file exists, empty).
    let out = eval_line(&format!("(port-p (open-input {path:?}))"), &env);
    assert_eq!(out, "T", "got: {out}");
}

#[test]
fn fence_attenuates_port_open_even_with_cli_grant() {
    // issue #255's "fence-attenuation probe": a port operation inside
    // (with-capabilities () ...) must fail even though the host/CLI granted
    // the capability, exactly like every other gated builtin (#320/#325).
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    let out = eval_line(
        "(with-capabilities '() (open-input \"/etc/hostname\"))",
        &env,
    );
    assert!(
        out.contains("capability denied: READ-FS") && out.contains("attenuated"),
        "got: {out}"
    );
    // Sanity: without the fence, the same grant works (fails only on
    // file-not-found style errors afterward, never a capability error).
    let out2 = eval_line("(open-input \"/etc/hostname\")", &env);
    assert!(!out2.contains("capability"), "got: {out2}");
}

#[test]
fn fence_attenuates_create_fs_too() {
    let env = env_with_ports();
    env.enable_feature("CREATE-FS");
    let path = temp_path("fence-create");
    let out = eval_line(
        &format!("(with-capabilities '() (open-output {path:?}))"),
        &env,
    );
    assert!(out.contains("capability denied: CREATE-FS"), "got: {out}");
}

// ── Round-trip byte exactness ───────────────────────────────────────────

#[test]
fn file_port_round_trips_every_byte_value_exactly() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let path = temp_path("allbytes");

    // Build (list->array (list 0 1 2 ... 255)) and write it, then read back.
    let elems: Vec<String> = (0..=255u32).map(|n| n.to_string()).collect();
    let src = format!(
        "(progn
           (with-open-port (op (open-output {path:?}))
             (write-bytes! op (list->array (list {list}))))
           (with-open-port (ip (open-input {path:?}))
             (array->list (read-all-bytes! ip))))",
        list = elems.join(" ")
    );
    let out = eval_line(&src, &env);
    // Printed as a list of Char literals; just check the byte count via
    // array-length* on a re-read instead of parsing the printed form.
    assert!(!out.starts_with("Error"), "got: {out}");

    let len_src = format!(
        "(with-open-port (ip (open-input {path:?})) (array-length* (read-all-bytes! ip)))",
        path = path
    );
    assert_eq!(eval_line(&len_src, &env), "256");

    // Verify exact bytes on the Rust side by reading the file directly.
    let bytes = std::fs::read(&path).expect("file should exist");
    assert_eq!(bytes.len(), 256);
    for (i, b) in bytes.iter().enumerate() {
        assert_eq!(*b as usize, i);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn memory_port_round_trips_invalid_utf8_bytes() {
    let env = env_with_ports();
    // 0xFF 0xFE 0x00 0x80 is not valid UTF-8 anywhere in it; the byte-array
    // port must not care (issue #255: "never implicit text coercion").
    let src = "(progn
        (def op (open-output-bytes))
        (write-bytes! op (list->array (list 255 254 0 128)))
        (array->list (output-contents op)))";
    let out = eval_line(src, &env);
    assert!(!out.contains("Error"), "got: {out}");
    let len = eval_line(
        "(array-length* (output-contents (let ((op (open-output-bytes))) (write-bytes! op (list->array (list 255 254 0 128))) op)))",
        &env,
    );
    assert_eq!(len, "4");
}

#[test]
fn input_bytes_port_reads_back_exact_array() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array (list 10 20 30))))
           (list (read-byte! ip) (read-byte! ip) (read-byte! ip) (read-byte! ip)))",
        &env,
    );
    assert_eq!(out, "(10 20 30 ())");
}

// ── EOF and partial read/write ──────────────────────────────────────────

#[test]
fn read_byte_returns_nil_at_eof() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array ())))
           (read-byte! ip))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn read_bytes_returns_short_array_at_eof_not_nil() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array (list 1 2))))
           (array-length* (read-bytes! ip 10)))",
        &env,
    );
    assert_eq!(
        out, "2",
        "a short/partial read must return the bytes available, not NIL"
    );

    let out_empty = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array ())))
           (array-length* (read-bytes! ip 10)))",
        &env,
    );
    assert_eq!(out_empty, "0");
}

#[test]
fn zero_length_read_returns_empty_array_without_consuming() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array (list 1 2 3))))
           (list (array-length* (read-bytes! ip 0)) (read-byte! ip)))",
        &env,
    );
    assert_eq!(out, "(0 1)");
}

#[test]
fn partial_write_count_matches_bytes_written() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (op (open-output-bytes)) (write-bytes! op (list->array (list 1 2 3 4 5))))",
        &env,
    );
    assert_eq!(out, "5");
}

// ── Closed-port errors and double-close ─────────────────────────────────

#[test]
fn operations_on_closed_port_signal_a_structured_error() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p (open-input-bytes (list->array (list 1 2 3))))
           (close! p)
           (handler-case (read-byte! p)
             (error (e) (list 'caught (cdr (assoc :operation (error-data e)))))))",
        &env,
    );
    assert_eq!(out, "(CAUGHT \"read-byte!\")", "got: {out}");
}

#[test]
fn write_to_closed_port_also_errors() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p (open-output-bytes))
           (close! p)
           (errorset '(write-byte! p 1)))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn double_close_is_a_silent_no_op() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p (open-output-bytes))
           (close! p)
           (close! p)
           (close! p)
           'still-fine)",
        &env,
    );
    assert_eq!(out, "STILL-FINE");
}

#[test]
fn open_p_reflects_close() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn (def p (open-output-bytes)) (list (open-p p) (progn (close! p) (open-p p))))",
        &env,
    );
    assert_eq!(out, "(T ())");
}

// ── WITH-OPEN-PORT unwind coverage ───────────────────────────────────────

#[test]
fn with_open_port_closes_on_normal_return() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p nil)
           (with-open-port (op (open-output-bytes)) (setq p op) 'done)
           (open-p p))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn with_open_port_closes_on_ordinary_error() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p nil)
           (handler-case
               (with-open-port (op (open-output-bytes)) (setq p op) (error \"boom\"))
             (error (e) 'caught))
           (open-p p))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn with_open_port_closes_on_throw() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p nil)
           (catch 'tag
             (with-open-port (op (open-output-bytes)) (setq p op) (throw 'tag 42)))
           (open-p p))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn with_open_port_closes_on_return_from() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p nil)
           (block b
             (with-open-port (op (open-output-bytes)) (setq p op) (return-from b 7)))
           (open-p p))",
        &env,
    );
    assert_eq!(out, "()");
}

#[test]
fn with_open_port_closes_on_go_unwind() {
    let env = env_with_ports();
    let out = eval_line(
        "(progn
           (def p nil)
           (prog (x)
             (with-open-port (op (open-output-bytes)) (setq p op) (go done))
             (setq x 'unreached)
             done)
           (open-p p))",
        &env,
    );
    assert_eq!(out, "()", "got: {out}");
}

#[test]
fn body_closing_the_port_itself_does_not_error_with_open_port() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (op (open-output-bytes)) (close! op) 'ok)",
        &env,
    );
    assert_eq!(out, "OK");
}

// ── Seek / position ──────────────────────────────────────────────────────

#[test]
fn file_and_memory_input_ports_are_seekable() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let path = temp_path("seek");
    eval_line(
        &format!(
            "(with-open-port (op (open-output {path:?})) (write-bytes! op (list->array (list 1 2 3 4 5))))"
        ),
        &env,
    );
    let out = eval_line(
        &format!(
            "(with-open-port (ip (open-input {path:?}))
               (seek! ip 3)
               (list (ports:position ip) (read-byte! ip)))"
        ),
        &env,
    );
    assert_eq!(out, "(3 4)");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn output_bytes_and_std_ports_are_not_seekable() {
    let env = env_with_ports();
    env.enable_feature("IO");
    assert_eq!(eval_line("(seekable-p (open-output-bytes))", &env), "()");
    assert_eq!(eval_line("(seekable-p (stdout))", &env), "()");
    assert_eq!(eval_line("(seekable-p (stdin))", &env), "()");
}

#[test]
fn seek_on_non_seekable_port_signals_unsupported_operation_error() {
    let env = env_with_ports();
    let out = eval_line("(errorset '(seek! (open-output-bytes) 0))", &env);
    assert_eq!(out, "()");
    let out = eval_line("(errorset '(ports:position (open-output-bytes)))", &env);
    assert_eq!(out, "()");
}

#[test]
fn import_does_not_clobber_the_prelude_position_helper() {
    // The Prelude's flat (POSITION item lst) list helper must survive
    // (import ports): PORTS:POSITION is deliberately unexported (see
    // lib/31-ports.lisp) precisely so this keeps working.
    let env = env_with_ports();
    assert_eq!(eval_line("(position 'c '(a b c))", &env), "2");
    // The qualified port operation still exists and works.
    let out = eval_line(
        "(with-open-port (ip (open-input-bytes (list->array (list 9))))
           (ports:position ip))",
        &env,
    );
    assert_eq!(out, "0");
}

// ── TEXT-module interplay ───────────────────────────────────────────────

#[test]
fn write_string_read_line_round_trip_through_text_module() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let path = temp_path("text");
    eval_line(
        &format!(
            "(with-open-port (op (open-output {path:?})) (write-string! op \"héllo\\nworld\"))"
        ),
        &env,
    );
    let out = eval_line(
        &format!(
            "(with-open-port (ip (open-input {path:?})) (list (read-line! ip) (read-line! ip) (read-line! ip)))"
        ),
        &env,
    );
    assert_eq!(out, "(\"héllo\" \"world\" ())");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn read_string_decodes_via_text_module() {
    let env = env_with_ports();
    let out = eval_line(
        "(with-open-port (op (open-output-bytes))
           (write-string! op \"世界\")
           (with-open-port (ip (open-input-bytes (output-contents op)))
             (read-string! ip 100)))",
        &env,
    );
    assert_eq!(out, "\"世界\"");
}

#[test]
fn bytes_written_via_ports_agree_with_text_string_to_utf8() {
    let env = env_with_ports();
    let out = eval_line(
        "(equal (array->list (let ((op (open-output-bytes))) (write-string! op \"café\") (output-contents op)))
                (array->list (text:string->utf8 \"café\")))",
        &env,
    );
    assert_eq!(out, "T");
}

// ── Introspection ────────────────────────────────────────────────────────

#[test]
fn kind_and_name_are_diagnostic() {
    let env = env_with_ports();
    assert_eq!(eval_line("(kind (open-output-bytes))", &env), "MEMORY");
    assert_eq!(eval_line("(kind (stdout))", &env), "STDOUT");
    assert_eq!(eval_line("(input-p (open-output-bytes))", &env), "()");
    assert_eq!(eval_line("(output-p (open-output-bytes))", &env), "T");
}

#[test]
fn port_p_is_false_for_non_ports() {
    let env = env_with_ports();
    for v in ["5", "\"hi\"", "'sym", "(list 1 2)", "nil"] {
        assert_eq!(
            eval_line(&format!("(port-p {v})"), &env),
            "()",
            "value: {v}"
        );
    }
}

#[test]
fn ports_print_as_opaque_diagnostic_objects() {
    let env = env_with_ports();
    let out = eval_line("(prin1-to-string (open-output-bytes))", &env);
    assert!(out.starts_with("\"#<port:memory"), "got: {out}");
}

#[test]
fn ports_compare_by_identity_not_structurally() {
    let env = env_with_ports();
    let out = eval_line(
        "(equal (open-input-bytes (list->array (list 1))) (open-input-bytes (list->array (list 1))))",
        &env,
    );
    assert_eq!(out, "()");
    let out2 = eval_line("(let ((p (open-output-bytes))) (eq p p))", &env);
    assert_eq!(out2, "T");
}

// ── Rust embedding API ───────────────────────────────────────────────────

#[test]
fn wrap_reader_exposes_a_port_without_a_raw_fd() {
    let env = env_with_ports();
    let data = b"hello from rust".to_vec();
    let port = LispVal::wrap_reader("rust-cursor", "test", Box::new(std::io::Cursor::new(data)));
    env.set("MY-PORT".to_string(), port);
    assert_eq!(eval_line("(port-p my-port)", &env), "T");
    assert_eq!(eval_line("(input-p my-port)", &env), "T");
    assert_eq!(eval_line("(output-p my-port)", &env), "()");
    let out = eval_line("(text:utf8->string (read-all-bytes! my-port))", &env);
    assert_eq!(out, "\"hello from rust\"");
}

#[test]
fn wrap_writer_exposes_a_port_without_a_raw_fd() {
    let env = env_with_ports();
    let buf: Shared<std::cell::RefCell<Vec<u8>>> = Shared::new(std::cell::RefCell::new(Vec::new()));

    struct SharedVecWriter(Shared<std::cell::RefCell<Vec<u8>>>);
    impl std::io::Write for SharedVecWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let port = LispVal::wrap_writer("rust-sink", "test", Box::new(SharedVecWriter(buf.clone())));
    env.set("MY-SINK".to_string(), port);
    eval_line("(write-string! my-sink \"hi there\")", &env);
    assert_eq!(&*buf.borrow(), b"hi there");
}

// ── Drop backstop ────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_an_unclosed_port_still_releases_its_file_descriptor() {
    let env = env_with_ports();
    env.enable_feature("READ-FS");
    let path = temp_path("dropfd");
    std::fs::write(&path, b"x").unwrap();

    let before = open_fd_count();
    for _ in 0..200 {
        // The returned port is never bound to anything; eval_line converts
        // it to a printed String and the LispVal (and its Shared<PortObj>)
        // is dropped when the call returns. Explicit close is never called.
        eval_line(&format!("(open-input {path:?})"), &env);
    }
    let after = open_fd_count();
    assert!(
        after <= before + 5,
        "file descriptors leaked: before={before} after={after} (Drop backstop not releasing files)"
    );
    let _ = std::fs::remove_file(&path);
}

// ── Sandboxing defaults ──────────────────────────────────────────────────

#[test]
fn new_sandboxed_has_no_port_capabilities_by_default() {
    let env = Environment::new_sandboxed();
    assert!(!env.feature_enabled("READ-FS"));
    assert!(!env.feature_enabled("CREATE-FS"));
    assert!(!env.feature_enabled("IO"));
}

// ── Miscellaneous kernel-level checks (Rust-side, via eval_str) ─────────

#[test]
fn read_bytes_element_type_is_char_like_string_to_utf8() {
    let env = env_with_ports();
    // read-bytes!/read-all-bytes! should produce arrays usable directly by
    // GET-CHAR-ARRAY-BYTES (the same helper WRITE-BYTES*/TEXT use) — i.e.
    // round-trip through write-bytes! again without conversion.
    let result = eval_str(
        "(with-open-port (op (open-output-bytes))
           (write-bytes! op (list->array (list 1 2 3)))
           (with-open-port (ip (open-input-bytes (output-contents op)))
             (let ((rd (read-all-bytes! ip)))
               (with-open-port (op2 (open-output-bytes))
                 (write-bytes! op2 rd)
                 (array->list (output-contents op2))))))",
        &env,
    );
    match result {
        Ok(LispVal::Cons { .. }) | Ok(LispVal::Nil) => {}
        other => panic!("expected a list result, got {other:?}"),
    }
}

#[test]
fn read_into_reader_reads_a_std_io_read_source() {
    // Exercises PortObj::read_bytes directly is not possible from the test
    // crate (PortObj's fields are private outside the crate), so this drives
    // it end-to-end through the Lisp surface instead — a Rust `Read` wrapped
    // via WRAP_READER, read in small chunks to exercise partial reads.
    let env = env_with_ports();
    let data: Vec<u8> = (0u8..=50).collect();
    let port = LispVal::wrap_reader(
        "chunked",
        "test",
        Box::new(std::io::Cursor::new(data.clone())),
    );
    env.set("CHUNKED".to_string(), port);
    eval_line(
        "(defun $drain-chunked (acc)
           (let ((b (read-byte! chunked)))
             (if (null b) acc ($drain-chunked (cons b acc)))))",
        &env,
    );
    let out = eval_line("(length ($drain-chunked ()))", &env);
    assert_eq!(out, data.len().to_string());
}
