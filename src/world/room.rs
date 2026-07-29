use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::entity::{Column, Torch, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    Combat,
    Boss,
    Shop,
}

/// Per-room floor animation pattern (picked at generation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorStyle {
    Ripple,
    Drift,
    Pulse,
    Diagonal,
    Tide,
    Vortex,
    Lattice,
    Bloom,
    Checker,
    Spiral,
    Rain,
    Zigzag,
    Contour,
    Strobe,
    Kaleido,
    Prism,
    Mandala,
    Mirror,
    Fractal,
    Crystal,
    Pinwheel,
    Tessellate,
}

impl FloorStyle {
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        match rng.random_range(0..22u8) {
            0 => Self::Ripple,
            1 => Self::Drift,
            2 => Self::Pulse,
            3 => Self::Diagonal,
            4 => Self::Tide,
            5 => Self::Vortex,
            6 => Self::Lattice,
            7 => Self::Bloom,
            8 => Self::Checker,
            9 => Self::Spiral,
            10 => Self::Rain,
            11 => Self::Zigzag,
            12 => Self::Contour,
            13 => Self::Strobe,
            14 => Self::Kaleido,
            15 => Self::Prism,
            16 => Self::Mandala,
            17 => Self::Mirror,
            18 => Self::Fractal,
            19 => Self::Crystal,
            20 => Self::Pinwheel,
            _ => Self::Tessellate,
        }
    }
}

/// Walkable island in a platform room (world units).
#[derive(Debug, Clone, Copy)]
pub struct Platform {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Platform {
    pub fn contains_body(&self, pos: Vec2, radius: f32) -> bool {
        pos.x - radius >= self.x
            && pos.y - radius >= self.y
            && pos.x + radius <= self.x + self.w
            && pos.y + radius <= self.y + self.h
    }

    pub fn clamp_inside(&self, pos: Vec2, radius: f32) -> Vec2 {
        let min_x = self.x + radius;
        let max_x = (self.x + self.w - radius).max(min_x);
        let min_y = self.y + radius;
        let max_y = (self.y + self.h - radius).max(min_y);
        Vec2::new(pos.x.clamp(min_x, max_x), pos.y.clamp(min_y, max_y))
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    pub fn sample(&self, rng: &mut ChaCha8Rng, radius: f32) -> Vec2 {
        let pad = radius + 0.15;
        let x0 = self.x + pad;
        let y0 = self.y + pad;
        let x1 = (self.x + self.w - pad).max(x0);
        let y1 = (self.y + self.h - pad).max(y0);
        Vec2::new(
            rng.random_range(x0..=x1),
            rng.random_range(y0..=y1),
        )
    }
}

#[derive(Debug, Clone)]
pub struct RoomState {
    pub kind: RoomKind,
    pub width: f32,
    pub height: f32,
    pub combat_index: u32,
    pub wave: u32,
    pub waves_total: u32,
    pub cleared: bool,
    pub doors_open: bool,
    pub spawn_timer: f32,
    /// Base hue for faded floor color (0..360 degrees).
    pub floor_hue: u16,
    pub floor_style: FloorStyle,
    pub torches: Vec<Torch>,
    /// Empty = solid floor. Non-empty = islands; void between requires a dash.
    pub platforms: Vec<Platform>,
    /// Sparse destructible pillars (mega halls).
    pub columns: Vec<Column>,
    pub mega: bool,
}

impl RoomState {
    pub fn new_combat(combat_index: u32, rng: &mut ChaCha8Rng) -> Self {
        let is_boss = combat_index > 0 && combat_index % 3 == 0;
        let kind = if is_boss {
            RoomKind::Boss
        } else {
            RoomKind::Combat
        };
        let waves_total = if is_boss {
            1
        } else {
            2 + rng.random_range(0..=2)
        };
        // Cathedral halls: ~4× area (2× each axis), never platforms/boss.
        let mega = !is_boss && rng.random_bool(0.16);
        let (width, height) = if mega {
            (92.0, 44.0)
        } else {
            (46.0, 22.0)
        };
        // Platform gauntlets only from room 4 onward (skip early tutorial rooms).
        let platforms = if !is_boss && !mega && combat_index >= 4 && rng.random_bool(0.42) {
            gen_platforms(width, height, rng)
        } else {
            Vec::new()
        };
        // Pillar alley: normal-sized room with sparse breakable cover (room 5+).
        let columns = if mega {
            gen_columns(width, height, rng)
        } else if !is_boss
            && platforms.is_empty()
            && combat_index >= 5
            && rng.random_bool(0.18)
        {
            gen_alley_columns(width, height, rng)
        } else {
            Vec::new()
        };
        let torches = if platforms.is_empty() {
            place_torches(width, height, rng)
        } else {
            Vec::new()
        };
        Self {
            kind,
            width,
            height,
            combat_index,
            wave: 0,
            waves_total,
            cleared: false,
            doors_open: false,
            spawn_timer: 0.35,
            floor_hue: rng.random_range(0..360),
            floor_style: FloorStyle::random(rng),
            torches,
            platforms,
            columns,
            mega,
        }
    }

