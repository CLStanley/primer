# Primer Text Editor — Enzyme Editing Actions

> **Status:** Proposed v1 — ready for review/dogfooding
>
> **Parent specification:** `MODAL_GRAMMAR.md`

## Purpose

Editing actions operate directly on the cursor or current selection. Primer keeps obvious mnemonic/conventional operations direct rather than hiding them behind historical operator grammar.

```text
selection → action
```

Examples:

```text
sld     select line → delete
slc     select line → comment
swm     select word → modify
sfy     select function → yank
sw-     select word → lowercase
sw+     select word → uppercase
```

## Core direct actions

| Key / Pattern | Meaning | Status | Rationale |
|---|---|---|---|
| `i` | insert | Planned/Preserve | Obvious mnemonic and established modal convention. |
| `a` | append | Planned/Preserve | Obvious mnemonic; no need to compress into `I`. |
| `o` | open new line below/under | Planned/Preserve | `o = open`; lowercase is below/under. |
| `O` | open new line above/over | Planned/Preserve | Shift counterpart to `o`. |
| `d` | delete selection | Planned | Remove the selected instance. |
| `D` | duplicate selection/line | Planned | Natural conceptual counterpart: delete removes an instance; duplicate adds another. |
| `y` | yank/copy selection | Preserve | Established modal convention. |
| `p` | paste | Preserve | Established modal convention. |
| `m` | modify selection | Planned | `m = modify`; avoids Vim-specific `c = change`. |
| `c` | comment/uncomment selection | Planned | Programming-centric mnemonic. |
| `J` | join lines | Planned | Independent obvious mnemonic; lowercase `j` remains move-down. |
| `<` | outdent | Preserve | Visual direction is intuitive. |
| `>` | indent | Preserve | Visual direction is intuitive. |
| `-` | lowercase selection | Planned | Reduce/lower case. |
| `+` | uppercase selection | Planned | Increase/raise case. |
| `u` | undo | Preserve | Direct convention. |
| `U` | redo | Preserve | Natural Shift counterpart. |
| `Esc` | cancel / safe normal state | Preserve | Universal safety/cancel behavior. |

`I` and `A` remain intentionally unassigned. Primer does not consume uppercase variants merely to conserve keys when `i = insert` and `a = append` are already clearer.

## Capitalization and independent uppercase mnemonics

Shift may produce a natural counterpart when one exists:

```text
u / U    undo / redo
d / D    delete / duplicate
o / O    new line below / new line above
```

An uppercase key may also stand independently when its mnemonic is obvious:

```text
j        move down
J        join lines
```

Primer must not invent a fake semantic relationship simply to justify an uppercase binding.

## Modify

`m` means **modify the selected text**. It removes/replaces the selected content and enters the appropriate insertion state so typing supplies the replacement.

```text
swm     select word → modify
slm     select line → modify
sipm    select inside pair → modify
```

## Replace

`r` introduces explicit substitution. It is distinct from `m`: Modify edits an existing selection; Replace substitutes a specific unit/value directly.

Initial Replace grammar:

```text
r c <char>    replace character with <char>
```

Example:

```text
rc;
```

means "replace character with `;`."

`r` remains a family prefix so future replacement operations can be added only when they have a real, mnemonic need.

## Comment

`c` toggles comments for the current selection using language-aware comment semantics.

```text
slc    select line → comment
sbc    select block → comment
sfc    select function → comment
```

## Delete, duplicate, yank, and paste

```text
d    delete selected thing
D    duplicate selected thing
y    yank/copy selected thing
p    paste
```

`d/D` are taught as a conceptual pair: delete removes an instance; duplicate creates another instance. This passes Enzyme's one-sentence-rule test even though the English words are not strict antonyms.

Paste-before/paste-after variants remain unassigned until Primer's actual selection semantics demonstrate a need.

## Insert and line creation

```text
i    insert
a    append
o    open line below/under
O    open line above/over
```

These operations are already mnemonic and familiar enough that replacing them would increase the learning curve rather than reduce it.

## Indentation and joining

```text
<    outdent
>    indent
J    join lines
```

`<` and `>` communicate direction visually. `J` is an independent mnemonic for Join because lowercase `j` must remain directional movement.

## Case conversion

```text
-    lowercase current selection
+    uppercase current selection
```

Examples:

```text
sw-    select word → lowercase
sw+    select word → uppercase
sl-    select line → lowercase
sl+    select line → uppercase
```

Toggle-case/title-case operations are intentionally unassigned until actual use demonstrates that they deserve dedicated grammar.

## Formatting

`f` introduces Format in the direct editing-action namespace:

```text
ff    format file
fb    format all open buffers
fp    format project/workspace
```

These commands describe intended scope. Primer should expose only variants the configured formatter/LSP/tooling can actually support; `fp` may require project-specific formatter integration rather than an LSP primitive.

## Undo / redo

```text
u    undo
U    redo
```

This remains the canonical simple example of Shift producing a natural counterpart.

## Design constraints

1. Actions operate on the current selection when an action has a meaningful selection target.
2. Prefer direct mnemonic verbs for common editing operations.
3. Preserve strong conventions (`i`, `a`, `o/O`, `y`, `p`, `u/U`, `Esc`) when they already pass the Enzyme test.
4. `m` means modify; `r` means explicit replace/substitute.
5. `c` is comment in the direct editing-action position.
6. Capitalization may encode a natural counterpart, but does not have to.
7. Obvious independent uppercase mnemonics such as `J = Join` are allowed when the lowercase key is already occupied by an essential operation.
8. Do not consume an uppercase key merely because it is available.
9. Selection plus action should read naturally as a short sentence.
10. Action-family prefixes such as `f = format` and `r = replace` may take a second token describing scope/object.

## Editing-action v1 unresolved concepts

Core everyday editing is now substantially assigned. Remaining concepts are intentionally open rather than missing accidentally:

- paste-before/paste-after behavior, if Primer needs distinct direct commands
- toggle case / title case, if dedicated operations prove useful
- replacement operations beyond `rc<char>`
- formatter capability semantics for `fb` and especially `fp`
- exact behavior of `D` with no explicit selection (duplicate line vs current selection/object)

These should be resolved through implementation and dogfooding rather than by assigning speculative keys.
