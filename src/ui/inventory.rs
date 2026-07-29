use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::skills;
use crate::proj_logic::{SpellKind, mark_color, mark_label, mod_mark_from_count};
use crate::world::{InvOverlay, World};

pub fn draw(frame: &mut Frame, area: Rect, world: &World) {
    let picking = world.stash_pick_open();
    let mod_menu = world.mod_menu_open();
    let mut lines = vec![
        Line::from(Span::styled(
            "INVENTORY (paused)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    if picking {
        lines.push(Line::from("j/k scroll · Enter equip · Esc/Backspace back"));
    } else if mod_menu {
        lines.push(Line::from(
            "j/k · Enter attach/change · Esc done · mods per projectile",
        ));
    } else {
        lines.push(Line::from("j/k move · h/l reorder slots"));
        lines.push(Line::from(
            "f Nucleus/Skills · Enter projectile/mods · Tab exit",
        ));
    }
    lines.push(Line::from(format!(
        "Mod capacity {}/projectile · {} slots",
        world.nucleus.mod_capacity,
        world.nucleus.slot_count()
    )));
    lines.push(Line::from(""));
    lines.push(section_header(
        "NUCLEUS",
        !picking && !mod_menu && world.inv_focus == 0,
        Color::LightCyan,
    ));

    for (i, slot) in world.nucleus.slots.iter().enumerate() {
        let selected = !picking && !mod_menu && world.inv_focus == 0 && i == world.inv_cursor;
        let opening = matches!(
            world.inv_overlay,
            InvOverlay::PickProjectile { slot: s }
                | InvOverlay::ModMenu { slot: s }
                | InvOverlay::PickMod { slot: s, .. } if s == i
        );
        let prefix = if opening {
            "*"
        } else if selected {
            ">"
        } else {
            " "
        };
        if let Some(id) = slot.projectile.as_ref() {
            if let Some(def) = world.lib.get(id) {
                lines.push(item_line(
                    world,
                    prefix,
                    &format!("[{i}] "),
                    id,
                    def.kind,
                    1,
                ));
            } else {
                lines.push(Line::from(format!("{prefix}[{i}] · empty ·")));
            }
            for mid in &slot.mods {
                if let Some(def) = world.lib.get(mid) {
                    lines.push(item_line(world, " ", "    ↳ ", mid, def.kind, 1));
                }
            }
            let free = world.nucleus.mod_capacity.saturating_sub(slot.mods.len());
            if free > 0 {
                lines.push(Line::from(Span::styled(
                    format!("      · {free} mod slot{}", if free == 1 { "" } else { "s" }),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else {
            lines.push(Line::from(format!("{prefix}[{i}] · empty ·")));
        }
    }

    lines.push(Line::from(""));
    lines.push(section_header(
        &format!(
            "SKILLS ({}/{} equipped)",
            world.skills.active.len(),
            world.skills.max_active
        ),
        !picking && !mod_menu && world.inv_focus == 1,
        Color::Yellow,
    ));
    if world.skills.owned.is_empty() {
        lines.push(Line::from("  (none — find skill pickups)"));
    }
    for (i, id) in world.skills.owned.iter().enumerate() {
        let selected = !picking && !mod_menu && world.inv_focus == 1 && i == world.skills.cursor;
        let prefix = if selected { ">" } else { " " };
        let def = skills::def(*id);
        let active = world.skills.has_active(*id);
        let state = if active { "ON " } else { "off" };
        lines.push(Line::from(vec![
            Span::raw(format!("{prefix}")),
            Span::styled(format!("{} ", def.glyph), Style::default().fg(def.color)),
            Span::styled(def.name.to_string(), Style::default().fg(def.color)),
            Span::raw(format!(" [{state}]")),
        ]));
    }

    if !picking && !mod_menu {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Info",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.extend(current_tooltip(world));
    }

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Nucleus ")),
        area,
    );

    match world.inv_overlay {
        InvOverlay::PickProjectile { slot } => draw_stash_picker(frame, area, world, slot, false),
        InvOverlay::PickMod { slot, .. } => draw_stash_picker(frame, area, world, slot, true),
        InvOverlay::ModMenu { slot } => draw_mod_menu(frame, area, world, slot),
        InvOverlay::None => {}
    }
}

fn draw_mod_menu(frame: &mut Frame, area: Rect, world: &World, slot: usize) {
    let w = area.width.min(46).max(30);
    let h = area.height.min(18).max(10);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let Some(ns) = world.nucleus.slots.get(slot) else {
        return;
    };
    let proj_name = ns
        .projectile
        .as_ref()
        .and_then(|id| world.lib.get(id).map(|d| d.name.clone()))
        .unwrap_or_else(|| "—".into());
    let cap = world.nucleus.mod_capacity;
    let cursor = world.mod_menu_cursor;

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            format!("Attach mods · {proj_name}"),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Capacity {cap} · Enter to set · Esc done")),
        Line::from(""),
    ];

    for i in 0..cap {
        let selected = cursor == i;
        let prefix = if selected { ">" } else { " " };
        if let Some(id) = ns.mods.get(i) {
            if let Some(def) = world.lib.get(id) {
                lines.push(item_line(
                    world,
                    prefix,
                    &format!("[{i}] "),
                    id,
                    def.kind,
                    1,
                ));
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!("{prefix}[{i}] · empty mod ·"),
                if selected {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            )));
        }
    }

    let change_sel = cursor == cap;
    lines.push(Line::from(Span::styled(
        format!(
            "{}Change projectile…",
            if change_sel { ">" } else { " " }
        ),
        if change_sel {
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        },
    )));
    let done_sel = cursor == cap + 1;
    lines.push(Line::from(Span::styled(
        format!("{}Done", if done_sel { ">" } else { " " }),
        if done_sel {
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        },
    )));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Mods ")
                .border_style(Style::default().fg(Color::LightYellow)),
        ),
        popup,
    );
}

fn draw_stash_picker(frame: &mut Frame, area: Rect, world: &World, slot: usize, mods: bool) {
    let w = area.width.min(46).max(30);
    let h = area.height.min(20).max(12);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let popup = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    let groups = world.grouped_stash();
    let total = 1 + groups.len();
    let cursor = world.stash_cursor.min(total.saturating_sub(1));

    let inner_h = h.saturating_sub(2) as usize;
    let header = 3usize;
    let footer = 3usize;
    let list_h = inner_h.saturating_sub(header + footer).max(3);
    let scroll = scroll_offset(cursor, total, list_h);

    let title = if mods {
        format!("Attach mod → slot [{slot}]")
    } else {
        format!("Projectile → slot [{slot}]")
    };
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("j/k scroll · {} unique", groups.len())),
        Line::from(""),
    ];

    for row in scroll..scroll.saturating_add(list_h).min(total) {
        let selected = cursor == row;
        let prefix = if selected { ">" } else { " " };
        if row == 0 {
            lines.push(Line::from(Span::styled(
                format!("{prefix}[none]"),
                if selected {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            )));
            continue;
        }
        let (id, count) = &groups[row - 1];
        if let Some(def) = world.lib.get(id) {
            lines.push(item_line(world, prefix, "", id, def.kind, *count));
        } else {
            lines.push(Line::from(format!("{prefix}{id}")));
        }
    }

    if scroll > 0 || scroll + list_h < total {
        let more_above = if scroll > 0 { "↑" } else { " " };
        let more_below = if scroll + list_h < total { "↓" } else { " " };
        lines.push(Line::from(Span::styled(
            format!("  {more_above} · {more_below}  ({}/{total})", cursor + 1),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(""));
    }

    lines.extend(stash_tooltip(world, &groups, cursor, mods));

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(if mods { " Mods " } else { " Projectiles " })
                .border_style(Style::default().fg(Color::LightMagenta)),
        ),
        popup,
    );
}

fn scroll_offset(cursor: usize, total: usize, view: usize) -> usize {
    if total <= view {
        return 0;
    }
    let max_start = total - view;
    let ideal = cursor.saturating_sub(view / 2);
    ideal.min(max_start)
}

fn section_header(title: &str, focused: bool, color: Color) -> Line<'static> {
    let label = if focused {
        format!("> {title}")
    } else {
        format!("  {title}")
    };
    Line::from(Span::styled(label, Style::default().fg(color)))
}

