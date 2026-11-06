# Primer Text Editor — Enzyme Motions Grammar

> **Status:** Active design
>
> **Primary implementation target:** Primer, the Rust/Helix fork
>
> **Core idea:** **Learn vocabulary, not shortcuts. Learn the language by using the editor, not by studying the editor.**

## Project Goal

Primer should preserve what makes terminal-native modal editors compelling—speed, tiny resource usage, instant startup, keyboard-first operation, SSH friendliness, Tree-sitter/LSP integration, and powerful editing—while dramatically lowering the barrier to entry.

A user should not need years of Vim muscle memory or a history lesson about why a command was assigned a particular key before they can benefit from modal editing.

The desired learning loop is:

```text
look up → recognize pattern → predict → stop looking up
```

The success test is not whether an expert can save one keystroke. It is whether a user can learn one pattern and correctly infer several commands they were never explicitly taught.

## Enzyme Motions

**Enzyme Motions** is Primer's intent-driven modal grammar. The name follows Primer's Helix/DNA ancestry: enzymes move along and operate on a double helix; Enzyme Motions provide a predictable language for moving through and operating on text and code.

Enzyme is not merely a remapped Helix keymap. The keymap is the physical representation of a small contextual command language.

The grammar should follow the order in which a person naturally thinks about an operation.

```text
Select the line, then delete it.
select → line → delete
s        l       d

Search the project for references.
search → project → references
/         p           r
```

These intentionally use different grammatical orders. Enzyme does **not** force every operation into one universal selection-first syntax.

## Design Philosophy

### 1. Lower the barrier, not the capability

Primer remains terminal-native, keyboard-first, fast-starting, low-resource, SSH-friendly, selection-first for editing, Tree-sitter aware, LSP aware, multiple-selection capable, and suitable for professional development.

We are reducing cognitive overhead, not capability.

### 2. Intent families, not one universal grammar

Enzyme is built as a collection of **intent families**. Each family gets the sentence structure that naturally fits the task.

```text
NAVIGATION
movement / go → relationship → object

SEARCH
search → scope → target

SELECTION / EDITING
select → object → action

APPLICATION COMMANDS
Leader → application family → operation
```

Selection-first is an excellent model for editing text. It does not need to become the grammar for searching, opening files, managing buffers, or every other editor operation.

### 3. Design and stabilize one family at a time

For each intent family:

1. List real actions in plain English.
2. Identify existing Helix behavior that already makes sense.
3. Design the family's sentence structure and vocabulary.
4. Check proposed keys against the planned-key table and stabilized families.
5. Resolve true conflicts; allow contextual reuse when grammatical position is unambiguous.
6. Implement the smallest coherent version.
7. Dogfood it in real editing.
8. Once predictable, mark that family **Stable**.
9. Treat stable vocabulary like an API.

Initial family order:

```text
1. Navigation
2. Search
3. Selection
4. Comment
5. Modification
6. Remaining editing/application families
```

### 4. Preserve conventions that already make sense

We do not replace familiar behavior merely because Vim or Helix also uses it.

Current examples:

```text
h j k l   immediate directional movement
/         search current document text
Esc       cancel / escape / safe normal state
Space     leader / Primer application namespace
y         yank/copy current selection
p         paste
Enter     accept/confirm where appropriate
u         undo
U         redo
```

The test is:

> **Would this convention already make reasonable sense to someone who has not studied Vim?**

### 5. Historical compatibility alone is not enough

A command does not earn its place merely because Vim has used it for decades. Bindings such as `gg`, `G`, `zR`, and `zM` may be familiar to experienced Vim users, but their meanings are not naturally deducible by a newcomer.

### 6. Predictability beats minimum keystrokes — but avoid typing full sentences

Optimize in this order:

1. Predictability
2. Consistency
3. Discoverability
4. Safety
5. Brevity

The real metric is **time from thought to action**, not raw key count.

However, grammatical explicitness must not grow without bound. If a simple reusable rule can remove an extra token while remaining easy to teach, prefer the reusable rule.

> **A concept that can be clearly taught in one sentence may become a grammar rule when that rule substantially compresses common commands.**

The goal is a small language, not literally typing English sentences to move around a file.

### 7. Capitalization means logical inverse

Capitalization is a first-class Enzyme compression rule:

