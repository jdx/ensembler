use thiserror::Error;

use crate::cmd::CmdResult;

/// Errors that can occur when executing commands.
#[derive(Error, Debug)]
pub enum Error {
    /// An I/O error occurred (e.g., command not found, permission denied).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Failed to join paths when setting up the command environment.
    #[error(transparent)]
    JoinPaths(#[from] std::env::JoinPathsError),

    /// A Unix-specific error occurred.
    #[cfg(unix)]
    #[error(transparent)]
    Nix(#[from] nix::errno::Errno),

    /// The command exited with a non-zero status code.
    ///
    /// Contains the program name, arguments, combined output, and result.
    #[error("{} exited with non-zero status: {}\n{}", .0.0, render_exit_status(&.0.3), .0.2)]
    ScriptFailed(Box<(String, Vec<String>, String, CmdResult)>),

    #[error("internal error: {0}")]
    Internal(String),
}

/// A specialized Result type for ensembler operations.
pub type Result<T> = std::result::Result<T, Error>;

fn render_exit_status(result: &CmdResult) -> String {
    match result.status.code() {
        Some(exit_status) => format!("exit code {exit_status}"),
        None => "no exit status".into(),
    }
}
