use helix_view::graphics::Rect;
use tui::buffer::Buffer as Surface;
use tui::text::Text;
use tui::widgets::{Paragraph, Widget};

use super::Dashboard;

pub fn render_dashboard(dashboard: &Dashboard, area: Rect, surface: &mut Surface) {
    let text = Text::from("PRIMER");
    let title = Paragraph::new(&text);

    title.render(area, surface);

    let _ = dashboard;
}