> **When a lowercase command/component has a clear logical inverse, the capital form represents that inverse.**

This rule should be taught near the beginning of Primer's tutorial, displayed in grammar/help documentation, and surfaced as a hint where useful.

Canonical example:

```text
u     undo
U     redo
```

Navigation can use the same rule where a concept naturally forms an inverse pair. For example, if `w` in a particular navigation position means **next word**, `W` may mean **previous word**.

Capitalization should invert the concept represented by the letter being shifted. Do not use uppercase for arbitrary unrelated behavior.

Explicit mnemonic vocabulary is still preferable when both sides already have simple words. For example, `s = start` and `e = end` are clearer than inventing `e = end` and `E = start` merely to use capitalization.

This replaces the earlier idea that uppercase might inconsistently mean "all" or "broader." If an operation needs an all/broader scope, that concept should be expressed separately unless it is itself the clear logical inverse in context.

### 8. Fast paths are allowed

Enzyme should not punish users for learning efficient existing motions. Helix's useful movement/selection behavior can remain as a fast path while explicit grammar exists for deliberate intent.

```text
w d       quick path: navigate/select word → delete
s w d     explicit path: select word → delete
```

## Normal Grammar vs Leader Grammar

### Normal mode: interact with text/code

Bare keys primarily handle navigation, selection, and direct text/code interaction.

```text
Move there.
Go to that structure.
Select this object.
Delete this selection.
Modify this selection.
Comment this selection.
```

### Leader: interact with Primer/project/tools

`Space` means roughly:

> **I want Primer itself to do something.**

Leader families are appropriate for files, buffers, windows, project operations, Git, language tooling, terminal/tools, configuration, and Primer help.

### Bare and Leader-prefixed letters are separate namespaces

A bare key and its Leader-prefixed equivalent do **not** consume the same vocabulary slot.

```text
s           Select
Space s     independently available application-level concept
```

Normal mode and Leader therefore provide separate mnemonic namespaces before capitalization and contextual continuation are even considered.

### Established direct conventions may bypass the boundary

The Normal/Leader distinction is a design rule, not dogma. Strong conventions such as `/`, `Esc`, `u/U`, `y`, and `p` may remain direct.

## Contextual Vocabulary and Grammatical Position

Letters do not need one global meaning across all Enzyme families. Meaning may depend on the grammatical question being asked when context makes the interpretation predictable.

```text
s f       select → function
/ f r     search → file → references
s b       select → block
/ b t     search → buffers → text
```

The real constraint is not globally unique letters; it is whether the user can predict meaning from the sentence being constructed.

## Planned Keybind Reference

This table is the central clash-checking reference while Enzyme is designed.

Statuses:

- **Preserve** — existing/conventional behavior we intend to retain.
- **Planned** — preferred Enzyme direction but not yet stabilized by implementation/use.
- **Stable** — implemented, dogfooded, and treated as established vocabulary.
- **Open** — concept exists but exact binding/grammar remains unresolved.

