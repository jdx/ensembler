use ensembler::{CmdLineRunner, CmdResult, Error};
use std::time::Duration;
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

    assert!(
        matches!(result, Err(Error::Io(_))),
        "Expected Io error, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_empty_output() {
    let result = CmdLineRunner::new("true").execute().await.unwrap();

    assert!(result.status.success());
    assert_eq!(result.stdout, "");
    assert_eq!(result.stderr, "");
}

#[tokio::test]
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
