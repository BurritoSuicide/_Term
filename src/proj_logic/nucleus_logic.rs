use rand::{Rng, RngExt};
use std::f32::consts::TAU;

use super::{SpellDef, SpellKind, SpellLibrary};

#[derive(Debug, Clone)]
pub struct PlannedShot {
    pub delay: f32,
    pub damage: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub radius: f32,
    pub explosion_radius: f32,
    pub angle_offset: f32,
    pub bounces: u32,
    pub pierce: u32,
    pub homing: f32,
    pub poison: f32,
    pub chain: u32,
    pub glyph: char,
    pub color_name: String,
    pub friendly_fire: bool,
    pub returning: bool,
    pub trail_fx: bool,
    pub trail_rainbow: bool,
    pub trail_bright: f32,
    pub orbiting: bool,
    pub orbit_radius: f32,
    pub orbit_then_fire: bool,
    pub orbit_launch_at: f32,
    pub gravity_well: bool,
    pub arc: bool,
    pub gated: bool,
    pub homing_ramp: bool,
    pub crit_bonus: f32,
    /// Behavior tag copied from the payload (voyager / neptune / …).
    pub special: String,
    /// Which projectile id produced this shot (for MK XP).
    pub source_id: String,
    pub gold_bonus: f32,
    pub xp_bonus: f32,
    pub poison_stacks: f32,
    pub fire_stacks: f32,
    pub vuln_bonus: f32,
    pub glow_green: bool,
    pub glow_red: bool,
    pub glow_halo: bool,
    pub ring_spawn: bool,
    pub spawn_radius: f32,
    /// Absolute offset from cast origin (link lines between splashes).
    pub spawn_ox: f32,
    pub spawn_oy: f32,
}

