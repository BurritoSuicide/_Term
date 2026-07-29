use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::ui::arena::title_bg_cell;

const LOGO: &[&str] = &[
    r"    $$$$$$\        $$$$$$$$\                                ",
    r"  $$$ ___$$$\      \__$$  __|                               ",
    r" $$ _/   \_$$\        $$ | $$$$$$\   $$$$$$\  $$$$$$\$$$$\  ",
    r"$$ / $$$$$\ $$\       $$ |$$  __$$\ $$  __$$\ $$  _$$  _$$\ ",
    r"$$ |$$  $$ |$$ |      $$ |$$$$$$$$ |$$ |  \__|$$ / $$ / $$ |",
    r"$$ |$$ /$$ |$$ |      $$ |$$   ____|$$ |      $$ | $$ | $$ |",
    r"$$ |\$$$$$$$$  |      $$ |\$$$$$$$\ $$ |      $$ | $$ | $$ |",
    r"\$$\ \________/$$$$$$\\__| \_______|\__|      \__| \__| \__|",
    r" \$$$\   $$$\  \______|                                     ",
    r"  \_$$$$$$  _|                                              ",
    r"    \______/                                                ",
];

const BLURB: &str = "A rougelike fully within the terminal! Infinitely stacking projectiles, with mods that can augment your weapons. Bosses, loot, and glory await you in this terminal.";

/// Draws the title panel; returns its rect for the active-window border.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) -> Rect {
    let panel = inset(area, 2, 1);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" $_Term ")
        .border_style(Style::default().fg(Color::Rgb(160, 120, 255)));
    let inner = block.inner(panel);
    frame.render_widget(block, panel);

    if inner.width < 8 || inner.height < 6 {
        return panel;
    }

    let w = inner.width as usize;
    let h = inner.height as usize;
    let t = app.ui_t;

    // Mandala floor fill.
    let mut cells: Vec<Vec<(char, Color)>> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| title_bg_cell(x, y, w, h, t))
                .collect()
        })
        .collect();

    // Centered ASCII logo.
    let logo_w = LOGO.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let logo_x = w.saturating_sub(logo_w) / 2;
    let logo_y = 1usize.min(h.saturating_sub(1));
    let pulse = 0.75 + 0.25 * (t * 2.4).sin();
    for (row, line) in LOGO.iter().enumerate() {
        let y = logo_y + row;
        if y >= h {
            break;
        }
        for (col, ch) in line.chars().enumerate() {
            let x = logo_x + col;
            if x >= w || ch == ' ' {
                continue;
            }
            let hue = 195.0 + col as f32 * 1.8 + row as f32 * 4.0 + t * 18.0;
            let color = crate::ui::arena::faded_hsv(hue, 0.85, (0.55 + 0.35 * pulse).min(0.95));
            // Brighter than floor so the logo pops.
            let color = match color {
                Color::Rgb(r, g, b) => Color::Rgb(
                    (r as f32 * 0.35 + 120.0 * pulse).min(255.0) as u8,
                    (g as f32 * 0.45 + 200.0 * pulse).min(255.0) as u8,
                    (b as f32 * 0.55 + 255.0 * pulse).min(255.0) as u8,
                ),
                other => other,
            };
            cells[y][x] = (ch, color);
        }
    }

    // Description under the logo.
    let blurb_y = logo_y + LOGO.len() + 1;
    if blurb_y < h {
        let max_w = w.saturating_sub(6).max(20);
        let wrapped = wrap_text(BLURB, max_w);
        let start_y = blurb_y.min(h.saturating_sub(wrapped.len().saturating_add(3)));
        for (i, line) in wrapped.iter().enumerate() {
            let y = start_y + i;
            if y >= h {
                break;
            }
            paint_centered(&mut cells, w, y, line, Color::Rgb(210, 200, 230));
        }
    }

    // Bottom-left controls / bottom-right seed.
    let controls = [
        "WASD move · Shift dash · Arrows aim",
        "Enter start · d random seed · Esc · q quit",
        "Type to edit seed · Tab inventory in-run",
    ];
    let seed_line = format!("Seed: {}_", app.seed_input);
    let bottom = h.saturating_sub(1);
    let ctrl_top = bottom.saturating_sub(controls.len().saturating_sub(1));
    for (i, line) in controls.iter().enumerate() {
        let y = ctrl_top + i;
        if y < h {
            paint_left(&mut cells, w, y, line, Color::Rgb(150, 160, 200));
        }
    }
    paint_right(&mut cells, w, bottom, &seed_line, Color::Rgb(255, 220, 120));

    let lines: Vec<Line> = cells
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(ch, color)| {
                        Span::styled(ch.to_string(), Style::default().fg(color))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
    panel
}

fn inset(area: Rect, pad_x: u16, pad_y: u16) -> Rect {
    let x = area.x.saturating_add(pad_x);
    let y = area.y.saturating_add(pad_y);
    let width = area.width.saturating_sub(pad_x.saturating_mul(2)).max(10);
    let height = area.height.saturating_sub(pad_y.saturating_mul(2)).max(8);
    // Keep inside parent.
    let width = width.min(area.width.saturating_sub(x.saturating_sub(area.x)));
    let height = height.min(area.height.saturating_sub(y.saturating_sub(area.y)));
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(12);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(cur);
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn paint_centered(cells: &mut [Vec<(char, Color)>], w: usize, y: usize, text: &str, color: Color) {
    let chars: Vec<char> = text.chars().collect();
    let start = w.saturating_sub(chars.len()) / 2;
    for (i, ch) in chars.into_iter().enumerate() {
        let x = start + i;
        if x < w {
            cells[y][x] = (ch, color);
        }
    }
}

fn paint_left(cells: &mut [Vec<(char, Color)>], w: usize, y: usize, text: &str, color: Color) {
    for (i, ch) in text.chars().enumerate() {
        if i + 1 >= w {
            break;
        }
        cells[y][i + 1] = (ch, color);
    }
}

fn paint_right(cells: &mut [Vec<(char, Color)>], w: usize, y: usize, text: &str, color: Color) {
    let chars: Vec<char> = text.chars().collect();
    let start = w.saturating_sub(chars.len() + 1);
    for (i, ch) in chars.into_iter().enumerate() {
        let x = start + i;
        if x < w {
            cells[y][x] = (ch, color);
        }
    }
}
