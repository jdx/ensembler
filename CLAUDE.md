# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Ensembler is a Rust library for executing external commands with async output handling and progress reporting. Built on Tokio, it provides line-based output capture, secret redaction, cancellation support, and integration with the `clx` progress bar crate.

## Build and Test Commands

```bash
# Full CI pipeline (clippy + tests + example)
mise run ci

# Individual commands
cargo clippy          # Lint
cargo test            # Run all tests
cargo run --example run  # Run the example

# Run a single test
cargo test test_name
```

## Architecture

The library has three source files:

- **src/lib.rs** - Public API exports (`CmdLineRunner`, `CmdResult`, `Error`, `Result`)
- **src/cmd.rs** - Core `CmdLineRunner` builder struct with fluent API for command execution
- **src/error.rs** - Error types using `thiserror`

### Key Design Patterns

- **Builder pattern**: `CmdLineRunner::new("cmd").arg("x").env("K","V").execute().await`
- **Line-based processing**: Output is read line-by-line via `BufReader`, not raw bytes
- **Concurrent I/O**: Tokio tasks independently handle stdout/stderr/stdin
- **Global PID tracking**: `RUNNING_PIDS` static `HashSet` enables `kill_all(signal)` for batch termination

### Platform Differences

- **Unix**: Uses `nix` crate for signal handling
- **Windows**: Commands wrapped with `cmd.exe /c`, uses `taskkill /F /T` for termination

## Testing

Tests are in `tests/integration.rs`. All tests are async using `#[tokio::test]`. The test suite covers basic execution, output capture, error handling, secret redaction, stdin piping, cancellation, and environment variables.

## Important Constraints

- MSRV is Rust 1.88.0 - don't use newer language features
- Never build with `--release` flag
