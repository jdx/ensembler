use ensembler::{CmdLineRunner, CmdResult, Error};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::io::IsTerminal;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::process::CommandExt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::{Command, Stdio};
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

#[tokio::test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn test_interactive_terminal_control() {
    const DRIVER: &str = "ENSEMBLER_INTERACTIVE_TEST_DRIVER";
    const CHILD: &str = "ENSEMBLER_INTERACTIVE_TEST_CHILD";
    const BACKGROUND: &str = "ENSEMBLER_INTERACTIVE_TEST_BACKGROUND";

    if let Ok(mode) = std::env::var(CHILD) {
        assert!(std::io::stdin().is_terminal());
        assert!(std::io::stdout().is_terminal());
        assert!(std::io::stderr().is_terminal());
        assert_eq!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            nix::unistd::getpgrp()
        );
        if mode == "failure" {
            panic!("intentional interactive child failure");
        }
        return;
    }

    if std::env::var_os(BACKGROUND).is_some() {
        assert_ne!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            nix::unistd::getpgrp()
        );
        let result = CmdLineRunner::new("true").interactive(true).execute().await;
        let Err(Error::Io(error)) = result else {
            panic!("Expected an I/O error, got {result:?}");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("foreground process group"));
        return;
    }

    if std::env::var_os(DRIVER).is_some() {
        let executable = std::env::current_exe().unwrap();

        let background_status = Command::new(&executable)
            .args([
                "--exact",
                "test_interactive_terminal_control",
                "--nocapture",
            ])
            .env(BACKGROUND, "1")
            .process_group(0)
            .status()
            .unwrap();
        assert!(
            background_status.success(),
            "background process test failed: {background_status}"
        );

        let result = CmdLineRunner::new(&executable)
            .args([
                "--exact",
                "test_interactive_terminal_control",
                "--nocapture",
            ])
            .stdin_string("ignored")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .redact(["ignored".to_string()])
            .env(CHILD, "success")
            .interactive(true)
            .execute()
            .await
            .unwrap();
        assert!(result.status.success());
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
        assert!(result.combined_output.is_empty());
        assert_eq!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            nix::unistd::getpgrp()
        );

        let result = CmdLineRunner::new(&executable)
            .args([
                "--exact",
                "test_interactive_terminal_control",
                "--nocapture",
            ])
            .env(CHILD, "failure")
            .interactive(true)
            .execute()
            .await;
        let Err(Error::ScriptFailed(details)) = result else {
            panic!("Expected ScriptFailed error, got {result:?}");
        };
        assert!(details.2.is_empty());
        assert!(details.3.stdout.is_empty());
        assert!(details.3.stderr.is_empty());
        assert!(details.3.combined_output.is_empty());
        assert_eq!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            nix::unistd::getpgrp()
        );

        let foreground_pgid = nix::unistd::getpgrp();
        let task = tokio::spawn(async {
            CmdLineRunner::new("sleep")
                .arg("10")
                .interactive(true)
                .execute()
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if nix::unistd::tcgetpgrp(std::io::stdin()).unwrap() != foreground_pgid {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            foreground_pgid
        );
        CmdLineRunner::kill_all(nix::sys::signal::Signal::SIGKILL);

        let result = CmdLineRunner::new("sleep")
            .arg("10")
            .interactive(true)
            .timeout(Duration::from_millis(100))
            .execute()
            .await;
        assert!(matches!(result, Err(Error::TimedOut)));
        assert_eq!(
            nix::unistd::tcgetpgrp(std::io::stdin()).unwrap(),
            nix::unistd::getpgrp()
        );
        return;
    }

    let executable = std::env::current_exe().unwrap();
    let mut script = Command::new("script");
    script.env(DRIVER, "1");

    #[cfg(target_os = "macos")]
    script.args(["-q", "/dev/null"]).arg(&executable).args([
        "--exact",
        "test_interactive_terminal_control",
        "--nocapture",
    ]);

    #[cfg(target_os = "linux")]
    {
        let executable = executable.to_string_lossy().replace('\'', "'\\''");
        let command =
            format!("'{executable}' --exact test_interactive_terminal_control --nocapture");
        script.args(["--quiet", "--return", "--command", &command, "/dev/null"]);
    }

    let status = script.status().unwrap();
    assert!(status.success(), "pseudo-terminal test failed: {status}");
}
