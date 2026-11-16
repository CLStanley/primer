//! PTY process backend for interactive terminal sessions.
//!
//! This module owns process I/O only. VT escape decoding, screen rendering,
//! editor focus, and key encoding belong to subsequent integration layers.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// A running terminal process with a resizeable pseudo-terminal.
///
/// The master stays alive as long as this object is retained. Reader and
/// writer are independent handles so an output pump need not block input.
pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    reader: Option<Box<dyn Read + Send>>,
}

impl PtyProcess {
    /// Spawn a command in a new PTY, starting in `working_directory`.
    ///
    /// `program` may be a shell or TUI such as lazygit. Arguments are
    /// passed directly, without shell interpretation.
    pub fn spawn(
        program: &str,
        args: &[&str],
        working_directory: &Path,
        columns: u16,
        rows: u16,
    ) -> Result<Self> {
        anyhow::ensure!(columns > 0 && rows > 0, "terminal dimensions must be nonzero");
        anyhow::ensure!(working_directory.is_dir(), "terminal working directory does not exist");

        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open pseudo-terminal")?;

        let mut command = CommandBuilder::new(program);
        command.args(args);
        command.cwd(working_directory);

        let child = pair
            .slave
            .spawn_command(command)
            .with_context(|| format!("failed to launch terminal command: {program}"))?;
        // The parent must close its copy of the slave; otherwise EOF and
        // hangup detection can be delayed after the child exits.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().context("failed to open PTY reader")?;
        let writer = pair.master.take_writer().context("failed to open PTY writer")?;

        Ok(Self {
            master: pair.master,
            child,
            writer: Arc::new(Mutex::new(writer)),
            reader: Some(reader),
        })
    }

    /// Move the PTY output reader into an asynchronous output-pump worker.
    ///
    /// This is intentionally one-shot; multiple consumers would split bytes.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    /// Obtain a shareable writer for terminal input.
    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    pub fn write_input(&self, bytes: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|_| anyhow::anyhow!("PTY writer lock poisoned"))?;
        writer.write_all(bytes).context("failed to write terminal input")?;
        writer.flush().context("failed to flush terminal input")
    }

    pub fn resize(&self, columns: u16, rows: u16) -> Result<()> {
        anyhow::ensure!(columns > 0 && rows > 0, "terminal dimensions must be nonzero");
        self.master
            .resize(PtySize {
                rows,
                cols: columns,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to resize terminal")
    }

    /// Nonblocking child status check. A running process returns None.
    pub fn try_wait(&mut self) -> Result<Option<portable_pty::ExitStatus>> {
        self.child.try_wait().context("failed to query terminal process")
    }

    /// Explicitly terminate a process when the owning session is closed.
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("failed to terminate terminal process")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_terminal_dimensions() {
        assert!(PtyProcess::spawn("sh", &[], Path::new("."), 0, 24).is_err());
        assert!(PtyProcess::spawn("sh", &[], Path::new("."), 80, 0).is_err());
    }

    #[test]
    fn rejects_missing_working_directory() {
        assert!(PtyProcess::spawn("sh", &[], Path::new("/primer-nonexistent-terminal-directory"), 80, 24).is_err());
    }
}
