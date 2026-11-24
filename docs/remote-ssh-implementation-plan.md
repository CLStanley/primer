# Remote SSH implementation plan

## Goal
Provide a native remote workflow in Primer, preserving Enzyme and the existing terminal and project interfaces.

## Phase 1: SSH connections
- Discover named hosts from OpenSSH configuration and allow explicit destinations.
- Use the system OpenSSH client, respecting configuration, agent, identity files, jump hosts and host-key verification.
- Launch interactive sessions in existing PTY-backed terminal buffers; retain authentication prompts.
- Provide dashboard actions and a dedicated remote key family without conflicts.
- Validate destinations and pass arguments directly, never interpolate into shell commands.

## Phase 2: Remote files
- Browse remote directories and open files through a transport abstraction.
- Track remote identity separately from local paths; avoid accidental local writes.
- Support explicit saves, conflict detection, transfer errors and disconnect recovery.
- Preserve unsaved buffers and never silently overwrite changed remote files.

## Phase 3: Remote projects
- Associate project context with host and remote root.
- Scope search, terminals and repository operations to the remote host.
- Evaluate remote language-server execution separately; do not imply local LSP works transparently.

## Validation
Test local SSH server, configured aliases, key and interactive authentication, unknown host keys, unavailable hosts, disconnects, paths containing spaces, and multiple sessions. Keep existing project and keymap regression tests passing.
