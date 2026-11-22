# Primer Projects — implementation plan

## Goal
Make projects first-class workspaces in Primer while preserving Helix's selection-first editing model and lightweight terminal UI.

## Scope and phases

### 1. Explicit project initialization and picker
- A directory becomes a project **only** when the user explicitly creates a `.primer.toml` marker in its root. Never infer projects from Git, Cargo, language files, or directory scans.
- Initial marker schema: `[project]` with a required nonempty `name`. Use TOML for future project settings.
- Provide an Initialize Project action that creates the marker safely (never overwrite an existing file), and validate malformed markers with actionable errors.
- Recognizing a marker does not silently activate or register a project.
- Replace the dashboard Projects placeholder with a searchable project picker.
- Register only directories containing a valid marker; open a selected project, and forget it without deleting its files or marker.
- Display project name and absolute root path; handle missing or inaccessible directories gracefully.
- Provide a Projects key family, starting with `Space p p` for the picker.
- Use Primer's existing rounded popup styling and keyboard navigation.

### 2. Persistence and recent projects
- Persist registered projects and recently opened project order in `~/.local/share/primer/projects.toml` (respecting XDG_DATA_HOME), not inside project repositories.
- Store canonical paths where possible; deduplicate entries.
- Support quick switching and opening a project from the dashboard.

### Configuration versus state
- `.primer.toml` is portable project configuration and may be committed to Git.
- `~/.local/share/primer/projects.toml` is a local registry, not project identity.
- `~/.local/state/primer/` (respecting XDG_STATE_HOME) is reserved for machine-specific session state.
- Global editor preferences remain in Primer's normal configuration.

### 3. Project-aware tools
- Define a single active project root for file picker, live grep, Git actions, and new embedded terminals.
- Preserve existing behavior when no project is active.
- Avoid silently changing the active project based on whichever file happens to be focused.

### 4. Workspace restoration (separate milestone)
- Persist and restore open file buffers and split layout.
- Decide separately whether terminal sessions should be restarted rather than pretending PTY state can be serialized.

## Initial switching behavior
Switching projects replaces the active project context rather than opening multiple concurrent project workspaces. Protect unsaved buffers with confirmation before closing them. Do not delete files, terminate running terminal processes, or discard edits silently.

## Acceptance criteria
- Initialize Project creates a valid `.primer.toml` in an explicitly selected directory without overwriting an existing marker.
- Unmarked directories are not accepted as projects.
- Project picker opens from dashboard and `Space p p`.
- User can add, find, open, and forget a project.
- Registered projects survive editor restart.
- Switching projects updates the active root consistently for file search, live grep, Git, and terminal startup.
- Missing directories produce actionable errors.
- Unsaved edits are never silently lost.
- Existing editor and terminal workflows remain functional.

## Implementation notes
Audit the existing dashboard, keymaps, file picker, grep, Git working-directory resolution, terminal startup, and config/state paths before coding. Land changes in small independently testable commits. Run Rust formatting, targeted tests, and release build before merging.

## Future enhancement: Git fetch/pull and live buffer synchronization

Track separately from the initial Projects implementation; this is a future Git integration milestone.

- Add Git **Fetch** to update remote-tracking refs without modifying the working tree.
- Add Git **Pull** for the current branch (with clear indication of its upstream and configurable merge/rebase/fast-forward policy); optionally support fetching another branch without switching to it.
- When Git operations change on-disk files, detect those changes and refresh corresponding open buffers automatically when they have no unsaved edits.
- Preserve cursor/selection and viewport positions as far as practical, clamping them when the updated file is shorter.
- Never silently discard unsaved edits; prompt to compare/reload or resolve conflicts when both the buffer and disk have changed.
- Handle removed/renamed files, merge conflicts, failed pulls, and changes from external Git processes.
- Refresh Git decorations, branch/status indicators, diagnostics, and file explorer after successful operations.
- Consider a file watcher plus existing editor disk-change/reload mechanisms instead of implementing a separate synchronization path.
- Test behavior across clean buffers, dirty buffers, multiple open views, and terminal-initiated Git commands.

**Desired experience:** fetch/pull from within Primer and see the open files update around the user without having to reopen them manually.
