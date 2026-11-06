# Primer Text Editor — Enzyme Selection Grammar

> **Status:** Proposed v1 — active implementation/dogfooding
>
> **Parent specification:** `MODAL_GRAMMAR.md`

## Purpose

Primer has two complementary ways to select text:

```text
s...    deliberately select a known object
v       enter interactive Select/Visual mode
```

`s` is the explicit object-selection grammar. `v` preserves Helix's interactive selection-extension concept: once Select/Visual mode is active, compatible motions extend the current selection instead of merely moving it.

Primer preserves Helix's first-class selection model. Enzyme changes the vocabulary used to express intent, not the underlying range/multiple-selection machinery.

## Core selection rule

> **`s + object` selects the whole object. `s + i + object` selects the contents inside that object.**

The whole-object form includes the object's boundaries when the object has meaningful boundaries. The `i` modifier removes those boundaries and selects the contents.

There is no `so...` family: selecting the object itself already means selecting the whole/around object. Primer only advertises `si<object>` combinations for which the underlying editor can define a useful and reliable inside range.

## Core object vocabulary

| Key | Object | Notes |
|---|---|---|
| `l` | line | Whole current line. |
| `w` | word | Word under/at cursor. |
| `W` | long word | Whitespace-delimited long word. |
| `f` | function/method | Language-neutral callable concept. |
| `F` | file | Complete contents of the current file/buffer. |
| `c` | comment | Tree-sitter comment object. |
| `C` | class/type | Structural type/class declaration. |
| `b` | structural block | **Tree-sitter/language-defined** block. |
| `s` | section | Paragraph/logical text section containing the cursor. |
| `p` | pair | Matching delimiters such as `()`, `[]`, `{}`, quotes, etc. |

Capitalization may distinguish related objects that naturally compete for the same mnemonic: `w/W` is word/long-word, `c/C` is comment/class, and `f/F` is function/file.

Enzyme deliberately uses **file**, not **document**, as its user-facing term for the complete editable contents. Helix may internally represent an open file as a Document; that is an implementation detail. File-wide commands also apply sensibly to unsaved buffers.

Structural objects should use Tree-sitter where practical. Section is Primer's user-facing term for the paragraph-like text unit traditionally called a paragraph by modal editors.

### Block and pair are intentionally different

A **block** is a semantic/syntactic object defined by the language's Tree-sitter support. Primer should not redefine a block as "whatever is inside braces." Different languages may recognize different structures as blocks.

A **pair** is a matching-delimiter relationship that remains useful even when the user does not yet understand a language's structural grammar. A brace-delimited construct may happen to be both a Tree-sitter block and a pair, but those are separate concepts.

## Direct object selection

| Key | Meaning |
|---|---|
| `sl` | select line |
| `sw` | select word |
| `sW` | select long word |
| `sf` | select current function/method |
| `sF` | select whole file |
| `sc` | select current comment |
| `sC` | select current class/type |
| `sb` | select current Tree-sitter structural block |
| `ss` | select current section |
| `sp` | select surrounding pair, including delimiters |

These acquire the containing/current object directly. They are for the thought "select this thing," rather than "start selecting while I move."

`sF` is Enzyme's select-all spelling. `F` means file throughout the object grammar, so the command reads directly as **select file** and is deliberately harder to trigger accidentally than a single immediate select-all key.

## Comment selection and comment actions

Comments are first-class Enzyme objects:

```text
sc     select current comment
sic    select inside current comment
```

Primer delegates both forms to Helix's Tree-sitter comment text objects. `sc` uses the language's `comment.around` range where available; `sic` uses `comment.inside`. This lets each language define comment boundaries rather than teaching Enzyme its own collection of `//`, `/* */`, `#`, and other delimiters.

Comment **selection** and comment **toggling** are separate concepts. `c` in an object slot means comment, while `Ctrl-C` remains the direct toggle-comment action. Leader comment actions remain available as well.

That permits compositional workflows such as:

```text
sl → Ctrl-C     select line → toggle comment
sf → Ctrl-C     select function → toggle comment
s2l → Ctrl-C    select two lines → toggle comment
```

## Inside selection

`i` means **inside** and appears between the Select verb and the object:

```text
s i [object]
```

Established examples:

| Key | Meaning |
|---|---|
| `sic` | select inside current comment using Tree-sitter's comment-inside range |
| `sip` | select inside surrounding pair, excluding delimiters |
| `sib` | select inside current Tree-sitter block when a reliable inside range exists |

The same rule can extend naturally to other objects only when Helix/Tree-sitter supplies a meaningful distinction between the whole object and its contents. Primer should not invent arbitrary inside semantics merely to make the grammar exhaustive.

For pairs:

```text
foo(alpha, beta)
   ^-----------^   sp  -> (alpha, beta)
    ^---------^    sip -> alpha, beta
```

Pair selection must use matching-pair semantics rather than treating a pair as a Tree-sitter block.

## Counts

Counts compose inside the Enzyme sentence. For selection, the count includes the current object and produces one contiguous selection:

```text
sw      current word
s2w     current word + next word
s3w     current word + next two words

sW      current long word
s2W     current + next long word

sl      current line
s2l     current + next line
```

The count is a repeat/object count, not an argument. This is distinct from navigation such as `gl225`, where `225` is an absolute line-number argument.

## Canceling an accidental/transient selection

Primer distinguishes **leaving a mode**, **canceling a transient operation**, and **undoing an edit**:

```text
Esc         exit/cancel the current mode or incomplete grammar sentence
Backspace   "oops, never mind" — cancel the current transient Enzyme operation
u           undo an actual edit
;           collapse the current selection at its active position
```

