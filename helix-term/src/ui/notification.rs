//! Shared transient notifications. Modal confirmations remain separate.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{job, ui::{popup::Popup, GitChangePreview}};

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

    fn prefix(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

/// Display the latest notification without requiring a confirmation.
/// Older timers cannot remove a newer notification.
pub(crate) fn notify(jobs: &mut job::Jobs, severity: Severity, title: impl Into<String>, message: impl Into<String>) {
    let title = format!("{}: {}", severity.prefix(), title.into());
    let message = message.into();
    jobs.spawn(async move {
        job::dispatch(move |_, compositor| {
            let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
            let popup = Popup::new(ID, GitChangePreview::new(message, Vec::new())
                .with_title(&title).compact())
                .with_border().top_right().with_scrollbar(false).auto_close(true);
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
