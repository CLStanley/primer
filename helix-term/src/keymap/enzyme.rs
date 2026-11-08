//! Primer-specific Enzyme motion and selection adapters.
//!
//! Helix owns language/text-object semantics wherever possible. Enzyme owns the
//! predictable grammar layered over those primitives.

use std::{cell::RefCell, num::NonZeroUsize};

use helix_core::{match_brackets, textobject, Range, Selection};
use helix_view::{document::Mode, keyboard::KeyCode, DocumentId, ViewId};
use crate::commands::{Context, MappableCommand};

#[derive(Clone)]
struct TransientSelection {
    document_id: DocumentId,
    view_id: ViewId,
    document_version: i32,
    before: Selection,
    after: Selection,
}

thread_local! {
    static TRANSIENT_SELECTION: RefCell<Option<TransientSelection>> = const { RefCell::new(None) };
}

fn remember_transient_selection(cx: &mut Context, action: impl FnOnce(&mut Context)) {
    let (document_id, view_id, document_version, before) = {
        let (view, doc) = current!(cx.editor);
        (doc.id(), view.id, doc.version(), doc.selection(view.id).clone())
    };
    action(cx);
    let after = {
        let (view, doc) = current!(cx.editor);
        if doc.id() != document_id || view.id != view_id || doc.version() != document_version {
            TRANSIENT_SELECTION.with(|saved| *saved.borrow_mut() = None);
            return;
        }
        doc.selection(view.id).clone()
    };
    TRANSIENT_SELECTION.with(|saved| {
        *saved.borrow_mut() = Some(TransientSelection { document_id, view_id, document_version, before, after });
    });
}

/// Primer's "oops, never mind" key. This never touches edit undo history.
fn enzyme_cancel_transient_selection(cx: &mut Context) {
    if cx.editor.mode == Mode::Select {
        let (view, doc) = current!(cx.editor);
        let selection = doc.selection(view.id).clone().transform(|range| Range::point(range.anchor));
        doc.set_selection(view.id, selection);
        cx.editor.mode = Mode::Normal;
        TRANSIENT_SELECTION.with(|slot| *slot.borrow_mut() = None);
        return;
    }
    let saved = TRANSIENT_SELECTION.with(|slot| slot.borrow_mut().take());
    let Some(saved) = saved else { return; };
    let (view, doc) = current!(cx.editor);
    if doc.id() != saved.document_id
        || view.id != saved.view_id
        || doc.version() != saved.document_version
        || doc.selection(view.id) != &saved.after
    { return; }
    doc.set_selection(view.id, saved.before);
}

/// `gsl` means content start; literal column zero deliberately lives at `g0`.
fn enzyme_goto_line_content_start(cx: &mut Context) {
    if cx.editor.mode == Mode::Select {
        MappableCommand::extend_to_first_nonwhitespace.execute(cx);
    } else {
        MappableCommand::goto_first_nonwhitespace.execute(cx);
    }
}

fn goto_word_boundary(cx: &mut Context, long_word: bool, start: bool) {
    let mode = cx.editor.mode;
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let selection = doc.selection(view.id).clone().transform(|range| {
        let cursor = range.cursor(text);
        let word = textobject::textobject_word(text, Range::point(cursor), textobject::TextObject::Inside, 1, long_word);
        let target = if start { word.from() } else { word.to().saturating_sub(1) };
        if mode == Mode::Select { Range::new(range.anchor, target) } else { Range::point(target) }
    });
    doc.set_selection(view.id, selection);
}

fn enzyme_goto_word_start(cx: &mut Context) { goto_word_boundary(cx, false, true); }
fn enzyme_goto_word_end(cx: &mut Context) { goto_word_boundary(cx, false, false); }
fn enzyme_goto_long_word_start(cx: &mut Context) { goto_word_boundary(cx, true, true); }
fn enzyme_goto_long_word_end(cx: &mut Context) { goto_word_boundary(cx, true, false); }

