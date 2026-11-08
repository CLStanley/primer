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

fn enzyme_goto_function_start(cx: &mut Context) { goto_textobject_boundary(cx, 'f', true); }
fn enzyme_goto_function_end(cx: &mut Context) { goto_textobject_boundary(cx, 'f', false); }
fn enzyme_goto_comment_start(cx: &mut Context) { goto_textobject_boundary(cx, 'c', true); }
fn enzyme_goto_comment_end(cx: &mut Context) { goto_textobject_boundary(cx, 'c', false); }
fn enzyme_goto_class_start(cx: &mut Context) { goto_textobject_boundary(cx, 't', true); }
fn enzyme_goto_class_end(cx: &mut Context) { goto_textobject_boundary(cx, 't', false); }
fn enzyme_goto_section_start(cx: &mut Context) { goto_textobject_boundary(cx, 'p', true); }
fn enzyme_goto_section_end(cx: &mut Context) { goto_textobject_boundary(cx, 'p', false); }
fn enzyme_goto_block_start(cx: &mut Context) { goto_textobject_boundary(cx, 'm', true); }
fn enzyme_goto_block_end(cx: &mut Context) { goto_textobject_boundary(cx, 'm', false); }

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

fn enzyme_select_line(cx: &mut Context) { remember_transient_selection(cx, |cx| MappableCommand::extend_line.execute(cx)); }
fn enzyme_select_word(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'w')); }
fn enzyme_select_long_word(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'W')); }
fn enzyme_select_function(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'f')); }
fn enzyme_select_comment(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'c')); }
fn enzyme_select_inside_comment(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, false, 'c')); }
fn enzyme_select_class(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 't')); }
fn enzyme_select_section(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'p')); }
fn enzyme_select_block(cx: &mut Context) { remember_transient_selection(cx, |cx| select_helix_textobject(cx, true, 'm')); }
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