For an Enzyme object selection, Backspace restores the selection/cursor state that existed immediately before the transient selection operation when that state is available. In interactive Select mode, Backspace abandons the current interactive range, returns to its anchor, and exits to Normal mode.

Backspace is **not** an editing undo mechanism. Once an action has changed the file, `u` remains the way to undo that edit.

## Interactive Select/Visual mode

```text
v    enter Select/Visual mode
```

Select/Visual mode supplies the selection intent implicitly. Therefore Enzyme does **not** add a redundant `se... = select → extend...` family.

> **In Select/Visual mode, a compatible movement/navigation sentence extends the selection to the destination described by that sentence.**

The grammar is therefore learned once. Not every Navigation command can sensibly extend a contiguous selection; cross-file semantic jumps may not. Primer preserves useful Helix behavior where possible without pretending impossible cross-buffer ranges exist.

## Multiple-selection grammar

Multiple selections remain first-class Helix selections. Enzyme supplies predictable grammar for common selection-building operations where doing so materially improves normal editing.

```text
s a ...    select → add ...
s r ...    select → remove ...
s k ...    select → keep/filter ...
s x ...    select → split ...
```

### Add matching selections

```text
san    select → add → next matching occurrence
saN    select → add → previous matching occurrence
saa    select → add → all matching occurrences
```

The match source is the current selection text/range, not necessarily a lexical word.

### Remove selections

```text
srn    select → remove → next selection
srN    select → remove → previous selection
sra    select → remove → all additional/secondary selections
```

`sra` leaves the primary selection intact.

### Keep/filter and split

```text
skm    select → keep → matching...
srm    select → remove → matching...

sx...  select → split ...
sxl    split selection into lines
sxm    split selection by matches...
```

## Advanced selection-set controls

Enzyme lowers the learning curve; it does not attempt to eliminate learning. Specialized controls may still require learning and reference.

Primer intentionally preserves these concise Helix controls rather than inventing a longer Enzyme namespace:

```text
(       rotate selections backward / change primary backward
)       rotate selections forward / change primary forward
,       keep only the primary selection
Alt+,   remove the primary selection
```

Other advanced Helix selection operations—alignment, selection-direction flipping, rotating selection contents, syntax-tree growth/shrinkage, sibling/parent/child operations, and similar tools—must remain reachable. They may retain sensible Helix bindings unless dogfooding demonstrates a meaningful reason to promote them into Enzyme grammar.

## Actions after selection

Selection hands off naturally to direct editing actions:

```text
sl Ctrl-C    select line → toggle comment
sw m         select word → modify
sf y         select function → yank
sic d        select inside comment → delete
sip d        select inside pair → delete
sF y         select file → yank
```

Editing actions operate across all active selections where supported.

## Helper examples

After `s`:

```text
SELECT…

l    line
w    word
W    long word
f    function/method
F    file
c    comment
C    class/type
b    block (Tree-sitter/language-defined)
s    section
p    pair
i    inside…
a    add selection…
r    remove selection…
k    keep/filter selections…
x    split selection…
```

Select/Visual mode should instead surface compatible motions/navigation because `v` already supplies the Select/Extend intent.

## Design constraints

1. `s` means deliberate object-selection grammar.
2. `v` preserves interactive Select/Visual mode.
3. `s + object` selects the whole object; `s + i + object` selects inside it when a meaningful inside range exists.
4. There is no redundant `so...` outside/around family.
5. `block` is Tree-sitter/language-defined; `pair` is a separate matching-delimiter concept.
6. `c` means comment and `C` means class/type in object slots.
7. `f` means function and `F` means file in object slots.
8. Enzyme uses **file** rather than **document** as user-facing terminology for the complete editable contents.
9. In Select/Visual mode, compatible navigation extends the selection; do not require a redundant `se` prefix.
10. Preserve Helix's first-class and multiple-selection model.
11. Reuse Navigation vocabulary rather than creating a parallel extension vocabulary.
12. `sF` is select file/select all.
13. Backspace means cancel the current transient Enzyme operation; Esc remains mode exit/cancel, and `u` remains editing undo.
14. `n/N` retain next/previous meaning inside selection-set operations.
15. Prefer the strongest mnemonic for common/core operations; rarer advanced controls may retain concise established bindings.
16. Primer must account for existing Helix motion/selection capabilities through an explicit parity audit before Enzyme v1 is declared complete.
17. Do not invent bindings merely to make the grammar exhaustive; preserve sensible conventions when they already satisfy Primer's philosophy.

## Selection v1 implementation checklist

`se...` is intentionally removed. `v` plus compatible motion is the extension model.

`so...` is intentionally removed. Selecting an object directly is the whole/around form; `si...` is the inside modifier.

The old proposed `spn/spN/spo` primary-selection namespace is intentionally removed. `sp` means select pair; Primer preserves Helix's concise `(`/`)`/`,`/`Alt+,` controls for primary-selection management.

Before Enzyme v1 is frozen, Primer still needs:

- implementation verification for `sp` / `sip` using true matching-pair semantics;
- implementation verification for `sb` / `sib` using Tree-sitter-defined block semantics;
- implementation verification for `sc` / `sic` across languages with comment text-object queries;
- Backspace transient-selection cancellation/restoration verification;
- Select/Visual-mode navigation parity for compatible Enzyme motions;
- a Helix parity audit to ensure advanced capabilities remain reachable without automatically remapping them;
- verification of regex selection/filter/split behavior for the common Enzyme multiple-selection grammar;
- verification of commands whose Helix behavior cannot sensibly map to a contiguous selection across buffers/files.