fn select_helix_textobject(cx: &mut Context, around: bool, object_key: char) {
    let command = if around { MappableCommand::select_textobject_around } else { MappableCommand::select_textobject_inner };
    command.execute(cx);
    let Some((callback, _kind)) = cx.on_next_key_callback.take() else {
        cx.editor.set_error("Enzyme expected Helix to request a text-object key");
        return;
    };
    let Ok(event) = object_key.to_string().parse() else {
        cx.editor.set_error("Enzyme could not construct a text-object key");
        return;
    };
    callback(cx, event);
}

/// Reuse Helix's text-object query, then collapse it to a structural boundary.
/// This keeps Primer from maintaining a second definition of syntax objects.
fn goto_textobject_boundary(cx: &mut Context, object_key: char, start: bool) {
    let mode = cx.editor.mode;
    let before = {
        let (view, doc) = current!(cx.editor);
        doc.selection(view.id).clone()
    };
    select_helix_textobject(cx, true, object_key);
    let (view, doc) = current!(cx.editor);
    let object = doc.selection(view.id).clone();
    let mut before_ranges = before.ranges().iter();
    let selection = object.transform(|range| {
        // Helix ranges use an exclusive end. A forward selection must extend
        // through the final character of the object, not stop just before it.
        // Normal-mode movement still lands on the final character itself.
        let target = if start {
            range.from()
        } else if mode == Mode::Select {
            range.to()
        } else {
            range.to().saturating_sub(1)
        };
        if mode == Mode::Select {
            let anchor = before_ranges.next().map_or(range.anchor, |old| old.anchor);
            Range::new(anchor, target)
        } else {
            Range::point(target)
        }
    });
    doc.set_selection(view.id, selection);
}

/// Select the enclosing Tree-sitter block captured by the language's
/// `block.around` query, rather than Helix's closest delimiter pair.
fn select_structural_block(cx: &mut Context) {
    let loader = cx.editor.syn_loader.load();
    let (view, doc) = current!(cx.editor);
    let Some(syntax) = doc.syntax() else {
        cx.editor.set_status("Syntax-tree is not available in current buffer");
        return;
    };
    let text = doc.text().slice(..);
    let selection = doc.selection(view.id).clone().transform(|range| {
        textobject::textobject_treesitter(
            text, range, textobject::TextObject::Around, "block", syntax, &loader, 1,
        )
    });
    doc.set_selection(view.id, selection);
}

fn goto_block_boundary(cx: &mut Context, start: bool) {
    let mode = cx.editor.mode;
    let before = {
        let (view, doc) = current!(cx.editor);
        doc.selection(view.id).clone()
    };
    select_structural_block(cx);
    let (view, doc) = current!(cx.editor);
    let mut before_ranges = before.ranges().iter();
    let selection = doc.selection(view.id).clone().transform(|range| {
        let original = before_ranges.next();
        // No matching block: preserve the original cursor/selection.
        if original.is_some_and(|old| *old == range) {
            return range;
        }
        let target = if start { range.from() }
            else if mode == Mode::Select { range.to() }
            else { range.to().saturating_sub(1) };
        if mode == Mode::Select {
            Range::new(original.map_or(range.anchor, |old| old.anchor), target)
        } else {
            Range::point(target)
        }
    });
    doc.set_selection(view.id, selection);
}

fn enzyme_goto_function_start(cx: &mut Context) { goto_textobject_boundary(cx, 'f', true); }
fn enzyme_goto_function_end(cx: &mut Context) { goto_textobject_boundary(cx, 'f', false); }
fn enzyme_goto_comment_start(cx: &mut Context) { goto_textobject_boundary(cx, 'c', true); }
fn enzyme_goto_comment_end(cx: &mut Context) { goto_textobject_boundary(cx, 'c', false); }
fn enzyme_goto_class_start(cx: &mut Context) { goto_textobject_boundary(cx, 't', true); }
fn enzyme_goto_class_end(cx: &mut Context) { goto_textobject_boundary(cx, 't', false); }
fn enzyme_goto_section_start(cx: &mut Context) { goto_textobject_boundary(cx, 'p', true); }
fn enzyme_goto_section_end(cx: &mut Context) { goto_textobject_boundary(cx, 'p', false); }
fn enzyme_goto_block_start(cx: &mut Context) { goto_block_boundary(cx, true); }
fn enzyme_goto_block_end(cx: &mut Context) { goto_block_boundary(cx, false); }

