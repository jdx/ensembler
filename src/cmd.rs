use crate::Result;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter};
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::{
    io::BufReader,
    process::Command,
    select,
    sync::{oneshot, Mutex},
};
use tokio_util::sync::CancellationToken;

use indexmap::IndexSet;
use std::sync::LazyLock as Lazy;

use crate::Error::ScriptFailed;
use clx::progress::{self, ProgressJob};

/// A builder for executing external commands with advanced output handling.
///
/// `CmdLineRunner` provides a fluent API for configuring and executing external
/// commands. It supports output capture, secret redaction, progress bar integration,
/// and cancellation.
///
/// # Example
///
/// ```no_run
/// use ensembler::CmdLineRunner;
///
/// #[tokio::main]
/// async fn main() -> ensembler::Result<()> {
///     let result = CmdLineRunner::new("ls")
///         .arg("-la")
///         .current_dir("/tmp")
///         .execute()
///         .await?;
///
///     println!("{}", result.stdout);
///     Ok(())
/// }
/// ```
pub struct CmdLineRunner {
    cmd: Command,
    program: String,
    args: Vec<String>,
    pr: Option<Arc<ProgressJob>>,
    stdin: Option<String>,
    redactions: IndexSet<String>,
    pass_signals: bool,
    show_stderr_on_error: bool,
    stderr_to_progress: bool,
    cancel: CancellationToken,
}

static RUNNING_PIDS: Lazy<std::sync::Mutex<HashSet<u32>>> = Lazy::new(Default::default);

