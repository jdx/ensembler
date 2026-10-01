# ensembler

A Rust library for executing external commands with advanced output handling and progress reporting.

## Features

- **Async execution** - Built on Tokio for non-blocking command execution
- **Output capture** - Capture stdout, stderr, and combined output
- **Progress integration** - Real-time progress bar updates via the `clx` crate
- **Secret redaction** - Automatically redact sensitive data from output
- **Cancellation** - Cancel running commands via `CancellationToken`
- **Interactive commands** - Run TUI programs that need the caller's terminal
- **Cross-platform** - Works on Unix and Windows

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ensembler = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Usage

> **Note:** Examples use Unix commands for clarity. On Windows, substitute equivalent
> commands (e.g., `dir` for `ls`, `type` for `cat`, `timeout` for `sleep`).

### Basic Command Execution

```rust
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let result = CmdLineRunner::new("echo")
        .arg("hello")
        .execute()
        .await?;

    println!("stdout: {}", result.stdout);
    Ok(())
}
```

### Capturing Output

```rust
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let result = CmdLineRunner::new("ls")
        .arg("-la")
        .current_dir("/tmp")
        .execute()
        .await?;

    println!("stdout: {}", result.stdout);
    println!("stderr: {}", result.stderr);
    println!("exit code: {:?}", result.status.code());
    Ok(())
}
```

### Redacting Secrets

Automatically hide sensitive data in command output:

```rust
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let api_key = "super-secret-key";

    let result = CmdLineRunner::new("echo")
        .arg(api_key)
        .redact(vec![api_key.to_string()])
        .execute()
        .await?;

    // Output will show "[redacted]" instead of the actual key
    assert!(!result.stdout.contains(api_key));
    Ok(())
}
```

### Piping Input via Stdin

```rust
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let result = CmdLineRunner::new("cat")
        .stdin_string("hello from stdin")
        .execute()
        .await?;

    assert_eq!(result.stdout.trim(), "hello from stdin");
    Ok(())
}
```

### Interactive Commands

TUI programs such as `vim`, `helix`, and `fzf` need the caller's terminal.
Enable interactive mode to inherit stdin, stdout, and stderr:

```rust,no_run
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    CmdLineRunner::new("vim")
        .arg("README.md")
        .interactive(true)
        .execute()
        .await?;
    Ok(())
}
```

The child's output is not captured or redacted,
so the returned output fields are empty.
On Unix the child also stays in the caller's process group
(normally commands get their own),
because a background process group can't use raw mode or the alternate screen.
Timeout and cancellation then kill only the direct child, not its descendants.

### Cancellation

Cancel long-running commands:

```rust
use ensembler::CmdLineRunner;
use tokio_util::sync::CancellationToken;
use std::time::Duration;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Cancel after 1 second
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        cancel_clone.cancel();
    });

    let result = CmdLineRunner::new("sleep")
        .arg("60")
        .with_cancel_token(cancel)
        .execute()
        .await;

    // Command was cancelled
    assert!(result.is_err());
    Ok(())
}
```

### Environment Variables

```rust
use ensembler::CmdLineRunner;

#[tokio::main]
async fn main() -> ensembler::Result<()> {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("echo $MY_VAR")
        .env("MY_VAR", "hello")
        .execute()
        .await?;

    assert_eq!(result.stdout.trim(), "hello");
    Ok(())
}
```

### Error Handling

```rust
use ensembler::{CmdLineRunner, Error};

#[tokio::main]
async fn main() {
    let result = CmdLineRunner::new("bash")
        .arg("-c")
        .arg("exit 42")
        .execute()
        .await;

    match result {
        Ok(_) => println!("Command succeeded"),
        Err(Error::ScriptFailed(details)) => {
            let (program, _args, output, cmd_result) = *details;
            println!("Command '{}' failed with exit code {:?}",
                program, cmd_result.status.code());
            println!("Output: {}", output);
        }
        Err(Error::Io(e)) => println!("IO error: {}", e),
        Err(e) => println!("Other error: {}", e),
    }
}
```

## License

MIT
