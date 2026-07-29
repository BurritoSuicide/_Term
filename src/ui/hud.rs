use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

use crate::app::{App, UiMode};
use crate::procgen::rooms;
use crate::proj_logic::{mark_color, mark_label};
use crate::world::entity::ActorKind;
use crate::world::World;

pub fn draw_side(frame: &mut Frame, area: Rect, world: &World, mode: UiMode) {
    let mut lines = vec![
        Line::from(Span::styled(
            "NUCLEUS",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "proj {}/{} · mods×{}",
            world.nucleus.occupied_projectiles(),
            world.nucleus.slot_count(),
            world.nucleus.mod_capacity
        )),
        Line::from(""),
    ];
    for (i, slot) in world.nucleus.slots.iter().enumerate() {
        if let Some(id) = slot.projectile.as_ref() {
            if let Some(def) = world.lib.get(id) {
                let mark = world.marks.get(id).mark;
                let color = mark_color(mark, world.anim_t);
                let firing = world.autofire_slot == i;
                lines.push(Line::from(vec![
                    Span::raw(format!("{}[{}] ", if firing { ">" } else { " " }, i + 1)),
                    Span::styled(
                        format!(
                            "{} {} {}",
                            def.rarity.pickup_glyph(),
                            def.name,
                            mark_label(mark)
                        ),
                        Style::default().fg(color),
                    ),
                ]));
                for mid in &slot.mods {
                    if let Some(mdef) = world.lib.get(mid) {
                        let mm = crate::proj_logic::mod_mark_from_count(world.count_owned(mid));
                        lines.push(Line::from(Span::styled(
                            format!(
                                "    ↳ {} {}",
                                mdef.name,
                                mark_label(mm)
                            ),
                            Style::default().fg(mark_color(mm, world.anim_t)),
                        )));
                    }
                }
            }
        } else {
            lines.push(Line::from(format!(" [{}] · empty ·", i + 1)));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "SKILLS {}/{}",
            world.skills.active.len(),
            world.skills.max_active
        ),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if world.skills.active.is_empty() {
        lines.push(Line::from(" none equipped"));
    }
    for id in &world.skills.active {
        let def = crate::skills::def(*id);
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", def.glyph), Style::default().fg(def.color)),
            Span::styled(def.name, Style::default().fg(def.color)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!("Credits {}", world.credits)));
    lines.push(Line::from(format!("Gold {}", world.gold)));
    lines.push(Line::from(format!("Army {}", world.minion_count())));
    if world.guard_charges > 0
        || world.fleet_dash
        || world.overclock_t > 0.0
        || world.twin_volley_t > 0.0
        || world.room_homing
        || world.proj_vuln_t > 0.0
    {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "BUFFS",
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )));
        if world.overclock_t > 0.0 {
            lines.push(Line::from(format!(" · Overclock {:.0}s", world.overclock_t)));
        }
        if world.fleet_dash {
            lines.push(Line::from(" · Fleet Dash"));
        }
        if world.twin_volley_t > 0.0 {
            lines.push(Line::from(format!(
                " · Twin Volley {:.0}s",
                world.twin_volley_t
            )));
        }
        if world.room_homing {
            lines.push(Line::from(" · Homing"));
        }
        if world.proj_vuln_t > 0.0 {
            lines.push(Line::from(format!(
                " · Vuln {:.0}s",
                world.proj_vuln_t
            )));
        }
        if world.guard_charges > 0 {
            lines.push(Line::from(format!(" · Guard ×{}", world.guard_charges)));
        }
    }
    if !world.boss_mods.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "BOSS MODS",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
        for m in &world.boss_mods {
            lines.push(Line::from(Span::styled(
                format!(" · {}", m.label()),
                Style::default().fg(Color::LightRed),
            )));
        }
    }
    if !world.level_mods.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "LEVEL MODS",
            Style::default()
                .fg(Color::Rgb(255, 160, 60))
                .add_modifier(Modifier::BOLD),
        )));
        for m in &world.level_mods {
            lines.push(Line::from(Span::styled(
                format!(" · {}", m.label()),
                Style::default().fg(Color::Rgb(255, 180, 90)),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(match mode {
        UiMode::Explore => "Tab → Inventory",
        UiMode::Inventory => "Tab → Explore",
    }));

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Nucleus ")),
        area,
    );
}

/// Equipped nucleus projectiles with mark progress %.
pub fn draw_marks(frame: &mut Frame, area: Rect, world: &World) {
    if area.height < 3 || area.width < 8 {
        return;
    }
    let mut lines = Vec::new();
    let mut any = false;
    for id in world.nucleus.filled_projectile_ids() {
        let Some(def) = world.lib.get(&id) else {
            continue;
        };
        any = true;
        let prog = world.marks.get(&id);
        let pct = ((prog.xp / prog.xp_to_next.max(1.0)) * 100.0).clamp(0.0, 99.0) as u32;
        let color = mark_color(prog.mark, world.anim_t);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {}", def.name, mark_label(prog.mark)),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {pct}%"), Style::default().fg(Color::Gray)),
        ]));
    }
    if !any {
        lines.push(Line::from(Span::styled(
            "no projectile slotted",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Marks ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn draw_journal(frame: &mut Frame, area: Rect, world: &World) {
    let h = area.height.saturating_sub(2).max(1) as usize;
    let lines: Vec<Line> = world
        .journal
        .iter()
        .rev()
        .take(h)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|entry| {
            let stamp = format!("{:>6.1}  ", entry.t);
            if let Some(rarity) = entry.pickup_rarity {
                let age = (world.anim_t - entry.t).max(0.0);
                let fade = (1.0 - age / 2.5).clamp(0.25, 1.0);
                let yellow = Color::Rgb(
                    (255.0 * fade) as u8,
                    (220.0 * fade) as u8,
                    (40.0 * fade) as u8,
                );
                let pulse = 0.55 + 0.45 * (world.anim_t * 4.2 + entry.t).sin().abs();
                let rc = rarity.color();
                let (rr, rg, rb) = match rc {
                    Color::LightGreen => (80, 255, 120),
                    Color::LightBlue | Color::Cyan => (80, 180, 255),
                    Color::LightMagenta | Color::Magenta => (230, 90, 255),
                    Color::Yellow | Color::LightYellow => (255, 230, 80),
                    Color::LightRed | Color::Red => (255, 90, 90),
                    Color::White => (230, 230, 240),
                    Color::Rgb(r, g, b) => (r, g, b),
                    _ => (180, 180, 190),
                };
                let body = Color::Rgb(
                    (rr as f32 * pulse) as u8,
                    (rg as f32 * pulse) as u8,
                    (rb as f32 * pulse) as u8,
                );
                Line::from(vec![
                    Span::styled(stamp, Style::default().fg(Color::DarkGray)),
                    Span::styled("pickup ", Style::default().fg(yellow)),
                    Span::styled(entry.text.clone(), Style::default().fg(body)),
                ])
            } else {
                let style = if entry.text.contains("→ MK") {
                    Style::default().fg(Color::LightMagenta)
                } else if entry.text.contains("CRIT") {
                    Style::default().fg(Color::LightYellow)
                } else if entry.text.contains("skill")
                    || entry.text.contains("Overclock")
                    || entry.text.contains("Fleet")
                    || entry.text.contains("Twin")
                    || entry.text.contains("Guard")
                    || entry.text.contains("Homing")
                {
                    Style::default().fg(Color::LightCyan)
                } else if entry.text.contains("boss") || entry.text.contains("reanim") {
                    Style::default().fg(Color::LightRed)
                } else if entry.text.contains("Re-animation") {
                    Style::default().fg(Color::LightGreen)
                } else {
                    Style::default().fg(Color::Gray)
                };
                Line::from(vec![
                    Span::styled(stamp, Style::default().fg(Color::DarkGray)),
                    Span::styled(entry.text.clone(), style),
                ])
            }
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" journalctl ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

/// Journal + boss/run (dash sits beside DPS; FPS in DPS title).
pub fn draw_gauges(frame: &mut Frame, area: Rect, world: &World, _app: &App, mode: UiMode) {
    if area.height < 6 || area.width < 8 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),   // journalctl — ≥10 history lines
            Constraint::Length(3), // Boss / Run
        ])
        .split(area);

    draw_journal(frame, chunks[0], world);

    if let Some((cur, max)) = world.boss_hp_totals() {
        let ratio = (cur / max.max(1.0)).clamp(0.0, 1.0);
        let n = world.living_bosses().count();
        let mods = world
            .boss_mods
            .iter()
            .map(|m| m.short())
            .collect::<Vec<_>>()
            .join("/");
        let title = if mods.is_empty() {
            format!(" Boss×{n} ")
        } else {
            format!(" Boss×{n} {mods} ")
        };
        frame.render_widget(
            Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(Style::default().fg(Color::LightRed)),
                )
                .gauge_style(
                    Style::default()
                        .fg(Color::LightRed)
                        .bg(Color::Rgb(40, 10, 10)),
                )
                .ratio(ratio as f64)
                .label(format!("{:.0}/{:.0}", cur, max)),
            chunks[1],
        );
    } else {
        let until_boss = rooms::rooms_until_boss(world.room.combat_index);
        let info = format!(
            "Rm {} · W {}/{} · Boss in {}\n{:?}",
            world.room.combat_index,
            world.room.wave,
            world.room.waves_total,
            until_boss,
            mode
        );
        frame.render_widget(
            Paragraph::new(info).block(Block::default().borders(Borders::ALL).title(" Run ")),
            chunks[1],
        );
    }
}

/// Dash cooldown as a purple waveform beside DPS.
/// High amplitude while recharging; calm when ready; one bright pulse on ready.
pub fn draw_dash_gauge(frame: &mut Frame, area: Rect, world: &World) {
    let charging = world.dash_cd > 0.0 || world.dash_time > 0.0;
    let pulse_t = world.dash_ready_pulse_t;
    let pulsing = pulse_t > 0.0;

    // One sine hump over the pulse window; otherwise charge→high, ready→calm.
    let visual_amp = if pulsing {
        let u = (pulse_t / 0.55).clamp(0.0, 1.0);
        0.30 + 0.85 * (u * std::f32::consts::PI).sin()
    } else if charging {
        let charge = world.dash_ready_ratio();
        // Strong right after dash, still lively as it fills.
        0.78 + 0.22 * (1.0 - charge)
    } else {
        0.26
    };

    let color = if pulsing {
        let u = (pulse_t / 0.55).clamp(0.0, 1.0);
        let bright = (u * std::f32::consts::PI).sin();
        Color::Rgb(
            (200.0 + 55.0 * bright) as u8,
            (120.0 + 80.0 * bright) as u8,
            255,
        )
    } else if world.fleet_dash {
        Color::Rgb(190, 130, 255)
    } else {
        Color::Rgb(160, 90, 230)
    };

    let cols = area.width.saturating_sub(2).max(6) as usize;
    let speed = if charging { 3.4 } else { 2.0 };
    let data: Vec<u64> = (0..cols)
        .map(|i| {
            let x = i as f32 / cols.max(1) as f32;
            let wave = (world.anim_t * speed + x * std::f32::consts::TAU * 1.6).sin() * 0.75
                + (world.anim_t * speed * 0.55 + x * std::f32::consts::TAU * 2.6).sin() * 0.25;
            let mid = 50.0;
            let swing = 46.0 * visual_amp;
            (mid + wave * swing).round().clamp(0.0, 100.0) as u64
        })
        .collect();

    let title = if pulsing {
        " Dash · READY ".to_string()
    } else if world.fleet_dash {
        " Dash ★ ".to_string()
    } else if charging {
        let ms = ((world.dash_cd.max(world.dash_time) * 1000.0 / 50.0).round() * 50.0).max(0.0)
            as i32;
        format!(" Dash · {ms}ms ")
    } else {
        " Dash ".to_string()
    };

    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(color)),
            )
            .data(&data)
            .max(100)
            .style(Style::default().fg(color)),
        area,
    );
}