impl CmdLineRunner {
    /// Creates a new command runner for the given program.
    ///
    /// On Windows, commands are automatically wrapped with `cmd.exe /c`.
    /// The command is configured with piped stdout/stderr and null stdin by default.
    pub fn new<P: AsRef<OsStr>>(program: P) -> Self {
        let program = program.as_ref().to_string_lossy().to_string();
        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg(&program);
            cmd
        } else {
            Command::new(&program)
        };
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        Self {
            cmd,
            program,
            args: vec![],
            pr: None,
            stdin: None,
            redactions: Default::default(),
            pass_signals: false,
            show_stderr_on_error: true,
            stderr_to_progress: false,
            cancel: CancellationToken::new(),
        }
    }

    /// Sends a signal to all running child processes.
    ///
    /// This is useful for graceful shutdown scenarios where you need to
    /// terminate all spawned processes.
    #[cfg(unix)]
    pub fn kill_all(signal: nix::sys::signal::Signal) {
        let Ok(pids) = RUNNING_PIDS.lock() else {
            debug!("Failed to acquire lock on RUNNING_PIDS");
            return;
        };
        for pid in pids.iter() {
            let pid = *pid as i32;
            trace!("{signal}: {pid}");
            if let Err(e) = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), signal) {
                debug!("Failed to kill cmd {pid}: {e}");
            }
        }
    }

    /// Terminates all running child processes on Windows.
    ///
    /// Uses `taskkill /F /T` to forcefully terminate process trees.
    #[cfg(windows)]
    pub fn kill_all() {
        let Ok(pids) = RUNNING_PIDS.lock() else {
            debug!("Failed to acquire lock on RUNNING_PIDS");
            return;
        };
        for pid in pids.iter() {
            if let Err(e) = Command::new("taskkill")
                .arg("/F")
                .arg("/T")
                .arg("/PID")
                .arg(pid.to_string())
                .spawn()
            {
                warn!("Failed to kill cmd {pid}: {e}");
            }
        }
    }

    /// Configures stdin handling for the command.
    pub fn stdin<T: Into<Stdio>>(mut self, cfg: T) -> Self {
        self.cmd.stdin(cfg);
        self
    }

    /// Configures stdout handling for the command.
    pub fn stdout<T: Into<Stdio>>(mut self, cfg: T) -> Self {
        self.cmd.stdout(cfg);
        self
    }

    /// Configures stderr handling for the command.
    pub fn stderr<T: Into<Stdio>>(mut self, cfg: T) -> Self {
        self.cmd.stderr(cfg);
        self
    }

    /// Adds strings to redact from command output.
    ///
    /// Any occurrence of these strings in stdout or stderr will be replaced
    /// with `[redacted]`. This is useful for hiding sensitive data like
    /// API keys or passwords.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ensembler::CmdLineRunner;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> ensembler::Result<()> {
    /// let result = CmdLineRunner::new("echo")
    ///     .arg("secret-api-key")
    ///     .redact(vec!["secret-api-key".to_string()])
    ///     .execute()
    ///     .await?;
    ///
    /// assert_eq!(result.stdout.trim(), "[redacted]");
    /// # Ok(())
    /// # }
    /// ```
    pub fn redact(mut self, redactions: impl IntoIterator<Item = String>) -> Self {
        for r in redactions {
            self.redactions.insert(r);
        }
        self
    }

    /// Attaches a progress bar to display command status.
    ///
    /// The progress bar will be updated with the command being run and
    /// its output. Uses the `clx` crate's progress bar system.
    pub fn with_pr(mut self, pr: Arc<ProgressJob>) -> Self {
        self.pr = Some(pr);
        self
    }

    /// Sets a cancellation token for the command.
    ///
    /// When the token is cancelled, the running process will be killed.
    pub fn with_cancel_token(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }

    /// Controls whether stderr is displayed when the command fails.
    ///
    /// Defaults to `true`.
    pub fn show_stderr_on_error(mut self, show: bool) -> Self {
        self.show_stderr_on_error = show;
        self
    }

    /// Routes stderr to the progress bar instead of printing it directly.
    ///
    /// When enabled, stderr lines update the progress bar's status.
    /// When disabled (default), stderr is printed above the progress bar.
    pub fn stderr_to_progress(mut self, enable: bool) -> Self {
        self.stderr_to_progress = enable;
        self
    }

    /// Sets the working directory for the command.
    pub fn current_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.cmd.current_dir(dir);
        self
    }

    /// Clears all environment variables for the command.
    pub fn env_clear(mut self) -> Self {
        self.cmd.env_clear();
        self
    }

    /// Sets an environment variable for the command.
    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.cmd.env(key, val);
        self
    }

    /// Sets multiple environment variables for the command.
    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.cmd.envs(vars);
        self
    }

    /// Adds an optional argument to the command.
    ///
    /// If `arg` is `None`, no argument is added.
    pub fn opt_arg<S: AsRef<OsStr>>(mut self, arg: Option<S>) -> Self {
        if let Some(arg) = arg {
            self.cmd.arg(arg);
        }
        self
    }

    /// Adds a single argument to the command.
    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.cmd.arg(arg.as_ref());
        self.args.push(arg.as_ref().to_string_lossy().to_string());
        self
    }

    /// Adds multiple arguments to the command.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = args
            .into_iter()
            .map(|s| s.as_ref().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        self.cmd.args(&args);
        self.args.extend(args);
        self
    }

    /// Enables passing signals to the child process.
    ///
    /// Note: This feature is not yet implemented.
    pub fn with_pass_signals(&mut self) -> &mut Self {
        self.pass_signals = true;
        self
    }

    /// Pipes a string to the command's stdin.
    ///
    /// This automatically configures stdin to be piped.
    pub fn stdin_string(mut self, input: impl Into<String>) -> Self {
        self.cmd.stdin(Stdio::piped());
        self.stdin = Some(input.into());
        self
    }

    /// Executes the command and waits for it to complete.
    ///
    /// Returns [`CmdResult`] containing captured stdout, stderr, and exit status
    /// on success. Returns an error if the command fails to start or exits with
    /// a non-zero status.
    ///
    /// # Errors
    ///
    /// - [`Error::Io`] if the command fails to start
    /// - [`Error::ScriptFailed`] if the command exits with a non-zero status
    pub async fn execute(mut self) -> Result<CmdResult> {
        debug!("$ {self}");
        let mut cp = self.cmd.spawn()?;
        let id = match cp.id() {
            Some(id) => id,
            None => {
                let _ = cp.kill().await;
                return Err(crate::Error::Internal("process has no id".to_string()));
            }
        };
        if let Err(e) = RUNNING_PIDS.lock().map(|mut pids| pids.insert(id)) {
            let _ = cp.kill().await;
            return Err(crate::Error::Internal(format!(
                "failed to lock RUNNING_PIDS: {e}"
            )));
        }
        trace!("Started process: {id} for {}", self.program);
        if let Some(pr) = &self.pr {
            // pr.prop("bin", &self.program);
            // pr.prop("args", &self.args);
            pr.prop("ensembler_cmd", &self.to_string());
            pr.prop("ensembler_stdout", &"".to_string());
            pr.set_status(progress::ProgressStatus::Running);
        }
        let result = Arc::new(Mutex::new(CmdResult::default()));
        let combined_output = Arc::new(Mutex::new(Vec::new()));
        let (stdout_flush, stdout_ready) = oneshot::channel();
        if let Some(stdout) = cp.stdout.take() {
            let result = result.clone();
            let combined_output = combined_output.clone();
            let redactions = self.redactions.clone();
            let pr = self.pr.clone();
            tokio::spawn(async move {
                let stdout = BufReader::new(stdout);
                let mut lines = stdout.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = redactions
                        .iter()
                        .fold(line, |acc, r| acc.replace(r, "[redacted]"));
                    let mut result = result.lock().await;
                    result.stdout += &line;
                    result.stdout += "\n";
                    result.combined_output += &line;
                    result.combined_output += "\n";
                    if let Some(pr) = &pr {
                        pr.prop("ensembler_stdout", &line);
                        pr.update();
                    }
                    combined_output.lock().await.push(line);
                }
                let _ = stdout_flush.send(());
            });
        } else {
            drop(stdout_flush);
        }
        let (stderr_flush, stderr_ready) = oneshot::channel();
        if let Some(stderr) = cp.stderr.take() {
            let result = result.clone();
            let combined_output = combined_output.clone();
            let redactions = self.redactions.clone();
            let pr = self.pr.clone();
            let stderr_to_progress = self.stderr_to_progress;
            tokio::spawn(async move {
                let stderr = BufReader::new(stderr);
                let mut lines = stderr.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = redactions
                        .iter()
                        .fold(line, |acc, r| acc.replace(r, "[redacted]"));
                    let mut result = result.lock().await;
                    result.stderr += &line;
                    result.stderr += "\n";
                    result.combined_output += &line;
                    result.combined_output += "\n";
                    if let Some(pr) = &pr {
                        if stderr_to_progress {
                            // Update progress bar like stdout does
                            pr.prop("ensembler_stdout", &line);
                            pr.update();
                        } else {
                            // Print above progress bars (current behavior)
                            pr.println(&line);
                        }
                    }
                    combined_output.lock().await.push(line);
                }
                let _ = stderr_flush.send(());
            });
        } else {
            drop(stderr_flush);
        }
        let (stdin_flush, stdin_ready) = oneshot::channel();
        if let Some(text) = self.stdin.take() {
            let Some(mut stdin) = cp.stdin.take() else {
                let _ = cp.kill().await;
                if let Err(e) = RUNNING_PIDS.lock().map(|mut pids| pids.remove(&id)) {
                    debug!("Failed to lock RUNNING_PIDS to remove pid {id}: {e}");
                }
                if let Some(pr) = &self.pr {
                    pr.set_status(progress::ProgressStatus::Failed);
                }
                return Err(crate::Error::Internal(
                    "stdin was requested but not available".to_string(),
                ));
            };
            tokio::spawn(async move {
                if let Err(e) = stdin.write_all(text.as_bytes()).await {
                    debug!("Failed to write to stdin: {e}");
                }
                let _ = stdin_flush.send(());
            });
        } else {
            drop(stdin_flush);
        }
        let status = loop {
            select! {
                _ = self.cancel.cancelled() => {
                    cp.kill().await?;
                }
                status = cp.wait() => {
                    break status?;
                }
            }
        };
        if let Err(e) = RUNNING_PIDS.lock().map(|mut pids| pids.remove(&id)) {
            debug!("Failed to lock RUNNING_PIDS to remove pid {id}: {e}");
        }
        result.lock().await.status = status;

        // these are sent when the process has flushed IO
        let _ = stdout_ready.await;
        let _ = stderr_ready.await;
        let _ = stdin_ready.await;

        if status.success() {
            if let Some(pr) = &self.pr {
                pr.set_status(progress::ProgressStatus::Done);
            }
        } else {
            let result = result.lock().await.to_owned();
            self.on_error(combined_output.lock().await.join("\n"), result)?;
        }

        let result = result.lock().await.to_owned();
        Ok(result)
    }

    fn on_error(&self, output: String, result: CmdResult) -> Result<()> {
        let output = output.trim().to_string();
        if let Some(pr) = &self.pr {
            pr.set_status(progress::ProgressStatus::Failed);
            if self.show_stderr_on_error {
                pr.println(&output);
            }
        }
        Err(ScriptFailed(Box::new((
            self.program.clone(),
            self.args.clone(),
            output,
            result,
        ))))?
    }
}

impl Display for CmdLineRunner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let args = self.args.join(" ");
        let mut cmd = format!("{} {}", &self.program, args);
        if cmd.starts_with("sh -o errexit -c ") {
            cmd = cmd[17..].to_string();
        }
        write!(f, "{cmd}")
    }
}

impl Debug for CmdLineRunner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let args = self.args.join(" ");
        write!(f, "{} {args}", self.program)
    }
}

/// The result of executing a command.
///
/// Contains the captured output streams and exit status.
#[derive(Debug, Default, Clone)]
pub struct CmdResult {
    /// The captured standard output.
    pub stdout: String,
    /// The captured standard error.
    pub stderr: String,
    /// Combined stdout and stderr in the order they were received.
    pub combined_output: String,
    /// The exit status of the process.
    pub status: ExitStatus,
}
