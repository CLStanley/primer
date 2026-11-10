# Primer embedded terminal integration

## Goal
Provide a reusable terminal workflow inside Primer, starting with Lazygit, without reimplementing advanced Git features.

## Design principles
- Preserve Helix/Primer editing behavior and keybindings outside terminal focus.
- Use a real pseudo-terminal (PTY) for interactive TUI programs.
- Keep terminal process lifecycle, resize handling, and input routing explicit.
- Preserve the existing theme and popup conventions.
- Support launching tools in the active repository or buffer directory.
- Never silently lose terminal sessions or editor buffer changes.

## Implementation phases
1. Audit existing terminal/compositor support and choose a PTY library or reusable subsystem.
2. Implement one managed terminal session with process spawning, input, output, resizing, and cleanup.
3. Render a floating terminal popup with keyboard focus and reliable escape/close behavior.
4. Add a command to launch Lazygit in the active repository.
5. Test alternate screen, colors, mouse input, Unicode, shell exit, and terminal resize.
6. Consider bottom splits, multiple persistent sessions, and other TUIs only after the first implementation is stable.

## Initial acceptance criteria
- Open Lazygit from Primer without leaving the editor.
- Navigate Lazygit normally, then return to the original editor buffer.
- Git working tree changes become visible when returning to Primer.
- Terminal failures show useful errors without crashing the editor.
- Build and interactive tests pass before merging.
