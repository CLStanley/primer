// Each component declares its own size constraints and gets fitted based on its parent.
// Q: how does this work with popups?
// cursive does compositor.screen_mut().add_layer_at(pos::absolute(x, y), <component>)
use helix_core::Position;
use helix_view::graphics::{CursorKind, Rect};

use tui::buffer::Buffer as Surface;

pub type Callback = Box<dyn FnOnce(&mut Compositor, &mut Context)>;
pub type SyncCallback = Box<dyn FnOnce(&mut Compositor, &mut Context) + Sync>;

// Cursive-inspired
pub enum EventResult {
    Ignored(Option<Callback>),
    Consumed(Option<Callback>),
}

use crate::job::Jobs;
use crate::ui::picker;
use helix_view::Editor;

pub use helix_view::input::Event;

pub struct Context<'a> {
    pub editor: &'a mut Editor,
    pub scroll: Option<usize>,
    pub jobs: &'a mut Jobs,
}

impl Context<'_> {
    /// Waits on all pending jobs, and then tries to flush all pending write
    /// operations for all documents.
    pub fn block_try_flush_writes(&mut self) -> anyhow::Result<()> {
        tokio::task::block_in_place(|| helix_lsp::block_on(self.jobs.finish(self.editor, None)))?;
        tokio::task::block_in_place(|| helix_lsp::block_on(self.editor.flush_writes()))?;
        Ok(())
    }
}

pub trait Component: Any + AnyComponent {
    /// Process input events, return true if handled.
    fn handle_event(&mut self, _event: &Event, _ctx: &mut Context) -> EventResult {
        EventResult::Ignored(None)
    }
    // , args: ()

    /// Should redraw? Useful for saving redraw cycles if we know component didn't change.
    fn should_update(&self) -> bool {
        true
    }

    /// Render the component onto the provided surface.
    fn render(&mut self, area: Rect, frame: &mut Surface, ctx: &mut Context);

    /// Get cursor position and cursor kind.
    fn cursor(&self, _area: Rect, _ctx: &Editor) -> (Option<Position>, CursorKind) {
        (None, CursorKind::Hidden)
    }

    /// May be used by the parent component to compute the child area.
    /// viewport is the maximum allowed area, and the child should stay within those bounds.
    ///
    /// The returned size might be larger than the viewport if the child is too big to fit.
    /// In this case the parent can use the values to calculate scroll.
    fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
        None
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn id(&self) -> Option<&'static str> {
        None
    }
}

pub struct Compositor {
    layers: Vec<Box<dyn Component>>,
    area: Rect,

    pub(crate) last_picker: Option<Box<dyn Component>>,
    pub(crate) full_redraw: bool,
}

impl Compositor {
    pub fn new(area: Rect) -> Self {
        Self {
            layers: Vec::new(),
            area,
            last_picker: None,
            full_redraw: false,
        }
    }

    pub fn size(&self) -> Rect {
        self.area
    }

    pub fn resize(&mut self, area: Rect) {
        self.area = area;
    }

    /// Add a layer to be rendered in front of all existing layers.
    pub fn push(&mut self, mut layer: Box<dyn Component>) {
        // immediately clear last_picker field to avoid excessive memory
        // consumption for picker with many items
        if layer.id() == Some(picker::ID) {
            self.last_picker = None;
        }
        let size = self.size();
        // trigger required_size on init
        layer.required_size((size.width, size.height));
        self.layers.push(layer);
    }

    /// Replace a component that has the given `id` with the new layer and if
    /// no component is found, push the layer normally.
    pub fn replace_or_push<T: Component>(&mut self, id: &'static str, layer: T) {
        if let Some(component) = self.find_id(id) {
            *component = layer;
        } else {
            self.push(Box::new(layer))
        }
    }

    pub fn pop(&mut self) -> Option<Box<dyn Component>> {
        self.layers.pop()
    }

    pub fn remove(&mut self, id: &'static str) -> Option<Box<dyn Component>> {
        let idx = self
            .layers
            .iter()
            .position(|layer| layer.id() == Some(id))?;
        Some(self.layers.remove(idx))
    }

