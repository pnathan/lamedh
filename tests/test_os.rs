//! Typed Linux/POSIX OS integration (issue #260, epic #253): the kernel
//! OS-*/OS-LINUX-* primitives (src/evaluator/builtins_os.rs, ChildObj in
//! src/lib.rs) wrapped by lib/41-os.lisp (the OS module) and
//! lib/42-os-linux.lisp (the OS-LINUX module).
//!
//! Coverage: capability gating (OS-ENV/OS-ENV-WRITE/OS-PROCESS/OS-SIGNAL
//! independently enforced) and fence attenuation, env/cwd/pid/ppid/hostname
//! read-write round trips, deterministic PRNG and secure random bytes,
//! monotonic time, process spawn (argv with no shell interpolation,
//! explicit/inherited environment, stdio pipes, wait/try-wait, kill/
//! terminate), use-after-reap rejection, structured error categories
//! (missing executable, unknown signal), the host OS policy hook, the Drop
//! backstop, and OS-LINUX file metadata/readlink.
//!
//! No external state; every filesystem operation stays inside a
//! process-unique temp directory that is cleaned up at the end of each test.

use lamedh::environment::Environment;
use lamedh::{OsOperation, Shared, eval_line, eval_str};

fn env_with_os() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn grant(env: &Shared<Environment>, caps: &[&str]) {
    for c in caps {
        env.enable_feature(c);
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lamedh-os-test-{}-{}-{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

// ── Capability gating ───────────────────────────────────────────────────

#[test]
fn os_env_reads_require_os_env() {
    let env = env_with_os();
    for form in ["(os:args)", "(os:cwd)", "(os:pid)", "(os:env-list)"] {
        let out = eval_line(form, &env);
        assert!(
            out.contains("OS-ENV capability") && out.contains("not enabled"),
            "{form} got: {out}"
        );
    }
}

#[test]
fn os_env_writes_require_os_env_write() {
    let env = env_with_os();
    env.enable_feature("OS-ENV");
    let out = eval_line("(os:env-set! \"LAMEDH_TEST_X\" \"1\")", &env);
    assert!(
        out.contains("OS-ENV-WRITE capability") && out.contains("not enabled"),
        "got: {out}"
    );
    let out2 = eval_line("(os:chdir! \"/tmp\")", &env);
    assert!(out2.contains("OS-ENV-WRITE capability"), "got: {out2}");
}

#[test]
fn os_spawn_requires_os_process() {
    let env = env_with_os();
    let out = eval_line("(os:spawn \"/bin/true\")", &env);
    assert!(
        out.contains("OS-PROCESS capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn os_signal_requires_os_signal() {
    let env = env_with_os();
    let out = eval_line("(os:signal! 1 ':term)", &env);
    assert!(
        out.contains("OS-SIGNAL capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn capabilities_are_independently_enforced() {
    let env = env_with_os();
    grant(&env, &["OS-ENV"]);
    // OS-ENV alone does not grant OS-ENV-WRITE, OS-PROCESS, or OS-SIGNAL.
    assert!(eval_line("(os:pid)", &env).parse::<i64>().is_ok());
    assert!(eval_line("(os:chdir! \"/tmp\")", &env).contains("OS-ENV-WRITE capability"));
    assert!(eval_line("(os:spawn \"/bin/true\")", &env).contains("OS-PROCESS capability"));
    assert!(eval_line("(os:signal! 1 ':term)", &env).contains("OS-SIGNAL capability"));
}

#[test]
fn fence_attenuates_os_process_even_with_cli_grant() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line("(with-capabilities '() (os:spawn \"/bin/true\"))", &env);
    assert!(
        out.contains("capability denied: OS-PROCESS") && out.contains("fence"),
        "got: {out}"
    );
}

#[test]
fn fence_attenuates_os_env_too() {
    let env = env_with_os();
    grant(&env, &["OS-ENV"]);
    let out = eval_line("(with-capabilities '() (os:pid))", &env);
    assert!(
        out.contains("capability denied: OS-ENV") && out.contains("fence"),
        "got: {out}"
    );
}

// ── Process identity / environment ──────────────────────────────────────

#[test]
fn pid_matches_the_actual_process() {
    let env = env_with_os();
    grant(&env, &["OS-ENV"]);
    let out = eval_line("(os:pid)", &env);
    let pid: u32 = out.parse().expect("pid should print as an integer");
    assert_eq!(pid, std::process::id());
}

#[test]
fn args_and_executable_path_are_nonempty() {
    let env = env_with_os();
    grant(&env, &["OS-ENV"]);
    let args = eval_line("(length (os:args))", &env);
    assert!(args.parse::<i64>().unwrap() >= 1, "got: {args}");
    let exe = eval_line("(stringp (os:executable-path))", &env);
    assert_eq!(exe, "T");
}

#[test]
fn env_get_set_unset_round_trip() {
    let env = env_with_os();
    grant(&env, &["OS-ENV", "OS-ENV-WRITE"]);
    assert_eq!(
        eval_line("(os:env-get \"LAMEDH_TEST_ROUNDTRIP\")", &env),
        "()"
    );
    eval_str("(os:env-set! \"LAMEDH_TEST_ROUNDTRIP\" \"hello\")", &env).unwrap();
    assert_eq!(
        eval_line("(os:env-get \"LAMEDH_TEST_ROUNDTRIP\")", &env),
        "\"hello\""
    );
    eval_str("(os:env-unset! \"LAMEDH_TEST_ROUNDTRIP\")", &env).unwrap();
    assert_eq!(
        eval_line("(os:env-get \"LAMEDH_TEST_ROUNDTRIP\")", &env),
        "()"
    );
}

#[test]
fn env_list_contains_a_known_var() {
    let env = env_with_os();
    grant(&env, &["OS-ENV", "OS-ENV-WRITE"]);
    eval_str("(os:env-set! \"LAMEDH_TEST_LIST\" \"present\")", &env).unwrap();
    let out = eval_line("(cdr (assoc \"LAMEDH_TEST_LIST\" (os:env-list)))", &env);
    assert_eq!(out, "\"present\"", "got: {out}");
}

#[test]
fn chdir_changes_cwd_and_is_visible_to_children() {
    let env = env_with_os();
    grant(&env, &["OS-ENV", "OS-ENV-WRITE"]);
    let dir = temp_dir("chdir");
    let before = eval_line("(os:cwd)", &env);
    eval_str(&format!("(os:chdir! {:?})", dir.to_string_lossy()), &env).unwrap();
    let after = eval_line("(os:cwd)", &env);
    assert_ne!(before, after);
    // std::fs::canonicalize both sides to tolerate /tmp -> symlink normalization.
    let after_path = after.trim_matches('"');
    assert_eq!(
        std::fs::canonicalize(after_path).unwrap(),
        std::fs::canonicalize(&dir).unwrap()
    );
    // Restore cwd so later tests in this process aren't affected (cwd is
    // process-wide, and Rust test binaries run tests on multiple threads
    // within one process).
    eval_str(&format!("(os:chdir! {:?})", before.trim_matches('"')), &env).unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ppid_and_hostname_are_available_on_linux() {
    let env = env_with_os();
    grant(&env, &["OS-ENV"]);
    let ppid = eval_line("(os:ppid)", &env);
    assert!(ppid.parse::<i64>().is_ok(), "got: {ppid}");
    let hostname = eval_line("(stringp (os:hostname))", &env);
    assert_eq!(hostname, "T");
}

// ── Time / randomness ────────────────────────────────────────────────────

#[test]
fn now_and_monotonic_are_no_capability() {
    let env = env_with_os();
    let out = eval_line("(consp (os:now))", &env);
    assert_eq!(out, "T");
    let mono = eval_line("(numberp (os:monotonic-nanos))", &env);
    assert_eq!(mono, "T");
}

#[test]
fn monotonic_elapsed_seconds_is_nonnegative_and_increases() {
    let env = env_with_os();
    let out = eval_line(
        "(let ((s (os:monotonic-nanos))) (os:sleep 10) (> (os:elapsed-seconds s) 0.0))",
        &env,
    );
    assert_eq!(out, "T", "got: {out}");
}

#[test]
fn prng_is_deterministic_given_the_same_seed() {
    let env = env_with_os();
    let a = eval_line(
        "(cdr (os:prng-next (car (os:prng-next (os:make-prng 7)))))",
        &env,
    );
    let b = eval_line(
        "(cdr (os:prng-next (car (os:prng-next (os:make-prng 7)))))",
        &env,
    );
    assert_eq!(a, b);
    let c = eval_line(
        "(cdr (os:prng-next (car (os:prng-next (os:make-prng 8)))))",
        &env,
    );
    assert_ne!(
        a, c,
        "different seeds should (overwhelmingly likely) diverge"
    );
}

#[test]
fn prng_next_does_not_mutate_the_seed_in_place() {
    let env = env_with_os();
    // Calling PRNG-NEXT on the same state twice must yield the same result
    // both times -- it's purely functional, not an implicit mutable
    // generator.
    let out = eval_line(
        "(let ((s (os:make-prng 99))) (list (equal (os:prng-next s) (os:prng-next s))))",
        &env,
    );
    assert_eq!(out, "(T)", "got: {out}");
}

#[test]
fn random_bytes_has_requested_length_and_varies() {
    let env = env_with_os();
    let len = eval_line("(array-length* (os:random-bytes 16))", &env);
    assert_eq!(len, "16");
    // The printer does not show array contents ("<array:16>" for any
    // 16-byte array), so compare with EQUAL rather than the printed form.
    let differ = eval_line(
        "(null (equal (os:random-bytes 16) (os:random-bytes 16)))",
        &env,
    );
    assert_eq!(
        differ, "T",
        "two 16-byte secure-random draws collided (astronomically unlikely)"
    );
}

// ── Process spawn / control ──────────────────────────────────────────────

#[test]
fn spawn_wait_exit_code_success() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/true\"))
                (status (os:process-wait! (os:process-handle r))))
           (list (os:exit-code status) (os:exit-success-p status)))",
        &env,
    );
    assert_eq!(out, "(0 T)", "got: {out}");
}

#[test]
fn spawn_argv_is_never_shell_interpreted() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    // An argv element containing shell metacharacters must be passed through
    // literally to the child, never interpreted -- the defining difference
    // from lib/07-shell.lisp's (shell "...") which DOES go through `sh -c`.
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/echo\" (list \"a;b|c$(echo d)\") :stdout ':pipe))
                (out (ports:read-line! (os:process-stdout r))))
           (os:process-wait! (os:process-handle r))
           out)",
        &env,
    );
    assert_eq!(out, "\"a;b|c$(echo d)\"", "got: {out}");
}

