//! # ensembler
//!
//! A library for executing external commands with advanced output handling
//! and progress reporting capabilities.
//!
//! ## Features
//!
//! - **Async execution** - Built on Tokio for non-blocking command execution
//! - **Output capture** - Capture stdout, stderr, and combined output
//! - **Progress integration** - Real-time progress bar updates via the `clx` crate
//! - **Secret redaction** - Automatically redact sensitive data from output
//! - **Cancellation** - Support for cancelling running commands via `CancellationToken`
//! - **Cross-platform** - Works on Unix and Windows
//!
//! ## Basic Usage
//!
//! ```no_run
//! use ensembler::CmdLineRunner;
//!
//! #[tokio::main]
//! async fn main() -> ensembler::Result<()> {
//!     let result = CmdLineRunner::new("echo")
//!         .arg("hello")
//!         .execute()
//!         .await?;
//!
//!     println!("stdout: {}", result.stdout);
//!     Ok(())
//! }
//! ```
//!
//! ## Redacting Secrets
//!
//! Sensitive data can be automatically redacted from command output:
//!
//! ```no_run
//! use ensembler::CmdLineRunner;
//!
//! #[tokio::main]
//! async fn main() -> ensembler::Result<()> {
//!     let api_key = "super-secret-key";
//!     let result = CmdLineRunner::new("echo")
//!         .arg(api_key)
//!         .redact(vec![api_key.to_string()])
//!         .execute()
//!         .await?;
//!
//!     // stdout will contain "[redacted]" instead of the actual key
//!     assert!(!result.stdout.contains(api_key));
//!     Ok(())
//! }
//! ```
//!
//! ## Stdin Input
//!
//! Pipe input to a command's stdin:
//!
//! ```no_run
//! use ensembler::CmdLineRunner;
//!
//! #[tokio::main]
//! async fn main() -> ensembler::Result<()> {
//!     let result = CmdLineRunner::new("cat")
//!         .stdin_string("hello from stdin")
//!         .execute()
//!         .await?;
//!
//!     assert_eq!(result.stdout.trim(), "hello from stdin");
//!     Ok(())
//! }
//! ```
//!
//! ## Cancellation
//!
//! Commands can be cancelled using a `CancellationToken`:
//!
//! ```no_run
//! use ensembler::CmdLineRunner;
//! use tokio_util::sync::CancellationToken;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> ensembler::Result<()> {
//!     let cancel = CancellationToken::new();
//!     let cancel_clone = cancel.clone();
//!
//!     // Cancel after 1 second
//!     tokio::spawn(async move {
//!         tokio::time::sleep(Duration::from_secs(1)).await;
//!         cancel_clone.cancel();
//!     });
//!
//!     let result = CmdLineRunner::new("sleep")
//!         .arg("60")
//!         .with_cancel_token(cancel)
//!         .execute()
//!         .await;
//!
//!     // Command was cancelled
//!     assert!(result.is_err());
//!     Ok(())
//! }
//! ```

#[macro_use]
extern crate log;
mod cmd;
mod error;

pub use cmd::{CmdLineRunner, CmdResult};
pub use error::{Error, Result};
