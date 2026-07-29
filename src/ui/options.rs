use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, Screen};

/// Draws the pause popup; returns its rect for the active-window border.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) -> Rect {
    let w = 28u16.min(area.width.saturating_sub(2)).max(22);
    let h = 8u16.min(area.height.saturating_sub(2)).max(7);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let exit_label = match app.screen {
        Screen::Title => "Exit",
        Screen::Game => "Exit to title",
    };

    let lines = vec![
        Line::from(Span::styled(
            "PAUSED",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {exit_label}"),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Enter confirm · Esc resume"),
    ];

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" @_Term ")
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        popup,
    );
    popup
}
