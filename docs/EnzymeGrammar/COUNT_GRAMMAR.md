# Primer Text Editor — Enzyme Count Grammar

> **Status:** Proposed v1 — active implementation/dogfooding
>
> **Related specifications:** `MODAL_GRAMMAR.md`, `NAVIGATION_GRAMMAR.md`, `SELECTION_GRAMMAR.md`

## Purpose

Counts are modifiers, not separate commands. A count answers **how many of the requested object or motion** should participate in the sentence.

Enzyme deliberately allows a count to appear after a grammatical prefix so the command can be read in the same order the user thinks it:

```text
s2w     select two words
g2nf    go to the second next function
```

Primer reuses Helix's existing count state rather than maintaining a second Enzyme-only numeric system. Helix's command-mode input already accepts a non-zero digit as a count when that digit is not a valid key in the currently pending keymap node, so counts compose naturally inside Enzyme prefixes.

## Selection counts

For object selection, the count **includes the current object**.

```text
sw      select the current word
s2w     select the current word and the next word
s3w     select the current word and the next two words

sW      select the current long-word
s2W     select the current long-word and the next long-word

sl      select the current line
s2l     select the current line and the next line
```

When selecting multiple adjacent objects with a count, the result should be **one contiguous selection** spanning the first through last requested object, including intervening text. Counts do not implicitly create multiple independent Helix selections.

Example:

```text
The quick brown fox
    ^

s2w -> [quick brown]
```

This keeps subsequent actions predictable:

```text
s2w d   select two words, then delete
s3w y   select three words, then yank
```

A count should only be advertised for an object when the underlying object has a useful repeat/sequential interpretation. Primer should not invent arbitrary count semantics merely for syntactic completeness.

## Navigation counts

Counts retain their ordinary repeat/distance meaning for navigation:

```text
gnf      go to next function
g2nf     go to the second next function

gns      go to next section
g3ns     go three sections forward
```

The same principle applies to other count-aware navigation sentences where Helix already supplies meaningful repeated motion behavior.

## Select/Visual mode

Select/Visual mode continues to provide selection intent implicitly. Counted compatible motions extend the active selection by the requested count rather than introducing a second count grammar.

```text
v
2w       extend through two word motions
```

Enzyme prefixes used from Select mode follow the same rule where the underlying motion supports counts.

## Backspace and counts

Backspace remains Primer's **"oops, never mind"** action regardless of count.

For a direct Enzyme object selection:

```text
cursor
s3w
Backspace
```

Backspace restores the exact selection/cursor state from before `s3w` when that transient state is still valid.

For interactive Select/Visual mode, the selection's anchor records where the interactive selection began. Backspace therefore collapses the selection back to that anchor and returns to Normal mode without modifying document contents:

```text
cursor
v
2w
Backspace
```

This returns to the cursor position at which Select mode began. `Esc` remains the ordinary mode-exit command; `u` remains edit undo.

## Design constraints

1. Counts are modifiers within existing Enzyme sentences, not a new verb family.
2. Selection counts include the current object.
3. Counted adjacent object selection produces one contiguous range unless a command explicitly belongs to the multiple-selection grammar.
4. Reuse Helix's count machinery wherever it already expresses the required behavior.
5. Counts may appear after an Enzyme prefix when the pending keymap leaves digits available for Helix's count parser (`s2w`, `g2nf`).
6. Do not assign count semantics to an object merely to make the grammar exhaustive.
7. Backspace cancels the complete transient counted selection just as it cancels the uncounted form.
