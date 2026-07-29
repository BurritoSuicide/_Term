use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Animated gradient border around a focused panel (crypt, weave, pause, …).
pub fn paint_animated_border(buf: &mut Buffer, area: Rect, t: f32) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let accents = [
        Color::Rgb(0, 220, 220),
        Color::Rgb(80, 160, 255),
        Color::Rgb(220, 80, 220),
        Color::Rgb(120, 255, 255),
        Color::Rgb(255, 220, 80),
        Color::Rgb(255, 120, 200),
    ];
    let perimeter = (area.width as usize + area.height as usize) * 2;
    let mut i = 0usize;

    let mut put = |x: u16, y: u16, ch: char| {
        let phase = (i as f32 / perimeter.max(1) as f32) + t * 0.4;
        let idx = ((phase * accents.len() as f32).rem_euclid(accents.len() as f32)) as usize;
        let next = (idx + 1) % accents.len();
        let frac = (phase * accents.len() as f32).rem_euclid(1.0);
        let color = lerp_color(accents[idx], accents[next], frac);
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(&ch.to_string());
            cell.set_fg(color);
        }
        i += 1;
    };

    for x in area.left()..area.right() {
        put(x, area.top(), '═');
    }
    for y in area.top() + 1..area.bottom() {
        put(area.right().saturating_sub(1), y, '║');
    }
    for x in (area.left()..area.right()).rev() {
        put(x, area.bottom().saturating_sub(1), '═');
    }
    for y in (area.top() + 1..area.bottom().saturating_sub(1)).rev() {
        put(area.left(), y, '║');
    }
    for (x, y, ch) in [
        (area.left(), area.top(), "╔"),
        (area.right().saturating_sub(1), area.top(), "╗"),
        (area.left(), area.bottom().saturating_sub(1), "╚"),
        (
            area.right().saturating_sub(1),
            area.bottom().saturating_sub(1),
            "╝",
        ),
    ] {
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_symbol(ch);
        }
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (ar, ag, ab) = match a {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (180, 200, 220),
    };
    let (br, bg, bb) = match b {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (180, 200, 220),
    };
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        (ar as f32 + (br as f32 - ar as f32) * t) as u8,
        (ag as f32 + (bg as f32 - ag as f32) * t) as u8,
        (ab as f32 + (bb as f32 - ab as f32) * t) as u8,
    )
}
