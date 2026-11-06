# Primer Text Editor — Leader / Application Grammar

> **Status:** Enzyme v1 application architecture
>
> **Parent specification:** `MODAL_GRAMMAR.md`

## Purpose

Normal-mode Enzyme grammar interacts with text/code. Leader grammar interacts with **Primer, the project, and tools**.

```text
bare key / normal grammar    text and code
Space ...                    Primer/application operations
```

`Space` is Primer's Leader. Bare and Leader-prefixed letters are separate namespaces.

## Core families

| Prefix | Family | Examples of responsibility |
|---|---|---|
| `Space ?` | Primer help | searchable keybind/grammar helper, tutorial/discovery |
| `Space j` | Jump | interactive visible-target jumping |
| `Space s` | search/discovery | project/buffer text search, references, symbols |
| `Space f` | files | open/find/recent/save-related file operations |
| `Space b` | buffers | switch/close/list/manage buffers |
| `Space w` | windows/views | split/focus/layout/editor panes |
| `Space g` | Git/VCS | status, hunks, blame, diff, VCS changes |
| `Space l` | language/LSP tools | code actions, rename, diagnostics UI, language commands |
| `Space t` | terminal/tools | terminal, tasks, external tools |
| `Space p` | project/workspace | project-level operations |
| `Space c` | configuration | Primer configuration/theme/settings |

## Code information and Help

```text
k          LSP hover / information about symbol under cursor
Space ?    Primer help / grammar and keybind discovery
```

Bare `k` is Primer's direct **hover/information** command. This intentionally replaces Helix's normal-mode `k = move up`; directional movement remains `h/j/k/l` only where that does not conflict would be contradictory, so implementation must resolve this explicitly: **Primer v1 retains `k = move up` as the fundamental directional motion, therefore hover must not actually steal bare `k` in Normal mode without changing that core rule.**

Because `k = up` is already a stabilized fundamental motion, the desired `k = hover` idea is recorded here as a conflict requiring an alternate namespace before implementation. `Space ?` remains Help.

## Search boundary

```text
/query    search current document forward
?query    search current document backward
n/N       next/previous result
```

Broader search/discovery uses `Space s ...`:

```text
Space s p t    search project text
Space s p r    search project references
Space s p s    search project symbols
Space s b t    search open buffers text
```

`Space f` remains the File family rather than being overloaded to mean Find.

## Jump

```text
Space j    interactive visible-target Jump
```

Jump enters a temporary labelled-target state over useful visible destinations. The user types the displayed target label to move directly there; `Esc` cancels safely. V1 keeps `Space j` direct; subcommands are added only if dogfooding demonstrates a need.

## Git boundary

VCS changes belong under `Space g ...`, not normal Navigation. This keeps editor history distinct from repository hunks.

## Language tooling boundary

Semantic movement remains normal Navigation:

```text
gd    definition
gD    declaration
gi    implementation
gt    type definition
```

Language operations that act through tooling rather than merely navigate belong under `Space l ...`, such as rename, code actions, diagnostic panels, or server-specific commands.

## Design constraints

1. `Space` is the Leader.
2. Leader means "ask Primer/project/tooling to do something."
3. Normal and Leader letters are independent namespaces.
4. Preserve `Space ?` as Primer help/discovery.
5. Preserve `/` and `?` as forward/backward document search.
6. `Space j` invokes interactive visible-target Jump.
7. Use `Space s` for broader search/discovery.
8. Keep `Space f` for actual file operations.
9. Do not sacrifice a stabilized fundamental motion for a secondary convenience binding; resolve collisions explicitly.
10. Dogfood application families after core Enzyme motions are usable.

## Implementation note: `k` hover conflict

The desired mnemonic `k = hover` conflicts directly with the foundational `h/j/k/l` movement set (`k = up`). Both cannot occupy bare Normal-mode `k`.

Do **not** silently replace upward movement during implementation. Before hover is bound, choose a contextual or Leader/LSP location that preserves both capabilities. The rest of Enzyme v1 can be implemented independently of this single unresolved collision.

## Deferred Leader details

Exact second-level commands inside Files, Buffers, Windows, Git, Language, Terminal, Project, and Configuration remain intentionally open. They do not block implementation of the Enzyme motion/selection core.