/// `gl<number>` is an argument-taking command, not a count.
fn await_line_number(cx: &mut Context, digits: String) {
    cx.editor.set_status(format!("GO TO LINE: {digits}"));
    cx.on_next_key(move |cx, event| match event.code {
        KeyCode::Char(ch) if ch.is_ascii_digit() => {
            let mut next = digits;
            next.push(ch);
            await_line_number(cx, next);
        }
        KeyCode::Backspace => {
            let mut next = digits;
            next.pop();
            cx.editor.set_status(format!("GO TO LINE: {next}"));
            await_line_number(cx, next);
        }
        KeyCode::Enter => {
            if let Ok(line) = digits.parse::<usize>() {
                if let Some(line) = NonZeroUsize::new(line) {
                    let old_count = cx.count;
                    cx.count = Some(line);
                    MappableCommand::goto_line.execute(cx);
                    cx.count = old_count;
                }
            }
        }
        KeyCode::Esc => {}
        _ => cx.editor.set_error("Line number accepts digits, Enter, Backspace, or Esc"),
    });
}

fn enzyme_goto_line_number(cx: &mut Context) { await_line_number(cx, String::new()); }

fn select_pair(cx: &mut Context, object: textobject::TextObject) {
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let syntax = doc.syntax();
    let selection = doc.selection(view.id).clone().transform(|range| {
        let cursor = range.cursor(text);
        if let Some(syntax) = syntax {
            if let Some(matching) = match_brackets::find_matching_bracket(syntax, text, cursor) {
                let start = cursor.min(matching);
                let end = cursor.max(matching);
                return match object {
                    textobject::TextObject::Around => Range::new(start, end + 1),
                    textobject::TextObject::Inside => Range::new(start + 1, end),
                    textobject::TextObject::Movement => range,
                };
            }
        }
        textobject::textobject_pair_surround_closest(syntax, text, range, object, 1)
    });
    doc.set_selection(view.id, selection);
}

/// Add literal occurrences of the primary selection, without changing the
/// search register or interpreting punctuation as regular expressions.
fn enzyme_add_matching(cx: &mut Context, direction: i8) {
    let (view, doc) = current!(cx.editor);
    let text = doc.text().slice(..);
    let before = doc.selection(view.id).clone();
    let primary = before.primary();
    if primary.from() == primary.to() {
        cx.editor.set_error("Select some text before adding matches");
        return;
    }

    // Match the exact selected text, including whitespace if requested.
    let needle = primary.fragment(text).to_string();
    if needle.is_empty() {
        cx.editor.set_error("Select text before adding matches");
        return;
    }
    let source = primary;
    let normalized = before;

    let haystack = text.to_string();
    let matches: Vec<Range> = haystack.match_indices(&needle).map(|(byte, matched)| {
        let start = haystack[..byte].chars().count();
        Range::new(start, start + matched.chars().count())
    }).filter(|candidate| {
        !normalized.ranges().iter().any(|existing| {
            candidate.from() < existing.to() && existing.from() < candidate.to()
        })
    }).collect();

    let selected: Vec<Range> = if direction == 0 {
        matches
    } else {
        let position = if direction > 0 { source.to() } else { source.from() };
        let candidate = if direction > 0 {
            matches.iter().find(|r| r.from() >= position).or_else(|| matches.first())
        } else {
            matches.iter().rev().find(|r| r.to() <= position).or_else(|| matches.last())
        };
        candidate.copied().into_iter().collect()
    };

    if selected.is_empty() {
        cx.editor.set_status("No additional matching occurrences");
        return;
    }

    let mut selection = normalized;
    for range in selected {
        selection = selection.push(range);
    }
    // Selection::push makes the newly added range primary. Restore the
    // original source as primary after sorting/normalization.
    if let Some(index) = selection.ranges().iter().position(|r| *r == source) {
        selection.set_primary_index(index);
    }
    doc.set_selection(view.id, selection);
}
fn enzyme_add_next_match(cx: &mut Context) { enzyme_add_matching(cx, 1); }
fn enzyme_add_prev_match(cx: &mut Context) { enzyme_add_matching(cx, -1); }
fn enzyme_add_all_matches(cx: &mut Context) { enzyme_add_matching(cx, 0); }

