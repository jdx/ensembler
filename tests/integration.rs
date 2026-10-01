use ensembler::{CmdLineRunner, CmdResult, Error};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_basic_execution() {
    let result = CmdLineRunner::new("echo")
        .arg("hello")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "hello");
}

#[tokio::test]
async fn test_multiple_args() {
    let result = CmdLineRunner::new("echo")
        .args(["hello", "world"])
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "hello world");
}

#[tokio::test]
#[cfg(unix)]
async fn test_stdout_capture() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo line1; echo line2; echo line3")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout, "line1\nline2\nline3\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_stdout_keeps_carriage_returns() {
    let result = CmdLineRunner::new("printf")
        .arg("one\\r\\ntwo\\nthree\\r\\n")
        .execute()
        .await
        .unwrap();

    assert_eq!(result.stdout, "one\r\ntwo\nthree\r\n");
    assert_eq!(result.combined_output, "one\r\ntwo\nthree\r\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_stderr_keeps_carriage_returns() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("printf 'one\\r\\ntwo\\n' >&2")
        .execute()
        .await
        .unwrap();

    assert_eq!(result.stderr, "one\r\ntwo\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_output_after_invalid_utf8_is_kept() {
    let result = CmdLineRunner::new("printf")
        .arg("bad \\377\\nafter\\n")
        .execute()
        .await
        .unwrap();

    assert_eq!(result.stdout, "bad \u{FFFD}\nafter\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_failure_output_keeps_carriage_returns() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("printf 'one\\r\\ntwo\\r\\n'; exit 1")
        .execute()
        .await;

    let Err(Error::ScriptFailed(details)) = result else {
        panic!("Expected ScriptFailed error, got {result:?}");
    };
    let (_program, _args, output, cmd_result) = *details;
    assert_eq!(output, "one\r\ntwo");
    assert_eq!(cmd_result.stdout, "one\r\ntwo\r\n");
}

#[tokio::test]
#[cfg(windows)]
async fn test_stdout_keeps_carriage_returns_on_windows() {
    // cmd's echo ends each line with \r\n.
    let result = CmdLineRunner::new("cmd")
        .args(["/C", "echo one& echo two"])
        .execute()
        .await
        .unwrap();

    assert_eq!(result.stdout, "one\r\ntwo\r\n");
}

#[tokio::test]
#[cfg(windows)]
async fn test_failure_output_keeps_carriage_returns_on_windows() {
    let result = CmdLineRunner::new("cmd")
        .args(["/C", "echo one& echo two& exit /b 1"])
        .execute()
        .await;

    let Err(Error::ScriptFailed(details)) = result else {
        panic!("Expected ScriptFailed error, got {result:?}");
    };
    let (_program, _args, output, cmd_result) = *details;
    assert_eq!(output, "one\r\ntwo");
    assert_eq!(cmd_result.stdout, "one\r\ntwo\r\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_stderr_capture() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo error >&2")
        .execute()
        .await
        .unwrap();

    // Command succeeds but has stderr output
    assert!(result.status.success());
    assert_eq!(result.stderr.trim(), "error");
}

#[tokio::test]
#[cfg(unix)]
async fn test_combined_output() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo stdout; echo stderr >&2")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert!(result.combined_output.contains("stdout"));
    assert!(result.combined_output.contains("stderr"));
}

#[tokio::test]
#[cfg(unix)]
async fn test_exit_code_failure() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("exit 42")
        .execute()
        .await;

    if let Err(Error::ScriptFailed(details)) = result {
        let (program, _args, _output, cmd_result) = *details;
        assert_eq!(program, "bash");
        assert_eq!(cmd_result.status.code(), Some(42));
    } else {
        panic!("Expected ScriptFailed error, got {:?}", result);
    }
}

#[tokio::test]
async fn test_redaction_stdout() {
    let result = CmdLineRunner::new("echo")
        .arg("my-secret-password")
        .redact(vec!["my-secret-password".to_string()])
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "[redacted]");
    assert!(!result.stdout.contains("my-secret-password"));
}

#[tokio::test]
#[cfg(unix)] // Windows echo includes quotes around args with spaces
async fn test_redaction_multiple() {
    let result = CmdLineRunner::new("echo")
        .arg("secret1 and secret2")
        .redact(vec!["secret1".to_string(), "secret2".to_string()])
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "[redacted] and [redacted]");
}

#[tokio::test]
#[cfg(unix)]
async fn test_redaction_stderr() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo my-api-key >&2")
        .redact(vec!["my-api-key".to_string()])
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stderr.trim(), "[redacted]");
    assert!(!result.stderr.contains("my-api-key"));
}

#[tokio::test]
#[cfg(unix)]
async fn test_environment_variable() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo $MY_TEST_VAR")
        .env("MY_TEST_VAR", "test_value")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "test_value");
}

