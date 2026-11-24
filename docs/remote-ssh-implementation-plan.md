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

Provide a separate theme-aware rounded SSH host pill in the statusline, alongside the project indicator. Reflect the relevant remote context and actual connection state, distinguish disconnected sessions, and hide the pill when no remote context is active. Show the active remote host on the dashboard as well. When multiple hosts are connected, avoid implying that a local buffer belongs to a remote host. Test failed authentication, unknown host keys, unavailable hosts, paths with spaces, multiple connections, stale mounts and unsaved buffers.

## Non-goals
Remote language-server orchestration, custom remote backend, remote debugging, Git/Docker integration, and remote build orchestration.

## Progress
Initial alias parser and destination validation implemented; all 35 library unit tests passed in local validation. Mount lifecycle and UI integration pending.
