use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use ratatui::style::Color;

use crate::world::boss::{self, BossMod};
use crate::world::entity::{Actor, ActorKind, BossForm, Vec2};
use crate::world::room::RoomState;

pub struct EnemySpawn {
    pub pos: Vec2,
    pub hp: f32,
    pub glyph: char,
    pub color: Color,
    pub kind: ActorKind,
    pub can_shoot: bool,
    pub radius: f32,
    pub splits_left: u8,
    pub boss_form: BossForm,
}

/// Soft-exponential pressure: tame early, brutal later.
pub fn scaled_enemy_count(combat_index: u32, wave: u32, rng: &mut ChaCha8Rng) -> usize {
    let room = combat_index.max(1) as f32;
    let wave_f = wave.max(1) as f32;
    let base = 2.4 * (1.38_f32).powf(room * 0.85) * (0.85 + 0.2 * wave_f);
    let jitter = rng.random_range(0.0..1.6);
    (base + jitter).round().clamp(3.0, 80.0) as usize
}

pub fn wave_enemies(
    room: &RoomState,
    wave: u32,
    rng: &mut ChaCha8Rng,
    boss_mods: &[BossMod],
) -> Vec<EnemySpawn> {
    if room.kind == crate::world::room::RoomKind::Boss {
        // Serpent every other boss (rooms 6, 12, …); classic otherwise (3, 9, …).
        if room.combat_index % 6 == 0 {
            return spawn_snake_boss(room, rng);
        }
        let radius = 1.15 + (room.combat_index as f32 * 0.02).min(0.35);
        let splits = if boss_mods.contains(&BossMod::Multiply) {
            2
        } else {
            0
        };
        return vec![EnemySpawn {
            pos: room.snap_to_footing(
                Vec2::new(room.width * 0.72, room.height * 0.5),
                radius,
            ),
            hp: boss::scaled_boss_hp(room.combat_index),
            glyph: 'M',
            color: Color::LightRed,
            kind: ActorKind::Boss,
            can_shoot: true,
            radius,
            splits_left: splits,
            boss_form: BossForm::Classic,
        }];
    }

    let mut count = scaled_enemy_count(room.combat_index, wave, rng);
    if room.mega {
        count = ((count as f32) * 1.35).round() as usize;
    }
    let mut out = Vec::with_capacity(count + 1);
    // Halved HP baseline — early rooms clear faster; still scales with room index.
    out.push(EnemySpawn {
        pos: room.sample_footing(rng, 0.55, true),
        hp: 13.0 + room.combat_index as f32 * 2.0,
        glyph: 's',
        color: Color::LightYellow,
        kind: ActorKind::Enemy,
        can_shoot: true,
        radius: 0.55,
        splits_left: 0,
        boss_form: BossForm::None,
    });

    for _i in 1..count {
        let elite = rng.random_bool(0.12 + (room.combat_index as f32 * 0.015).min(0.25) as f64);
        let shooter = rng.random_bool((0.18 + room.combat_index as f32 * 0.03).min(0.55) as f64);
        out.push(EnemySpawn {
            pos: room.sample_footing(rng, 0.55, true),
            hp: if elite { 21.0 } else { 10.0 }
                + (1.22_f32).powf(room.combat_index as f32) * 1.25,
            glyph: if shooter {
                's'
            } else if elite {
                'E'
            } else {
                'e'
            },
            color: if shooter {
                Color::LightYellow
            } else if elite {
                Color::LightMagenta
            } else {
                Color::Red
            },
            kind: ActorKind::Enemy,
            can_shoot: shooter,
            radius: if elite { 0.65 } else { 0.55 },
            splits_left: 0,
            boss_form: BossForm::None,
        });
    }
    out
}

fn spawn_snake_boss(room: &RoomState, rng: &mut ChaCha8Rng) -> Vec<EnemySpawn> {
    let head_hp = boss::scaled_boss_hp(room.combat_index) * 0.85;
    let seg_hp = head_hp * 0.22;
    let head_pos = room.snap_to_footing(
        Vec2::new(room.width * 0.7, room.height * 0.5 + rng.random_range(-2.0..2.0)),
        0.9,
    );
    let segments = 7 + (room.combat_index / 3).min(5) as usize;
    let mut out = Vec::with_capacity(segments + 1);
    out.push(EnemySpawn {
        pos: head_pos,
        hp: head_hp,
        glyph: 'S',
        color: Color::LightGreen,
        kind: ActorKind::Boss,
        can_shoot: true,
        radius: 0.95,
        splits_left: 0,
        boss_form: BossForm::SnakeHead,
    });
    for i in 0..segments {
        let t = (i + 1) as f32;
        out.push(EnemySpawn {
            pos: head_pos + Vec2::new(t * 0.95, (i as f32 * 0.15).sin() * 0.4),
            hp: seg_hp,
            glyph: if i % 2 == 0 { 'o' } else { 'O' },
            color: Color::Green,
            kind: ActorKind::Boss,
            can_shoot: false,
            radius: (0.7 - i as f32 * 0.02).max(0.45),
            splits_left: 0,
            boss_form: BossForm::SnakeBody,
        });
    }
    out
}

pub fn make_actor(id: u64, spawn: EnemySpawn) -> Actor {
    Actor {
        id,
        kind: spawn.kind,
        pos: spawn.pos,
        vel: Vec2::ZERO,
        facing: Vec2::new(-1.0, 0.0),
        hp: spawn.hp,
        max_hp: spawn.hp,
        radius: spawn.radius,
        glyph: spawn.glyph,
        color: spawn.color,
        formation_local: Vec2::ZERO,
        formation_index: 0,
        poison_timer: 0.0,
        poison_stacks: 0.0,
        fire_stacks: 0.0,
        vuln_bonus: 0.0,
        kill_gold_bonus: 0.0,
        kill_xp_bonus: 0.0,
        kill_source: String::new(),
        can_shoot: spawn.can_shoot || spawn.boss_form == BossForm::SnakeHead,
        shoot_cd: if spawn.can_shoot { 0.4 } else { 0.0 },
        splits_left: spawn.splits_left,
        boss_gen: 0,
        boss_form: spawn.boss_form,
        trail: vec![spawn.pos; 48],
        reanim_tier: 0,
        is_dummy: false,
    }
}
