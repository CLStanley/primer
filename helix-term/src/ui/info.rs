use helix_view::editor::UiBorderStyle;
use tui::widgets::BorderType;
use crate::compositor::{Component, Context};
use helix_view::graphics::{Margin, Rect};
use helix_view::info::Info;
use tui::buffer::Buffer as Surface;
use tui::text::Text;
use tui::widgets::{Block, Paragraph, Widget};

impl Component for Info {
    fn render(&mut self, viewport: Rect, surface: &mut Surface, cx: &mut Context) {
        let text_style = cx.editor.theme.get("ui.text.info");
        let popup_style = cx.editor.theme.get("ui.popup.info");

        // Calculate the area of the terminal to modify. Because we want to
        // render at the bottom right, we use the viewport's width and height
        // which evaluate to the most bottom right coordinate.
        let width = self.width + 2 + 2; // +2 for border, +2 for margin
        let height = self.height + 2; // +2 for border
        // Rect coordinates are absolute. Account for the pane origin when
        // anchoring the popup to its bottom-right corner.
        let area = viewport.intersection(Rect::new(
            viewport.x.saturating_add(viewport.width.saturating_sub(width)),
            viewport.y.saturating_add(viewport.height.saturating_sub(height + 2)),
            width,
            height,
        ));
        surface.clear_with(area, popup_style);

        let block = Block::bordered()
            .title(self.title.as_ref())
            .border_type(match cx.editor.config().border_style { UiBorderStyle::Square => BorderType::Plain, UiBorderStyle::Rounded => BorderType::Rounded, UiBorderStyle::Double => BorderType::Double })
            .border_style(cx.editor.theme.get("ui.window"));

        let margin = Margin::horizontal(1);
        let inner = block.inner(area).inner(margin);
        block.render(area, surface);

        Paragraph::new(&Text::from(self.text.as_str()))
            .style(text_style)
            .render(inner, surface);
    }
}