#[test]
fn spawn_stdio_pipe_round_trip_through_cat() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/cat\" () :stdin ':pipe :stdout ':pipe))
                (h (os:process-handle r))
                (sin (os:process-stdin r))
                (sout (os:process-stdout r)))
           (ports:write-string! sin \"round-trip\")
           (ports:close! sin)
           (let ((reply (ports:read-line! sout)))
             (os:process-wait! h)
             reply))",
        &env,
    );
    assert_eq!(out, "\"round-trip\"", "got: {out}");
}

#[test]
fn spawn_explicit_environment_replaces_ambient_env() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    // With :INHERIT-ENV NIL, the child's environment is exactly the :ENV
    // alist -- not this process's ambient environment.
    let out = eval_line(
        "(let* ((r (os:spawn \"/usr/bin/env\" () :inherit-env nil
                              :env (list (cons \"LAMEDH_ONLY\" \"yes\"))
                              :stdout ':pipe))
                (h (os:process-handle r))
                (sout (os:process-stdout r))
                (lines (ports:read-all-bytes! sout)))
           (os:process-wait! h)
           (text:utf8->string-lossy lines))",
        &env,
    );
    assert_eq!(
        out, "\"LAMEDH_ONLY=yes\\n\"",
        "expected exactly one env line with an empty ambient environment, got: {out}"
    );
}