    pub fn new_shop(combat_index: u32) -> Self {
        let width = 40.0;
        let height = 18.0;
        let mut rng = ChaCha8Rng::seed_from_u64(combat_index as u64 + 99);
        Self {
            kind: RoomKind::Shop,
            width,
            height,
            combat_index,
            wave: 0,
            waves_total: 0,
            cleared: true,
            doors_open: true,
            spawn_timer: 0.0,
            floor_hue: rng.random_range(0..360),
            floor_style: FloorStyle::random(&mut rng),
            torches: place_torches(width, height, &mut rng),
            platforms: Vec::new(),
            columns: Vec::new(),
            mega: false,
        }
    }

    pub fn has_platforms(&self) -> bool {
        !self.platforms.is_empty()
    }

    pub fn has_columns(&self) -> bool {
        !self.columns.is_empty()
    }

    /// Normal-sized combat room with breakable pillars (not a cathedral mega).
    pub fn is_pillar_alley(&self) -> bool {
        !self.mega && self.has_columns() && !self.has_platforms()
    }

    /// Push a body out of solid columns (simple radial resolve).
    pub fn resolve_columns(&self, pos: Vec2, radius: f32) -> Vec2 {
        let mut p = pos;
        for col in &self.columns {
            if col.hp <= 0.0 {
                continue;
            }
            let min_dist = col.radius + radius;
            let d = p.dist(col.pos);
            if d < min_dist && d > 0.001 {
                let push = (p - col.pos).normalized() * (min_dist - d);
                p += push;
            } else if d <= 0.001 {
                p.x += min_dist;
            }
        }
        self.clamp(p, radius)
    }

    pub fn contains(&self, pos: Vec2, radius: f32) -> bool {
        pos.x - radius >= 1.0
            && pos.y - radius >= 1.0
            && pos.x + radius <= self.width - 1.0
            && pos.y + radius <= self.height - 1.0
    }

    pub fn clamp(&self, pos: Vec2, radius: f32) -> Vec2 {
        Vec2::new(
            pos.x.clamp(1.0 + radius, self.width - 1.0 - radius),
            pos.y.clamp(1.0 + radius, self.height - 1.0 - radius),
        )
    }

    pub fn on_footing(&self, pos: Vec2, radius: f32) -> bool {
        if self.platforms.is_empty() {
            return self.contains(pos, radius);
        }
        self.platforms.iter().any(|p| p.contains_body(pos, radius))
    }

    /// Walk / enemy move: stay on platforms when present. Dash uses `allow_void`.
    pub fn clamp_move(&self, from: Vec2, to: Vec2, radius: f32, allow_void: bool) -> Vec2 {
        let to = self.clamp(to, radius);
        let to = self.resolve_columns(to, radius);
        if self.platforms.is_empty() || allow_void || self.on_footing(to, radius) {
            return to;
        }
        let x_only = self.resolve_columns(self.clamp(Vec2::new(to.x, from.y), radius), radius);
        if self.on_footing(x_only, radius) {
            return x_only;
        }
        let y_only = self.resolve_columns(self.clamp(Vec2::new(from.x, to.y), radius), radius);
        if self.on_footing(y_only, radius) {
            return y_only;
        }
        from
    }