#[tokio::test]
#[cfg(unix)]
async fn test_environment_multiple() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo $VAR1 $VAR2")
        .envs([("VAR1", "first"), ("VAR2", "second")])
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "first second");
}

#[tokio::test]
#[cfg(unix)]
async fn test_current_dir() {
    let result = CmdLineRunner::new("pwd")
        .current_dir("/tmp")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    // On macOS, /tmp is a symlink to /private/tmp
    assert!(
        result.stdout.trim() == "/tmp" || result.stdout.trim() == "/private/tmp",
        "Expected /tmp or /private/tmp, got {}",
        result.stdout.trim()
    );
}

#[tokio::test]
#[cfg(unix)]
async fn test_stdin_string() {
    let result = CmdLineRunner::new("cat")
        .stdin_string("hello from stdin")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "hello from stdin");
}

#[tokio::test]
#[cfg(unix)]
async fn test_stdin_multiline() {
    let result = CmdLineRunner::new("cat")
        .stdin_string("line1\nline2\nline3")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout, "line1\nline2\nline3\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_cancellation() {
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Spawn task that will cancel after a short delay
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel_clone.cancel();
    });

    let result = CmdLineRunner::new("sleep")
        .arg("10")
        .with_cancel_token(cancel)
        .execute()
        .await;

    // The command should have been cancelled with specific error type
    assert!(
        matches!(result, Err(Error::Cancelled)),
        "Expected Cancelled error, got {:?}",
        result
    );
}

#[tokio::test]
#[cfg(unix)]
async fn test_opt_arg_some() {
    let result = CmdLineRunner::new("echo")
        .opt_arg(Some("-n"))
        .arg("no_newline")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    // Note: The library uses line-based reading which adds a newline after each line.
    // Even though `echo -n` suppresses the trailing newline, the library's line reader
    // adds one back. This is expected behavior for line-based output capture.
    assert_eq!(result.stdout, "no_newline\n");
}

#[tokio::test]
async fn test_opt_arg_none() {
    let result = CmdLineRunner::new("echo")
        .opt_arg(None::<&str>)
        .arg("with_newline")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "with_newline");
}

#[tokio::test]
async fn test_command_not_found() {
    let result = CmdLineRunner::new("nonexistent_command_xyz123")
        .execute()
        .await;

    // On Windows with cmd.exe wrapping, this may be ScriptFailed instead of Io
    assert!(
        matches!(result, Err(Error::Io(_)) | Err(Error::ScriptFailed(_))),
        "Expected Io or ScriptFailed error, got {:?}",
        result
    );
}

#[tokio::test]
#[cfg(unix)]
async fn test_empty_output() {
    let result = CmdLineRunner::new("true").execute().await.unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
}

#[tokio::test]
#[cfg(unix)]
async fn test_large_output() {
    // Generate 1000 lines of output
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("for i in $(seq 1 1000); do echo \"line $i\"; done")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    let line_count = result.stdout.lines().count();
    assert_eq!(line_count, 1000);
}

#[tokio::test]
async fn test_display_format() {
    let runner = CmdLineRunner::new("echo").arg("hello").arg("world");
    let display = format!("{}", runner);
    assert_eq!(display, "echo hello world");
}

#[tokio::test]
async fn test_debug_format() {
    let runner = CmdLineRunner::new("echo").arg("hello").arg("world");
    let debug = format!("{:?}", runner);
    assert_eq!(debug, "echo hello world");
}

#[tokio::test]
async fn test_cmd_result_default() {
    let result = CmdResult::default();
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
    assert_eq!(result.combined_output, "");
}

#[tokio::test]
#[cfg(unix)]
async fn test_error_message_contains_program_name() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("exit 1")
        .execute()
        .await;

    let error_msg = format!("{}", result.unwrap_err());
    assert!(error_msg.contains("bash"));
    assert!(error_msg.contains("exit code 1"));
}

#[tokio::test]
#[cfg(unix)] // Windows echo includes quotes around args with spaces
async fn test_special_characters_in_args() {
    let result = CmdLineRunner::new("echo")
        .arg("hello world")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "hello world");
}

#[tokio::test]
#[cfg(unix)]
async fn test_newlines_in_output() {
    let result = CmdLineRunner::new("printf")
        .arg("a\nb\nc")
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    // Note: printf outputs "a\nb\nc" without trailing newline, but the library's
    // line-based reader adds a newline after the last line. This is expected behavior.
    assert_eq!(result.stdout, "a\nb\nc\n");
}

#[tokio::test]
#[cfg(unix)]
async fn test_allow_non_zero() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo 'output'; exit 42")
        .allow_non_zero(true)
        .execute()
        .await
        .unwrap();

    // Command returned Ok even though exit code was non-zero
    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout.trim(), "output");
}