#[test]
fn spawn_try_wait_is_nonblocking_then_wait_reaps() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/sleep\" (list \"0.2\")))
                (h (os:process-handle r))
                (immediate (os:process-try-wait! h)))
           (let ((status (os:process-wait! h)))
             (list (null immediate) (os:exit-success-p status))))",
        &env,
    );
    assert_eq!(out, "(T T)", "got: {out}");
}

#[test]
fn spawn_kill_terminates_with_signal() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/sleep\" (list \"10\")))
                (h (os:process-handle r)))
           (os:process-kill! h)
           (os:exit-signal (os:process-wait! h)))",
        &env,
    );
    assert_eq!(out, "9", "got: {out} (SIGKILL)");
}

#[test]
fn spawn_terminate_sends_sigterm() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/sleep\" (list \"10\")))
                (h (os:process-handle r)))
           (os:process-terminate! h)
           (os:exit-signal (os:process-wait! h)))",
        &env,
    );
    assert_eq!(out, "15", "got: {out} (SIGTERM)");
}

#[test]
fn spawn_missing_executable_is_structured_not_found_error() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(handler-case (os:spawn \"/no/such/binary-lamedh-260\")
           (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))",
        &env,
    );
    assert!(
        out.contains(":CAUGHT") && out.contains(":NOT-FOUND"),
        "got: {out}"
    );
}

