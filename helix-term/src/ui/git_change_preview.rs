use crate::compositor::{Component, Context};
use helix_view::graphics::{Margin, Rect};
use tui::{
    buffer::Buffer,
    text::{Span, Spans, Text},
    widgets::{Paragraph, Widget, Wrap},
};

/// A read-only, cursor-anchored Git preview with character-level removal highlights.
pub struct GitChangePreview {
    lines: Vec<(String, String, String, String)>,
    heading: String,
    title: String,
}

impl GitChangePreview {
    pub fn new(heading: String, lines: Vec<(String, String, String, String)>) -> Self {
        Self { heading, lines, title: "Git change preview · HEAD".to_owned() }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    fn text(&self, cx: Option<&Context>) -> Text<'static> {
        let normal = cx.map(|cx| cx.editor.theme.get("ui.text")).unwrap_or_default();
        let removed = cx.map(|cx| cx.editor.theme.get("diff.minus")).unwrap_or_default();
        let title = cx.map(|cx| cx.editor.theme.get("ui.text.focus")).unwrap_or_default();
        let mut lines = vec![
            Spans::from(Span::styled(self.title.clone(), title)),
            Spans::from(""),
            Spans::from(self.heading.clone()),
            Spans::from(""),
        ];
        for (prefix, before, changed, after) in &self.lines {
            lines.push(Spans::from(vec![
                Span::styled(prefix.clone(), normal),
                Span::styled(before.clone(), normal),
                Span::styled(changed.clone(), removed),
                Span::styled(after.clone(), normal),
            ]));
        }
        Text::from(lines)
    }
}

impl Component for GitChangePreview {
    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let text = self.text(Some(cx));
        Paragraph::new(&text)
            .wrap(Wrap { trim: false })
            .scroll((cx.scroll.unwrap_or_default() as u16, 0))
            .render(area.inner(Margin::all(1)), surface);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        let text = self.text(None);
        let (width, height) = super::text::required_size(&text, viewport.0.saturating_sub(2).max(1));
        Some((width.saturating_add(2), height.saturating_add(2)))
    }
}