/// Pure sine waveform — amplitude scales with DPS vs average enemy HP (smoothed).
pub fn draw_dps_waveform(frame: &mut Frame, area: Rect, world: &World, now: f32, fps: f32) {
    let amp = world.dps_wave_amp.clamp(0.0, 1.15);
    // Always show a calm resting wave; DPS grows amplitude from there up to full swing.
    const BASE_AMP: f32 = 0.32;
    let visual_amp = (BASE_AMP + amp * (1.0 - BASE_AMP)).clamp(BASE_AMP, 1.15);
    let color_t = world.dps_color_t.max(0.0);
    let color = dps_heat_color(color_t, world.anim_t);
    let overkill = world.dps_overkill_ratio();

    let cols = area.width.saturating_sub(2).max(8) as usize;
    // Gentle travel; slightly quicker as amplitude rises.
    let speed = 2.2 + visual_amp * 1.8;
    let data: Vec<u64> = (0..cols)
        .map(|i| {
            let x = i as f32 / cols.max(1) as f32;
            // Two soft harmonics for a smoother ocean-like crest.
            let wave = (world.anim_t * speed + x * std::f32::consts::TAU * 1.5).sin() * 0.72
                + (world.anim_t * speed * 0.55 + x * std::f32::consts::TAU * 2.8).sin() * 0.28;
            // Center line at 50; amplitude grows with overkill (full swing near 10×).
            let mid = 50.0;
            let swing = 46.0 * visual_amp;
            (mid + wave * swing).round().clamp(0.0, 100.0) as u64
        })
        .collect();

    let title = if overkill >= 10.0 {
        format!(" DPS {:.0} ★ · FPS {:.0} ", now, fps)
    } else if overkill >= 2.0 {
        format!(" DPS {:.0} ~ · FPS {:.0} ", now, fps)
    } else {
        format!(" DPS {:.0} · FPS {:.0} ", now, fps)
    };

    frame.render_widget(
        Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(color)),
            )
            .data(&data)
            .max(100)
            .style(Style::default().fg(color)),
        area,
    );
}