| Namespace | Key / Pattern | Meaning | Status | Notes |
|---|---|---|---|---|
| Global rule | `lowercase / Uppercase` | action / logical inverse | Planned | Capitalization inverts the concept represented by that letter when a clear inverse exists. |
| Normal | `h j k l` | left / down / up / right | Preserve | Fundamental immediate movement. |
| Normal | `g ...` | Go / Navigation grammar | Planned | First Enzyme family being designed. |
| Normal | `g s [object]` | go to start of object | Planned | `s = start`. |
| Normal | `g e [object]` | go to end of object | Planned | `e = end`. |
| Normal | `g s l` | go to start of current line | Planned | `g → start → line`. |
| Normal | `g e l` | go to end of current line | Planned | `g → end → line`. |
| Normal | `g s d` | go to start of document | Planned | Replaces opaque top-of-file grammar. |
| Normal | `g e d` | go to end of document | Planned | Replaces opaque bottom-of-file grammar. |
| Normal | `g s f` | go to start of function/method | Planned | Structural/Tree-sitter navigation. |
| Normal | `g e f` | go to end of function/method | Planned | Structural/Tree-sitter navigation. |
| Normal | `g s c` | go to start of class | Planned | Structural/Tree-sitter navigation. |
| Normal | `g e c` | go to end of class | Planned | Structural/Tree-sitter navigation. |
| Normal | `g s w` | go to start of next word | Planned | Explicit word-boundary navigation candidate. |
| Normal | `g e w` | go to end of next word | Planned | Explicit word-boundary navigation candidate. |
| Normal | `g s W` | go to start of previous word | Planned | `W` is logical directional inverse of `w` in this context. |
| Normal | `g e W` | go to end of previous word | Planned | `W` is logical directional inverse of `w` in this context. |
| Normal | `w` | word navigation/selection fast path | Preserve | Exact Primer semantics to confirm while Navigation is finalized. |
| Normal | `/text` | search current document for text | Preserve | Conventional direct search behavior. |
| Normal | `/ [scope] [target]` | expanded Search grammar | Planned | Second family to design. |
| Normal | `/ p r` | search project for references to current symbol | Planned | LSP semantic references. |
| Normal | `/ f r` | search file for references | Planned | Semantics/feasibility to verify. |
| Normal | `/ b r` | search buffers for references | Planned | Semantics/feasibility to verify. |
| Normal | `s [object]` | explicit Select grammar | Planned | Third family to design. |
| Normal | `s l` | select line | Planned | Avoids collision with bare `l = right`. |
| Normal | `s w` | select word | Planned | Coexists with word-navigation fast path. |
| Normal | `s b` | select structural block | Planned | Prefer syntax-aware semantics. |
| Normal | `s f` | select function/method | Planned | Context makes `f = function`. |
| Normal | `d` | delete current selection | Planned | Editing action. |
| Normal | `y` | yank/copy current selection | Preserve | Established modal convention. |
| Normal | `p` | paste | Preserve | Established modal convention. |
| Normal | `m` | modify current selection | Planned | Preferred over historical `c = change`; not stabilized. |
| Normal | `c` | comment current selection | Planned | Programming-centric candidate; not stabilized. |
| Normal | `u` | undo | Preserve | Mnemonic action. |
| Normal | `U` | redo | Preserve | Logical inverse of `u`. |
| Normal | `Esc` | cancel / return to safe state | Preserve | Incomplete Enzyme sentences must be safely cancellable. |
| Normal | `?` | "What is this?" / LSP hover | Planned | Ask about code under cursor/selection. |
| Leader | `Space` | Primer/application namespace | Preserve | Separate namespace from normal grammar. |
| Leader | `Space ?` | Primer help / grammar & keybind discovery | Planned | "How do I use Primer?" |
| Leader | `Space f ...` | file/application family | Open | Design later. |
| Leader | `Space b ...` | buffer/application family | Open | Design later. |
| Leader | `Space w ...` | window/application family | Open | Design later. |
| Leader | `Space g ...` | Git/application family | Open | Design later. |
| Leader | `Space l ...` | language/LSP application family | Open | Semantic navigation may remain under normal `g`. |
| Leader | `Space t ...` | terminal/tools family | Open | Design later. |

Update this table whenever a family stabilizes or a planned binding changes. Before assigning a key, check both its namespace and grammatical context here.

## `?`: Code Question vs Primer Help

```text
?           What is this? / LSP hover for symbol under cursor or selection
Space ?     How do I use Primer? / searchable grammar and keybind help
```

Bare `?` interacts with code; `Space ?` interacts with Primer. Definition navigation belongs in Navigation because the user's intent is to **go** somewhere.

## Navigation Grammar — Active Design

Navigation is the first family to design completely and stabilize.

### Preserved immediate movement

```text
h     left
j     down
k     up
l     right
```

These remain direct because they are fundamental modal movement and useful enough to preserve.

### Current sentence structure

The strongest current candidate is:

> **go → relationship/boundary → object**

```text
g [relationship] [object]
```

Current boundary vocabulary:

```text
s     start
 e    end
```

Examples:

```text
g s l     go → start → line
g e l     go → end → line

g s d     go → start → document
g e d     go → end → document

g s f     go → start → function
g e f     go → end → function

g s c     go → start → class
g e c     go → end → class
```

Word navigation demonstrates capitalization as compression:

```text
g s w     go → start → next word
g e w     go → end → next word

g s W     go → start → previous word
g e W     go → end → previous word
```

