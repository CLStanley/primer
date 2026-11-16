# Primer Projects — implementation plan

## Goal
Make projects first-class workspaces in Primer while preserving Helix's selection-first editing model and lightweight terminal UI.

## Scope and phases

### 1. Project picker and discovery
- Replace the dashboard Projects placeholder with a searchable project picker.
- Register existing directories as projects, open a selected project, and forget a project without deleting its files.
- Display project name and absolute root path; handle missing or inaccessible directories gracefully.
- Provide a Projects key family, starting with `Space p p` for the picker.
- Use Primer's existing rounded popup styling and keyboard navigation.

### 2. Persistence and recent projects
- Persist registered projects and recently opened project order in a user-state file, not inside project repositories.
- Store canonical paths where possible; deduplicate entries.
- Support quick switching and opening a project from the dashboard.

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
- Project picker opens from dashboard and `Space p p`.
- User can add, find, open, and forget a project.
- Registered projects survive editor restart.
- Switching projects updates the active root consistently for file search, live grep, Git, and terminal startup.
- Missing directories produce actionable errors.
- Unsaved edits are never silently lost.
- Existing editor and terminal workflows remain functional.

## Implementation notes
Audit the existing dashboard, keymaps, file picker, grep, Git working-directory resolution, terminal startup, and config/state paths before coding. Land changes in small independently testable commits. Run Rust formatting, targeted tests, and release build before merging.