    /// Bring an existing session back instead of spawning another PTY.
    pub fn raise_type<T: 'static>(&mut self) -> bool {
        let kind = std::any::type_name::<T>();
        let Some(index) = self.layers.iter().position(|layer| layer.type_name() == kind) else {
            return false;
        };
        let component = self.layers.remove(index);
        self.layers.push(component);
        self.need_full_redraw();
        true
    }

    pub fn remove_type<T: 'static>(&mut self) {
        let type_name = std::any::type_name::<T>();
        self.layers
            .retain(|component| component.type_name() != type_name);
    }
    /// Look up a PTY by its backing editor document, not layer order.
    pub fn find_terminal_document(
        &mut self,
        id: helix_view::DocumentId,
    ) -> Option<&mut crate::ui::terminal::TerminalView> {
        self.layers.iter_mut().find_map(|layer| {
            let terminal = layer.as_any_mut()
                .downcast_mut::<crate::ui::terminal::TerminalView>()?;
            (terminal.document_id() == id).then_some(terminal)
        })
    }

    /// Remove only the terminal session associated with the given document.
    pub fn remove_terminal_document(&mut self, id: helix_view::DocumentId) {
        self.layers.retain(|layer| {
            layer.as_any()
                .downcast_ref::<crate::ui::terminal::TerminalView>()
                .is_none_or(|terminal| terminal.document_id() != id)
        });
        self.need_full_redraw();
    }

    /// Keep the selected terminal above the editor and every other terminal
    /// below it. Document selection, not compositor insertion order, owns focus.
    pub fn sync_terminal_document(&mut self, editor: &Editor) {
        let Some(active) = editor.tree.try_get(editor.tree.focus).map(|view| view.doc) else {
            return;
        };
        let terminal_kind = std::any::type_name::<crate::ui::terminal::TerminalView>();
        let editor_kind = std::any::type_name::<crate::ui::EditorView>();
        let Some(editor_index) = self.layers.iter().position(|layer| layer.type_name() == editor_kind) else {
            return;
        };
        let active_terminal = self.layers.iter().position(|layer| {
            layer.type_name() == terminal_kind
                && layer.as_any()
                    .downcast_ref::<crate::ui::terminal::TerminalView>()
                    .is_some_and(|terminal| terminal.document_id() == active)
        });
        let Some(index) = active_terminal else {
            // A file is active. Ensure every terminal stays behind EditorView.
            let mut changed = false;
            while let Some(index) = self.layers.iter().enumerate().find_map(|(index, layer)| {
                (index > self.layers.iter().position(|l| l.type_name() == editor_kind)?
                    && layer.type_name() == terminal_kind).then_some(index)
            }) {
                let terminal = self.layers.remove(index);
                let editor_index = self.layers.iter().position(|l| l.type_name() == editor_kind).unwrap();
                self.layers.insert(editor_index, terminal);
                changed = true;
            }
            if changed { self.need_full_redraw(); }
            return;
        };
        // Move all inactive terminals behind the editor before raising the
        // selected session. This keeps input and rendering on one session.
        let active_is_front = index > editor_index && !self.layers.iter().enumerate().any(|(i, layer)| i > index && layer.type_name() == terminal_kind);
        let any_inactive_front = self.layers.iter().enumerate().any(|(i, layer)| i > editor_index && i != index && layer.type_name() == terminal_kind);
        if active_is_front && !any_inactive_front { return; }
        let active_layer = self.layers.remove(index);
        while let Some(index) = self.layers.iter().enumerate().find_map(|(index, layer)| {
            (index > self.layers.iter().position(|l| l.type_name() == editor_kind)?
                && layer.type_name() == terminal_kind).then_some(index)
        }) {
            let layer = self.layers.remove(index);
            let editor_index = self.layers.iter().position(|l| l.type_name() == editor_kind).unwrap();
            self.layers.insert(editor_index, layer);
        }
        let editor_index = self.layers.iter().position(|l| l.type_name() == editor_kind).unwrap();
        self.layers.insert(editor_index + 1, active_layer);
        self.need_full_redraw();
    }

    pub fn handle_event(&mut self, event: &Event, cx: &mut Context) -> bool {
        self.sync_terminal_document(cx.editor);
        // If it is a key event, a macro is being recorded, and a macro isn't being replayed,
        // push the key event to the recording.
        if let (Event::Key(key), Some((_, keys))) = (event, &mut cx.editor.macro_recording) {
            if cx.editor.macro_replaying.is_empty() {
                keys.push(*key);
            }
        }

        let mut callbacks = Vec::new();
        let mut consumed = false;

        // propagate events through the layers until we either find a layer that consumes it or we
        // run out of layers (event bubbling), starting at the front layer and then moving to the
        // background.
        for layer in self.layers.iter_mut().rev() {
            match layer.handle_event(event, cx) {
                EventResult::Consumed(Some(callback)) => {
                    callbacks.push(callback);
                    consumed = true;
                    break;
                }
                EventResult::Consumed(None) => {
                    consumed = true;
                    break;
                }
                EventResult::Ignored(Some(callback)) => {
                    callbacks.push(callback);
                }
                EventResult::Ignored(None) => {}
            };
        }

        for callback in callbacks {
            callback(self, cx)
        }
        self.sync_terminal_document(cx.editor);

        // The dashboard remains underneath dashboard-launched pickers so Esc
        // returns home. Once a real file is opened, dismiss that home layer.
        // An explicit New file action dismisses it in the dashboard itself.
        if self.find::<crate::ui::dashboard::Dashboard>().is_some()
            && cx.editor.documents().any(|doc| doc.path().is_some())
        {
            self.remove_type::<crate::ui::dashboard::Dashboard>();
        }

        consumed
    }

    pub fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        self.sync_terminal_document(cx.editor);
        let terminal_kind = std::any::type_name::<crate::ui::terminal::TerminalView>();
        let editor_kind = std::any::type_name::<crate::ui::EditorView>();

        // Draw the editor's real split tree first, including the shared
        // bufferline, pane borders and statuslines.
        for layer in self.layers.iter_mut() {
            if layer.type_name() == editor_kind {
                layer.render(area, surface, cx);
            }
        }

        // Paint every visible terminal in its own editor view. A terminal
        // document may be displayed in more than one split.
        let panes: Vec<_> = cx.editor.tree.views()
            .filter(|(view, _)| cx.editor.terminal_documents.contains(&view.doc))
            .map(|(view, focused)| (view.doc, view.area, focused))
            .collect();
        for (id, pane, focused) in panes {
            if let Some(terminal) = self.find_terminal_document(id) {
                // The editor reserves the bottom row of each view for its
                // statusline. Paint only the terminal's content area.
                let content = pane.clip_bottom(1);
                terminal.render(content, surface, cx);
                if focused {
                    // The terminal statusline reflects PTY input mode and size.
                    let (columns, rows) = terminal.terminal().screen().size();
                    let input_mode = terminal.input_mode() == crate::terminal::TerminalInputMode::Terminal;
                    let name = cx.editor.terminal_names.get(&id).map(String::as_str).unwrap_or("terminal");
                    if let Some(view) = cx.editor.tree.views().find(|(view, _)| view.doc == id).map(|(view, _)| view) {
                        if let Some(doc) = cx.editor.documents.get(&id) {
                            let spinners = crate::ui::ProgressSpinners::default();
                            let mut context = crate::ui::statusline::RenderContext::new(cx.editor, doc, view, true, &spinners);
                            context.terminal = Some(crate::ui::statusline::TerminalStatus {
                                name, input_mode, columns, rows,
                            });
                            let status = pane.clip_top(pane.height.saturating_sub(1));
                            crate::ui::statusline::render(&mut context, status, surface);
                        }
                    }
                }
            }
        }

        // Make adjacent split boundaries visible without consuming PTY cells.
        // The separator is painted over the last column of the left pane.
        let views: Vec<_> = cx.editor.tree.views().map(|(view, _)| view.area).collect();
        let separator_style = cx.editor.theme.get("ui.window");
        for pane in &views {
            if pane.x > area.x && views.iter().any(|other| {
                other.x.saturating_add(other.width) == pane.x
                    && other.y < pane.y.saturating_add(pane.height)
                    && pane.y < other.y.saturating_add(other.height)
            }) {
                let x = pane.x - 1;
                for y in pane.y..pane.y.saturating_add(pane.height) {
                    surface.set_string(x, y, "│", separator_style);
                }
            }
            if pane.y > area.y && views.iter().any(|other| {
                other.y.saturating_add(other.height) == pane.y
                    && other.x < pane.x.saturating_add(pane.width)
                    && pane.x < other.x.saturating_add(other.width)
            }) {
                let y = pane.y - 1;
                for x in pane.x..pane.x.saturating_add(pane.width) {
                    surface.set_string(x, y, "─", separator_style);
                }
            }
        }

        // Dialogs, prompts and other overlays remain above the editor panes.
        for layer in self.layers.iter_mut() {
            if layer.type_name() != editor_kind && layer.type_name() != terminal_kind {
                layer.render(area, surface, cx);
            }
        }

        // EditorView draws its Which-Key popup before terminal pane surfaces.
        // Repaint the shared popup above PTYs while a terminal has focus.
        if let Some(pane) = cx.editor.tree.try_get(cx.editor.tree.focus)
            .filter(|view| cx.editor.terminal_documents.contains(&view.doc))
            .map(|view| view.area)
        {
            if let Some(mut info) = cx.editor.autoinfo.take() {
                info.render(pane, surface, cx);
                cx.editor.autoinfo = Some(info);
            }
        }
    }

    pub fn cursor(&self, area: Rect, editor: &Editor) -> (Option<Position>, CursorKind) {
        for layer in self.layers.iter().rev() {
            if let Some(terminal) = layer.as_any().downcast_ref::<crate::ui::terminal::TerminalView>() {
                let Some(view) = editor.tree.try_get(editor.tree.focus) else { continue };
                if view.doc != terminal.document_id() { continue; }
                return terminal.cursor(view.area.clip_bottom(1), editor);
            }
            if let (Some(pos), kind) = layer.cursor(area, editor) {
                return (Some(pos), kind);
            }
        }
        (None, CursorKind::Hidden)
    }

    pub fn has_component(&self, type_name: &str) -> bool {
        self.layers
            .iter()
            .any(|component| component.type_name() == type_name)
    }

    pub fn find<T: 'static>(&mut self) -> Option<&mut T> {
        let type_name = std::any::type_name::<T>();
        self.layers
            .iter_mut()
            .find(|component| component.type_name() == type_name)
            .and_then(|component| component.as_any_mut().downcast_mut())
    }

    pub fn find_id<T: 'static>(&mut self, id: &'static str) -> Option<&mut T> {
        self.layers
            .iter_mut()
            .find(|component| component.id() == Some(id))
            .and_then(|component| component.as_any_mut().downcast_mut())
    }

    pub fn need_full_redraw(&mut self) {
        self.full_redraw = true;
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

// View casting, taken straight from Cursive

use std::any::Any;

/// A view that can be downcasted to its concrete type.
///
/// This trait is automatically implemented for any `T: Component`.
pub trait AnyComponent {
    /// Downcast self to a `Any`.
    fn as_any(&self) -> &dyn Any;

    /// Downcast self to a mutable `Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Returns a boxed any from a boxed self.
    ///
    /// Can be used before `Box::downcast()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use helix_term::{ui::Text, compositor::Component};
    /// let boxed: Box<dyn Component> = Box::new(Text::new("text".to_string()));
    /// let text: Box<Text> = boxed.as_boxed_any().downcast().unwrap();
    /// ```
    fn as_boxed_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Component> AnyComponent for T {
    /// Downcast self to a `Any`.
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// Downcast self to a mutable `Any`.
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn as_boxed_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl dyn AnyComponent {
    /// Attempts to downcast `self` to a concrete type.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    /// Attempts to downcast `self` to a concrete type.
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }

    /// Attempts to downcast `Box<Self>` to a concrete type.
    pub fn downcast<T: Any>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        // Do the check here + unwrap, so the error
        // value is `Self` and not `dyn Any`.
        if self.as_any().is::<T>() {
            Ok(self.as_boxed_any().downcast().unwrap())
        } else {
            Err(self)
        }
    }

    /// Checks if this view is of type `T`.
    pub fn is<T: Any>(&mut self) -> bool {
        self.as_any().is::<T>()
    }
}