Here `W` is not an arbitrary alternate word motion. It is the logical directional inverse of `w` in this grammatical position.

### Navigation family acceptance test

If a user learns:

```text
g s l     go start line
g e l     go end line
g s f     go start function
```

they should be able to infer:

```text
g e f     go end function
```

If a relationship repeatedly requires explanation beyond the small set of documented Enzyme rules, redesign it.

### Navigation intents still to resolve

Before Navigation v1 is frozen, we need explicit answers for at least:

```text
next / previous function
next / previous class or structural sibling
definition of current symbol
implementation of current symbol
declaration/type definition where applicable
matching bracket / delimiter / syntax pair
next / previous diagnostic
next / previous search result
back to previous location / forward in location history
jump to visible word/target on screen
paragraph/block boundaries if useful
next / previous change (Git or editor change history, if this belongs in Navigation)
next / previous selection or multiple-selection target, if useful
```

Not every item must become a `g` sequence. The purpose of the Navigation pass is to decide which are true text/code navigation, which already have strong conventional fast paths, and which belong under Leader/application tooling instead.

## Search Grammar — Current Direction

Search is the second family to design after Navigation.

Base behavior remains conventional:

```text
/foo
```

Expanded Search follows:

> **search → scope → target**

Candidate scopes:

```text
p   project
f   file
b   buffers
```

Candidate targets:

```text
t   text
r   references to current symbol
s   symbols
```

Examples:

```text
/ p t     search project text
/ p r     search project references
/ p s     search project symbols
/ f t     search file text
/ f r     search file references
/ b t     search buffers text
```

Not every theoretical combination must be implemented. Primer should expose combinations that are semantically useful and technically sound.

`references` means references to the current semantic symbol, normally using LSP semantics. `symbols` means discovering semantic symbols more generally.

The implementation must eventually solve how expanded `/` grammar coexists with literal `/text` input without making normal search awkward.

## Selection Grammar — Current Direction

Selection is the third family to design.

Fast navigation selections remain useful:

```text
w       navigate/select word
w d     use that selection and delete
```

Explicit object acquisition uses Select:

```text
s l     select line
s w     select word
s b     select block
s f     select function
```

Then actions operate on the visible selection:

```text
s l d   select line → delete
s l c   select line → comment
s l y   select line → yank
s b d   select block → delete
s b c   select block → comment
s f m   select function → modify
```

Every incomplete editing sentence must leave the document unharmed. `Esc` cancels safely.

## Discoverability and In-Editor Teaching

Primer's helper should teach grammar, not merely list shortcuts.

At the top of the helper/tutorial, teach the small high-leverage rules, including:

```text
• Normal keys act on/navigate text and code; Leader operates Primer/project/tools.
• Commands are built from mnemonic vocabulary.
• A capital letter represents the logical inverse of its lowercase action when a clear inverse exists.
• Esc cancels an unfinished command.
• Space+? opens Primer help.
```

Contextual menus should show the sentence being constructed. For example:

```text
GO WHERE?

s   start of…
e   end of…
```

Then after `g s`:

```text
GO › START OF WHAT?

l   line
d   document
f   function
c   class
w   next word
W   previous word
```

Likewise `s` can show:

```text
SELECT WHAT?

l   line
w   word
b   block
f   function
```

`Space ?` should provide searchable explanations and related commands, with the goal of moving users from lookup to recognition to prediction.

## Real Tasks the Grammar Must Eventually Solve

High-value tasks already identified include:

```text
navigate line/document/structural boundaries
navigate semantic definitions and implementations
navigate diagnostics and location history
jump to a visible word
select a whole line
select a structural block
select a function/class
comment a line or block
delete a line or block
modify the word under the cursor
find all references to a symbol
search text in file/buffers/project
open/switch files and buffers
fold/unfold code predictably
```

Structural operations should use Tree-sitter where practical. Semantic operations such as references, definitions, implementations, hover, and rename should use LSP semantics where appropriate.

## Measuring Success

Useful measures include:

- Can someone with no Vim experience become productive quickly?
- How many concepts must they explicitly learn?
- After learning one command, how many related commands can they infer?
- After ordinary use, how often do they still need documentation for common operations?
- Can they correctly guess an unfamiliar command?
- Does advanced functionality reuse vocabulary learned during basic usage?

