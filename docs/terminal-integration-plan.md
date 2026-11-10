# Primer terminal buffers and terminal mode

## Goal
Make interactive PTY sessions first-class Primer tabs and split panes, starting with a shell and Lazygit. Floating terminals are an optional presentation added later.

## Architecture decisions
- **Terminal session:** owns a PTY master, child process, terminal emulator screen, and lifecycle. Use a proven VT parser/emulator rather than interpreting escape sequences ourselves.
- **Terminal buffer identity:** a terminal tab references a session ID; it is not a text Document or a saveable file. Avoid forcing PTY output through Rope or the LSP/document pipeline.
- **View integration:** render terminal screen cells in the existing editor surface; support resize from the pane dimensions, redraw on output, and preserve sessions when switching tabs.
- **Input modes:** focused terminal starts in Terminal Mode, forwarding keys (including Escape) to the PTY. A dedicated escape chord returns to Primer navigation. Normal mode retains buffer/tab/split commands.
- **Safety:** closing a running session asks before terminating it; never auto-save file buffers or kill background sessions when changing focus.
- **Working directory:** new terminals start in the active buffer's repository root or parent directory, with a fallback to the editor launch directory.
- **Lazygit:** launch in a terminal session, not through a separate custom Git UI.

## Implementation milestones
1. **Foundation:** define terminal session and buffer/view ownership boundaries; choose PTY and VT emulator dependencies; add focused tests for lifecycle.
2. **First interactive shell:** create PTY, spawn shell, forward keyboard input, render output, handle resize and exit.
3. **Terminal Mode:** distinguish editor navigation from PTY input; implement a documented escape chord and mode indicator.
4. **Tabs and splits:** integrate terminal session handles with view focus, close, and tab navigation.
5. **Lazygit command:** open Lazygit in the active repository, return to editor without losing other sessions.
6. **Hardening:** alternate screen, mouse, Unicode, colors, scrollback, shell exit, resize, and editor Git refresh.
7. **Optional:** floating terminal presentation and multiple reusable terminal sessions.

## Acceptance criteria for first usable release
- Open an interactive shell as a Primer tab.
- Enter Terminal Mode automatically when focused; return to editor navigation with Ctrl-\\ followed by Ctrl-n.
- Switch to another file tab and back without killing the shell.
- Launch Lazygit in a terminal tab, navigate it, and exit safely.
- Handle terminal process exit and resize without crashing.
- Build and test on Linux before expanding platform support.

## Out of scope for the first iteration
- Replacing Primer's existing text buffer implementation with terminal output.
- Reimplementing Lazygit operations.
- Terminal multiplexing, detachable sessions, and floating overlays.
