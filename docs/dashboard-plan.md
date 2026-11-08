# Primer Dashboard Implementation

## Goal
Introduce a visually polished, keyboard-driven startup dashboard inspired by the user's Neovim dashboard, while preserving Primer's Helix-style selection-first editing workflow.

## Initial scope
- Display a dashboard when launching without a file or project target.
- Provide recent files and recent projects, with keyboard-driven selection.
- Provide discoverable actions for opening a file, finding files, opening a project, and accessing help.
- Match Primer's existing theme and visual design; avoid introducing a separate styling system.
- Allow a seamless transition from the dashboard to the editor.

## Implementation notes
- Inspect existing startup, UI rendering, theme, file picker, and project state before choosing architecture.
- Reuse existing components and commands rather than duplicating navigation logic.
- Keep dashboard presentation modular and avoid modifying the motion grammar in this feature.
- Add tests for startup routing, keyboard interactions, and recent-item handling where applicable.

## Acceptance criteria
- No-file launch shows the dashboard; opening a file directly continues to open the editor.
- Dashboard actions are keyboard-accessible and respond consistently.
- Recent entries handle missing or moved paths gracefully.
- Existing editing workflows and motion bindings remain unaffected.

## Status
Planning scaffold only; dashboard functionality has not yet been implemented.