A useful conceptual metric is the **inference rate**:

> How many useful commands become predictable for every new piece of vocabulary explicitly learned?

## Primer / Helix Implementation Direction

Primer uses Helix because Helix already provides selection-first editing, first-class selections, Tree-sitter, LSP, terminal-native lightweight operation, Rust implementation, and a discoverable keymap/popup system that can evolve into a grammar tutor.

Enzyme should reuse Helix command and selection primitives rather than create a parallel editing engine wherever possible.

The development baseline is established: Primer builds and runs locally, runtime/languages are configured, and `feature/enzyme-motions` is the working branch for the first behavior changes.

## Relationship to the QuickShell Keybinding Helper

The QuickShell helper remains useful as a system-wide reference and can eventually include Primer as another topic. Primer itself should contain enough discoverability that a user does not require the external helper to learn Enzyme.

## Design Constraints

- Preserve broadly recognizable conventions when they reduce cognitive load.
- Do not preserve arbitrary historical behavior solely for compatibility.
- Normal mode primarily handles navigation, selection, and direct text/code interaction.
- Leader primarily handles Primer/application/project/tool operations.
- Bare and Leader-prefixed letters are separate namespaces.
- Direct conventions may intentionally bypass the namespace boundary.
- Design one intent family at a time and stabilize it before casually consuming conflicting vocabulary elsewhere.
- Meaning may depend on grammatical position when context makes it predictable.
- A learned primitive should help predict unfamiliar commands.
- Related operations should share vocabulary.
- Prefer ordinary-language mnemonics that a non-Vim user can understand.
- Small, explicitly taught grammar rules are acceptable when they substantially reduce command length.
- Capitalization represents logical inverse when a clear inverse exists; do not overload uppercase with unrelated meanings.
- Do not add grammatical tokens merely for theoretical purity if a reusable rule expresses the same idea cleanly.
- Selection should be visible before destructive editing actions.
- Incomplete grammar sentences should not mutate the document.
- `Esc` should safely cancel incomplete grammar state.
- Structural editing should use syntax-aware objects where practical.
- Semantic code operations should use LSP where practical.
- Common operations should remain short, but predictability outranks absolute brevity.
- Exceptions should be rare and documented.
- If many exceptions are required, redesign the grammar rather than asking the user to memorize them.
- Help should teach vocabulary and relationships, not merely list shortcuts.
- Basic usage should naturally teach concepts needed for advanced usage.

## Open Questions

- What is the complete Navigation v1 vocabulary?
- What should next/previous mean structurally, and where should capitalization provide the inverse?
- Which existing Helix word/navigation motions remain useful as direct fast paths?
- Should method and function be one user-facing semantic object? Current direction: likely yes.
- How should definition, implementation, declaration/type definition fit the `g` grammar?
- How should matching pairs fit Navigation?
- How should diagnostics and location history fit Navigation?
- How should jump-to-visible-target fit Navigation?
- How should expanded `/` grammar coexist with literal `/text` search without ambiguity?
- Which Search scope/target combinations are actually useful and technically meaningful?
- Is `m = modify` consistently comfortable enough to replace Helix/Vim-style `c = change`? Current direction: **yes**.
- Is `c = comment` the best direct editing meaning? Current direction: **yes**.
- How should repeated Select-object commands behave?
- What is the eventual fold grammar?
- How much contextual guidance should appear automatically before it becomes distracting?

## Immediate Roadmap

```text
1. Keep this specification and planned-key table current.
2. Finish Navigation v1 grammar and clash-check it.
3. Compare Navigation against existing Helix behavior and preserve useful fast paths.
4. Implement and dogfood Navigation v1.
5. Stabilize Navigation vocabulary.
6. Design Search using / and scope → target grammar.
7. Implement, dogfood, and stabilize Search v1.
8. Design explicit Selection grammar while preserving useful Helix fast paths.
9. Implement, dogfood, and stabilize Selection v1.
10. Continue family-by-family with Comment, Modification, folds, and application grammar.
11. Only after Enzyme is comfortable move on to Primer dashboard, file tree, and other UI/workflow features.
```

The guiding principle remains:

> **A user should learn Primer's language by using it. Documentation should teach the first patterns; the patterns should teach the rest.**
