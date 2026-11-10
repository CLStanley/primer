# Primer Git integration expansion

## Goal
Make common Git workflows accessible within Primer without leaving the editor.

## Proposed milestones
1. Audit existing Git functionality in helix-vcs, commands, dashboard, and UI.
2. Repository status picker: branch, changed files, staged/unstaged/untracked states, and open selected file.
3. Diff viewing and hunk navigation.
4. Stage/unstage changes with safe handling of destructive operations.
5. Commit workflow and dashboard/command-menu integration.

## Design constraints
- Native Primer UI and existing theme conventions.
- Keybindings consistent with existing command families.
- Safe, reversible operations with visible error messages.
- Worktree and detached-HEAD support.
- Incremental build and interactive verification.
