use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::world::entity::{ActorKind, Vec2};
use crate::world::room::FloorStyle;
use crate::world::{RoomKind, World};

pub fn draw(frame: &mut Frame, area: Rect, world: &World) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(match world.room.kind {
            RoomKind::Boss => " Boss Arena ",
            RoomKind::Shop => " Crypt Shop ",
            RoomKind::Combat if world.room.mega => " Cathedral Hall ",
            RoomKind::Combat if world.room.has_platforms() => " Platform Gauntlet ",
            RoomKind::Combat if world.room.is_pillar_alley() => " Pillar Alley ",
            RoomKind::Combat => " @_Term Crypt ",
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    let rw = world.room.width.max(1.0);
    let rh = world.room.height.max(1.0);

    let mut cells: Vec<Vec<(char, Color)>> =
        vec![vec![(' ', Color::Reset); inner.width as usize]; inner.height as usize];

    // Floor: subtle wave, or grey platforms over void in gauntlet rooms.
    let platform_mode = world.room.has_platforms();
    for y in 0..inner.height as usize {
        for x in 0..inner.width as usize {
            let world_pos = screen_to_world(x, y, rw, rh, inner);
            if platform_mode && !world.room.platforms.iter().any(|p| {
                world_pos.x >= p.x
                    && world_pos.y >= p.y
                    && world_pos.x <= p.x + p.w
                    && world_pos.y <= p.y + p.h
            }) {
                // Keep outer wall strip even over void.
                if jagged_wall_depth(x, y, inner.width as usize, inner.height as usize, world) > 0
                {
                    cells[y][x] = jagged_wall_cell(x, y, world);
                } else {
                    cells[y][x] = (' ', Color::Rgb(6, 6, 10));
                }
                continue;
            }
            if jagged_wall_depth(x, y, inner.width as usize, inner.height as usize, world) > 0 {
                cells[y][x] = jagged_wall_cell(x, y, world);
                continue;
            }
            cells[y][x] = floor_cell(
                x,
                y,
                inner.width as usize,
                inner.height as usize,
                world,
                platform_mode,
            );
        }
    }

    if platform_mode {
        paint_platform_borders(&mut cells, inner, world, rw, rh);
    }

    // Destructible columns (mega halls)
    for col in &world.room.columns {
        let (px, py) = project(col.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let cracked = col.hp < col.max_hp * 0.45;
            cells[y][x] = (
                if cracked { '▓' } else { '█' },
                Color::Rgb(110, 110, 120),
            );
            if y > 0 {
                cells[y - 1][x] = ('╥', Color::Rgb(90, 90, 100));
            }
        }
    }

    // Torch warmth, then player / projectile blue light.
    apply_scene_light(&mut cells, inner, world, rw, rh);

    // Always show wall doors: entrance (left) sealed behind you, exit (right)
    // locked until the room clears — Isaac-style 3-tile vertical gaps.
    paint_wall_door(
        &mut cells,
        inner,
        world.room.entrance_door(),
        rw,
        rh,
        DoorFace::West,
        true, // entrance is always "open" visually (you came through it)
    );
    paint_wall_door(
        &mut cells,
        inner,
        world.room.exit_door(),
        rw,
        rh,
        DoorFace::East,
        world.room.doors_open || world.room.kind == RoomKind::Shop,
    );

    // Torches on / near walls
    for (i, torch) in world.room.torches.iter().enumerate() {
        let (px, py) = project(torch.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let flicker = ((world.anim_t * 9.0) + i as f32 * 1.7).sin();
            let glyph = if flicker > 0.35 {
                '░'
            } else if flicker > -0.2 {
                'î'
            } else {
                'i'
            };
            cells[y][x] = (
                glyph,
                if flicker > 0.0 {
                    Color::Yellow
                } else {
                    Color::Rgb(220, 120, 40)
                },
            );
        }
    }

    // Shop totems — pedestal + name above + price under
    if world.room.kind == RoomKind::Shop {
        paint_shop_totems(&mut cells, inner, world, rw, rh);
    }

    // Level-mod gas clouds (under actors)
    for cloud in &world.gas_clouds {
        paint_gas_cloud(&mut cells, inner, cloud, world, rw, rh);
    }

    // Lava waveforms
    if world
        .level_mods
        .iter()
        .any(|m| matches!(m, crate::world::LevelMod::SomethingIsLava))
    {
        paint_lava_waves(&mut cells, inner, world, rw, rh);
    }

    // Stampede bison
    for beast in &world.stampede {
        let (px, py) = project(beast.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            cells[y][x] = ('B', Color::Rgb(180, 120, 60));
            if x > 0 {
                cells[y][x - 1] = ('▬', Color::Rgb(140, 90, 40));
            }
        }
    }

    // Corpses — fade into the floor over their remaining life.
    for c in &world.corpses {
        let (px, py) = project(c.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let t = (c.life / c.max_life.max(0.01)).clamp(0.0, 1.0);
            let color = if t > 0.55 {
                c.color
            } else {
                Color::Rgb(
                    (40.0 + 30.0 * t) as u8,
                    (40.0 + 30.0 * t) as u8,
                    (48.0 + 30.0 * t) as u8,
                )
            };
            let glyph = if t > 0.35 { c.glyph } else { '·' };
            cells[y][x] = (glyph, color);
        }
    }

    // Pickups — spells (rarity) and skills (bright glyph)
    for p in &world.pickups {
        let (px, py) = project(p.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let flash = p.pulse.sin() > 0.0;
            match &p.kind {
                crate::world::entity::PickupKind::Spell { rarity, .. } => {
                    let glyph = if flash {
                        rarity.pickup_glyph()
                    } else {
                        match rarity {
                            crate::proj_logic::Rarity::Mythical => '✧',
                            crate::proj_logic::Rarity::Legendary => '☆',
                            _ => rarity.pickup_glyph(),
                        }
                    };
                    cells[y][x] = (glyph, rarity.color());
                    if matches!(
                        rarity,
                        crate::proj_logic::Rarity::Rare
                            | crate::proj_logic::Rarity::Legendary
                            | crate::proj_logic::Rarity::Mythical
                    ) {
                        paint_halo(&mut cells, inner, x, y, rarity.color());
                    }
                }
                crate::world::entity::PickupKind::Skill { skill_id } => {
                    let def = crate::skills::def(*skill_id);
                    let glyph = if flash { def.glyph } else { '✦' };
                    cells[y][x] = (glyph, def.color);
                    paint_halo(&mut cells, inner, x, y, def.color);
                }
                crate::world::entity::PickupKind::Temp(boost) => {
                    let glyph = if flash { boost.glyph() } else { '✦' };
                    cells[y][x] = (glyph, boost.color());
                    paint_halo(&mut cells, inner, x, y, boost.color());
                }
            }
        }
    }

    // Dash / misc particles
    for p in &world.particles {
        let (px, py) = project(p.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            cells[y][x] = (p.glyph, p.color);
        }
    }

    // Explosion rings
    for fx in &world.explosions {
        let t = (fx.life / fx.max_life).clamp(0.0, 1.0);
        let radius = fx.radius * (1.15 - t * 0.35);
        paint_explosion_ring(&mut cells, inner, fx.pos, radius, rw, rh, fx.color, t);
    }

    // Orbit sawblades (ally-safe)
    for blade in &world.orbit_blades {
        let (px, py) = project(blade.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            cells[y][x] = ('¤', Color::LightYellow);
        }
    }

    // Daisymania flowers
    for (i, daisy) in world.daisies.iter().enumerate() {
        let (px, py) = project(daisy.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let pulse = ((world.anim_t * 6.0) + i as f32).sin();
            cells[y][x] = (
                if pulse > 0.0 { '*' } else { '❀' },
                Color::Rgb(255, 220, 120),
            );
        }
    }

    // Projectile trails (player: longer + blue-lit), then heads with a glow halo
    for p in &world.projectiles {
        let player_shot = p.owner_is_player_side;
        let trail_dim = if player_shot { 0.42 } else { 0.45 };
        for (i, pos) in p.trail.iter().enumerate() {
            let (px, py) = project(*pos, rw, rh, inner);
            if let Some((x, y)) = in_bounds(px, py, inner) {
                let progress = i as f32 / p.trail.len().max(1) as f32;
                let near = i + 3 >= p.trail.len();
                let glyph = if near {
                    if player_shot {
                        '○'
                    } else {
                        '◦'
                    }
                } else if player_shot {
                    if progress > 0.65 {
                        '•'
                    } else if progress > 0.35 {
                        '∙'
                    } else {
                        '·'
                    }
                } else {
                    '·'
                };
                let color = if p.trail_rainbow {
                    rainbow_color(world.anim_t + progress * 3.0 + i as f32 * 0.2)
                } else if p.trail_fx {
                    brighten(dim_color(p.color, trail_dim + i as f32 * 0.08), p.trail_bright)
                } else if player_shot {
                    blue_trail_color(progress)
                } else {
                    dim_color(p.color, trail_dim + i as f32 * 0.08)
                };
                cells[y][x] = (glyph, color);
                // Extra trail width for player shots
                if player_shot && near {
                    for (ox, oy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
                        let nx = x as isize + ox;
                        let ny = y as isize + oy;
                        if nx >= 0
                            && ny >= 0
                            && nx < inner.width as isize
                            && ny < inner.height as isize
                        {
                            let nx = nx as usize;
                            let ny = ny as usize;
                            if is_floorish(cells[ny][nx].0) {
                                cells[ny][nx] = ('·', blue_trail_color(progress * 0.7));
                            }
                        }
                    }
                }
            }
        }
        let (px, py) = project(p.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let halo = if player_shot {
                [
                    (-1isize, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                    (1, 1),
                ]
                .as_slice()
            } else {
                [(-1isize, 0), (1, 0), (0, -1), (0, 1)].as_slice()
            };
            for &(ox, oy) in halo {
                let nx = x as isize + ox;
                let ny = y as isize + oy;
                if nx >= 0
                    && ny >= 0
                    && nx < inner.width as isize
                    && ny < inner.height as isize
                {
                    let nx = nx as usize;
                    let ny = ny as usize;
                    if is_floorish(cells[ny][nx].0) {
                        let (ch, c) = if p.glow_halo {
                            ('˚', Color::Rgb(220, 120, 255))
                        } else if p.glow_green {
                            ('·', Color::Rgb(80, 220, 110))
                        } else if p.glow_red {
                            ('·', Color::Rgb(255, 90, 60))
                        } else if player_shot {
                            ('·', Color::Rgb(70, 120, 220))
                        } else {
                            ('·', dim_color(p.color, 0.55))
                        };
                        cells[ny][nx] = (ch, c);
                    }
                }
            }
            // Extra top halo cell for vuln.
            if p.glow_halo && y > 0 && is_floorish(cells[y - 1][x].0) {
                cells[y - 1][x] = ('◯', Color::Rgb(230, 140, 255));
            }
            let glyph = if !player_shot {
                match p.glyph {
                    '•' | '*' | '|' => '●',
                    other => other,
                }
            } else {
                match p.glyph {
                    '*' | '.' => '◆',
                    '~' => '≈',
                    other => other,
                }
            };
            let head = if player_shot {
                Color::Rgb(180, 200, 255)
            } else {
                p.color
            };
            cells[y][x] = (glyph, head);
        }
    }

    // Actors y-sorted for fake depth
    let mut draw_list: Vec<_> = world.actors.iter().collect();
    draw_list.sort_by(|a, b| {
        a.pos
            .y
            .partial_cmp(&b.pos.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for a in draw_list {
        let (px, py) = project(a.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            cells[y][x] = (a.glyph, a.color);
            // tall cue for larger bosses
            if y > 0 && a.kind == ActorKind::Boss && a.radius >= 0.85 {
                cells[y - 1][x] = ('▲', a.color);
            }
        }
    }

    // Player on top — aura already lit; local cast flash + aim tip
    let (px, py) = project(world.player.pos, rw, rh, inner);
    if let Some((x, y)) = in_bounds(px, py, inner) {
        let flash = world.player_flash > 0.0;
        let color = if flash {
            Color::White
        } else {
            Color::Rgb(200, 180, 255)
        };
        cells[y][x] = (if flash { '◎' } else { '@' }, color);
        if flash {
            for (ox, oy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as isize + ox;
                let ny = y as isize + oy;
                if nx >= 0
                    && ny >= 0
                    && nx < inner.width as isize
                    && ny < inner.height as isize
                {
                    let nx = nx as usize;
                    let ny = ny as usize;
                    if !matches!(cells[ny][nx].0, '#' | '▶' | '◀' | 'X' | 'D' | '▓' | '█') {
                        cells[ny][nx] = ('·', Color::Rgb(160, 140, 255));
                    }
                }
            }
        }
        let tip = world.player.pos + world.player.facing * 1.2;
        let (tx, ty) = project(tip, rw, rh, inner);
        if let Some((fx, fy)) = in_bounds(tx, ty, inner) {
            if (fx, fy) != (x, y) {
                cells[fy][fx] = ('+', Color::Rgb(140, 170, 255));
            }
        }
        paint_player_hp_bar(&mut cells, inner, x, y, world);
    }

    // Floating damage numbers (fade via dimming)
    for n in &world.damage_numbers {
        let (px, py) = project(n.pos, rw, rh, inner);
        if let Some((x, y)) = in_bounds(px, py, inner) {
            let t = (n.life / n.max_life).clamp(0.0, 1.0);
            let text = if n.crit {
                format!("{:.0}!", n.amount)
            } else {
                format!("{:.0}", n.amount)
            };
            let color = if n.crit {
                Color::Rgb(
                    (255.0 * t) as u8,
                    (220.0 * t) as u8,
                    (80.0 * t) as u8,
                )
            } else {
                Color::Rgb(
                    (220.0 * t) as u8,
                    (220.0 * t) as u8,
                    (230.0 * t) as u8,
                )
            };
            paint_centered_label(&mut cells, inner, x, y, &text, color);
        }
    }

    let lines: Vec<Line> = {
        let cells = apply_screen_shake(cells, world.shake_offset(), rw, rh, inner);
        cells
            .into_iter()
            .map(|row| {
                Line::from(
                    row.into_iter()
                        .map(|(ch, color)| Span::styled(ch.to_string(), Style::default().fg(color)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    };

    frame.render_widget(Paragraph::new(lines), inner);
}

#[derive(Clone, Copy)]
enum DoorFace {
    West,
    East,
}

fn paint_wall_door(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    world_pos: Vec2,
    rw: f32,
    rh: f32,
    face: DoorFace,
    open: bool,
) {
    let (_, cy) = project(world_pos, rw, rh, area);
    let wall_x = match face {
        DoorFace::West => 0usize,
        DoorFace::East => area.width.saturating_sub(1) as usize,
    };
    let mid = cy as isize;
    for dy in -1..=1 {
        let y = mid + dy;
        if y < 0 || y >= area.height as isize {
            continue;
        }
        let y = y as usize;
        let open_mid = match face {
            DoorFace::West => '◀',
            DoorFace::East => '▶',
        };
        let (ch, color) = if open {
            match dy {
                0 => (open_mid, Color::LightYellow),
                _ => ('┊', Color::Yellow),
            }
        } else {
            match dy {
                0 => ('X', Color::Red),
                _ => ('═', Color::DarkGray),
            }
        };
        // Carve the wall tile itself.
        cells[y][wall_x] = (ch, color);
        // Also mark the floor tile just inside so the door reads clearly.
        let inward = match face {
            DoorFace::West => 1usize,
            DoorFace::East => wall_x.saturating_sub(1),
        };
        if inward < area.width as usize && inward != wall_x {
            let floor_ch = if open && dy == 0 {
                ('D', Color::LightYellow)
            } else if open {
                ('·', Color::Yellow)
            } else if dy == 0 {
                ('x', Color::Red)
            } else {
                ('=', Color::DarkGray)
            };
            cells[y][inward] = floor_ch;
        }
    }
}

fn paint_shop_totems(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    world: &World,
    rw: f32,
    rh: f32,
) {
    for t in &world.shop_totems {
        let (px, py) = project(t.pos, rw, rh, area);
        let Some((x, y)) = in_bounds(px, py, area) else {
            continue;
        };
        let color = if t.sold {
            Color::DarkGray
        } else {
            world.shop_offer_color(&t.offer)
        };
        let glyph = if t.sold {
            '×'
        } else {
            world.shop_offer_glyph(&t.offer)
        };
        // Pedestal base
        if y + 1 < area.height as usize {
            cells[y + 1][x] = ('╥', Color::Rgb(120, 110, 90));
        }
        cells[y][x] = (glyph, color);
        // Name above totem
        let name = if t.sold {
            "sold".to_string()
        } else {
            let mut n = world.shop_offer_label(&t.offer);
            if shop_can_merge(world, &t.offer) {
                n.push_str(" [CAN_MERGE]");
            }
            n
        };
        paint_centered_label(cells, area, x, y.saturating_sub(2), &name, color);
        // Description under the name / above the glyph
        if !t.sold {
            if let Some(desc) = shop_short_desc(world, &t.offer) {
                paint_centered_label(
                    cells,
                    area,
                    x,
                    y.saturating_sub(1),
                    &desc,
                    Color::Rgb(150, 150, 160),
                );
            }
        }
        // Price under pedestal
        if !t.sold && y + 2 < area.height as usize {
            let price = format!("{}g", t.price);
            paint_centered_label(
                cells,
                area,
                x,
                y + 2,
                &price,
                Color::LightYellow,
            );
        }
    }

    // Far-right reroll totem
    let (px, py) = project(world.shop_reroll_pos, rw, rh, area);
    if let Some((x, y)) = in_bounds(px, py, area) {
        let near = world.player.pos.dist(world.shop_reroll_pos) < 2.0;
        let color = if near {
            Color::LightMagenta
        } else {
            Color::Magenta
        };
        if y + 1 < area.height as usize {
            cells[y + 1][x] = ('╥', Color::Rgb(120, 90, 120));
        }
        cells[y][x] = ('⟳', color);
        paint_centered_label(cells, area, x, y.saturating_sub(1), "Reroll", color);
        if y + 2 < area.height as usize {
            paint_centered_label(
                cells,
                area,
                x,
                y + 2,
                &format!("{}g", world.shop_reroll_cost),
                Color::LightYellow,
            );
        }
    }
}

fn shop_can_merge(world: &World, offer: &crate::world::entity::ShopOffer) -> bool {
    let crate::world::entity::ShopOffer::Spell(id) = offer else {
        return false;
    };
    let Some(def) = world.lib.get(id) else {
        return false;
    };
    matches!(
        def.kind,
        crate::proj_logic::SpellKind::Modifier | crate::proj_logic::SpellKind::Chaos
    ) && world.count_owned(id) > 0
}

fn shop_short_desc(world: &World, offer: &crate::world::entity::ShopOffer) -> Option<String> {
    match offer {
        crate::world::entity::ShopOffer::Spell(id) => world.lib.get(id).map(|d| {
            let mut s = d.description.clone();
            if s.chars().count() > 28 {
                s = s.chars().take(25).collect::<String>() + "...";
            }
            s
        }),
        crate::world::entity::ShopOffer::Skill(id) => {
            let d = crate::skills::def(*id);
            let mut s = d.description.to_string();
            if s.chars().count() > 28 {
                s = s.chars().take(25).collect::<String>() + "...";
            }
            Some(s)
        }
        crate::world::entity::ShopOffer::Credit => Some("Re-anim credit".into()),
        crate::world::entity::ShopOffer::Heal => Some("Restore 35 HP".into()),
        crate::world::entity::ShopOffer::SkillSlot => Some("+1 skill slot".into()),
    }
}

fn paint_player_hp_bar(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    px: usize,
    py: usize,
    world: &World,
) {
    let max_hp = world.player.max_hp.max(1.0);
    let hp = world.player.hp.max(0.0);
    let ratio = (hp / max_hp).clamp(0.0, 1.0);
    let bar_w = 7usize;
    let filled = ((ratio * bar_w as f32).round() as usize).min(bar_w);
    let color = if ratio > 0.55 {
        Color::LightGreen
    } else if ratio > 0.30 {
        Color::Yellow
    } else {
        Color::LightRed
    };
    let dim = Color::Rgb(50, 50, 55);

    // Half-height HP bar under the player.
    let bar_y = py + 1;
    if bar_y < area.height as usize {
        let start = px as isize - (bar_w as isize / 2);
        for i in 0..bar_w {
            let x = start + i as isize;
            if x < 0 || x >= area.width as isize {
                continue;
            }
            let ch = if i < filled { '▄' } else { '▂' };
            cells[bar_y][x as usize] = (ch, if i < filled { color } else { dim });
        }
    }

    let label = format!("{:.0}/{:.0}", hp, max_hp);
    let label_y = py + 2;
    if label_y < area.height as usize {
        let chars: Vec<char> = label.chars().collect();
        let start = px as isize - (chars.len() as isize / 2);
        for (i, ch) in chars.into_iter().enumerate() {
            let x = start + i as isize;
            if x < 0 || x >= area.width as isize {
                continue;
            }
            cells[label_y][x as usize] = (ch, color);
        }
    }
}

fn paint_centered_label(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    cx: usize,
    y: usize,
    text: &str,
    color: Color,
) {
    if y >= area.height as usize {
        return;
    }
    let chars: Vec<char> = text.chars().take(14).collect();
    if chars.is_empty() {
        return;
    }
    let start = cx as isize - (chars.len() as isize / 2);
    for (i, ch) in chars.into_iter().enumerate() {
        let x = start + i as isize;
        if x < 0 || x >= area.width as isize {
            continue;
        }
        cells[y][x as usize] = (ch, color);
    }
}

fn paint_gas_cloud(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    cloud: &crate::world::entity::GasCloud,
    world: &World,
    rw: f32,
    rh: f32,
) {
    let fade = (cloud.life / cloud.max_life).clamp(0.0, 1.0);
    let pulse = 0.7 + 0.3 * (world.anim_t * 3.5 + cloud.pos.x).sin();
    let steps = (cloud.radius * 8.0).clamp(6.0, 20.0) as i32;
    for i in 0..steps {
        for ring in 1..=3 {
            let a = (i as f32 / steps as f32) * std::f32::consts::TAU + world.anim_t * 0.4;
            let r = cloud.radius * (ring as f32 / 3.0) * pulse;
            let p = Vec2::new(cloud.pos.x + a.cos() * r, cloud.pos.y + a.sin() * r * 0.75);
            let (px, py) = project(p, rw, rh, area);
            if let Some((x, y)) = in_bounds(px, py, area) {
                if is_floorish(cells[y][x].0) || matches!(cells[y][x].0, '~' | '≈' | '░') {
                    let c = cloud.kind.color();
                    let dim = match c {
                        Color::Rgb(r, g, b) => Color::Rgb(
                            (r as f32 * fade * 0.85) as u8,
                            (g as f32 * fade * 0.85) as u8,
                            (b as f32 * fade * 0.85) as u8,
                        ),
                        other => other,
                    };
                    cells[y][x] = (cloud.kind.glyph(), dim);
                }
            }
        }
    }
    let (cx, cy) = project(cloud.pos, rw, rh, area);
    if let Some((x, y)) = in_bounds(cx, cy, area) {
        cells[y][x] = (cloud.kind.glyph(), cloud.kind.color());
    }
}

fn paint_lava_waves(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    world: &World,
    rw: f32,
    rh: f32,
) {
    let w = area.width as usize;
    let h = area.height as usize;
    for sx in 0..w {
        let world_x = (sx as f32 / w.saturating_sub(1).max(1) as f32) * rw;
        let top_y = world.lava_top_y(world_x);
        let bot_y = world.lava_bottom_y(world_x);
        let flicker = 0.75 + 0.25 * (world.anim_t * 8.0 + sx as f32 * 0.3).sin();
        let color = Color::Rgb(
            (255.0 * flicker) as u8,
            (90.0 + 50.0 * flicker) as u8,
            20,
        );
        // Fill from top edge down to wave
        let (_, ty) = project(Vec2::new(world_x, top_y), rw, rh, area);
        let top_row = (ty as usize).min(h.saturating_sub(1));
        for y in 0..=top_row {
            if is_floorish(cells[y][sx].0) || matches!(cells[y][sx].0, ' ' | '·' | '.' | '~') {
                let glyph = if y == top_row {
                    if flicker > 0.9 {
                        '≈'
                    } else {
                        '~'
                    }
                } else {
                    '░'
                };
                cells[y][sx] = (glyph, color);
            }
        }
        // Fill from bottom wave to floor
        let (_, by) = project(Vec2::new(world_x, bot_y), rw, rh, area);
        let bot_row = (by as usize).min(h.saturating_sub(1));
        for y in bot_row..h {
            if is_floorish(cells[y][sx].0) || matches!(cells[y][sx].0, ' ' | '·' | '.' | '~') {
                let glyph = if y == bot_row {
                    if flicker > 0.9 {
                        '≈'
                    } else {
                        '~'
                    }
                } else {
                    '░'
                };
                cells[y][sx] = (glyph, color);
            }
        }
    }
}

fn project(pos: Vec2, rw: f32, rh: f32, area: Rect) -> (u16, u16) {
    let x = ((pos.x / rw) * (area.width.saturating_sub(1) as f32)).round() as i32;
    let y = ((pos.y / rh) * (area.height.saturating_sub(1) as f32)).round() as i32;
    (x.clamp(0, i32::MAX) as u16, y.clamp(0, i32::MAX) as u16)
}

fn apply_screen_shake(
    cells: Vec<Vec<(char, Color)>>,
    shake: Vec2,
    rw: f32,
    rh: f32,
    area: Rect,
) -> Vec<Vec<(char, Color)>> {
    if shake.length() < 0.02 {
        return cells;
    }
    let h = cells.len();
    let w = cells.first().map(|r| r.len()).unwrap_or(0);
    if h == 0 || w == 0 {
        return cells;
    }
    let dx = ((shake.x / rw) * area.width as f32).round() as i32;
    let dy = ((shake.y / rh) * area.height as f32).round() as i32;
    if dx == 0 && dy == 0 {
        return cells;
    }
    let mut out = vec![vec![(' ', Color::Rgb(6, 6, 10)); w]; h];
    for y in 0..h {
        for x in 0..w {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                out[ny as usize][nx as usize] = cells[y][x];
            }
        }
    }
    out
}

fn in_bounds(x: u16, y: u16, area: Rect) -> Option<(usize, usize)> {
    if x < area.width && y < area.height {
        Some((x as usize, y as usize))
    } else {
        None
    }
}

fn paint_halo(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    x: usize,
    y: usize,
    color: Color,
) {
    for (ox, oy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
        let nx = x as isize + ox;
        let ny = y as isize + oy;
        if nx >= 0 && ny >= 0 && nx < area.width as isize && ny < area.height as isize {
            let nx = nx as usize;
            let ny = ny as usize;
            if matches!(cells[ny][nx].0, '.' | '·' | ':' | '░' | '▒') {
                cells[ny][nx] = ('.', color);
            }
        }
    }
}

fn screen_to_world(x: usize, y: usize, rw: f32, rh: f32, area: Rect) -> Vec2 {
    let nx = x as f32 / area.width.saturating_sub(1).max(1) as f32;
    let ny = y as f32 / area.height.saturating_sub(1).max(1) as f32;
    Vec2::new(nx * rw, ny * rh)
}

fn on_any_platform(world: &World, pos: Vec2) -> bool {
    world.room.platforms.iter().any(|p| {
        pos.x >= p.x && pos.y >= p.y && pos.x <= p.x + p.w && pos.y <= p.y + p.h
    })
}

fn paint_platform_borders(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    world: &World,
    rw: f32,
    rh: f32,
) {
    let h = area.height as usize;
    let w = area.width as usize;
    if h < 3 || w < 3 {
        return;
    }
    let mut on = vec![vec![false; w]; h];
    for y in 0..h {
        for x in 0..w {
            let edge = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            if edge {
                continue;
            }
            on[y][x] = on_any_platform(world, screen_to_world(x, y, rw, rh, area));
        }
    }

    let border = Color::Rgb(160, 160, 170);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if !on[y][x] {
                continue;
            }
            let n = !on[y - 1][x];
            let s = !on[y + 1][x];
            let west = !on[y][x - 1];
            let e = !on[y][x + 1];
            if !(n || s || west || e) {
                continue;
            }
            let ch = match (n, s, west, e) {
                (true, false, true, false) => '┌',
                (true, false, false, true) => '┐',
                (false, true, true, false) => '└',
                (false, true, false, true) => '┘',
                (true, false, true, true) => '┬',
                (false, true, true, true) => '┴',
                (true, true, true, false) => '├',
                (true, true, false, true) => '┤',
                (true, true, true, true) => '┼',
                (true, true, false, false) => '│',
                (false, false, true, true) => '─',
                (true, false, _, _) | (false, true, _, _) => '─',
                (_, _, true, false) | (_, _, false, true) => '│',
                _ => '─',
            };
            cells[y][x] = (ch, border);
        }
    }
}

/// Per-room animated floor: colorful but faded so actors still read clearly.
fn floor_cell(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    world: &World,
    platform_grey: bool,
) -> (char, Color) {
    let t = world.anim_t;
    let fx = x as f32;
    let fy = y as f32;
    let nx = if width > 1 {
        fx / (width - 1) as f32
    } else {
        0.5
    };
    let ny = if height > 1 {
        fy / (height - 1) as f32
    } else {
        0.5
    };
    let cx = fx - width as f32 * 0.5;
    let cy = fy - height as f32 * 0.5;
    let dist = (cx * cx + cy * cy).sqrt();
    let angle = cy.atan2(cx);

    let (wave, glyph) = match world.room.floor_style {
        FloorStyle::Ripple => {
            let w = (dist * 0.55 - t * 2.2).sin();
            let g = if w > 0.45 {
                '≈'
            } else if w > 0.0 {
                '~'
            } else if w > -0.45 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Drift => {
            let w = ((fx * 0.35) + t * 1.8).sin() * 0.7
                + ((fy * 0.12) + t * 0.6).cos() * 0.3;
            let g = if w > 0.4 {
                '═'
            } else if w > 0.0 {
                '─'
            } else if w > -0.35 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Pulse => {
            let breath = (t * 1.4).sin();
            let radial = (dist * 0.2).sin();
            let w = breath * 0.55 + radial * 0.45;
            let g = if breath > 0.5 {
                '≈'
            } else if breath > 0.0 {
                '~'
            } else if breath > -0.4 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Diagonal => {
            let w = ((fx + fy) * 0.28 - t * 1.6).sin();
            let g = if w > 0.4 {
                '╱'
            } else if w > 0.0 {
                '·'
            } else if w > -0.4 {
                '╲'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Tide => {
            let w = ((fy * 0.4) + t * 1.5).sin() * 0.75
                + ((fx * 0.15) + t * 0.4).cos() * 0.25;
            let g = if w > 0.5 {
                '≈'
            } else if w > 0.1 {
                '~'
            } else if w > -0.3 {
                '·'
            } else {
                ':'
            };
            (w, g)
        }
        FloorStyle::Vortex => {
            let w = (angle * 2.0 + dist * 0.35 - t * 1.7).sin();
            let g = if w > 0.45 {
                '◎'
            } else if w > 0.1 {
                '◦'
            } else if w > -0.35 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Lattice => {
            let a = ((fx * 0.55) + t * 1.1).sin();
            let b = ((fy * 0.55) - t * 0.9).cos();
            let w = a * b;
            let g = if (x + y) % 2 == 0 {
                if w > 0.2 { '+' } else { '·' }
            } else if w > 0.35 {
                '+'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Bloom => {
            let petals = (angle * 3.0 + t * 1.2).sin();
            let ring = (dist * 0.4 - t * 1.5).cos();
            let w = petals * 0.55 + ring * 0.45;
            let g = if w > 0.5 {
                '*'
            } else if w > 0.15 {
                '·'
            } else if w > -0.3 {
                '˚'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Checker => {
            let pulse = (t * 2.2).sin();
            let cell = ((x / 2) + (y / 2)) % 2 == 0;
            let w = if cell { pulse } else { -pulse };
            let g = if cell {
                if pulse > 0.2 { '▪' } else { '·' }
            } else if pulse > 0.35 {
                ':'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Spiral => {
            let w = (angle * 2.5 + dist * 0.45 - t * 2.0).sin();
            let g = if w > 0.5 {
                '@'
            } else if w > 0.1 {
                'o'
            } else if w > -0.35 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Rain => {
            let streak = ((fx * 0.9) + (fy * 2.4) - t * 6.0).sin();
            let w = streak;
            let g = if streak > 0.55 {
                '|'
            } else if streak > 0.1 {
                ':'
            } else if streak > -0.4 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Zigzag => {
            let w = ((fx * 0.45 - fy * 0.45) + t * 1.7).sin();
            let g = if w > 0.45 {
                '⋀'
            } else if w > 0.0 {
                '·'
            } else if w > -0.45 {
                '⋁'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Contour => {
            let w = (dist * 0.5 - t * 1.3).sin() * 0.7 + ((fx + fy) * 0.08).cos() * 0.3;
            let g = if w > 0.55 {
                '='
            } else if w > 0.15 {
                '-'
            } else if w > -0.25 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Strobe => {
            let flash = (t * 5.5 + (x + y) as f32 * 0.15).sin();
            let w = flash;
            let g = if flash > 0.65 {
                '*'
            } else if flash > 0.15 {
                '+'
            } else if flash > -0.4 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Kaleido => {
            // 6-fold mirrored petals.
            let folded = (angle.abs() * 3.0 / std::f32::consts::PI).fract() * std::f32::consts::PI
                / 3.0;
            let w = (folded * 4.0 + dist * 0.55 - t * 2.4).sin()
                * (angle.cos() * 2.0 + t).cos();
            let g = if w > 0.45 {
                '❋'
            } else if w > 0.1 {
                '*'
            } else if w > -0.3 {
                '·'
            } else {
                '˚'
            };
            (w, g)
        }
        FloorStyle::Prism => {
            let bands = ((nx * 6.0 + ny * 2.0 - t * 1.5).sin()
                + (nx * 2.0 - ny * 5.0 + t * 1.1).cos())
                * 0.5;
            let w = bands;
            let g = if bands > 0.4 {
                '◇'
            } else if bands > 0.0 {
                '◈'
            } else if bands > -0.35 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Mandala => {
            let rings = (dist * 0.65 - t * 1.8).sin();
            let spokes = (angle * 6.0 + t * 0.9).cos();
            let w = rings * 0.55 + spokes * 0.45;
            let g = if w > 0.55 {
                '✽'
            } else if w > 0.15 {
                '✦'
            } else if w > -0.25 {
                '·'
            } else {
                '∘'
            };
            (w, g)
        }
        FloorStyle::Mirror => {
            // Quad symmetry about room center.
            let mx = (nx - 0.5).abs();
            let my = (ny - 0.5).abs();
            let w = ((mx * 9.0 + my * 9.0) - t * 2.0).sin()
                * ((mx - my) * 7.0 + t).cos();
            let g = if w > 0.4 {
                '╬'
            } else if w > 0.0 {
                '┼'
            } else if w > -0.4 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Fractal => {
            let mut zx = (nx - 0.5) * 2.4;
            let mut zy = (ny - 0.5) * 2.4;
            let cx = (t * 0.15).sin() * 0.35 - 0.2;
            let cy = (t * 0.11).cos() * 0.35;
            let mut escaped = 0.0f32;
            for i in 0..6 {
                let x2 = zx * zx - zy * zy + cx;
                zy = 2.0 * zx * zy + cy;
                zx = x2;
                if zx * zx + zy * zy > 4.0 {
                    escaped = (i as f32 + 1.0) / 6.0;
                    break;
                }
            }
            let w = escaped * 2.0 - 1.0 + (t * 2.0).sin() * 0.15;
            let g = if escaped > 0.7 {
                '◈'
            } else if escaped > 0.35 {
                '*'
            } else if escaped > 0.05 {
                '·'
            } else {
                ':'
            };
            (w, g)
        }
        FloorStyle::Crystal => {
            let facets = ((fx * 0.4).sin() * (fy * 0.4).cos()
                + ((fx + fy) * 0.25 - t * 1.6).sin())
                * 0.7;
            let w = facets;
            let g = if facets > 0.45 {
                '◆'
            } else if facets > 0.1 {
                '◇'
            } else if facets > -0.3 {
                '·'
            } else {
                '.'
            };
            (w, g)
        }
        FloorStyle::Pinwheel => {
            let spin = angle + dist * 0.25 - t * 2.2;
            let arms = (spin * 4.0).sin();
            let w = arms * (0.55 + 0.45 * (dist * 0.3).cos());
            let g = if arms > 0.5 {
                '✸'
            } else if arms > 0.0 {
                '/'
            } else if arms > -0.5 {
                '\\'
            } else {
                '·'
            };
            (w, g)
        }
        FloorStyle::Tessellate => {
            let tri = ((fx * 0.55 + t).sin() + (fy * 0.55 - t * 0.7).sin()
                + ((fx + fy) * 0.35).cos())
                / 3.0;
            let w = tri;
            let g = if tri > 0.35 {
                '▲'
            } else if tri > 0.0 {
                '△'
            } else if tri > -0.35 {
                '·'
            } else {
                '▽'
            };
            (w, g)
        }
    };

    // Depth fade toward the "back" of the room + wave modulation.
    // Keep value low so @ / enemies / projectiles stay readable on top.
    let kaleido = matches!(
        world.room.floor_style,
        FloorStyle::Kaleido
            | FloorStyle::Prism
            | FloorStyle::Mandala
            | FloorStyle::Mirror
            | FloorStyle::Fractal
            | FloorStyle::Crystal
            | FloorStyle::Pinwheel
            | FloorStyle::Tessellate
    );
    let depth = ny * 0.22 + wave * 0.08;
    let base_v = if platform_grey { 0.18 } else { 0.20 };
    let value = (base_v + depth * 0.10 + wave.abs() * 0.04).clamp(0.12, 0.34);
    let hue = world.room.floor_hue as f32
        + wave * if kaleido { 55.0 } else { 28.0 }
        + (nx - ny) * if kaleido { 36.0 } else { 18.0 }
        + t * if kaleido { 14.0 } else { 6.0 }
        + if kaleido {
            angle.to_degrees() * 0.35
        } else {
            0.0
        };
    let sat = if platform_grey {
        0.28
    } else if kaleido {
        0.55
    } else {
        0.38
    };
    let color = faded_hsv(hue, sat, value);
    (glyph, color)
}

/// Soft pastel RGB from HSV — colorful floors that stay behind combat.
pub fn faded_hsv(h: f32, s: f32, v: f32) -> Color {
    let (r, g, b) = hsv_to_rgb(h.rem_euclid(360.0), s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    Color::Rgb(r, g, b)
}

/// Mandala-style animated floor cell for the title screen background.
pub fn title_bg_cell(x: usize, y: usize, width: usize, height: usize, t: f32) -> (char, Color) {
    let fx = x as f32;
    let fy = y as f32;
    let nx = if width > 1 {
        fx / (width - 1) as f32
    } else {
        0.5
    };
    let ny = if height > 1 {
        fy / (height - 1) as f32
    } else {
        0.5
    };
    let cx = fx - width as f32 * 0.5;
    let cy = fy - height as f32 * 0.5;
    let dist = (cx * cx + cy * cy).sqrt();
    let angle = cy.atan2(cx);

    let rings = (dist * 0.65 - t * 1.8).sin();
    let spokes = (angle * 6.0 + t * 0.9).cos();
    let wave = rings * 0.55 + spokes * 0.45;
    let glyph = if wave > 0.55 {
        '✽'
    } else if wave > 0.15 {
        '✦'
    } else if wave > -0.25 {
        '·'
    } else {
        '∘'
    };

    let hue = 265.0 + wave * 55.0 + (nx - ny) * 36.0 + t * 14.0 + angle.to_degrees() * 0.35;
    let value = (0.14 + ny * 0.08 + wave.abs() * 0.05).clamp(0.10, 0.28);
    (glyph, faded_hsv(hue, 0.52, value))
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (rp, gp, bp) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((rp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((gp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((bp + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn is_floorish(ch: char) -> bool {
    matches!(
        ch,
        '.' | '·' | ':' | ' ' | '░' | '▒' | '~' | '≈' | '─' | '═' | '╱' | '╲' | '+' | '*' | '˚'
            | '◦' | '◎' | '∙' | '❋' | '◇' | '◈' | '✽' | '✦' | '∘' | '╬' | '┼' | '◆' | '✸' | '/'
            | '\\' | '▲' | '△' | '▽'
    )
}

/// How many cells deep a jagged wall occupies at (x,y). 0 = floor.
fn jagged_wall_depth(x: usize, y: usize, w: usize, h: usize, world: &World) -> u8 {
    if w < 4 || h < 4 {
        return if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
            1
        } else {
            0
        };
    }
    let seed = world.room.combat_index.wrapping_mul(7919) ^ world.room.floor_hue as u32;
    let dist_left = x;
    let dist_right = w - 1 - x;
    let dist_top = y;
    let dist_bot = h - 1 - y;
    let edge_dist = dist_left
        .min(dist_right)
        .min(dist_top)
        .min(dist_bot);

    // Along-edge coordinate for noise.
    let along = if dist_left == edge_dist || dist_right == edge_dist {
        y
    } else {
        x
    };
    let n = wall_noise(along as u32, seed);
    // Depth 1..=3: always at least the outer ring, often jutting inward.
    let depth = 1 + (n % 3) as usize;
    if edge_dist < depth {
        (edge_dist + 1) as u8
    } else {
        0
    }
}

fn wall_noise(along: u32, seed: u32) -> u32 {
    let mut v = along
        .wrapping_mul(374761393)
        .wrapping_add(seed.wrapping_mul(668265263));
    v = (v ^ (v >> 13)).wrapping_mul(1274126177);
    v ^ (v >> 16)
}

fn jagged_wall_cell(x: usize, y: usize, world: &World) -> (char, Color) {
    let seed = world.room.combat_index.wrapping_mul(7919) ^ world.room.floor_hue as u32;
    let n = wall_noise((x as u32).wrapping_mul(31).wrapping_add(y as u32), seed);
    let glyph = match n % 5 {
        0 => '█',
        1 => '▓',
        2 => '▒',
        3 => '#',
        _ => '▀',
    };
    let shade = 55 + (n % 40) as u8;
    (
        glyph,
        Color::Rgb(shade.saturating_sub(8), shade.saturating_sub(4), shade + 6),
    )
}

fn blue_trail_color(progress: f32) -> Color {
    // Tail → head: deep indigo → bright blue-violet
    let t = progress.clamp(0.0, 1.0);
    Color::Rgb(
        (40.0 + t * 120.0) as u8,
        (50.0 + t * 100.0) as u8,
        (120.0 + t * 135.0) as u8,
    )
}

fn rainbow_color(t: f32) -> Color {
    let h = (t * 90.0).rem_euclid(360.0);
    let (r, g, b) = hsv_to_rgb(h, 0.85, 1.0);
    Color::Rgb(r, g, b)
}

fn brighten(color: Color, amount: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32 * amount).min(255.0)) as u8,
            ((g as f32 * amount).min(255.0)) as u8,
            ((b as f32 * amount).min(255.0)) as u8,
        ),
        other => other,
    }
}

fn dim_color(color: Color, amount: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * amount) as u8,
            (g as f32 * amount) as u8,
            (b as f32 * amount) as u8,
        ),
        Color::Yellow | Color::LightYellow => Color::Rgb(140, 120, 40),
        Color::Red | Color::LightRed => Color::Rgb(140, 50, 40),
        Color::Cyan | Color::LightBlue => Color::Rgb(40, 110, 130),
        Color::Green | Color::LightGreen => Color::Rgb(40, 120, 60),
        Color::Magenta | Color::LightMagenta => Color::Rgb(120, 50, 120),
        Color::Blue => Color::Rgb(40, 60, 130),
        Color::White => Color::Gray,
        other => other,
    }
}

fn apply_scene_light(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    world: &World,
    rw: f32,
    rh: f32,
) {
    let torch_screens: Vec<(usize, usize, f32)> = world
        .room
        .torches
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let (x, y) = project(t.pos, rw, rh, area);
            in_bounds(x, y, area).map(|(x, y)| {
                let flicker = 0.75 + 0.25 * ((world.anim_t * 7.0) + i as f32).sin();
                (x, y, flicker)
            })
        })
        .collect();

    let (px, py) = project(world.player.pos, rw, rh, area);
    let player_screen = in_bounds(px, py, area);

    let mut shot_lights: Vec<(usize, usize, f32)> = Vec::new();
    for p in &world.projectiles {
        if !p.owner_is_player_side {
            continue;
        }
        let (sx, sy) = project(p.pos, rw, rh, area);
        if let Some((x, y)) = in_bounds(sx, sy, area) {
            shot_lights.push((x, y, 1.0));
        }
        // Trail samples cast softer light
        for (i, pos) in p.trail.iter().enumerate() {
            if i % 2 != 0 {
                continue;
            }
            let (sx, sy) = project(*pos, rw, rh, area);
            if let Some((x, y)) = in_bounds(sx, sy, area) {
                let strength = 0.35 + 0.45 * (i as f32 / p.trail.len().max(1) as f32);
                shot_lights.push((x, y, strength));
            }
        }
    }

    let pulse = 0.85 + 0.15 * (world.anim_t * 3.2).sin();

    for y in 0..area.height as usize {
        for x in 0..area.width as usize {
            let (ch, base) = cells[y][x];
            if matches!(ch, '#' | '█' | '▓' | '▒' | '▀' | ' ') {
                continue;
            }

            let mut warmth = 0.0f32;
            for &(tx, ty, flicker) in &torch_screens {
                let dx = x as f32 - tx as f32;
                let dy = y as f32 - ty as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 7.0 {
                    warmth = warmth.max((1.0 - dist / 7.0) * flicker);
                }
            }

            let mut blue = 0.0f32;
            if let Some((psx, psy)) = player_screen {
                let dx = x as f32 - psx as f32;
                let dy = y as f32 - psy as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 6.5 {
                    blue = blue.max((1.0 - dist / 6.5).powf(1.15) * pulse);
                }
            }
            for &(lx, ly, strength) in &shot_lights {
                let dx = x as f32 - lx as f32;
                let dy = y as f32 - ly as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 4.0 {
                    blue = blue.max((1.0 - dist / 4.0) * strength * 0.85);
                }
            }

            if warmth > 0.05 || blue > 0.05 {
                cells[y][x] = (ch, blend_lights(base, warmth, blue));
            }
        }
    }
}

fn blend_lights(base: Color, warmth: f32, blue: f32) -> Color {
    let (mut r, mut g, mut b) = match base {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        Color::DarkGray => (40.0, 40.0, 40.0),
        Color::Gray => (80.0, 80.0, 80.0),
        _ => (50.0, 70.0, 55.0),
    };
    let w = warmth.clamp(0.0, 1.0) * 0.55;
    if w > 0.0 {
        r += (255.0 - r) * w * 0.45;
        g += (180.0 - g) * w * 0.28;
        b *= 1.0 - w * 0.35;
    }
    let bl = blue.clamp(0.0, 1.0);
    if bl > 0.0 {
        // Bright blue / purple mage aura
        r += (140.0 - r) * bl * 0.55;
        g += (110.0 - g) * bl * 0.35;
        b += (255.0 - b) * bl * 0.75;
        // Lift overall so the aura pops on dark floors
        let lift = bl * 55.0;
        r = (r + lift * 0.35).min(255.0);
        g = (g + lift * 0.25).min(255.0);
        b = (b + lift * 0.55).min(255.0);
    }
    Color::Rgb(r as u8, g as u8, b as u8)
}

fn paint_explosion_ring(
    cells: &mut [Vec<(char, Color)>],
    area: Rect,
    pos: Vec2,
    radius: f32,
    rw: f32,
    rh: f32,
    color: Color,
    life_t: f32,
) {
    let steps = (radius * 10.0).clamp(8.0, 28.0) as i32;
    for i in 0..steps {
        let a = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let p = Vec2::new(pos.x + a.cos() * radius, pos.y + a.sin() * radius * 0.75);
        let (px, py) = project(p, rw, rh, area);
        if let Some((x, y)) = in_bounds(px, py, area) {
            let glyph = if life_t > 0.6 {
                '*'
            } else if life_t > 0.3 {
                '+'
            } else {
                '.'
            };
            cells[y][x] = (glyph, color);
        }
    }
    // Hot center
    let (cx, cy) = project(pos, rw, rh, area);
    if let Some((x, y)) = in_bounds(cx, cy, area) {
        cells[y][x] = ('█', Color::White);
    }
}

pub fn _modifier_demo() -> Modifier {
    Modifier::BOLD
}