#[tokio::test]
#[cfg(unix)]
async fn test_allow_non_zero_false() {
    // Default behavior: non-zero exit is an error
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("exit 1")
        .allow_non_zero(false)
        .execute()
        .await;

    assert!(matches!(result, Err(Error::ScriptFailed(_))));
}

#[tokio::test]
#[cfg(unix)]
async fn test_timeout() {
    let start = Instant::now();
    let result = CmdLineRunner::new("sleep")
        .arg("10")
        .timeout(Duration::from_millis(100))
        .execute()
        .await;

    let elapsed = start.elapsed();

    // Should have timed out with specific error type
    assert!(
        matches!(result, Err(Error::TimedOut)),
        "Expected TimedOut error, got {:?}",
        result
    );

    // Should have returned quickly (well under 10 seconds)
    assert!(elapsed < Duration::from_secs(1));
}

#[tokio::test]
async fn test_timeout_not_reached() {
    // Command completes before timeout
    let result = CmdLineRunner::new("echo")
        .arg("fast")
        .timeout(Duration::from_secs(10))
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout.trim(), "fast");
}

#[cfg(unix)]
fn pgid_of(pid: &str) -> String {
    let out = std::process::Command::new("ps")
        .args(["-o", "pgid=", "-p", pid])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_stays_in_callers_process_group() {
    let dir = std::env::temp_dir().join(format!("ensembler-interactive-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let interactive_out = dir.join("interactive");
    let piped_out = dir.join("piped");

    // The path is passed as $1 so a TMPDIR with spaces or metacharacters is safe.
    let script = r#"ps -o pgid= -p $$ > "$1""#;
    CmdLineRunner::new("sh")
        .args(["-c", script, "sh"])
        .arg(&interactive_out)
        .interactive(true)
        .execute()
        .await
        .unwrap();
    CmdLineRunner::new("sh")
        .args(["-c", script, "sh"])
        .arg(&piped_out)
        .execute()
        .await
        .unwrap();

    let ours = pgid_of(&std::process::id().to_string());
    let read = |p: &std::path::Path| std::fs::read_to_string(p).unwrap().trim().to_string();
    assert_eq!(read(&interactive_out), ours);
    assert_ne!(read(&piped_out), ours);
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_does_not_capture_output_or_pipe_stdin() {
    // The child writes to both streams, so captured output would show up in the
    // result. Had `stdin_string` stayed piped, the runner would fail for lack of a
    // stdin handle.
    let result = CmdLineRunner::new("sh")
        .arg("-c")
        .arg("echo out; echo err >&2")
        .stdin_string("ignored")
        .interactive(true)
        .execute()
        .await
        .unwrap();

    assert!(result.status.success());
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
    assert!(result.combined_output.is_empty());
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_cancellation_kills_child() {
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        trigger.cancel();
    });

    let start = Instant::now();
    let result = CmdLineRunner::new("sleep")
        .arg("30")
        .interactive(true)
        .with_cancel_token(cancel)
        .execute()
        .await;

    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_timeout_kills_child() {
    let start = Instant::now();
    let result = CmdLineRunner::new("sleep")
        .arg("30")
        .interactive(true)
        .timeout(Duration::from_millis(100))
        .execute()
        .await;

    assert!(matches!(result, Err(Error::TimedOut)));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_cancellation_lets_child_clean_up() {
    let dir = std::env::temp_dir().join(format!(
        "ensembler-interactive-term-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let ready = dir.join("ready");
    let marker = dir.join("cleaned-up");

    // Like a TUI restoring the terminal on SIGTERM; SIGKILL would skip the trap.
    // It reports in only once the trap is installed, so the signal can't beat it.
    let script = r#"trap 'echo cleaned > "$2"; exit 0' TERM; echo ready > "$1"; while :; do sleep 0.1; done"#;
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let watched = ready.clone();
    tokio::spawn(async move {
        let start = Instant::now();
        while !watched.exists() && start.elapsed() < Duration::from_secs(10) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        trigger.cancel();
    });
    let result = CmdLineRunner::new("sh")
        .args(["-c", script, "sh"])
        .arg(&ready)
        .arg(&marker)
        .interactive(true)
        .with_cancel_token(cancel)
        .execute()
        .await;

    assert!(matches!(result, Err(Error::Cancelled)));
    assert!(marker.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
#[cfg(unix)]
async fn test_interactive_timeout_kills_child_that_ignores_sigterm() {
    let start = Instant::now();
    let result = CmdLineRunner::new("sh")
        .args(["-c", "trap '' TERM; while :; do sleep 0.1; done"])
        .interactive(true)
        .timeout(Duration::from_millis(100))
        .execute()
        .await;

    assert!(matches!(result, Err(Error::TimedOut)));
    assert!(start.elapsed() < Duration::from_secs(10));
}
