use ratatui::style::Color;

use crate::proj_logic::PlannedShot;

use super::entity::{PendingShot, Projectile, Vec2};

pub fn color_from_name(name: &str) -> Color {
    match name {
        "Yellow" | "LightYellow" => Color::Yellow,
        "Red" | "LightRed" => Color::Red,
        "Cyan" => Color::Cyan,
        "Green" => Color::Green,
        "White" => Color::White,
        "Blue" | "LightBlue" => Color::Blue,
        "Magenta" | "LightMagenta" => Color::Magenta,
        "LightGreen" => Color::LightGreen,
        "Gray" => Color::Gray,
        "DarkGray" => Color::DarkGray,
        _ => Color::White,
    }
}

pub fn pending_from_shot(
    shot: PlannedShot,
    origin: Vec2,
    facing: Vec2,
    owner_is_player_side: bool,
) -> PendingShot {
    PendingShot {
        delay: shot.delay,
        origin,
        facing,
        shot,
        owner_is_player_side,
    }
}

pub fn spawn_projectile(id: u64, pending: &PendingShot) -> Projectile {
    let base_angle = pending.facing.angle() + pending.shot.angle_offset;
    let dir = Vec2::from_angle(base_angle);
    let player_side = pending.owner_is_player_side;
    let orbiting = pending.shot.orbiting;
    let orbit_radius = if orbiting {
        pending.shot.orbit_radius.max(1.6)
    } else {
        0.0
    };
    let has_abs =
        pending.shot.spawn_ox.abs() > 0.01 || pending.shot.spawn_oy.abs() > 0.01;
    let pos = if has_abs {
        pending.origin + Vec2::new(pending.shot.spawn_ox, pending.shot.spawn_oy)
    } else if pending.shot.ring_spawn {
        pending.origin + Vec2::from_angle(base_angle) * pending.shot.spawn_radius.max(0.5)
    } else if orbiting {
        pending.origin + Vec2::from_angle(base_angle) * orbit_radius
    } else {
        pending.origin + dir * 0.8
    };
    let vel = if orbiting {
        Vec2::ZERO
    } else if pending.shot.ring_spawn || has_abs {
        dir * pending.shot.speed * 0.35
    } else {
        dir * pending.shot.speed
    };
    Projectile {
        id,
        pos,
        vel,
        damage: pending.shot.damage,
        radius: pending.shot.radius * if player_side { 1.35 } else { 1.0 },
        explosion_radius: pending.shot.explosion_radius,
        lifetime: pending.shot.lifetime,
        max_lifetime: pending.shot.lifetime,
        age: 0.0,
        bounces: pending.shot.bounces,
        pierce: pending.shot.pierce,
        pierced: Vec::new(),
        homing: pending.shot.homing,
        poison: pending.shot.poison,
        chain: pending.shot.chain,
        glyph: pending.shot.glyph,
        color: color_from_name(&pending.shot.color_name),
        owner_is_player_side: player_side,
        friendly_fire: pending.shot.friendly_fire,
        chained: 0,
        trail: Vec::new(),
        returning: pending.shot.returning,
        returned: false,
        trail_fx: pending.shot.trail_fx,
        trail_rainbow: pending.shot.trail_rainbow,
        trail_bright: pending.shot.trail_bright.max(1.0),
        orbiting,
        orbit_angle: base_angle,
        orbit_radius,
        gravity_well: pending.shot.gravity_well,
        arc: pending.shot.arc,
        gated: pending.shot.gated,
        gated_done: false,
        orbit_then_fire: pending.shot.orbit_then_fire,
        orbit_launch_at: pending.shot.orbit_launch_at,
        homing_ramp: pending.shot.homing_ramp,
        crit_bonus: pending.shot.crit_bonus,
        lock_target: None,
        is_flock: pending.shot.special == "flock",
        source_id: pending.shot.source_id.clone(),
        gold_bonus: pending.shot.gold_bonus,
        xp_bonus: pending.shot.xp_bonus,
        poison_stacks: pending.shot.poison_stacks,
        fire_stacks: pending.shot.fire_stacks,
        vuln_bonus: pending.shot.vuln_bonus,
        glow_green: pending.shot.glow_green,
        glow_red: pending.shot.glow_red,
        glow_halo: pending.shot.glow_halo,
        ring_spawn: pending.shot.ring_spawn || has_abs,
        spawn_radius: pending.shot.spawn_radius,
        spawn_offset: if has_abs {
            Vec2::new(pending.shot.spawn_ox, pending.shot.spawn_oy)
        } else {
            Vec2::ZERO
        },
    }
}