#[test]
fn use_after_reap_is_structured_closed_error() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/true\")) (h (os:process-handle r)))
           (os:process-wait! h)
           (handler-case (os:process-terminate! h)
             (error (e) (list ':caught (cdr (assoc ':category (error-data e)))))))",
        &env,
    );
    assert!(
        out.contains(":CAUGHT") && out.contains(":CLOSED"),
        "got: {out}"
    );
}

#[test]
fn process_alive_p_reflects_reap_state() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/true\")) (h (os:process-handle r)))
           (let ((before (os:process-alive-p h)))
             (os:process-wait! h)
             (list (not (null before)) (os:process-alive-p h))))",
        &env,
    );
    assert_eq!(out, "(T ())", "got: {out}");
}

#[test]
fn process_p_predicate() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(let* ((r (os:spawn \"/bin/true\")))
           (os:process-wait! (os:process-handle r))
           (list (os:process-p (os:process-handle r)) (os:process-p 5)))",
        &env,
    );
    assert_eq!(out, "(T ())", "got: {out}");
}

// ── Signals ──────────────────────────────────────────────────────────────

#[test]
fn signal_unknown_name_is_structured_invalid_argument_error() {
    let env = env_with_os();
    grant(&env, &["OS-SIGNAL"]);
    let out = eval_line(
        &format!(
            "(handler-case (os:signal! {} ':not-a-real-signal)
               (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))",
            std::process::id()
        ),
        &env,
    );
    assert!(
        out.contains(":CAUGHT") && out.contains(":INVALID-ARGUMENT"),
        "got: {out}"
    );
}