    /// After a dash across void, land on the nearest platform.
    pub fn snap_to_footing(&self, pos: Vec2, radius: f32) -> Vec2 {
        let pos = self.clamp(pos, radius);
        if self.platforms.is_empty() || self.on_footing(pos, radius) {
            return pos;
        }
        let mut best = pos;
        let mut best_d = f32::MAX;
        for p in &self.platforms {
            let c = p.clamp_inside(pos, radius);
            let d = c.dist(pos);
            if d < best_d {
                best_d = d;
                best = c;
            }
        }
        best
    }

    pub fn sample_footing(&self, rng: &mut ChaCha8Rng, radius: f32, prefer_right: bool) -> Vec2 {
        if self.platforms.is_empty() {
            let x0 = if prefer_right {
                self.width * 0.55
            } else {
                3.0
            };
            return Vec2::new(
                rng.random_range(x0..(self.width - 3.0)),
                rng.random_range(3.0..(self.height - 3.0)),
            );
        }
        let mut idxs: Vec<usize> = (0..self.platforms.len()).collect();
        if prefer_right {
            idxs.sort_by(|&a, &b| {
                self.platforms[b]
                    .center()
                    .x
                    .partial_cmp(&self.platforms[a].center().x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Bias toward right half of platforms.
            let rightish: Vec<usize> = idxs
                .iter()
                .copied()
                .filter(|&i| self.platforms[i].center().x > self.width * 0.4)
                .collect();
            if !rightish.is_empty() {
                idxs = rightish;
            }
        }
        let i = idxs[rng.random_range(0..idxs.len())];
        self.platforms[i].sample(rng, radius)
    }

    pub fn entrance_door(&self) -> Vec2 {
        Vec2::new(2.0, self.height * 0.5)
    }

    pub fn exit_door(&self) -> Vec2 {
        Vec2::new(self.width - 2.0, self.height * 0.5)
    }
}

/// Gaps stay ~3.8–4.5 so a normal dash (~5.3 units) can bridge them.
fn gen_platforms(width: f32, height: f32, rng: &mut ChaCha8Rng) -> Vec<Platform> {
    let mid_y = height * 0.5;
    // Shared entrance / exit pads (spawn + doors).
    let mut platforms = vec![
        Platform {
            x: 1.5,
            y: mid_y - 4.0,
            w: 10.0,
            h: 8.0,
        }, // ends ~11.5
        Platform {
            x: 31.5,
            y: mid_y - 4.0,
            w: (width - 1.5) - 31.5,
            h: 8.0,
        }, // starts 31.5
    ];

    match rng.random_range(0..5) {
        0 => {
            // Single center stone (gap 4.0 from enter, 4.0 to exit)
            platforms.push(Platform {
                x: 15.5,
                y: mid_y - 3.5,
                w: 12.0,
                h: 7.0,
            }); // 15.5..27.5
        }
        1 => {
            // Split high / low mid — dash between lanes (~4 gap in y)
            platforms.push(Platform {
                x: 15.5,
                y: 2.2,
                w: 12.0,
                h: 5.6,
            }); // y 2.2..7.8
            platforms.push(Platform {
                x: 15.5,
                y: 11.8,
                w: 12.0,
                h: 5.6,
            }); // y 11.8..17.4
        }
        2 => {
            // Offset stepping stones (gaps ~4)
            platforms.push(Platform {
                x: 15.5,
                y: mid_y - 6.2,
                w: 7.5,
                h: 5.2,
            }); // ends 23.0
            platforms.push(Platform {
                x: 27.0,
                y: mid_y + 0.8,
                w: 7.5,
                h: 5.2,
            }); // 27..34.5 — kisses exit pad as a landing lane
        }
        3 => {
            // Bridge run — narrow horizontal span between enter/exit pads.
            platforms.push(Platform {
                x: 12.8,
                y: mid_y - 1.6,
                w: 17.5,
                h: 3.2,
            }); // ~4 gap from each pad vertically tight; dash along the ribbon
        }
        _ => {
            // Donut ring — corner islands + thin mid pads, large central void.
            let pad_w = 7.0;
            let pad_h = 4.2;
            platforms.push(Platform {
                x: 13.0,
                y: 2.0,
                w: pad_w,
                h: pad_h,
            }); // NW
            platforms.push(Platform {
                x: 26.0,
                y: 2.0,
                w: pad_w,
                h: pad_h,
            }); // NE
            platforms.push(Platform {
                x: 13.0,
                y: height - 2.0 - pad_h,
                w: pad_w,
                h: pad_h,
            }); // SW
            platforms.push(Platform {
                x: 26.0,
                y: height - 2.0 - pad_h,
                w: pad_w,
                h: pad_h,
            }); // SE
            // Thin mid connectors (north/south of center void)
            platforms.push(Platform {
                x: 19.5,
                y: mid_y - 5.8,
                w: 6.0,
                h: 2.4,
            });
            platforms.push(Platform {
                x: 19.5,
                y: mid_y + 3.4,
                w: 6.0,
                h: 2.4,
            });
        }
    }
    platforms
}

fn gen_columns(width: f32, height: f32, rng: &mut ChaCha8Rng) -> Vec<Column> {
    gen_columns_grid(width, height, rng, 14.0, 10.0, 0.55)
}

/// Sparser pillars for normal-sized "pillar alley" rooms.
fn gen_alley_columns(width: f32, height: f32, rng: &mut ChaCha8Rng) -> Vec<Column> {
    gen_columns_grid(width, height, rng, 11.0, 8.0, 0.42)
}

fn gen_columns_grid(
    width: f32,
    height: f32,
    rng: &mut ChaCha8Rng,
    x_step: f32,
    y_step: f32,
    chance: f64,
) -> Vec<Column> {
    let mut cols = Vec::new();
    let mut x = 12.0;
    while x < width - 10.0 {
        let mut y = 6.0;
        while y < height - 6.0 {
            let near_door = x > width - 14.0 && (y - height * 0.5).abs() < 5.0;
            let near_spawn = x < 14.0 && (y - height * 0.5).abs() < 5.0;
            if !near_door && !near_spawn && rng.random_bool(chance) {
                let hp = 28.0 + rng.random_range(0.0..18.0);
                cols.push(Column {
                    pos: Vec2::new(
                        x + rng.random_range(-1.2..1.2),
                        y + rng.random_range(-1.0..1.0),
                    ),
                    radius: 0.85 + rng.random_range(0.0..0.35),
                    hp,
                    max_hp: hp,
                });
            }
            y += y_step;
        }
        x += x_step;
    }
    cols
}

fn place_torches(width: f32, height: f32, rng: &mut ChaCha8Rng) -> Vec<Torch> {
    let mut torches = Vec::new();
    // Top / bottom walls
    let cols = 3 + rng.random_range(0..=2);
    for i in 0..cols {
        let t = (i as f32 + 0.5) / cols as f32;
        let x = 3.0 + t * (width - 6.0);
        torches.push(Torch {
            pos: Vec2::new(x, 1.4),
        });
        torches.push(Torch {
            pos: Vec2::new(x + rng.random_range(-0.4..0.4), height - 1.4),
        });
    }
    // Side walls (avoid door midlines)
    torches.push(Torch {
        pos: Vec2::new(1.4, height * 0.25),
    });
    torches.push(Torch {
        pos: Vec2::new(1.4, height * 0.75),
    });
    torches.push(Torch {
        pos: Vec2::new(width - 1.4, height * 0.25),
    });
    torches.push(Torch {
        pos: Vec2::new(width - 1.4, height * 0.75),
    });
    torches
}