/// Color track: blue (0) → yellow → orange → red (1 @ 10×) → purple+blue tint (>1).
fn dps_heat_color(t: f32, anim_t: f32) -> Color {
    let stops: [(f32, (u8, u8, u8)); 5] = [
        (0.00, (50, 140, 255)),   // blue
        (0.33, (255, 230, 60)),   // yellow
        (0.66, (255, 140, 40)),   // orange
        (1.00, (255, 50, 55)),    // red
        (1.85, (180, 70, 255)),   // purple
    ];

    if t <= 0.0 {
        return Color::Rgb(stops[0].1 .0, stops[0].1 .1, stops[0].1 .2);
    }

    let (r, g, b) = if t >= 1.0 {
        // Past 10×: purple with shifting blue tints.
        let u = ((t - 1.0) / 0.85).clamp(0.0, 1.0);
        let (pr, pg, pb) = lerp_rgb(stops[3].1, stops[4].1, u);
        let blue_pulse = 0.35 + 0.35 * (anim_t * 2.8).sin();
        (
            (pr as f32 * (1.0 - blue_pulse * 0.25) + 70.0 * blue_pulse).clamp(0.0, 255.0) as u8,
            (pg as f32 * (1.0 - blue_pulse * 0.15) + 110.0 * blue_pulse * 0.4).clamp(0.0, 255.0)
                as u8,
            (pb as f32 * (1.0 - blue_pulse * 0.2) + 255.0 * blue_pulse).clamp(0.0, 255.0) as u8,
        )
    } else {
        // 0..1 across blue → yellow → orange → red
        let mut out = stops[0].1;
        for w in stops.windows(2) {
            let (t0, c0) = w[0];
            let (t1, c1) = w[1];
            if t1 > 1.0 {
                break;
            }
            if t >= t0 && t <= t1 {
                let u = if (t1 - t0).abs() < f32::EPSILON {
                    0.0
                } else {
                    (t - t0) / (t1 - t0)
                };
                out = lerp_rgb(c0, c1, u);
                break;
            }
            if t > t1 {
                out = c1;
            }
        }
        out
    };
    Color::Rgb(r, g, b)
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t).round() as u8,
    )
}

pub fn draw_status(frame: &mut Frame, area: Rect, world: &World) {
    let msg = if world.message_timer > 0.0 {
        world.message.as_str()
    } else if world.actors.iter().any(|a| a.kind == ActorKind::Boss) {
        "Boss fight · Shift dash · watch wallfire / splits"
    } else if world.room.cleared {
        "Room clear · Nucleus idle · reach the door (Enter)"
    } else {
        "Shift dash · Esc pause · Tab inventory · walk corpses to re-animate"
    };
    frame.render_widget(
        Paragraph::new(msg).style(Style::default().fg(Color::Gray)),
        area,
    );
}
