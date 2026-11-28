# Remote SSH implementation plan

## Goal
Lightweight remote editing with one local Primer installation. No Primer software on the remote host.

## Phase 1: Connections
Discover literal OpenSSH aliases and allow explicit destinations. Use OpenSSH and SSHFS, preserving host-key verification and authentication. Manage dedicated mount directories, connection state, failures and reconnects. Never interpolate untrusted input into shell commands. Initial support targets Unix remote paths and Linux clients.

## Phase 2: Files
Reuse existing file picker, explorer, live grep and buffers through SSHFS. Local language servers and formatters are best-effort. Protect unsaved buffers and prevent writes into an unmounted mount directory. Keep filesystem operations responsive on disconnect.

## Phase 3: Projects and terminals
Persist remote host and remote root separately from ephemeral mount paths. Integrate local and remote projects in the existing registry and picker. Mount before activating remote projects. Ordinary integrated terminals launch SSH in the remote project directory. Git and Docker are entirely user-managed through the remote shell; disable native local Git actions in remote contexts without changing local functionality.

## Phase 4: Polish and validation
Dashboard, keymaps, status, documentation and regression tests.

Provide a separate theme-aware rounded SSH host pill in the statusline, alongside the project indicator. Reflect the relevant remote context and actual connection state, distinguish disconnected sessions, and hide the pill when no remote context is active. Show the active remote host on the dashboard as well. Allow only one active remote environment per Primer instance. Require disconnect before connecting to another host; allow changing the directory on the current host. Return to the dashboard after connecting, and route ordinary file and terminal actions through the active environment. Test failed authentication, unknown host keys, unavailable hosts, paths with spaces, attempted duplicate connections, stale mounts and unsaved buffers.

## Non-goals
Remote language-server orchestration, custom remote backend, remote debugging, Git/Docker integration, and remote build orchestration.

## Progress
Connection and mount lifecycle, dashboard connection controls, saved hosts, directory changes, disconnect confirmation and SSH status indicator have been implemented and exercised interactively. Finish environment-aware actions, remote terminals, remote projects and lifecycle regression validation before merging.

## Deferred follow-up: shared UI and code consistency

After Remote SSH is complete, undertake a dedicated cleanup and consolidation effort. Finish migrating transient messages to the shared notification system, retaining existing stack spacing, independent expiration and severity styling. Audit repeated UI implementations and consolidate reusable confirmation, selection, input, information and progress windows where appropriate. Centralize theme-aware sizing, borders, padding, alignment and interaction conventions. Preserve modal interaction where a decision is required; avoid modal success messages. Document shared component APIs, usage conventions and examples, update contributor-facing documentation, and add regression coverage. Prefer reuse for future features rather than feature-specific window styling. Keep this follow-up separate from the remaining SSH implementation.