impl Default for PlannedShot {
    fn default() -> Self {
        Self {
            delay: 0.0,
            damage: 3.0,
            speed: 14.0,
            lifetime: 0.35,
            radius: 0.3,
            explosion_radius: 0.0,
            angle_offset: 0.0,
            bounces: 0,
            pierce: 0,
            homing: 0.0,
            poison: 0.0,
            chain: 0,
            glyph: '.',
            color_name: "Gray".into(),
            friendly_fire: false,
            returning: false,
            trail_fx: false,
            trail_rainbow: false,
            trail_bright: 1.0,
            orbiting: false,
            orbit_radius: 0.0,
            orbit_then_fire: false,
            orbit_launch_at: 0.7,
            gravity_well: false,
            arc: false,
            gated: false,
            homing_ramp: false,
            crit_bonus: 0.0,
            special: String::new(),
            source_id: String::new(),
            gold_bonus: 0.0,
            xp_bonus: 0.0,
            poison_stacks: 0.0,
            fire_stacks: 0.0,
            vuln_bonus: 0.0,
            glow_green: false,
            glow_red: false,
            glow_halo: false,
            ring_spawn: false,
            spawn_radius: 0.0,
            spawn_ox: 0.0,
            spawn_oy: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NucleusPlan {
    pub shots: Vec<PlannedShot>,
    pub mana_cost: i32,
    /// Fire interval multiplier (< 1 = faster).
    pub fire_interval_mult: f32,
}

#[derive(Debug, Clone)]
struct NucleusEvalState {
    speed_mult: f32,
    lifetime_mult: f32,
    damage_mult: f32,
    radius_mult: f32,
    spread: f32,
    bounces: u32,
    pierce: u32,
    extra_count: u32,
    homing: f32,
    fire_interval_mult: f32,
    flat_damage: f32,
    gold_bonus: f32,
    xp_bonus: f32,
    poison_stacks: f32,
    fire_stacks: f32,
    vuln_bonus: f32,
    glow_green: bool,
    glow_red: bool,
    glow_halo: bool,
    /// One-shot modifiers for the next payload.
    next_crit_bonus: f32,
    next_cd_mult: f32,
    next_gated: bool,
    /// Multiply following payload's shot count (Scatter / Proj_Dupe).
    next_shot_mult: u32,
    next_dmg_scale: f32,
    /// Copy_Fail outcome for the next deck item.
    copy_next: bool,
    skip_next: bool,
}

impl Default for NucleusEvalState {
    fn default() -> Self {
        Self {
            speed_mult: 1.0,
            lifetime_mult: 1.0,
            damage_mult: 1.0,
            radius_mult: 1.0,
            spread: 0.0,
            bounces: 0,
            pierce: 0,
            extra_count: 0,
            homing: 0.0,
            fire_interval_mult: 1.0,
            flat_damage: 0.0,
            gold_bonus: 0.0,
            xp_bonus: 0.0,
            poison_stacks: 0.0,
            fire_stacks: 0.0,
            vuln_bonus: 0.0,
            glow_green: false,
            glow_red: false,
            glow_halo: false,
            next_crit_bonus: 0.0,
            next_cd_mult: 1.0,
            next_gated: false,
            next_shot_mult: 1,
            next_dmg_scale: 1.0,
            copy_next: false,
            skip_next: false,
        }
    }
}

impl NucleusEvalState {
    fn apply_modifier(&mut self, def: &SpellDef, mod_strength: f32) {
        if def.speed_mult > 0.0 {
            let base = def.speed_mult;
            let bonus = (base - 1.0) * mod_strength + 1.0;
            self.speed_mult *= if base > 1.0 { bonus } else { base };
        }
        if def.lifetime_mult > 0.0 {
            self.lifetime_mult *= def.lifetime_mult;
        }
        if def.damage_mult > 0.0 {
            self.damage_mult *= def.damage_mult;
        }
        if def.radius_mult > 0.0 {
            let base = def.radius_mult;
            let bonus = (base - 1.0) * mod_strength + 1.0;
            self.radius_mult *= if base > 1.0 { bonus } else { base };
        }
        self.bounces += def.add_bounces;
        self.pierce += def.add_pierce;
        self.spread += def.add_spread;
        self.extra_count += def.add_count;
        if def.homing > 0.0 {
            self.homing = self.homing.max(def.homing);
        }
        match def.special.as_str() {
            "fire_rate" => {
                // Base −10%; each extra owned copy adds −5% (via mod_strength).
                let cut = 0.10 * mod_strength;
                self.fire_interval_mult *= (1.0 - cut).max(0.12);
            }
            "gated" => {
                self.next_gated = true;
            }
            "black_cat" => {
                // Base +10% crit; +5% per extra copy.
                self.next_crit_bonus += 0.10 * mod_strength;
            }
            "dedown" => {
                // Base −10% fire delay; −5% per extra copy.
                let cut = 0.10 * mod_strength;
                self.next_cd_mult *= (1.0 - cut).max(0.15);
            }
            "scatter" => {
                self.next_shot_mult = self.next_shot_mult.saturating_mul(4);
                self.next_dmg_scale *= 0.40; // −60% damage
            }
            "proj_dupe" => {
                self.next_shot_mult = self.next_shot_mult.saturating_mul(2);
            }
            "midas_touch" => {
                self.gold_bonus += 0.20 * mod_strength;
            }
            "proj_xp_gain" => {
                self.xp_bonus += 0.30 * mod_strength;
            }
            "glob_xp" => {
                // Applied at kill time from ownership; equipping still tags the shot.
                self.xp_bonus += 0.0;
            }
            "auto_lock" => {
                self.homing += 0.10 * mod_strength;
            }
            "dps_plus" => {
                self.flat_damage += 5.0 * mod_strength;
            }
            "bounce" => {
                // n copies → +n bounces (base 1, +1 per upgrade).
                let n = ((mod_strength - 1.0) / 0.5 + 1.0).round().max(1.0) as u32;
                self.bounces += n;
            }
            "poison_mod" => {
                let n = ((mod_strength - 1.0) / 0.5 + 1.0).round().max(1.0);
                self.poison_stacks += 5.0 + (n - 1.0) * 2.0;
                self.glow_green = true;
            }
            "fire_mod" => {
                let n = ((mod_strength - 1.0) / 0.5 + 1.0).round().max(1.0);
                self.fire_stacks += 10.0 + (n - 1.0) * 2.0;
                self.glow_red = true;
            }
            "vuln_mod" => {
                self.vuln_bonus += 0.05 * mod_strength;
                self.glow_halo = true;
            }
            _ => {}
        }
    }

    fn take_oneshot(&mut self) -> (f32, f32, bool, u32, f32) {
        let crit = self.next_crit_bonus;
        let cd = self.next_cd_mult;
        let gated = self.next_gated;
        let shot_mult = self.next_shot_mult.max(1);
        let dmg_scale = self.next_dmg_scale;
        self.next_crit_bonus = 0.0;
        self.next_cd_mult = 1.0;
        self.next_gated = false;
        self.next_shot_mult = 1;
        self.next_dmg_scale = 1.0;
        (crit, cd, gated, shot_mult, dmg_scale)
    }

    fn emit_from_payload(
        &self,
        def: &SpellDef,
        damage_mult: f32,
        crit_bonus: f32,
        gated: bool,
        loadout_has: &dyn Fn(&str) -> bool,
    ) -> Vec<PlannedShot> {
        let glyph = def.glyph.chars().next().unwrap_or('*');
        let color_name = format!("{:?}", def.color);
        let base_damage = def.damage * self.damage_mult * damage_mult + self.flat_damage;
        let base_speed = def.speed * self.speed_mult;
        let base_life = def.lifetime * self.lifetime_mult;
        let base_radius = def.radius.max(0.25) * self.radius_mult;
        let homing = self.homing.max(def.homing);

        let mut shots = match def.special.as_str() {
            "plague" => {
                let count = (def.count.max(8) + self.extra_count).max(1);
                (0..count)
                    .map(|i| PlannedShot {
                        damage: base_damage,
                        speed: base_speed.max(4.0),
                        lifetime: base_life.max(2.5),
                        radius: base_radius,
                        angle_offset: (i as f32) * (TAU / count as f32),
                        homing: homing.max(0.35),
                        poison: def.poison.max(2.0),
                        glyph,
                        color_name: color_name.clone(),
                        homing_ramp: true,
                        gated,
                        crit_bonus,
                        special: "plague".into(),
                        source_id: def.id.clone(),
                        ..PlannedShot::default()
                    })
                    .collect()
            }
            "arc" => {
                vec![PlannedShot {
                    damage: base_damage,
                    speed: base_speed,
                    lifetime: base_life,
                    radius: base_radius,
                    explosion_radius: def.explosion_radius,
                    bounces: self.bounces + def.bounces,
                    pierce: self.pierce + def.pierce,
                    homing,
                    poison: def.poison,
                    glyph,
                    color_name: color_name.clone(),
                    arc: true,
                    gated,
                    crit_bonus,
                    special: "arc".into(),
                    source_id: def.id.clone(),
                    ..PlannedShot::default()
                }]
            }
            "awp" => {
                vec![PlannedShot {
                    damage: base_damage * 3.0,
                    speed: base_speed.max(38.0),
                    lifetime: base_life.max(1.5),
                    radius: base_radius,
                    bounces: self.bounces + def.bounces,
                    pierce: (self.pierce + def.pierce).max(1),
                    homing,
                    poison: def.poison,
                    glyph,
                    color_name: color_name.clone(),
                    trail_fx: true,
                    trail_bright: 1.4,
                    gated,
                    crit_bonus: crit_bonus + 0.10,
                    special: "awp".into(),
                    source_id: def.id.clone(),
                    ..PlannedShot::default()
                }]
            }
            "taze" => {
                vec![PlannedShot {
                    damage: base_damage,
                    speed: base_speed.max(12.0),
                    lifetime: base_life.max(2.0),
                    radius: base_radius,
                    explosion_radius: def.explosion_radius.max(2.2),
                    pierce: self.pierce + def.pierce,
                    homing: homing.max(2.2),
                    poison: def.poison,
                    glyph,
                    color_name: color_name.clone(),
                    trail_fx: true,
                    gated,
                    crit_bonus,
                    special: "taze".into(),
                    source_id: def.id.clone(),
                    ..PlannedShot::default()
                }]
            }
            "flock" => {
                let count = (def.count.max(6) + self.extra_count).max(1);
                (0..count)
                    .map(|i| PlannedShot {
                        damage: base_damage,
                        speed: base_speed.max(16.0),
                        lifetime: base_life.max(6.0),
                        radius: base_radius,
                        angle_offset: (i as f32) * (TAU / count as f32),
                        pierce: self.pierce + def.pierce,
                        glyph,
                        color_name: color_name.clone(),
                        orbiting: true,
                        orbit_radius: def.orbit_radius.max(1.9),
                        gated,
                        crit_bonus,
                        special: "flock".into(),
                        source_id: def.id.clone(),
                        ..PlannedShot::default()
                    })
                    .collect()
            }
            "thorn_blade" => {
                let count = (def.count.max(5) + self.extra_count).max(1);
                let launch_at = def.orbit_duration.max(0.55);
                (0..count)
                    .map(|i| PlannedShot {
                        damage: base_damage,
                        speed: base_speed,
                        lifetime: base_life.max(2.2),
                        radius: base_radius,
                        angle_offset: (i as f32) * (TAU / count as f32),
                        pierce: self.pierce + def.pierce,
                        poison: def.poison.max(2.5),
                        glyph,
                        color_name: color_name.clone(),
                        orbiting: true,
                        orbit_radius: def.orbit_radius.max(1.6),
                        orbit_then_fire: true,
                        orbit_launch_at: launch_at,
                        gated,
                        crit_bonus,
                        special: "thorn_blade".into(),
                        source_id: def.id.clone(),
                        ..PlannedShot::default()
                    })
                    .collect()
            }
            "voyager" | "neptune" => {
                let count = def.count.max(1);
                vec![PlannedShot {
                    delay: 0.0,
                    damage: base_damage,
                    speed: base_speed,
                    lifetime: base_life,
                    radius: base_radius,
                    homing,
                    poison: def.poison,
                    glyph,
                    color_name: color_name.clone(),
                    gated,
                    crit_bonus,
                    special: def.special.clone(),
                    source_id: def.id.clone(),
                    chain: count,
                    ..PlannedShot::default()
                }]
            }
            "pitter" | "patter" => self.emit_splash_ring(
                def,
                base_damage,
                base_speed,
                base_life,
                base_radius,
                glyph,
                &color_name,
                gated,
                crit_bonus,
                loadout_has,
            ),
            _ => {
                let count = (def.count.max(1) + self.extra_count).max(1);
                let spread = self.spread + def.spread;
                let mut shots = Vec::new();
                for i in 0..count {
                    let angle_offset = if count == 1 {
                        0.0
                    } else {
                        let mid = (count - 1) as f32 / 2.0;
                        (i as f32 - mid) * spread.max(0.12)
                    };
                    shots.push(PlannedShot {
                        damage: base_damage,
                        speed: base_speed,
                        lifetime: base_life,
                        radius: base_radius,
                        explosion_radius: def.explosion_radius,
                        angle_offset,
                        bounces: self.bounces + def.bounces,
                        pierce: self.pierce + def.pierce,
                        homing,
                        poison: def.poison,
                        chain: def.chain,
                        glyph,
                        color_name: color_name.clone(),
                        gated,
                        crit_bonus,
                        special: def.special.clone(),
                        source_id: def.id.clone(),
                        ..PlannedShot::default()
                    });
                }
                shots
            }
        };

        for s in &mut shots {
            self.decorate_shot(s);
        }
        shots
    }

    fn decorate_shot(&self, shot: &mut PlannedShot) {
        shot.bounces = shot.bounces.max(self.bounces);
        if self.homing > shot.homing {
            shot.homing = self.homing;
        }
        shot.gold_bonus = self.gold_bonus;
        shot.xp_bonus = self.xp_bonus;
        shot.poison_stacks = self.poison_stacks;
        shot.fire_stacks = self.fire_stacks;
        shot.vuln_bonus = self.vuln_bonus;
        shot.glow_green = self.glow_green;
        shot.glow_red = self.glow_red;
        shot.glow_halo = self.glow_halo;
    }

    fn emit_splash_ring(
        &self,
        def: &SpellDef,
        base_damage: f32,
        base_speed: f32,
        base_life: f32,
        base_radius: f32,
        glyph: char,
        color_name: &str,
        gated: bool,
        crit_bonus: f32,
        loadout_has: &dyn Fn(&str) -> bool,
    ) -> Vec<PlannedShot> {
        let is_pitter = def.special == "pitter";
        let has_partner = if is_pitter {
            loadout_has("patter")
        } else {
            loadout_has("pitter")
        };
        let mut ring_r = def.orbit_radius.max(if is_pitter { 2.0 } else { 4.0 });
        let mut rings = 1u32;
        if is_pitter && has_partner {
            rings = 2;
            ring_r *= 1.10;
        }
        let count = (def.count.max(1) + self.extra_count).max(1);
        let mut shots = Vec::new();
        for ring_i in 0..rings {
            let r = ring_r * (1.0 + ring_i as f32 * 0.08);
            let phase = ring_i as f32 * 0.15;
            for i in 0..count {
                let angle = (i as f32) * (TAU / count as f32) + phase;
                shots.push(PlannedShot {
                    damage: base_damage,
                    speed: base_speed.max(1.0),
                    lifetime: base_life.max(0.22),
                    radius: base_radius,
                    explosion_radius: def.explosion_radius.max(0.7),
                    angle_offset: angle,
                    bounces: self.bounces + def.bounces,
                    pierce: self.pierce + def.pierce,
                    glyph,
                    color_name: color_name.to_string(),
                    gated,
                    crit_bonus,
                    special: def.special.clone(),
                    source_id: def.id.clone(),
                    ring_spawn: true,
                    spawn_radius: r,
                    ..PlannedShot::default()
                });
            }
            if !is_pitter && has_partner {
                for i in 0..count {
                    let a0 = (i as f32) * (TAU / count as f32) + phase;
                    let a1 = ((i + 1) as f32) * (TAU / count as f32) + phase;
                    let x0 = a0.cos() * r;
                    let y0 = a0.sin() * r;
                    let x1 = a1.cos() * r;
                    let y1 = a1.sin() * r;
                    const SEGMENTS: u32 = 3;
                    for s in 1..=SEGMENTS {
                        let t = s as f32 / (SEGMENTS as f32 + 1.0);
                        let ox = x0 + (x1 - x0) * t;
                        let oy = y0 + (y1 - y0) * t;
                        shots.push(PlannedShot {
                            damage: base_damage * 0.55,
                            speed: 0.5,
                            lifetime: base_life.max(0.2) * 0.85,
                            radius: base_radius * 0.75,
                            explosion_radius: def.explosion_radius.max(0.55) * 0.7,
                            glyph: '·',
                            color_name: color_name.to_string(),
                            gated,
                            crit_bonus,
                            special: "patter_link".into(),
                            source_id: def.id.clone(),
                            spawn_ox: ox,
                            spawn_oy: oy,
                            ..PlannedShot::default()
                        });
                    }
                }
            }
        }
        shots
    }

    fn payload_self_cd(def: &SpellDef) -> f32 {
        match def.special.as_str() {
            "arc" => 1.10,
            "thorn_blade" => 1.05,
            "awp" => 1.30,
            _ => 1.0,
        }
    }
}

/// Scatter / Proj_Dupe: replicate a payload's shots with optional damage scale + fan.
fn multiply_shots(base: Vec<PlannedShot>, mult: u32, dmg_scale: f32) -> Vec<PlannedShot> {
    let mult = mult.max(1);
    if mult == 1 && (dmg_scale - 1.0).abs() < 0.001 {
        return base;
    }
    let mut out = Vec::with_capacity(base.len() * mult as usize);
    let mid = (mult - 1) as f32 / 2.0;
    for i in 0..mult {
        let fan = (i as f32 - mid) * 0.14;
        for shot in &base {
            let mut s = shot.clone();
            s.damage *= dmg_scale;
            s.angle_offset += fan;
            out.push(s);
        }
    }
    out
}

/// Evaluate a single nucleus slot: attached mods (left→right), then the projectile.
/// Cooldown mods on this slot set `fire_interval_mult` for the wait until the next slot fires.
pub fn evaluate_slot(
    slot: &super::nucleus::NucleusSlot,
    lib: &SpellLibrary,
    mod_mark_strength: &dyn Fn(&str) -> f32,
    proj_damage_mult: &dyn Fn(&str) -> f32,
    loadout_has: &dyn Fn(&str) -> bool,
    rng: &mut impl Rng,
) -> NucleusPlan {
    let mut plan = NucleusPlan {
        fire_interval_mult: 1.0,
        ..NucleusPlan::default()
    };
    let mut state = NucleusEvalState::default();

    let mut sequence: Vec<String> = slot.mods.clone();
    if let Some(p) = &slot.projectile {
        sequence.push(p.clone());
    }

    let mut i = 0usize;
    while i < sequence.len() {
        let id = &sequence[i];
        let Some(def) = lib.get(id) else {
            i += 1;
            continue;
        };
        plan.mana_cost += def.mana_cost;

        if state.skip_next {
            state.skip_next = false;
            i += 1;
            continue;
        }

        let copy = state.copy_next;
        state.copy_next = false;
        let repeats = if copy { 2 } else { 1 };

        for _ in 0..repeats {
            match def.kind {
                SpellKind::Modifier => {
                    let strength = mod_mark_strength(&def.id);
                    state.apply_modifier(def, strength);
                }
                SpellKind::Chaos => {
                    if def.special == "copy_fail" {
                        if rng.random_bool(0.5) {
                            state.copy_next = true;
                        } else {
                            state.skip_next = true;
                        }
                    } else {
                        let strength = mod_mark_strength(&def.id);
                        state.apply_modifier(def, strength);
                    }
                }
                SpellKind::Payload => {
                    let dmg = proj_damage_mult(&def.id);
                    let (crit, cd, gated, shot_mult, dmg_scale) = state.take_oneshot();
                    plan.fire_interval_mult *= cd;
                    plan.fire_interval_mult *= NucleusEvalState::payload_self_cd(def);
                    let base = state.emit_from_payload(def, dmg, crit, gated, loadout_has);
                    plan.shots.extend(multiply_shots(base, shot_mult, dmg_scale));
                }
            }
        }
        i += 1;
    }
    plan.fire_interval_mult *= state.fire_interval_mult;
    plan.fire_interval_mult = plan.fire_interval_mult.clamp(0.25, 3.0);

    if plan.shots.is_empty() {
        plan.shots.push(PlannedShot::default());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proj_logic::{SpellLibrary, Nucleus};
    use crate::proj_logic::nucleus::NucleusSlot;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// Evaluate every filled projectile slot into one combined plan (tests).
    fn evaluate_nucleus(
        nucleus: &Nucleus,
        lib: &SpellLibrary,
        mod_mark_strength: &dyn Fn(&str) -> f32,
        proj_damage_mult: &dyn Fn(&str) -> f32,
        rng: &mut impl Rng,
    ) -> NucleusPlan {
        let mut plan = NucleusPlan {
            fire_interval_mult: 1.0,
            ..NucleusPlan::default()
        };
        for slot in &nucleus.slots {
            if slot.projectile.is_none() {
                continue;
            }
            let part = evaluate_slot(slot, lib, mod_mark_strength, proj_damage_mult, &|_| false, rng);
            plan.mana_cost += part.mana_cost;
            plan.shots.extend(part.shots);
            plan.fire_interval_mult *= part.fire_interval_mult;
        }
        plan.fire_interval_mult = plan.fire_interval_mult.clamp(0.25, 3.0);
        if plan.shots.is_empty() {
            plan.shots.push(PlannedShot::default());
        }
        plan
    }

    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(1)
    }

    fn with_proj(proj: &str, mods: &[&str]) -> Nucleus {
        let mut n = Nucleus::new(1, Some(proj.into()), mods.len().max(1));
        n.slots[0].mods = mods.iter().map(|m| (*m).into()).collect();
        n
    }

    #[test]
    fn p_cannon_has_small_blast() {
        let lib = SpellLibrary::load_embedded();
        let n = with_proj("p_cannon", &[]);
        let plan = evaluate_nucleus(&n, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(!plan.shots.is_empty());
        assert!(plan.shots[0].explosion_radius > 0.0);
    }

    #[test]
    fn amplify_grows_radius() {
        let lib = SpellLibrary::load_embedded();
        let base = with_proj("p_cannon", &[]);
        let amp = with_proj("p_cannon", &["amplify_proj"]);
        let a = evaluate_nucleus(&base, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        let b = evaluate_nucleus(&amp, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(b.shots[0].radius > a.shots[0].radius);
    }

    #[test]
    fn t_happy_speeds_fire_rate() {
        let lib = SpellLibrary::load_embedded();
        let n = with_proj("p_cannon", &["t_happy"]);
        let plan = evaluate_nucleus(&n, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(plan.fire_interval_mult < 1.0);
    }

    #[test]
    fn plague_sprays_many() {
        let lib = SpellLibrary::load_embedded();
        let n = with_proj("plague", &[]);
        let plan = evaluate_nucleus(&n, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(plan.shots.len() >= 8);
        assert!(plan.shots[0].homing_ramp);
    }

    #[test]
    fn stone_age_slows_fire() {
        let lib = SpellLibrary::load_embedded();
        let n = with_proj("stone_age", &[]);
        let plan = evaluate_nucleus(&n, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(plan.fire_interval_mult > 1.0);
        assert!(plan.shots[0].arc);
    }

    #[test]
    fn awp_is_heavy_sniper() {
        let lib = SpellLibrary::load_embedded();
        let cannon = with_proj("p_cannon", &[]);
        let awp = with_proj("awp", &[]);
        let a = evaluate_nucleus(&cannon, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        let b = evaluate_nucleus(&awp, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert!(b.shots[0].damage > a.shots[0].damage * 3.0);
        assert!((b.shots[0].crit_bonus - 0.10).abs() < 0.001);
        assert!(b.fire_interval_mult >= 1.30);
    }

    #[test]
    fn taze_and_flock_exist() {
        let lib = SpellLibrary::load_embedded();
        let taze = with_proj("taze", &[]);
        let flock = with_proj("flock", &[]);
        let t = evaluate_nucleus(&taze, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        let f = evaluate_nucleus(&flock, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert_eq!(t.shots[0].special, "taze");
        assert!(t.shots[0].explosion_radius >= 2.0);
        assert!(f.shots.len() >= 6);
        assert!(f.shots.iter().all(|s| s.orbiting && s.special == "flock"));
    }

    #[test]
    fn scatter_quads_with_damage_cut() {
        let lib = SpellLibrary::load_embedded();
        let base = with_proj("p_cannon", &[]);
        let scat = with_proj("p_cannon", &["scatter"]);
        let a = evaluate_nucleus(&base, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        let b = evaluate_nucleus(&scat, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert_eq!(b.shots.len(), a.shots.len() * 4);
        assert!((b.shots[0].damage - a.shots[0].damage * 0.4).abs() < 0.01);
    }

    #[test]
    fn proj_dupe_doubles() {
        let lib = SpellLibrary::load_embedded();
        let base = with_proj("p_cannon", &[]);
        let dupe = with_proj("p_cannon", &["proj_dupe"]);
        let a = evaluate_nucleus(&base, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        let b = evaluate_nucleus(&dupe, &lib, &|_| 1.0, &|_| 1.0, &mut rng());
        assert_eq!(b.shots.len(), a.shots.len() * 2);
    }

    #[test]
    fn evaluate_slot_ignores_other_slots() {
        let lib = SpellLibrary::load_embedded();
        let mut n = Nucleus::new(2, Some("p_cannon".into()), 1);
        n.slots[1] = NucleusSlot {
            projectile: Some("awp".into()),
            mods: Vec::new(),
        };
        let plan = evaluate_slot(&n.slots[0], &lib, &|_| 1.0, &|_| 1.0, &|_| false, &mut rng());
        assert!(plan.shots[0].damage < 40.0);
    }
}
