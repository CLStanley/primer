//! Connects a PTY process to its VT screen without blocking the UI thread.
//!
//! A dedicated reader thread owns the PTY output reader. The editor/event loop
//! must call `drain_output` to apply queued bytes to the screen; the reader
//! never mutates UI state. Rendering and editor wakeups are separate work.

use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::{Context, Result};

use super::pty::PtyProcess;
use super::screen::TerminalScreen;

enum OutputEvent {
    Data(Vec<u8>),
    Closed,
    Error(String),
}

/// A live PTY session and its current VT screen.
pub struct RunningTerminal {
    process: PtyProcess,
    screen: TerminalScreen,
    output: Receiver<OutputEvent>,
    output_closed: bool,
}

impl RunningTerminal {
    pub fn spawn(
        program: &str,
        args: &[&str],
        working_directory: &Path,
        columns: u16,
        rows: u16,
        scrollback: usize,
    ) -> Result<Self> {
        // Validate screen dimensions before spawning a child process.
        let screen = TerminalScreen::new(columns, rows, scrollback)?;
        let mut process = PtyProcess::spawn(program, args, working_directory, columns, rows)?;
        let mut reader = process.take_reader().context("PTY reader already consumed")?;
        let (sender, output) = mpsc::channel();

        thread::Builder::new()
            .name("primer-terminal-output".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = sender.send(OutputEvent::Closed);
                            break;
                        }
                        Ok(n) => {
                            if sender.send(OutputEvent::Data(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        // Linux PTY masters commonly report EIO after the slave exits.
                        #[cfg(unix)]
                        Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                            let _ = sender.send(OutputEvent::Closed);
                            break;
                        },
                        Err(error) => {
                            let _ = sender.send(OutputEvent::Error(error.to_string()));
                            break;
                        }
                    }
                }
            })
            .context("failed to start PTY output reader")?;

        Ok(Self {
            process,
            screen,
            output,
            output_closed: false,
        })
    }

    /// Apply queued PTY bytes in arrival order. Never blocks waiting for output.
    ///
    /// Returns whether the visible screen might have changed. Call this from
    /// the editor's event loop, followed by a redraw when true.
    pub fn drain_output(&mut self) -> Result<bool> {
        let mut changed = false;
        loop {
            match self.output.try_recv() {
                Ok(OutputEvent::Data(bytes)) => {
                    self.screen.process(&bytes);
                    changed = true;
                }
                Ok(OutputEvent::Closed) => {
                    self.output_closed = true;
                    break;
                }
                Ok(OutputEvent::Error(error)) => {
                    self.output_closed = true;
                    return Err(anyhow::anyhow!("PTY output reader failed: {error}"));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.output_closed = true;
                    break;
                }
            }
        }
        Ok(changed)
    }

    pub fn screen(&self) -> &TerminalScreen {
        &self.screen
    }

    pub fn write_input(&self, bytes: &[u8]) -> Result<()> {
        self.process.write_input(bytes)
    }

    /// Resize the underlying PTY first; only update the screen on success.
    pub fn resize(&mut self, columns: u16, rows: u16) -> Result<()> {
        anyhow::ensure!(columns > 0 && rows > 0, "terminal dimensions must be nonzero");
        self.process.resize(columns, rows)?;
        self.screen.resize(columns, rows)
    }

    pub fn output_closed(&self) -> bool {
        self.output_closed
    }

    pub fn try_wait(&mut self) -> Result<Option<portable_pty::ExitStatus>> {
        self.process.try_wait()
    }

    pub fn kill(&mut self) -> Result<()> {
        self.process.kill()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn connects_shell_output_to_screen() {
        let mut terminal = RunningTerminal::spawn(
            "sh",
            &["-c", "printf primer-pty-ready"],
            Path::new("."),
            80,
            24,
            100,
        ).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            terminal.drain_output().unwrap();
            if terminal.screen().text_snapshot().contains("primer-pty-ready") {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("PTY output did not reach VT screen");
    }
}
