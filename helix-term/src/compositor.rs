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

    /// Keep a long-lived component alive but render the editor above it.
    /// Used while a terminal session is not the active view.
    pub fn park_below_editor<T: 'static>(&mut self) -> bool {
        let kind = std::any::type_name::<T>();
        let editor_kind = std::any::type_name::<crate::ui::EditorView>();
        let Some(index) = self.layers.iter().position(|layer| layer.type_name() == kind) else {
            return false;
        };
        let component = self.layers.remove(index);
        let Some(editor_index) = self.layers.iter().position(|layer| layer.type_name() == editor_kind) else {
            self.layers.insert(index, component);
            return false;
        };
        self.layers.insert(editor_index, component);
        self.need_full_redraw();
        true
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
    /// The focused editor document is the source of truth for whether the
    /// persistent terminal session or the text editor is the active view.
    /// Reorder only these two layers; picker/prompt overlays remain on top.
    pub fn sync_terminal_document(&mut self, editor: &Editor) {
        let Some(active) = editor.tree.try_get(editor.tree.focus).map(|view| view.doc) else {
            return;
        };
        let terminal_kind = std::any::type_name::<crate::ui::terminal::TerminalView>();
        let editor_kind = std::any::type_name::<crate::ui::EditorView>();
        let Some(terminal_index) = self.layers.iter().position(|layer| layer.type_name() == terminal_kind) else {
            return;
        };
        let terminal_id = self.layers[terminal_index].as_any()
            .downcast_ref::<crate::ui::terminal::TerminalView>()
            .map(|terminal| terminal.document_id());
        let Some(terminal_id) = terminal_id else { return };
        let Some(editor_index) = self.layers.iter().position(|layer| layer.type_name() == editor_kind) else {
            return;
        };
        let terminal_should_be_front = active == terminal_id;
        let already_front = terminal_index > editor_index;
        if terminal_should_be_front == already_front {
            return;
        }
        let component = self.layers.remove(terminal_index);
        let editor_index = self.layers.iter().position(|layer| layer.type_name() == editor_kind).unwrap();
        self.layers.insert(if terminal_should_be_front { editor_index + 1 } else { editor_index }, component);
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
        // A parked terminal is a live session, not a background image.
        // Rendering it behind transparent editor buffers leaks its old
        // contents through scratch buffers and creates overlapping chrome.
        let editor_kind = std::any::type_name::<crate::ui::EditorView>();
        let terminal_kind = std::any::type_name::<crate::ui::terminal::TerminalView>();
        let editor_index = self.layers.iter().position(|layer| layer.type_name() == editor_kind);
        let terminal_index = self.layers.iter().position(|layer| layer.type_name() == terminal_kind);
        for (index, layer) in self.layers.iter_mut().enumerate() {
            // Render exactly one full-screen view. The inactive editor must
            // not bleed through a transparent terminal (or vice versa).
            if (layer.type_name() == terminal_kind && editor_index.is_some_and(|editor| index < editor))
                || (layer.type_name() == editor_kind && terminal_index.is_some_and(|terminal| index < terminal))
            {
                continue;
            }
            layer.render(area, surface, cx);
        }
    }

    pub fn cursor(&self, area: Rect, editor: &Editor) -> (Option<Position>, CursorKind) {
        for layer in self.layers.iter().rev() {
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
