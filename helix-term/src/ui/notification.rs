//! Shared transient notifications. Modal confirmations remain separate.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{job, compositor::{Component, Context}, ui::popup::Popup};
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

/// Display the latest notification without requiring a confirmation.
/// Older timers cannot remove a newer notification.
pub(crate) fn notify(jobs: &mut job::Jobs, severity: Severity, title: impl Into<String>, message: impl Into<String>) {
    let title = title.into();
    let message = message.into();
    jobs.spawn(async move {
        job::dispatch(move |_, compositor| {
            let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
            let popup = Popup::new(ID, Notification { severity, title, message })
                .with_border().with_border_scope(severity.scope()).top_right().with_scrollbar(false).auto_close(true);
            compositor.replace_or_push(ID, popup);
            if let Some(duration) = severity.duration() {
                tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    job::dispatch(move |_, compositor| {
                        if GENERATION.load(Ordering::Relaxed) == generation {
                            compositor.remove(ID);
                        }
                    }).await;
                });
            }
        }).await;
        Ok(())
    });
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