/// Remove the nearest secondary selection in the requested direction.
/// The primary selection stays intact so repeated removal is predictable.
fn enzyme_remove_neighbor(cx: &mut Context, forward: bool) {
    let (view, doc) = current!(cx.editor);
    let selection = doc.selection(view.id).clone();
    if selection.len() <= 1 {
        cx.editor.set_status("No secondary selections to remove");
        return;
    }
    let primary = selection.primary_index();
    let count = selection.len();
    let index = if forward {
        (1..count).map(|step| (primary + step) % count)
            .find(|&index| index != primary)
    } else {
        (1..count).map(|step| (primary + count - step) % count)
            .find(|&index| index != primary)
    };
    if let Some(index) = index {
        doc.set_selection(view.id, selection.remove(index));
    }
}

fn enzyme_remove_next_selection(cx: &mut Context) { enzyme_remove_neighbor(cx, true); }
fn enzyme_remove_prev_selection(cx: &mut Context) { enzyme_remove_neighbor(cx, false); }

fn enzyme_select_line(cx: &mut Context) { remember_transient_selection(cx, |cx| MappableCommand::extend_line.execute(cx)); }
fn enzyme_select_word(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, false, 'w')); }
fn enzyme_select_long_word(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'W')); }
fn enzyme_select_function(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'f')); }
fn enzyme_select_comment(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'c')); }
fn enzyme_select_inside_comment(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, false, 'c')); }
fn enzyme_select_class(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 't')); }
fn enzyme_select_section(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'p')); }
fn enzyme_select_block(cx: &mut Context) { remember_transient_selection(cx, select_structural_block); }
fn enzyme_select_pair(cx: &mut Context) { remember_transient_selection(cx, |cx| select_pair(cx, textobject::TextObject::Around)); }
fn enzyme_select_inside_pair(cx: &mut Context) { remember_transient_selection(cx, |cx| select_pair(cx, textobject::TextObject::Inside)); }
fn enzyme_select_file(cx: &mut Context) { remember_transient_selection(cx, |cx| MappableCommand::select_all.execute(cx)); }