#[test]
fn signal_sends_sigterm_to_an_external_pid() {
    // Spawn a process entirely outside Lisp's OS:SPAWN (a plain
    // std::process::Child), then use OS:SIGNAL! -- gated by OS-SIGNAL, not
    // OS-PROCESS, since it isn't an owned OS:CHILD handle -- to signal it.
    //
    // Build the environment BEFORE spawning the child, and give the child a
    // generous lifetime: env_with_os() loads the whole stdlib and, on a
    // heavily loaded machine with parallel test threads, used to take longer
    // than the old `sleep 10` -- the child then exited normally on its own,
    // kill(2) still succeeded on the fresh zombie (so OS:SIGNAL! returned T),
    // and wait() reported a NORMAL exit instead of SIGTERM. That made this
    // test flake under load in ANY feature configuration.
    let env = env_with_os();
    grant(&env, &["OS-SIGNAL"]);

    let mut child = std::process::Command::new("/bin/sleep")
        .arg("300")
        .spawn()
        .unwrap();
    let pid = child.id();
    let out = eval_line(&format!("(os:signal! {pid} ':term)"), &env);
    assert_eq!(out, "T", "got: {out}");

    let status = child.wait().unwrap();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(status.signal(), Some(15));
}

// ── Host OS policy hook ────────────────────────────────────────────────

#[test]
fn policy_hook_denies_spawn_even_with_capability_granted() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    env.set_os_policy(|op| !matches!(op, OsOperation::Spawn { .. }));
    let out = eval_line(
        "(handler-case (os:spawn \"/bin/true\")
           (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))",
        &env,
    );
    assert!(
        out.contains(":CAUGHT") && out.contains(":POLICY-DENIED"),
        "got: {out}"
    );
}

