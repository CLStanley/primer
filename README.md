<div align="center">

<h1>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo_dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="logo_light.svg">
  <img alt="Helix" height="128" src="logo_light.svg">
</picture>
</h1>

[![Build status](https://github.com/helix-editor/helix/actions/workflows/build.yml/badge.svg)](https://github.com/helix-editor/helix/actions)
[![GitHub Release](https://img.shields.io/github/v/release/helix-editor/helix)](https://github.com/helix-editor/helix/releases/latest)
[![Documentation](https://shields.io/badge/-documentation-452859)](https://docs.helix-editor.com/)
[![GitHub contributors](https://img.shields.io/github/contributors/helix-editor/helix)](https://github.com/helix-editor/helix/graphs/contributors)
[![Matrix Space](https://img.shields.io/matrix/helix-community:matrix.org)](https://matrix.to/#/#helix-community:matrix.org)

</div>

![Screenshot](./screenshot.png)

A [Kakoune](https://github.com/mawww/kakoune) / [Neovim](https://github.com/neovim/neovim) inspired editor, written in Rust.

The editing model is very heavily based on Kakoune; during development I found
myself agreeing with most of Kakoune's design decisions.

For more information, see the [website](https://helix-editor.com) or
[documentation](https://docs.helix-editor.com/).

All shortcuts/keymaps can be found [in the documentation on the website](https://docs.helix-editor.com/keymap.html).

[Troubleshooting](https://github.com/helix-editor/helix/wiki/Troubleshooting)

# Features

- Vim-like modal editing
- Multiple selections
- Built-in language server support
- Smart, incremental syntax highlighting and code editing via tree-sitter

Although it's primarily a terminal-based editor, I am interested in exploring
a custom renderer (similar to Emacs) using wgpu.

Note: Only certain languages have indentation definitions at the moment. Check
`runtime/queries/<lang>/` for `indents.scm`.

# Installation

[Installation documentation](https://docs.helix-editor.com/install.html).

[![Packaging status](https://repology.org/badge/vertical-allrepos/helix-editor.svg?exclude_unsupported=1)](https://repology.org/project/helix-editor/versions)

# Contributing

Contributing guidelines can be found [here](./docs/CONTRIBUTING.md).

# Getting help

Your question might already be answered on the [FAQ](https://github.com/helix-editor/helix/wiki/FAQ).

Discuss the project on the community [Matrix Space](https://matrix.to/#/#helix-community:matrix.org) (make sure to join `#helix-editor:matrix.org` if you're on a client that doesn't support Matrix Spaces yet).

# Credits

Thanks to [@jakenvac](https://github.com/jakenvac) for designing the logo!

## Primer-specific features and customization

Primer builds on Helix's selection-first editing model with a dashboard, floating command/search prompts, rounded Powerline-style statusline and buffer tabs, and configurable UI borders.

- [Primer appearance and visual settings](book/src/primer-appearance.md) — ready-to-use configuration, supported values, defaults, and theme color keys.
- [Configuration guide](book/src/configuration.md) — where to place settings and how to reload them.
- [Theme guide](book/src/themes.md) — how to customize component colors without editing Rust code.

The UI uses terminal characters for rounded corners (╭ ╮ ╰ ╯), not pixel-level window rounding. Powerline caps require a font containing the `` and `` glyphs.

### Primer buffer navigation

| Key | Action |
| --- | --- |
| `Shift-H` / `[b` / `Space b p` | Previous buffer |
| `Shift-L` / `]b` / `Space b n` | Next buffer |
| `Space b b` | Buffer picker |
| `Space b N` | New empty buffer |
| `Space b q` | Close current buffer (respects unsaved changes); show dashboard when the last buffer closes |

### Embedded terminal

Primer includes PTY-backed terminal buffers that share the editor's tabs, splits, and statusline.

| Key | Action |
| --- | --- |
| `Space t t` | Open a terminal buffer |
| `Space t h` | Open a terminal in a horizontal split |
| `Space t v` | Open a terminal in a vertical split |
| `Space t n` | Rename a terminal buffer |
| `Ctrl+Space` (Terminal Mode) | Switch to read-only Normal Mode |
| `Esc` (Terminal Mode) | Send Escape to the running application |
| `v` (Normal Mode) | Select terminal output |
| `y` (Select Mode) | Yank selected output |
| `i` (Normal Mode) | Return to Terminal Mode |

Terminal Normal/Select modes support basic output navigation and copying; they are not full editable documents. Interactive applications such as lazygit and lazydocker have been tested. `btop` currently has a known rendering issue in the embedded terminal.

### Planned: language-specific statusline pills

During shared UI consolidation, investigate distinct, theme-aware colors for
the language pill in the statusline. Allow per-language theme overrides and
retain a default fallback for languages without a specific color. Reuse the
shared pill renderer, preserve readable foreground contrast, and keep the
existing branch, project and mode indicators visually consistent. Exact color
assignments and configuration keys are not finalized.

### Planned: configurable terminal application launcher

Deferred until after the shared UI and architecture consolidation. Replace
application-specific integrations with user-defined terminal launchers.

- Define a display name, executable command and optional arguments in configuration.
- Allow a custom keybinding and optional dashboard entry for each launcher.
- Reuse the embedded terminal, project/remote working directory handling,
  dashboard components and notification system.
- Report missing executables and launch failures clearly; do not require
  applications to be installed for Primer itself to start.
- Respect the active local or remote environment when launching applications.

Illustrative configuration (proposed syntax, not implemented):

```toml
[[terminal.applications]]
name = "LazyGit"
command = "lazygit"
keybind = "space t g"
dashboard = true

[[terminal.applications]]
name = "LazyDocker"
command = "lazydocker"
keybind = "space t d"
dashboard = false
```

LazyGit and LazyDocker are examples, not built-in integrations. The final
configuration schema and keybinding conflict handling will be decided during
implementation.

### Planned: project workspace sessions

Implement in a separate project-sessions feature branch after the SSH work.
A project is a registered directory; its workspace is the current editing
environment, and a saved session is an optional persistent snapshot.

- Isolate project buffers and restore document order, active buffer, cursor
  and scroll positions, splits, and terminal names and working directories.
- Use stable local paths or remote host/path identities, not temporary mounts.
- Offer restore or fresh start when opening a saved session, and save or
  leave the previous snapshot unchanged when leaving a workspace.
- Keep document saving independent from workspace saving. Never silently
  discard modified documents or terminate running terminal processes.
- Preserve live terminals across switches where safe; recreate terminal
  configuration on restart, without promising to restore running processes.
- Provide a way to clear a saved workspace without unregistering the project.
- Consolidate relevant decisions into one transition dialog. Skip irrelevant
  questions and honor automatic preferences; use notifications for progress,
  outcomes and actual errors. Preserve the original workspace on cancellation
  or failure wherever possible.

Proposed configuration (not implemented):

```toml
[editor.project-sessions]
enabled = true
on-open = "ask"  # ask, restore, fresh
on-close = "ask" # ask, save, discard
```

Apply the lifecycle consistently to switching, clearing projects, SSH
disconnects and quitting. The exact schema and terminal lifecycle remain
implementation decisions.

### Projects

Projects are remembered locally in `$XDG_DATA_HOME/primer/projects.toml` (default `~/.local/share/primer/projects.toml`). Add a Git repository or any directory with `Space p a`; switch with `Space p p`, forget with `Space p f`, clear with `Space p c`. Forgetting requires the exact project name and never deletes files. Switching preserves open buffers. Missing directories are omitted from the picker; identical names are distinguished by their paths.

Dashboard shortcuts: `p` switch, `b` register, `x` forget, `c` clear, `t` terminal.