fn item_line(
    world: &World,
    prefix: &str,
    mid: &str,
    id: &str,
    kind: SpellKind,
    count: usize,
) -> Line<'static> {
    let def = world.lib.get(id);
    let name = def.map(|d| d.name.as_str()).unwrap_or(id);
    let rarity = def.map(|d| d.rarity);
    let mark = match kind {
        SpellKind::Payload => world.marks.get(id).mark,
        SpellKind::Modifier | SpellKind::Chaos => {
            mod_mark_from_count(world.count_owned(id).max(count))
        }
    };
    let color = mark_color(mark, world.anim_t);
    let kind_tag = match kind {
        SpellKind::Payload => "Projectile",
        SpellKind::Modifier | SpellKind::Chaos => "Mod",
    };
    let mut spans = vec![
        Span::raw(format!("{prefix}{mid}")),
        Span::styled(
            format!("{} ", rarity.map(|r| r.pickup_glyph()).unwrap_or('·')),
            Style::default().fg(color),
        ),
        Span::styled(
            format!("{name} {}", mark_label(mark)),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{kind_tag}]"),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if count > 1 {
        spans.push(Span::styled(
            format!(" x{count}"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn stash_tooltip(
    world: &World,
    groups: &[(String, usize)],
    cursor: usize,
    mods: bool,
) -> Vec<Line<'static>> {
    if cursor == 0 {
        return vec![Line::from(""), Line::from(if mods {
            "Clear this mod attachment"
        } else {
            "Clear projectile (mods return to stash)"
        })];
    }
    let pick_i = cursor - 1;
    match groups
        .get(pick_i)
        .and_then(|(id, _)| world.lib.get(id).map(|d| (id.clone(), d)))
    {
        Some((id, s)) => {
            let kind = match s.kind {
                SpellKind::Payload => "Projectile",
                _ => "Mod",
            };
            let mark = match s.kind {
                SpellKind::Payload => world.marks.get(&id).mark,
                _ => mod_mark_from_count(world.count_owned(&id)),
            };
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("{} · {} · {}", s.rarity.label(), kind, mark_label(mark)),
                    Style::default().fg(mark_color(mark, world.anim_t)),
                )),
                Line::from(truncate(&s.description, 44)),
            ]
        }
        None => vec![Line::from(""), Line::from("Select an item")],
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn current_tooltip(world: &World) -> Vec<Line<'static>> {
    match world.inv_focus {
        1 => {
            if let Some(id) = world.skills.owned.get(world.skills.cursor) {
                let def = skills::def(*id);
                vec![
                    Line::from(Span::styled(
                        "Passive skill · Enter to toggle",
                        Style::default().fg(def.color),
                    )),
                    Line::from(def.description.to_string()),
                ]
            } else {
                vec![Line::from("No skills")]
            }
        }
        _ => match world.nucleus.slots.get(world.inv_cursor) {
            Some(slot) if slot.projectile.is_some() => {
                let id = slot.projectile.as_ref().unwrap();
                let Some(s) = world.lib.get(id) else {
                    return vec![Line::from("Unknown projectile")];
                };
                let mark = world.marks.get(id).mark;
                vec![
                    Line::from(Span::styled(
                        format!(
                            "{} · Projectile · {}",
                            s.rarity.label(),
                            mark_label(mark)
                        ),
                        Style::default().fg(mark_color(mark, world.anim_t)),
                    )),
                    Line::from(s.description.clone()),
                    Line::from(format!(
                        "Enter — attach mods ({}/{})",
                        slot.mods.len(),
                        world.nucleus.mod_capacity
                    )),
                ]
            }
            _ => vec![Line::from("Empty slot — Enter to pick a projectile")],
        },
    }
}
