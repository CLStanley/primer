# Primer Text Editor — Enzyme Search Grammar

> **Status:** Proposed v1 — ready for review/dogfooding
>
> **Parent specification:** `MODAL_GRAMMAR.md`

## Purpose

Primer preserves `/` exactly for the familiar operation it already represents across editors and terminal tools: **search for text in the current document**.

Broader discovery is an application/project concern and therefore belongs behind Primer's Leader (`Space`). This avoids making literal `/text` ambiguous and keeps the conventional search path completely untouched.

## Conventional document search

```text
/query    search current document for text
n         next result
N         previous result
```

These are direct conventions. `/` never waits to determine whether the first characters of a query are an Enzyme scope command.

## Expanded search namespace

Expanded Search lives behind:

```text
Space s ...    search / discovery
```

`Space` remains Primer's Leader. `s` is available here because bare `s = select` and `Space+s = search` occupy different grammatical namespaces.

The sentence is:

```text
Space s [scope] [target]
search → scope → target
```

### Scope vocabulary

| Key | Scope |
|---|---|
| `f` | current file/document |
| `b` | open buffers |
| `p` | project/workspace |

### Target vocabulary

| Key | Target |
|---|---|
| `t` | text |
| `r` | references to symbol under cursor/selection |
| `s` | symbols |
| `d` | definitions |
| `i` | implementations |
| `T` | types/type definitions where supported |

Not every scope/target Cartesian product must exist. The helper should advertise only combinations Primer can implement meaningfully.

## Planned combinations

```text
Space s f t    search current file text
Space s b t    search open-buffer text
Space s p t    search project text
Space s p r    search project references
Space s p s    search project/workspace symbols
Space s f s    search document symbols
Space s p d    search project definitions, where meaningful
Space s p i    search project implementations, where meaningful
Space s p T    search project types, where meaningful
```

The current-file text form is grammatically valid but `/query` remains the preferred fast path.

LSP reference queries are semantic symbol queries and may not support arbitrary file/buffer scoping. Primer should not expose a combination merely because the grammar can spell it.

## Relationship to Navigation

Search discovers candidates; Navigation goes directly to known semantic destinations.

```text
gd    go definition
gD    go declaration
gi    go implementation
gt    go type definition

Space s p r    search project references
Space s p s    search project symbols
```

## Relationship to `?`

```text
?          what is this? / LSP hover
Space ?    how do I use Primer? / grammar and keybind help
```

## Helper behavior

Typing `/` opens ordinary document text search with no Enzyme parsing layer.

Typing `Space s` opens the Search helper:

```text
SEARCH…

f    file
a    [unassigned]
b    buffers
p    project
```

After `Space s p`:

```text
SEARCH → PROJECT FOR…

t    text
r    references
s    symbols
d    definitions
i    implementations
T    types
```

The helper should omit unsupported combinations rather than teach commands that cannot work.

## Design constraints

1. `/query` remains ordinary current-document text search.
2. `n/N` remain next/previous result.
3. Expanded discovery belongs behind `Space s` because it operates on buffers/projects/tooling rather than merely the current text surface.
4. Scope precedes target: "search project for references."
5. Only advertise combinations with real semantics.
6. Prefer LSP for semantic targets and a text-search backend for text.
7. Bare `s = select` and `Space+s = search` are intentionally independent namespaces.

## Search v1 unresolved implementation semantics

The grammar no longer has an ambiguity between `/query` and expanded search. Remaining implementation questions are capability questions:

- Which semantic combinations Helix's current LSP abstractions already support.
- Which project/buffer text-search operations require Primer additions.
- Whether project definition/implementation/type searches add enough value beyond workspace-symbol search plus direct `gd/gi/gt` navigation.
