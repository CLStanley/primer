# Live Grep and zk-compatible Notes — implementation plan

This work builds on the existing Primer dashboard and Helix picker interfaces. The goal is to reuse existing editor navigation, not create another editing mode.

## Phase 1: Live Grep

- Search the working directory recursively using ripgrep, honoring ignore rules.
- Update results while the query changes (debounced and cancellable).
- Display path, line number, and matching text, with preview and Enter-to-jump.
- Handle missing `rg`, empty queries, no results, and filenames containing spaces safely.
- Reuse the existing picker styling and keyboard navigation.
- Verify performance and avoid blocking the UI thread.

## Phase 2: Notes (existing Neovim behavior)

Source of truth: `CLStanley/dotfiles/portable/nvim/.config/nvim/lua/notes.lua`, plus `agenda.lua` for later agenda parity.

- Notebook default: `~/NAS/Notes`, with a configurable override (Neovim currently uses `NVIM_NOTES_DIR` and sets `ZK_NOTEBOOK_DIR`).
- Keep existing folders: `00 Inbox`, `01 Daily`, `02 Projects`, `03 Knowledge`, `04 Reference`, `05 Ideas`, `99 Archive`, `Agenda/{Todos,Events,Reminders}`.
- Match the five note templates, title slugging, YAML fields, and date formatting.
- Daily note: `01 Daily/YYYY-MM-DD.md` with existing headings.
- File picker and content grep scoped to notebook, plus category pickers.
- Preserve `.zk/config.toml`, wiki links, tags, and Markdown files; do not introduce a database.
- Support links, backlinks, outgoing links, and tags by invoking `zk` where appropriate; confirm available CLI commands before implementing.
- Preserve existing Neovim files; avoid overwriting an existing note when creating one.

## Phase 3: Agenda

Port todo, event, reminder, recurring reminder, and completion workflows from `agenda.lua` after notes parity is stable.

## Verification

Build with `cargo build --release`, run relevant Rust tests, then interactively test navigation, grep updates, Markdown creation, note links, and missing dependency handling. Do not merge until verified.