#[test]
fn policy_hook_can_scope_spawn_to_one_program() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    env.set_os_policy(|op| match op {
        OsOperation::Spawn { program, .. } => *program == "/bin/true",
        _ => true,
    });
    let ok = eval_line(
        "(os:exit-success-p (os:process-wait! (os:process-handle (os:spawn \"/bin/true\"))))",
        &env,
    );
    assert_eq!(ok, "T", "got: {ok}");
    let denied = eval_line(
        "(handler-case (os:spawn \"/bin/false\")
           (error (e) (cdr (assoc ':category (error-data e)))))",
        &env,
    );
    assert_eq!(denied, ":POLICY-DENIED", "got: {denied}");
}

#[test]
fn policy_hook_denies_signal_by_pid() {
    let env = env_with_os();
    grant(&env, &["OS-SIGNAL"]);
    env.set_os_policy(|op| !matches!(op, OsOperation::Signal { .. }));
    // A policy denial must fire even for a signal that would otherwise be a
    // harmless no-op-ish choice against our own PID -- SIGCHLD is ignored by
    // default, so this would be safe to actually deliver, but the policy
    // should deny it before send_signal is ever attempted.
    let out = eval_line(
        &format!(
            "(handler-case (os:signal! {} ':chld)
               (error (e) (cdr (assoc ':category (error-data e)))))",
            std::process::id()
        ),
        &env,
    );
    assert_eq!(out, ":POLICY-DENIED", "got: {out}");
}

#[test]
fn no_policy_installed_allows_by_default() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let out = eval_line(
        "(os:exit-success-p (os:process-wait! (os:process-handle (os:spawn \"/bin/true\"))))",
        &env,
    );
    assert_eq!(out, "T", "got: {out}");
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
fn dropping_an_unreaped_but_already_exited_child_does_not_leak_or_zombie() {
    let env = env_with_os();
    grant(&env, &["OS-PROCESS"]);
    let before_fds = open_fd_count();
    for _ in 0..50 {
        // Never call PROCESS-WAIT!/TRY-WAIT! -- the handle (and its stdio
        // pipe fds) is dropped as soon as this LET scope exits, exercising
        // the Drop backstop's best-effort non-blocking reap.
        eval_line(
            "(let ((r (os:spawn \"/bin/true\" () :stdout ':pipe))) (os:process-id (os:process-handle r)))",
            &env,
        );
    }
    // Give the (already near-instant) child processes a moment to actually
    // exit before asserting on fd count.
    std::thread::sleep(std::time::Duration::from_millis(200));
    let after_fds = open_fd_count();
    assert!(
        after_fds <= before_fds + 10,
        "fds leaked: before={before_fds} after={after_fds} (Drop backstop not releasing child stdio)"
    );
}

// ── OS-LINUX: file metadata / links ───────────────────────────────────────

#[test]
fn stat_reports_size_and_kind_for_a_regular_file() {
    let env = env_with_os();
    grant(&env, &["READ-FS"]);
    let dir = temp_dir("stat");
    let file = dir.join("hello.txt");
    std::fs::write(&file, b"hello world").unwrap();
    let out = eval_line(
        &format!(
            "(let ((s (os-linux:stat {:?})))
               (list (os-linux:stat-size s) (os-linux:stat-file-p s) (os-linux:stat-directory-p s)))",
            file.to_string_lossy()
        ),
        &env,
    );
    assert_eq!(out, "(11 T ())", "got: {out}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stat_directory_p_true_for_a_directory() {
    let env = env_with_os();
    grant(&env, &["READ-FS"]);
    let dir = temp_dir("statdir");
    let out = eval_line(
        &format!(
            "(os-linux:stat-directory-p (os-linux:stat {:?}))",
            dir.to_string_lossy()
        ),
        &env,
    );
    assert_eq!(out, "T", "got: {out}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stat_missing_path_is_structured_not_found_error() {
    let env = env_with_os();
    grant(&env, &["READ-FS"]);
    let out = eval_line(
        "(handler-case (os-linux:stat \"/no/such/path-lamedh-260\")
           (error (e) (cdr (assoc ':category (error-data e)))))",
        &env,
    );
    assert_eq!(out, ":NOT-FOUND", "got: {out}");
}

#[test]
fn stat_requires_read_fs() {
    let env = env_with_os();
    let out = eval_line("(os-linux:stat \"/tmp\")", &env);
    assert!(out.contains("READ-FS capability"), "got: {out}");
}

#[test]
fn readlink_resolves_a_symlink_target() {
    let env = env_with_os();
    grant(&env, &["READ-FS"]);
    let dir = temp_dir("readlink");
    let target = dir.join("target.txt");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.join("link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let out = eval_line(
        &format!("(os-linux:readlink {:?})", link.to_string_lossy()),
        &env,
    );
    assert_eq!(
        out,
        format!("{:?}", target.to_string_lossy().into_owned()),
        "got: {out}"
    );
    // LSTAT sees the link itself, not its target.
    let is_symlink = eval_line(
        &format!(
            "(os-linux:stat-symlink-p (os-linux:lstat {:?}))",
            link.to_string_lossy()
        ),
        &env,
    );
    assert_eq!(is_symlink, "T");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn readlink_on_a_non_symlink_is_a_structured_error() {
    let env = env_with_os();
    grant(&env, &["READ-FS"]);
    let dir = temp_dir("readlink-notlink");
    let file = dir.join("plain.txt");
    std::fs::write(&file, b"x").unwrap();
    let out = eval_line(
        &format!(
            "(handler-case (os-linux:readlink {:?})
               (error (e) (cdr (assoc ':category (error-data e)))))",
            file.to_string_lossy()
        ),
        &env,
    );
    assert_eq!(out, ":INVALID-ARGUMENT", "got: {out}");
    std::fs::remove_dir_all(&dir).ok();
}

// ── Module surface ────────────────────────────────────────────────────────

#[test]
fn os_and_os_linux_are_requireable_from_prelude() {
    let env = Environment::with_prelude();
    assert!(!env.is_bound("OS:PID"));
    assert_eq!(eval_line("(require 'os)", &env), "OS");
    assert!(env.is_bound("OS:PID"));
    assert_eq!(eval_line("(require 'os-linux)", &env), "OS-LINUX");
    assert!(env.is_bound("OS-LINUX:STAT"));
}
