//! Shared transient notifications. Modal confirmations remain separate.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{job, compositor::{Component, Context, Event, EventResult}, key, ui::popup::Popup};
use helix_view::graphics::{Margin, Rect};
use tui::{buffer::Buffer, text::{Span, Spans, Text}, widgets::{Paragraph, Widget, Wrap}};

const ID: &str = "primer-notification";
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub(crate) enum Severity {
    Success,
    Info,
    Warning,
    Error,
}

impl Severity {
    fn duration(self) -> Option<Duration> {
        match self {
            Self::Success | Self::Info => Some(Duration::from_secs(3)),
            Self::Warning => Some(Duration::from_secs(5)),
            Self::Error => None,
        }
    }

    fn scope(self) -> &'static str {
        match self {
            Self::Success => "diff.plus",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Self::Success => "✓",
            Self::Info => "ⓘ",
            Self::Warning => "⚠",
            Self::Error => "×",
        }
    }

}

struct Notification {
    severity: Severity,
    title: String,
    message: String,
}

impl Notification {
    fn text(&self, cx: Option<&Context>) -> Text<'static> {
        let heading = cx.map(|cx| cx.editor.theme.get(self.severity.scope())).unwrap_or_default();
        let body = cx.map(|cx| cx.editor.theme.get("ui.text")).unwrap_or_default();
        let mut lines = vec![Spans::from(Span::styled(format!("{} {}", self.severity.symbol(), self.title), heading))];
        lines.extend(self.message.lines().map(|line| Spans::from(Span::styled(line.to_owned(), body))));
        Text::from(lines)
    }
}

impl Component for Notification {
    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let text = self.text(Some(cx));
        Paragraph::new(&text).wrap(Wrap { trim: false })
            .render(area.inner(Margin::all(1)), surface);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        let width = viewport.0.min(44);
        let text = self.text(None);
        let (_, height) = super::text::required_size(&text, width.saturating_sub(2).max(1));
        Some((width, height.saturating_add(2)))
    }
}

const MAX_VISIBLE: usize = 3;

#[derive(Default)]
struct NotificationStack {
    entries: Vec<(u64, Option<&'static str>, Popup<Notification>)>,
}

impl Component for NotificationStack {
    fn id(&self) -> Option<&'static str> { Some(ID) }

    fn render(&mut self, area: Rect, surface: &mut Buffer, cx: &mut Context) {
        let mut viewport = area;
        for (_, _, popup) in self.entries.iter_mut().rev() {
            if viewport.height < 6 { break; }
            let rect = popup.area(viewport, cx.editor);
            popup.render(viewport, surface, cx);
            let used = rect.bottom().saturating_sub(viewport.y);
            viewport.y = viewport.y.saturating_add(used);
            viewport.height = viewport.height.saturating_sub(used);
        }
    }

    fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
        if matches!(event, Event::Key(k) if *k == key!(Esc)) {
            return EventResult::Ignored(Some(Box::new(|compositor, _| {
                compositor.remove(ID);
            })));
        }
        EventResult::Ignored(None)
    }
}

/// Display a notification with an independent lifetime.
pub(crate) fn notify(jobs: &mut job::Jobs, severity: Severity, title: impl Into<String>, message: impl Into<String>) {
    let title = title.into();
    let message = message.into();
    jobs.spawn(async move {
        dispatch_notification(severity, title, message).await;
        Ok(())
    });
}

pub(crate) async fn dispatch_notification(severity: Severity, title: impl Into<String>, message: impl Into<String>) {
    let title = title.into();
    let message = message.into();
    job::dispatch(move |_, compositor| show(compositor, severity, title, message)).await;
}

pub(crate) fn show(compositor: &mut crate::compositor::Compositor, severity: Severity, title: impl Into<String>, message: impl Into<String>) {
    show_operation(compositor, None, severity, title, message, false);
}

/// Replace a named operation without disturbing unrelated notifications.
pub(crate) fn show_operation(compositor: &mut crate::compositor::Compositor, operation: Option<&'static str>, severity: Severity, title: impl Into<String>, message: impl Into<String>, persistent: bool) {
    let title = title.into();
    let message = message.into();
            let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
            let popup = Popup::new(ID, Notification { severity, title, message })
                .with_border().with_border_scope(severity.scope()).top_right().with_scrollbar(false);
            if compositor.find_id::<NotificationStack>(ID).is_none() {
                compositor.push(Box::new(NotificationStack::default()));
            }
            if let Some(stack) = compositor.find_id::<NotificationStack>(ID) {
                if let Some(operation) = operation {
                    stack.entries.retain(|(_, key, _)| *key != Some(operation));
                }
                stack.entries.push((generation, operation, popup));
                if stack.entries.len() > MAX_VISIBLE {
                    stack.entries.remove(0);
                }
            }
            if let Some(duration) = if persistent { None } else { severity.duration() } {
                tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    job::dispatch(move |_, compositor| {
                        if let Some(stack) = compositor.find_id::<NotificationStack>(ID) {
                            stack.entries.retain(|(id, _, _)| *id != generation);
                            if stack.entries.is_empty() {
                                compositor.remove(ID);
                            }
                        }
                    }).await;
                });
            }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn severity_durations() {
        assert_eq!(Severity::Success.duration(), Some(Duration::from_secs(3)));
        assert_eq!(Severity::Info.duration(), Some(Duration::from_secs(3)));
        assert_eq!(Severity::Warning.duration(), Some(Duration::from_secs(5)));
        assert_eq!(Severity::Error.duration(), None);
    }
}