#[allow(non_upper_case_globals)]
impl MappableCommand {
    pub const enzyme_cancel_transient_selection: Self = Self::Static { name: "enzyme_cancel_transient_selection", fun: enzyme_cancel_transient_selection, doc: "Cancel current transient Enzyme selection" };
    pub const enzyme_goto_line_content_start: Self = Self::Static { name: "enzyme_goto_line_content_start", fun: enzyme_goto_line_content_start, doc: "Go to first non-whitespace character of line" };
    pub const enzyme_goto_line_number: Self = Self::Static { name: "enzyme_goto_line_number", fun: enzyme_goto_line_number, doc: "Go to absolute line number" };
    pub const enzyme_goto_word_start: Self = Self::Static { name: "enzyme_goto_word_start", fun: enzyme_goto_word_start, doc: "Go to start of current word" };
    pub const enzyme_goto_word_end: Self = Self::Static { name: "enzyme_goto_word_end", fun: enzyme_goto_word_end, doc: "Go to end of current word" };
    pub const enzyme_goto_long_word_start: Self = Self::Static { name: "enzyme_goto_long_word_start", fun: enzyme_goto_long_word_start, doc: "Go to start of current long word" };
    pub const enzyme_goto_long_word_end: Self = Self::Static { name: "enzyme_goto_long_word_end", fun: enzyme_goto_long_word_end, doc: "Go to end of current long word" };
    pub const enzyme_goto_function_start: Self = Self::Static { name: "enzyme_goto_function_start", fun: enzyme_goto_function_start, doc: "Go to start of current function" };
    pub const enzyme_goto_function_end: Self = Self::Static { name: "enzyme_goto_function_end", fun: enzyme_goto_function_end, doc: "Go to end of current function" };
    pub const enzyme_goto_comment_start: Self = Self::Static { name: "enzyme_goto_comment_start", fun: enzyme_goto_comment_start, doc: "Go to start of current comment" };
    pub const enzyme_goto_comment_end: Self = Self::Static { name: "enzyme_goto_comment_end", fun: enzyme_goto_comment_end, doc: "Go to end of current comment" };
    pub const enzyme_goto_class_start: Self = Self::Static { name: "enzyme_goto_class_start", fun: enzyme_goto_class_start, doc: "Go to start of current class/type" };
    pub const enzyme_goto_class_end: Self = Self::Static { name: "enzyme_goto_class_end", fun: enzyme_goto_class_end, doc: "Go to end of current class/type" };
    pub const enzyme_goto_section_start: Self = Self::Static { name: "enzyme_goto_section_start", fun: enzyme_goto_section_start, doc: "Go to start of current section" };
    pub const enzyme_goto_section_end: Self = Self::Static { name: "enzyme_goto_section_end", fun: enzyme_goto_section_end, doc: "Go to end of current section" };
    pub const enzyme_goto_block_start: Self = Self::Static { name: "enzyme_goto_block_start", fun: enzyme_goto_block_start, doc: "Go to start of current structural block" };
    pub const enzyme_goto_block_end: Self = Self::Static { name: "enzyme_goto_block_end", fun: enzyme_goto_block_end, doc: "Go to end of current structural block" };
    pub const enzyme_add_next_match: Self = Self::Static { name: "enzyme_add_next_match", fun: enzyme_add_next_match, doc: "Add next matching occurrence" };
    pub const enzyme_add_prev_match: Self = Self::Static { name: "enzyme_add_prev_match", fun: enzyme_add_prev_match, doc: "Add previous matching occurrence" };
    pub const enzyme_add_all_matches: Self = Self::Static { name: "enzyme_add_all_matches", fun: enzyme_add_all_matches, doc: "Add all matching occurrences" };
    pub const enzyme_remove_next_selection: Self = Self::Static { name: "enzyme_remove_next_selection", fun: enzyme_remove_next_selection, doc: "Remove next secondary selection" };
    pub const enzyme_remove_prev_selection: Self = Self::Static { name: "enzyme_remove_prev_selection", fun: enzyme_remove_prev_selection, doc: "Remove previous secondary selection" };
    pub const enzyme_select_line: Self = Self::Static { name: "enzyme_select_line", fun: enzyme_select_line, doc: "Select current line" };
    pub const enzyme_select_word: Self = Self::Static { name: "enzyme_select_word", fun: enzyme_select_word, doc: "Select current word" };
    pub const enzyme_select_long_word: Self = Self::Static { name: "enzyme_select_long_word", fun: enzyme_select_long_word, doc: "Select current long word" };
    pub const enzyme_select_function: Self = Self::Static { name: "enzyme_select_function", fun: enzyme_select_function, doc: "Select current function" };
    pub const enzyme_select_comment: Self = Self::Static { name: "enzyme_select_comment", fun: enzyme_select_comment, doc: "Select current comment" };
    pub const enzyme_select_inside_comment: Self = Self::Static { name: "enzyme_select_inside_comment", fun: enzyme_select_inside_comment, doc: "Select inside current comment" };
    pub const enzyme_select_class: Self = Self::Static { name: "enzyme_select_class", fun: enzyme_select_class, doc: "Select current class/type" };
    pub const enzyme_select_section: Self = Self::Static { name: "enzyme_select_section", fun: enzyme_select_section, doc: "Select current section" };
    pub const enzyme_select_block: Self = Self::Static { name: "enzyme_select_block", fun: enzyme_select_block, doc: "Select current structural block" };
    pub const enzyme_select_pair: Self = Self::Static { name: "enzyme_select_pair", fun: enzyme_select_pair, doc: "Select closest matching pair" };
    pub const enzyme_select_inside_pair: Self = Self::Static { name: "enzyme_select_inside_pair", fun: enzyme_select_inside_pair, doc: "Select inside closest matching pair" };
    pub const enzyme_select_file: Self = Self::Static { name: "enzyme_select_file", fun: enzyme_select_file, doc: "Select whole file" };
}
