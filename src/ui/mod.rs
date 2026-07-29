pub mod arena;
pub mod hud;
pub mod inventory;
pub mod options;
pub mod shop;
pub mod title;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::{App, Screen, UiMode};
use crate::world::GamePhase;

/// Draws the frame and returns the active-window rect for the animated border.
pub fn draw(frame: &mut Frame, app: &App) -> Option<Rect> {
    let area = frame.area();
    let mut focus = match app.screen {
        Screen::Title => Some(title::draw(frame, app, area)),
        Screen::Game => draw_game(frame, app, area),
    };

    if app.options_open {
        focus = Some(options::draw(frame, area, app));
    }

    focus
}

fn draw_game(frame: &mut Frame, app: &App, area: Rect) -> Option<Rect> {
    let world = app.world.as_ref()?;

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(1)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(root[0]);

    // Left column: DPS + Dash above the arena.
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(6)])
        .split(top[0]);
    let top_bar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(24), Constraint::Length(22)])
        .split(left[0]);
    hud::draw_dps_waveform(
        frame,
        top_bar[0],
        world,
        world.current_dps(),
        app.fps_history.back().copied().unwrap_or(0.0),
    );
    hud::draw_dash_gauge(frame, top_bar[1], world);
    arena::draw(frame, left[1], world);

    // Side: Nucleus → Marks → gauges (journalctl …)
    let side_area = top[1];
    let payload_n = world.nucleus.filled_projectile_ids().len().max(1);
    // borders + title + one line per payload
    let marks_h = (2 + payload_n as u16 + 1).clamp(4, 10);
    // Favor journal space (≥10 lines ≈ 12+ borders + run strip).
    let gauge_h = 22u16
        .min(side_area.height.saturating_sub(marks_h + 3))
        .max(15.min(side_area.height.saturating_sub(marks_h)));
    let loadout_h = side_area.height.saturating_sub(gauge_h + marks_h);

    let loadout_rect = Rect {
        x: side_area.x,
        y: side_area.y,
        width: side_area.width,
        height: loadout_h,
    };
    let marks_rect = Rect {
        x: side_area.x,
        y: side_area.y.saturating_add(loadout_h),
        width: side_area.width,
        height: marks_h,
    };
    let gauges_rect = Rect {
        x: side_area.x,
        y: side_area.y.saturating_add(loadout_h + marks_h),
        width: side_area.width,
        height: gauge_h,
    };

    let side_focus = if world.phase == GamePhase::Shop && app.ui_mode == UiMode::Inventory {
        inventory::draw(frame, loadout_rect, world);
        true
    } else if world.phase == GamePhase::Shop {
        shop::draw(frame, loadout_rect, world);
        true
    } else if app.ui_mode == UiMode::Inventory {
        inventory::draw(frame, loadout_rect, world);
        true
    } else {
        hud::draw_side(frame, loadout_rect, world, app.ui_mode);
        false
    };
    hud::draw_marks(frame, marks_rect, world);
    hud::draw_gauges(frame, gauges_rect, world, app, app.ui_mode);
    hud::draw_status(frame, root[1], world);

    if world.phase == GamePhase::Dead {
        draw_death_overlay(frame, area, world);
    }

    Some(if side_focus { top[1] } else { left[1] })
}

fn draw_death_overlay(frame: &mut Frame, area: Rect, world: &crate::world::World) {
    use ratatui::style::{Modifier, Style};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let popup = centered(area, 60, 10);
    frame.render_widget(Clear, popup);
    let text = format!(
        "YOU DIED\nSeed: {} ({})\nRooms cleared: {}\nBosses: {}\nNucleus: {}\n\nPress Enter for title · q quit",
        world.seed_label,
        world.seed,
        world.rooms_cleared,
        world.boss_kills,
        world
            .nucleus
            .filled_ids()
            .iter()
            .map(|id| world
                .lib
                .get(id)
                .map(|s| s.name.as_str())
                .unwrap_or(id))
            .collect::<Vec<_>>()
            .join(" > ")
    );
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .style(Style::default().add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL).title(" Permadeath ")),
        popup,
    );
}

fn centered(area: Rect, percent_x: u16, height: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height) / 2),
            Constraint::Length(height),
            Constraint::Percentage((100 - height) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
