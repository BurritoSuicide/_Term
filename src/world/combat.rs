use std::collections::VecDeque;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use ratatui::style::Color;

use crate::procgen::{rooms, wave_enemies};
use crate::procgen::waves::{make_actor, EnemySpawn};
use crate::necro::{formation_local, world_offset};
use crate::skills::{SkillId, SkillLoadout};
use crate::proj_logic::{
    NucleusPlan, MarkBook, Nucleus, SpellKind, SpellLibrary, evaluate_slot, mark_label,
    mod_strength,
};

use super::boss::{self, BossMod};
use super::entity::{
    Actor, ActorKind, BossForm, Corpse, Daisy, DamageNumber, EntityId, ExplosionFx, GasCloud,
    GasKind, JournalEntry, OrbitBlade, Particle, PendingShot, Pickup, PickupKind, Projectile,
    ShopOffer, ShopTotem, StampedeBeast, TempBoost, Vec2,
};
use super::level_mod::{self, LevelMod};
use super::projectiles::{pending_from_shot, spawn_projectile};
use super::room::{RoomKind, RoomState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Playing,
    Shop,
    Dead,
}

/// Which half of the stash the picker currently shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashFilter {
    Projectiles,
    Mods,
}

/// Inventory overlay: projectile pick, mod-attach menu, or mod pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvOverlay {
    None,
    /// Pick a projectile for this nucleus slot.
    PickProjectile { slot: usize },
    /// Attach/replace mods on a filled projectile slot.
    ModMenu { slot: usize },
    /// Pick a mod for attachment index on a slot.
    PickMod { slot: usize, mod_idx: usize },
}

pub struct World {
    pub seed: u64,
    pub seed_label: String,
    pub rng: ChaCha8Rng,
    pub lib: SpellLibrary,
    pub nucleus: Nucleus,
    pub marks: MarkBook,
    pub stash: Vec<String>,
    pub stash_filter: StashFilter,
    pub skills: SkillLoadout,
    pub room: RoomState,
    pub phase: GamePhase,
    pub player: Actor,
    pub actors: Vec<Actor>,
    pub corpses: Vec<Corpse>,
    pub projectiles: Vec<Projectile>,
    pub pickups: Vec<Pickup>,
    pub pending: Vec<PendingShot>,
    pub orbit_blades: Vec<OrbitBlade>,
    pub explosions: Vec<ExplosionFx>,
    pub particles: Vec<Particle>,
    pub autofire_cd: f32,
    /// Temporary combat buffs from floor powerups.
    pub overclock_t: f32,
    pub fleet_dash: bool,
    pub twin_volley_t: f32,
    pub guard_charges: u32,
    /// Homing aura from temp pickup — lasts until room clear.
    pub room_homing: bool,
    pub temp_pickup_cd: f32,
    pub dash_cd: f32,
    pub dash_cd_max: f32,
    pub dash_time: f32,
    pub dash_dir: Vec2,
    /// One-shot ready flash timer (seconds remaining).
    pub dash_ready_pulse_t: f32,
    /// Edge-detect for firing the ready pulse once per charge.
    pub dash_was_ready: bool,
    pub invuln: f32,
    pub anim_t: f32,
    pub player_flash: f32,
    /// Flock debuff: take +20% damage from enemy projectiles while > 0.
    pub proj_vuln_t: f32,
    /// Screen shake intensity (decays each frame).
    pub shake: f32,
    pub boss_mods: Vec<BossMod>,
    pub wall_volley_cd: f32,
    pub tremor_cd: f32,
    pub credits: i32,
    pub gold: i32,
    pub next_id: EntityId,
    pub message: String,
    pub message_timer: f32,
    pub inv_cursor: usize,
    pub stash_cursor: usize,
    /// Main inventory: 0 = nucleus, 1 = skills. Stash is a modal overlay.
    pub inv_focus: u8,
    /// Nucleus inventory overlay (projectile / mod menus).
    pub inv_overlay: InvOverlay,
    /// Cursor inside the mod-attach menu (mod rows + change proj + done).
    pub mod_menu_cursor: usize,
    /// Which projectile slot autofire will fire next.
    pub autofire_slot: usize,
    pub shop_totems: Vec<ShopTotem>,
    pub shop_reroll_pos: Vec2,
    pub shop_reroll_cost: i32,
    /// Shop training dummy — T to toggle; enables autofire + inventory testing.
    pub shop_dummy_active: bool,
    pub shop_dummy_id: Option<EntityId>,
    pub rooms_cleared: u32,
    pub boss_kills: u32,
    pub daisies: Vec<Daisy>,
    /// Rolling event log (journalctl).
    pub journal: VecDeque<JournalEntry>,
    /// Recent damage events (anim_t, amount) for DPS window.
    pub damage_events: VecDeque<(f32, f32)>,
    /// Sparkline history of DPS samples (kept for reference; wave uses smoothed amp).
    pub dps_history: VecDeque<f32>,
    /// Smoothed wave amplitude 0..1 (from DPS ÷ avg enemy HP, full at 10×).
    pub dps_wave_amp: f32,
    /// Smoothed color progress: 0 = blue, 1 = red @ 10×, >1 toward purple.
    pub dps_color_t: f32,
    /// Floating damage readouts.
    pub damage_numbers: Vec<DamageNumber>,
    /// Combat-room hazard modifiers.
    pub level_mods: Vec<LevelMod>,
    pub gas_clouds: Vec<GasCloud>,
    pub stampede: Vec<StampedeBeast>,
    pub gas_leak_cd: f32,
    pub bullet_hell_cd: f32,
    pub stampede_cd: f32,
    /// Staggered wave spawn queue (combat rooms).
    pub pending_spawns: VecDeque<EnemySpawn>,
    pub spawn_next_in: f32,
    pub wave_spawn_total: u32,
    pub wave_spawned: u32,
}

impl World {
    pub fn new(seed: u64, seed_label: String) -> Self {
        let mut rng = crate::procgen::seed::rng_from_seed(seed);
        let lib = SpellLibrary::load_embedded();
        let nucleus = Nucleus::starter(&lib);
        let room = rooms::next_combat_room(1, &mut rng);
        let spawn = room.snap_to_footing(Vec2::new(4.0, room.height * 0.5), 0.55);
        let player = Actor {
            id: 1,
            kind: ActorKind::Player,
            pos: spawn,
            vel: Vec2::ZERO,
            facing: Vec2::new(1.0, 0.0),
            hp: 100.0,
            max_hp: 100.0,
            radius: 0.55,
            glyph: '@',
            color: Color::LightCyan,
            formation_local: Vec2::ZERO,
            formation_index: 0,
            poison_timer: 0.0,
            poison_stacks: 0.0,
            fire_stacks: 0.0,
            vuln_bonus: 0.0,
            kill_gold_bonus: 0.0,
            kill_xp_bonus: 0.0,
            kill_source: String::new(),
            can_shoot: false,
            shoot_cd: 0.0,
            splits_left: 0,
            boss_gen: 0,
            boss_form: BossForm::None,
            trail: Vec::new(),
            reanim_tier: 0,
            is_dummy: false,
        };
        let mut world = Self {
            seed,
            seed_label,
            rng,
            lib,
            nucleus,
            marks: MarkBook::default(),
            stash: vec!["amplify_proj".into(), "fast_fast".into(), "t_happy".into()],
            stash_filter: StashFilter::Projectiles,
            skills: SkillLoadout::new_empty(),
            room,
            phase: GamePhase::Playing,
            player,
            actors: Vec::new(),
            corpses: Vec::new(),
            projectiles: Vec::new(),
            pickups: Vec::new(),
            pending: Vec::new(),
            orbit_blades: Vec::new(),
            explosions: Vec::new(),
            particles: Vec::new(),
            autofire_cd: 1.0,
            overclock_t: 0.0,
            fleet_dash: false,
            twin_volley_t: 0.0,
            guard_charges: 0,
            room_homing: false,
            temp_pickup_cd: 4.0,
            dash_cd: 0.0,
            dash_cd_max: 1.15,
            dash_time: 0.0,
            dash_dir: Vec2::new(1.0, 0.0),
            dash_ready_pulse_t: 0.0,
            dash_was_ready: true, // start ready without a pulse
            invuln: 0.0,
            anim_t: 0.0,
            player_flash: 0.0,
            proj_vuln_t: 0.0,
            shake: 0.0,
            boss_mods: Vec::new(),
            wall_volley_cd: 0.8,
            tremor_cd: 1.5,
            credits: 2,
            gold: 15,
            next_id: 10,
            message: "Nucleus autofires · walk corpses to re-animate".into(),
            message_timer: 4.0,
            inv_cursor: 0,
            stash_cursor: 0,
            inv_focus: 0,
            inv_overlay: InvOverlay::None,
            mod_menu_cursor: 0,
            autofire_slot: 0,
            shop_totems: Vec::new(),
            shop_reroll_pos: Vec2::ZERO,
            shop_reroll_cost: 15,
            shop_dummy_active: false,
            shop_dummy_id: None,
            rooms_cleared: 0,
            boss_kills: 0,
            daisies: Vec::new(),
            journal: VecDeque::new(),
            damage_events: VecDeque::new(),
            dps_history: VecDeque::new(),
            dps_wave_amp: 0.0,
            dps_color_t: 0.0,
            damage_numbers: Vec::new(),
            level_mods: Vec::new(),
            gas_clouds: Vec::new(),
            stampede: Vec::new(),
            gas_leak_cd: 1.5,
            bullet_hell_cd: 2.0,
            stampede_cd: 3.0,
            pending_spawns: VecDeque::new(),
            spawn_next_in: 0.0,
            wave_spawn_total: 0,
            wave_spawned: 0,
        };
        // Spawn the opening wave immediately so the first room is never empty.
        world.room.spawn_timer = 0.0;
        world.spawn_wave_if_needed();
        world
    }

    fn alloc_id(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.message_timer = 2.8;
    }

    /// Append a line to the rolling journalctl window.
    pub fn journal(&mut self, msg: impl Into<String>) {
        self.journal.push_back(JournalEntry {
            t: self.anim_t,
            text: msg.into(),
            pickup_rarity: None,
        });
        while self.journal.len() > 64 {
            self.journal.pop_front();
        }
    }

    pub fn journal_pickup(&mut self, name: &str, rarity: crate::proj_logic::Rarity) {
        self.journal.push_back(JournalEntry {
            t: self.anim_t,
            text: format!("{name} [{}]", rarity.label()),
            pickup_rarity: Some(rarity),
        });
        while self.journal.len() > 64 {
            self.journal.pop_front();
        }
    }

    pub fn record_damage(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        self.damage_events.push_back((self.anim_t, amount));
    }

    pub fn current_dps(&self) -> f32 {
        let window = 1.0;
        self.damage_events
            .iter()
            .filter(|(t, _)| self.anim_t - *t <= window)
            .map(|(_, d)| *d)
            .sum()
    }

    /// DPS ÷ average living-enemy HP (or last known room baseline). Used for overkill FX.
    pub fn dps_overkill_ratio(&self) -> f32 {
        let avg = self.avg_enemy_hp().max(1.0);
        self.current_dps() / avg
    }

    pub fn avg_enemy_hp(&self) -> f32 {
        let living: Vec<f32> = self
            .actors
            .iter()
            .filter(|a| {
                matches!(a.kind, ActorKind::Enemy | ActorKind::Boss)
                    && (!a.is_dummy || self.shop_dummy_active)
            })
            .map(|a| a.max_hp)
            .collect();
        if living.is_empty() {
            12.0 + self.room.combat_index as f32 * 1.5
        } else {
            living.iter().sum::<f32>() / living.len() as f32
        }
    }

    fn sample_dps_history(&mut self) {
        let dps = self.current_dps();
        self.dps_history.push_back(dps);
        while self.dps_history.len() > 48 {
            self.dps_history.pop_front();
        }
        while self
            .damage_events
            .front()
            .is_some_and(|(t, _)| self.anim_t - *t > 2.5)
        {
            self.damage_events.pop_front();
        }

        // Smooth amplitude toward overkill / 10× (1.0 = full wave height).
        let overkill = self.dps_overkill_ratio();
        let target_amp = (overkill / 10.0).clamp(0.0, 1.15);
        self.dps_wave_amp += (target_amp - self.dps_wave_amp) * 0.14;

        // Color track: stay blue until 2×, then 2→10 maps 0→1, beyond 10 climbs past 1.
        let target_color = if overkill <= 2.0 {
            0.0
        } else {
            ((overkill - 2.0) / 8.0).clamp(0.0, 1.0) + ((overkill - 10.0).max(0.0) / 8.0).min(0.85)
        };
        self.dps_color_t += (target_color - self.dps_color_t) * 0.12;
    }

    pub fn minion_count(&self) -> usize {
        self.actors
            .iter()
            .filter(|a| a.kind == ActorKind::Minion)
            .count()
    }

    pub fn enemy_count(&self) -> usize {
        self.actors
            .iter()
            .filter(|a| {
                matches!(a.kind, ActorKind::Enemy | ActorKind::Boss) && !a.is_dummy
            })
            .count()
    }

    /// Total copies of a spell id owned across equipped nucleus slots and the stash.
    pub fn count_owned(&self, id: &str) -> usize {
        self.nucleus
            .filled_ids()
            .iter()
            .filter(|s| s.as_str() == id)
            .count()
            + self.stash.iter().filter(|s| s.as_str() == id).count()
    }

    /// Damage multiplier granted by the army's highest re-animation tier.
    pub fn reanim_damage_mult(&self) -> f32 {
        1.0 + 0.10 * self.max_reanim_tier() as f32
    }

    /// Highest re-animation tier among living minions (0 if none).
    pub fn max_reanim_tier(&self) -> u8 {
        self.actors
            .iter()
            .filter(|a| a.kind == ActorKind::Minion)
            .map(|a| a.reanim_tier)
            .max()
            .unwrap_or(0)
    }

    /// Index of the nearest minion within shield range of the player, if any.
    fn shield_minion_idx(&self) -> Option<usize> {
        let radius = 2.5;
        self.actors
            .iter()
            .enumerate()
            .filter(|(_, a)| a.kind == ActorKind::Minion && a.pos.dist(self.player.pos) < radius)
            .min_by(|(_, a), (_, b)| {
                a.pos
                    .dist(self.player.pos)
                    .partial_cmp(&b.pos.dist(self.player.pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    pub fn move_player(&mut self, dir: Vec2, dt: f32) {
        if self.phase != GamePhase::Playing && self.phase != GamePhase::Shop {
            return;
        }
        // While dashing, movement is handled by update_dash.
        if self.dash_time > 0.0 {
            return;
        }
        if dir.length() > 0.0 {
            let mut speed = 10.5;
            if self.skills.has_active(SkillId::FleetBoots) {
                speed *= 1.15;
            }
            self.player.facing = dir.normalized();
            let next = self.player.pos + dir.normalized() * speed * dt;
            self.player.pos =
                self.room
                    .clamp_move(self.player.pos, next, self.player.radius, false);
        }
    }

    pub fn try_dash(&mut self, prefer_dir: Vec2) {
        if self.phase != GamePhase::Playing || self.dash_cd > 0.0 || self.dash_time > 0.0 {
            return;
        }
        let dir = if prefer_dir.length() > 0.05 {
            prefer_dir.normalized()
        } else if self.player.facing.length() > 0.05 {
            self.player.facing.normalized()
        } else {
            Vec2::new(1.0, 0.0)
        };
        self.dash_dir = dir;
        self.dash_time = 0.14;
        let mut cd = 1.15;
        if self.skills.has_active(SkillId::MomentumDash) {
            cd *= 0.6;
        }
        if self.fleet_dash {
            cd *= 0.2; // 80% less cooldown
        }
        self.dash_cd = cd;
        self.dash_cd_max = cd;
        if self.skills.has_active(SkillId::PhaseStep) {
            self.invuln = self.invuln.max(0.22);
        }
        self.spawn_dash_particles(dir);
    }

    fn spawn_dash_particles(&mut self, dir: Vec2) {
        let back = dir * -1.0;
        for i in 0..10 {
            let spread = (i as f32 - 4.5) * 0.12;
            let side = Vec2::new(-dir.y, dir.x) * spread;
            self.particles.push(Particle {
                pos: self.player.pos,
                vel: back * (8.0 + i as f32 * 0.7) + side * 6.0,
                life: 0.28 + (i % 3) as f32 * 0.05,
                max_life: 0.35,
                glyph: if i % 2 == 0 { '·' } else { '*' },
                color: if self.skills.has_active(SkillId::PhaseStep) {
                    Color::LightBlue
                } else {
                    Color::Cyan
                },
            });
        }
    }

    pub fn aim_player(&mut self, dir: Vec2) {
        if dir.length() > 0.0 {
            self.player.facing = dir.normalized();
        }
    }

    /// Apply a Nucleus fire plan for the player (no minion echo — Nucleus autofires alone).
    fn apply_nucleus_plan(&mut self, plan: &NucleusPlan) {
        self.player_flash = self.player_flash.max(0.22);
        let origin = self.player.pos;
        let facing = self.player.facing;
        for shot in &plan.shots {
            match shot.special.as_str() {
                "voyager" => self.fire_voyager(shot),
                "neptune" => self.fire_neptune(shot),
                _ => {
                    let mut s = shot.clone();
                    if self.room_homing {
                        s.homing = s.homing.max(1.6);
                    }
                    self.enqueue_shot(s, origin, facing, true, 0.0);
                }
            }
        }
    }

    fn enqueue_shot(
        &mut self,
        shot: crate::proj_logic::PlannedShot,
        origin: Vec2,
        facing: Vec2,
        owner_is_player_side: bool,
        extra_delay: f32,
    ) {
        let twin = self.twin_volley_t > 0.0;
        let mut pending = pending_from_shot(shot.clone(), origin, facing, owner_is_player_side);
        pending.delay += extra_delay;
        self.pending.push(pending);
        if twin {
            let side = Vec2::new(-facing.y, facing.x) * 0.4;
            let mut twin_pending =
                pending_from_shot(shot, origin + side, facing, owner_is_player_side);
            twin_pending.delay += extra_delay;
            self.pending.push(twin_pending);
        }
    }

    fn fire_voyager(&mut self, shot: &crate::proj_logic::PlannedShot) {
        let n = shot.chain.max(5) as usize;
        let enemies: Vec<Vec2> = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .map(|a| a.pos)
            .collect();
        let targets: Vec<Vec2> = if enemies.is_empty() {
            (0..n.min(5))
                .map(|i| {
                    self.player.pos
                        + self.player.facing * (4.0 + i as f32)
                        + Vec2::new(-self.player.facing.y, self.player.facing.x)
                            * ((i as f32 - 2.0) * 0.8)
                })
                .collect()
        } else {
            enemies.into_iter().take(n).collect()
        };
        for (i, target) in targets.into_iter().enumerate() {
            let above = Vec2::new(target.x, target.y - 2.4);
            let mut laser = shot.clone();
            laser.special.clear();
            laser.chain = 0;
            laser.delay = 0.12 + 0.07 * i as f32;
            laser.speed = shot.speed.max(28.0);
            laser.lifetime = 0.7;
            laser.homing = if self.room_homing { 1.2 } else { 0.15 };
            laser.pierce = 1;
            laser.glyph = '|';
            laser.color_name = "LightYellow".into();
            let facing = Vec2::new(0.0, 1.0); // downward
            self.enqueue_shot(laser, above, facing, true, 0.0);
            // Satellite spark particle
            self.particles.push(Particle {
                pos: above,
                vel: Vec2::ZERO,
                life: 0.35,
                max_life: 0.35,
                glyph: 'v',
                color: Color::LightYellow,
            });
        }
    }

    fn fire_neptune(&mut self, shot: &crate::proj_logic::PlannedShot) {
        let n = shot.chain.max(10) as usize;
        let target = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .min_by(|a, b| {
                a.pos
                    .dist(self.player.pos)
                    .partial_cmp(&b.pos.dist(self.player.pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.pos)
            .unwrap_or(self.player.pos + self.player.facing * 10.0);

        let w = self.room.width;
        let h = self.room.height;
        for i in 0..n {
            let (origin, facing) = if i < n / 2 {
                // Bottom edge
                let x = 2.0 + (w - 4.0) * (i as f32 + 0.5) / (n / 2).max(1) as f32;
                let origin = Vec2::new(x, h - 1.5);
                let facing = (target - origin).normalized();
                (origin, facing)
            } else {
                // Side edges
                let j = i - n / 2;
                let left = j % 2 == 0;
                let y = 2.0 + (h - 4.0) * ((j / 2) as f32 + 0.5) / ((n - n / 2).max(1) as f32 / 2.0).max(1.0);
                let origin = if left {
                    Vec2::new(1.5, y)
                } else {
                    Vec2::new(w - 1.5, y)
                };
                let facing = (target - origin).normalized();
                (origin, facing)
            };
            let mut bolt = shot.clone();
            bolt.special.clear();
            bolt.chain = 0;
            bolt.delay = 0.05 * i as f32;
            bolt.homing = shot.homing.max(2.8);
            if self.room_homing {
                bolt.homing = bolt.homing.max(3.2);
            }
            bolt.speed = shot.speed.max(15.0);
            bolt.glyph = '*';
            bolt.color_name = "LightBlue".into();
            bolt.trail_fx = true;
            self.enqueue_shot(bolt, origin, facing, true, 0.0);
        }
    }

    /// Absorb the next damaging hit with a Guard charge, if available.
    fn try_absorb_hit(&mut self) -> bool {
        if self.guard_charges == 0 {
            return false;
        }
        self.guard_charges -= 1;
        self.journal("Guard absorbed hit");
        self.toast("Guard blocked the hit");
        self.player_flash = self.player_flash.max(0.35);
        true
    }

    fn clear_room_buffs(&mut self) {
        self.fleet_dash = false;
        self.overclock_t = 0.0;
        self.twin_volley_t = 0.0;
        self.room_homing = false;
        self.proj_vuln_t = 0.0;
        // Stacking DoTs / vuln clear with the room.
        self.player.poison_stacks = 0.0;
        self.player.fire_stacks = 0.0;
        self.player.vuln_bonus = 0.0;
        for a in &mut self.actors {
            a.poison_stacks = 0.0;
            a.fire_stacks = 0.0;
            a.vuln_bonus = 0.0;
            a.kill_gold_bonus = 0.0;
            a.kill_xp_bonus = 0.0;
            a.kill_source.clear();
        }
        // Guard charges persist across rooms unless spent.
    }

    fn player_proj_vuln_mult(&self) -> f32 {
        if self.proj_vuln_t > 0.0 {
            1.2
        } else {
            1.0
        }
    }

    /// Flock birds eat enemy shots; each deflect launches that bird and applies vuln.
    fn resolve_flock_deflects(&mut self) {
        let flock_idx: Vec<usize> = self
            .projectiles
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_flock && p.orbiting && p.lifetime > 0.0)
            .map(|(i, _)| i)
            .collect();
        if flock_idx.is_empty() {
            return;
        }
        let enemy_shots: Vec<(usize, Vec2)> = self
            .projectiles
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.owner_is_player_side && p.lifetime > 0.0)
            .map(|(i, p)| (i, p.pos))
            .collect();
        if enemy_shots.is_empty() {
            return;
        }

        let mut kill_shots = Vec::new();
        let mut launch_birds = Vec::new();
        for &(ei, epos) in &enemy_shots {
            for &fi in &flock_idx {
                if kill_shots.contains(&ei) || launch_birds.contains(&fi) {
                    continue;
                }
                if self.projectiles[fi].pos.dist(epos) < 0.75 {
                    kill_shots.push(ei);
                    launch_birds.push(fi);
                }
            }
        }
        if launch_birds.is_empty() {
            return;
        }

        let targets: Vec<Vec2> = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .map(|a| a.pos)
            .collect();

        for fi in launch_birds {
            let bird = &mut self.projectiles[fi];
            bird.orbiting = false;
            let aim = targets
                .iter()
                .min_by(|a, b| {
                    bird.pos
                        .dist(**a)
                        .partial_cmp(&bird.pos.dist(**b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
                .unwrap_or(bird.pos + Vec2::new(1.0, 0.0));
            let dir = (aim - bird.pos).normalized();
            bird.vel = dir * 18.0;
            bird.homing = bird.homing.max(1.4);
            bird.lifetime = bird.lifetime.max(1.6);
            bird.glyph = '»';
        }
        for ei in kill_shots.into_iter().rev() {
            if ei < self.projectiles.len() {
                self.projectiles[ei].lifetime = 0.0;
            }
        }
        self.proj_vuln_t = self.proj_vuln_t.max(2.0);
        self.toast("Flock broke · +20% vuln (2s)");
        self.journal("Flock deflect");
    }

    fn tick_temp_buffs(&mut self, dt: f32) {
        self.overclock_t = (self.overclock_t - dt).max(0.0);
        self.twin_volley_t = (self.twin_volley_t - dt).max(0.0);
    }

    fn update_temp_pickups(&mut self, dt: f32) {
        if self.room.cleared || self.room.kind == RoomKind::Shop || self.phase != GamePhase::Playing
        {
            return;
        }
        self.temp_pickup_cd -= dt;
        if self.temp_pickup_cd > 0.0 {
            return;
        }
        self.temp_pickup_cd = 8.0 + self.rng.random_range(0.0..4.0); // ~10s average
        if self.skills.has_active(SkillId::StarterChest) {
            self.temp_pickup_cd *= 0.5;
        }
        let kinds = TempBoost::all();
        let boost = kinds[self.rng.random_range(0..kinds.len())];
        let pos = self.room.sample_footing(&mut self.rng, 0.4, false);
        self.spawn_temp_pickup_at(pos, boost);
    }

    fn apply_temp_boost(&mut self, boost: TempBoost) {
        match boost {
            TempBoost::Overclock => {
                self.overclock_t = self.overclock_t.max(5.0);
                self.toast(format!("{} · 2× fire rate (5s)", boost.label()));
                self.journal(boost.label());
            }
            TempBoost::FleetDash => {
                self.fleet_dash = true;
                self.toast(format!("{} · until room clear", boost.label()));
                self.journal(boost.label());
            }
            TempBoost::TwinVolley => {
                self.twin_volley_t = self.twin_volley_t.max(20.0);
                self.toast(format!("{} · double shots (20s)", boost.label()));
                self.journal(boost.label());
            }
            TempBoost::Guard => {
                self.guard_charges = self.guard_charges.saturating_add(1);
                self.toast(format!("{} · next hit blocked", boost.label()));
                self.journal(boost.label());
            }
            TempBoost::Homing => {
                self.room_homing = true;
                self.toast(format!("{} · until room clear", boost.label()));
                self.journal(boost.label());
            }
        }
    }

    fn spawn_temp_pickup_at(&mut self, pos: Vec2, boost: TempBoost) {
        let id = self.alloc_id();
        self.pickups.push(Pickup {
            id,
            pos,
            kind: PickupKind::Temp(boost),
            pulse: self.rng.random_range(0.0..std::f32::consts::TAU),
        });
    }

    fn spawn_starter_chest_pickup(&mut self) {
        if !self.skills.has_active(SkillId::StarterChest) {
            return;
        }
        let kinds = TempBoost::all();
        let boost = kinds[self.rng.random_range(0..kinds.len())];
        let pos = self.room.snap_to_footing(
            self.player.pos + self.player.facing * 2.2,
            0.4,
        );
        self.spawn_temp_pickup_at(pos, boost);
    }

    /// Manual raise: nearest corpse within range, spends a credit.
    pub fn try_resurrect(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        if self.credits <= 0 {
            self.toast("No re-animation credits");
            return;
        }
        let Some((idx, _)) = self
            .corpses
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                self.player
                    .pos
                    .dist(a.pos)
                    .partial_cmp(&self.player.pos.dist(b.pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            self.toast("No corpses nearby");
            return;
        };
        if self.player.pos.dist(self.corpses[idx].pos) > 3.5 {
            self.toast("Get closer to a corpse");
            return;
        }
        self.raise_corpse(idx);
    }

    /// Auto-raise: walking over a corpse re-animates it for free (still spends a credit).
    fn update_reanimation(&mut self) {
        if self.phase != GamePhase::Playing || self.credits <= 0 || self.corpses.is_empty() {
            return;
        }
        let Some(idx) = self
            .corpses
            .iter()
            .position(|c| self.player.pos.dist(c.pos) < 1.2)
        else {
            return;
        };
        self.raise_corpse(idx);
    }

    /// Shared re-animation logic used by both manual (R) and walk-over raises.
    fn raise_corpse(&mut self, idx: usize) {
        let corpse = self.corpses.remove(idx);
        self.credits -= 1;
        let index = self.minion_count();
        let local = formation_local(index);
        let spawn_pos = self.room.snap_to_footing(
            self.player.pos + world_offset(local, self.player.facing),
            0.5,
        );
        let mut hp = corpse.max_hp * 0.7;
        if self.skills.has_active(SkillId::GraveBond) {
            hp *= 1.25;
        }
        let minion = Actor {
            id: self.alloc_id(),
            kind: ActorKind::Minion,
            pos: spawn_pos,
            vel: Vec2::ZERO,
            facing: self.player.facing,
            hp,
            max_hp: hp,
            radius: 0.5,
            glyph: 'u',
            color: Color::LightGreen,
            formation_local: local,
            formation_index: index,
            poison_timer: 0.0,
            poison_stacks: 0.0,
            fire_stacks: 0.0,
            vuln_bonus: 0.0,
            kill_gold_bonus: 0.0,
            kill_xp_bonus: 0.0,
            kill_source: String::new(),
            can_shoot: false,
            shoot_cd: 0.0,
            splits_left: 0,
            boss_gen: 0,
            boss_form: BossForm::None,
            trail: Vec::new(),
            reanim_tier: 0,
            is_dummy: false,
        };
        self.actors.push(minion);
        self.reindex_minions();
        self.journal("Re-animation");
        self.toast(format!("Re-animation · credits {}", self.credits));
        self.merge_reanimations();
    }

    /// Merge 10 same-tier minions into 2 minions one tier higher, cascading upward.
    fn merge_reanimations(&mut self) {
        loop {
            let mut by_tier: std::collections::HashMap<u8, Vec<usize>> =
                std::collections::HashMap::new();
            for (i, a) in self.actors.iter().enumerate() {
                if a.kind == ActorKind::Minion {
                    by_tier.entry(a.reanim_tier).or_default().push(i);
                }
            }
            let Some((&tier, idxs)) = by_tier.iter().find(|(_, v)| v.len() >= 10) else {
                break;
            };
            let take: Vec<usize> = idxs.iter().take(10).copied().collect();
            let avg_hp: f32 =
                take.iter().map(|&i| self.actors[i].hp).sum::<f32>() / take.len() as f32;
            let mut remove_idx = take.clone();
            remove_idx.sort_unstable_by(|a, b| b.cmp(a));
            for i in remove_idx {
                self.actors.remove(i);
            }
            let new_tier = tier + 1;
            let new_hp = (avg_hp * 1.2).max(1.0);
            let facing = self.player.facing;
            for _ in 0..2 {
                let index = self.minion_count();
                let local = formation_local(index);
                let spawn_pos = self.room.snap_to_footing(
                    self.player.pos + world_offset(local, facing),
                    0.5,
                );
                let id = self.alloc_id();
                self.actors.push(Actor {
                    id,
                    kind: ActorKind::Minion,
                    pos: spawn_pos,
                    vel: Vec2::ZERO,
                    facing,
                    hp: new_hp,
                    max_hp: new_hp,
                    radius: 0.5 + new_tier as f32 * 0.05,
                    glyph: 'U',
                    color: Color::LightGreen,
                    formation_local: local,
                    formation_index: index,
                    poison_timer: 0.0,
                    poison_stacks: 0.0,
                    fire_stacks: 0.0,
                    vuln_bonus: 0.0,
                    kill_gold_bonus: 0.0,
                    kill_xp_bonus: 0.0,
                    kill_source: String::new(),
                    can_shoot: false,
                    shoot_cd: 0.0,
                    splits_left: 0,
                    boss_gen: 0,
                    boss_form: BossForm::None,
                    trail: Vec::new(),
                    reanim_tier: new_tier,
                    is_dummy: false,
                });
            }
            self.reindex_minions();
            self.journal(format!("reanim merge → tier {new_tier}"));
        }
    }

    fn reindex_minions(&mut self) {
        let facing = self.player.facing;
        let mut i = 0usize;
        for actor in &mut self.actors {
            if actor.kind != ActorKind::Minion {
                continue;
            }
            actor.formation_index = i;
            actor.formation_local = formation_local(i);
            i += 1;
        }
        let _ = facing;
    }

    pub fn try_advance_door(&mut self) -> bool {
        if self.phase == GamePhase::Shop {
            return false;
        }
        let near_exit = self.player.pos.dist(self.room.exit_door()) <= 2.8;
        if !self.room.doors_open {
            if near_exit {
                self.toast("Exit sealed — clear all waves first");
            }
            return false;
        }
        if !near_exit {
            self.toast("Move to the right-wall door (▶), then Enter");
            return false;
        }
        let was_boss = self.room.kind == RoomKind::Boss;
        self.rooms_cleared += 1;
        if was_boss {
            self.boss_kills += 1;
            self.enter_shop();
            return true;
        }
        self.enter_next_combat();
        true
    }

    fn enter_next_combat(&mut self) {
        let next = self.room.combat_index + 1;
        self.room = rooms::next_combat_room(next, &mut self.rng);
        self.phase = GamePhase::Playing;
        self.boss_mods.clear();
        self.clear_level_hazards();
        self.level_mods = level_mod::roll_level_mods(
            self.room.combat_index,
            self.room.kind == RoomKind::Boss,
            &mut self.rng,
        );
        self.shake = 0.0;
        self.player.pos = self.room.snap_to_footing(
            Vec2::new(4.0, self.room.height * 0.5),
            self.player.radius,
        );
        self.projectiles.clear();
        self.pending.clear();
        self.pickups.clear();
        self.daisies.clear();
        self.damage_numbers.clear();
        self.pending_spawns.clear();
        self.spawn_next_in = 0.0;
        self.wave_spawn_total = 0;
        self.wave_spawned = 0;
        // Keep minions; clear living enemies
        self.actors
            .retain(|a| a.kind == ActorKind::Minion);
        for a in &mut self.actors {
            a.pos = self.room.snap_to_footing(a.pos, a.radius);
        }
        self.reindex_minions();
        self.corpses.clear();
        self.room.spawn_timer = 0.0;
        self.clear_room_buffs();
        self.temp_pickup_cd = if self.skills.has_active(SkillId::StarterChest) {
            2.0
        } else {
            4.0
        };
        self.spawn_wave_if_needed();
        self.spawn_starter_chest_pickup();
        let mut label = if self.room.kind == RoomKind::Boss {
            "BOSS ROOM".to_string()
        } else if self.room.mega {
            "Cathedral · 4× hall".into()
        } else if self.room.has_platforms() {
            "Platform room · dash the gaps".into()
        } else if self.room.is_pillar_alley() {
            "Pillar alley · break cover".into()
        } else {
            "New room".into()
        };
        if !self.level_mods.is_empty() {
            label = format!("{label} · {}", level_mod::mods_label(&self.level_mods));
            self.journal(format!("level mod {}", level_mod::mods_label(&self.level_mods)));
        }
        self.toast(format!(
            "{label} · wave {}/{}",
            self.room.wave, self.room.waves_total
        ));
    }

    fn clear_level_hazards(&mut self) {
        self.gas_clouds.clear();
        self.stampede.clear();
        self.gas_leak_cd = 1.2;
        self.bullet_hell_cd = 1.8;
        self.stampede_cd = 2.5;
    }

    fn enter_shop(&mut self) {
        self.room = rooms::shop_after_boss(self.room.combat_index);
        self.phase = GamePhase::Shop;
        self.boss_mods.clear();
        self.clear_level_hazards();
        self.level_mods.clear();
        self.shake = 0.0;
        self.player.pos = Vec2::new(4.0, self.room.height * 0.5);
        self.actors
            .retain(|a| a.kind == ActorKind::Minion);
        self.projectiles.clear();
        self.pending.clear();
        self.corpses.clear();
        self.pickups.clear();
        self.daisies.clear();
        self.damage_numbers.clear();
        self.pending_spawns.clear();
        self.spawn_next_in = 0.0;
        self.wave_spawn_total = 0;
        self.wave_spawned = 0;
        self.clear_room_buffs();
        self.shop_dummy_active = false;
        self.shop_dummy_id = None;
        // One free nucleus projectile slot per boss clear.
        self.nucleus.expand_slot();
        // Every 5 bosses: +1 mod attachment per projectile.
        if self.boss_kills > 0 && self.boss_kills % 5 == 0 {
            self.nucleus.expand_mod_capacity();
            self.toast(format!(
                "Shop · +1 slot · mod capacity → {} · T dummy",
                self.nucleus.mod_capacity
            ));
            self.journal(format!(
                "mod capacity → {}",
                self.nucleus.mod_capacity
            ));
        } else {
            self.toast("Shop · +1 nucleus slot · T dummy · walk totems · c continue");
        }
        self.shop_reroll_cost = 15;
        self.stock_shop_totems();
        self.reindex_minions();
        self.journal(format!("nucleus slots → {}", self.nucleus.slot_count()));
    }

    pub fn leave_shop(&mut self) {
        if self.phase != GamePhase::Shop {
            return;
        }
        self.shop_totems.clear();
        self.despawn_shop_dummy();
        self.enter_next_combat();
    }

    /// Toggle the shop target dummy (T). Enables autofire for loadout / DPS testing.
    pub fn toggle_shop_dummy(&mut self) {
        if self.phase != GamePhase::Shop {
            return;
        }
        if self.shop_dummy_active {
            self.despawn_shop_dummy();
            self.projectiles.clear();
            self.pending.clear();
            self.toast("Dummy dismissed · autofire off");
            return;
        }
        self.spawn_shop_dummy();
        self.autofire_cd = 0.35;
        self.toast("Dummy online · Tab inventory · autofire on");
        self.journal("shop dummy");
    }

    fn spawn_shop_dummy(&mut self) {
        self.despawn_shop_dummy();
        let id = self.alloc_id();
        let pos = Vec2::new(self.room.width * 0.55, self.room.height * 0.5);
        self.actors.push(Actor {
            id,
            kind: ActorKind::Enemy,
            pos,
            vel: Vec2::ZERO,
            facing: Vec2::new(-1.0, 0.0),
            hp: 5_000.0,
            max_hp: 5_000.0,
            radius: 0.85,
            glyph: 'D',
            color: Color::LightYellow,
            formation_local: Vec2::ZERO,
            formation_index: 0,
            poison_timer: 0.0,
            poison_stacks: 0.0,
            fire_stacks: 0.0,
            vuln_bonus: 0.0,
            kill_gold_bonus: 0.0,
            kill_xp_bonus: 0.0,
            kill_source: String::new(),
            can_shoot: false,
            shoot_cd: 0.0,
            splits_left: 0,
            boss_gen: 0,
            boss_form: BossForm::None,
            trail: Vec::new(),
            reanim_tier: 0,
            is_dummy: true,
        });
        self.shop_dummy_id = Some(id);
        self.shop_dummy_active = true;
    }

    fn despawn_shop_dummy(&mut self) {
        if let Some(id) = self.shop_dummy_id.take() {
            self.actors.retain(|a| a.id != id);
        }
        self.shop_dummy_active = false;
    }

    fn regen_shop_dummy(&mut self) {
        let Some(id) = self.shop_dummy_id else {
            return;
        };
        if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
            a.hp = a.max_hp;
            a.poison_timer = 0.0;
            a.poison_stacks = 0.0;
            a.fire_stacks = 0.0;
            a.vuln_bonus = 0.0;
        } else if self.shop_dummy_active {
            // Was somehow removed — respawn.
            self.spawn_shop_dummy();
        }
    }

    fn stock_shop_totems(&mut self) {
        let w = self.room.width;
        let h = self.room.height;
        let y = h * 0.48;
        // Five offer pads across the hall; reroll totem on the far right.
        let xs = [8.0, 13.5, 19.0, 24.5, 30.0];
        let mut totems = Vec::with_capacity(5);
        for &x in &xs {
            let offer = self.roll_shop_offer();
            let price = self.price_for_offer(&offer);
            totems.push(ShopTotem {
                pos: Vec2::new(x, y),
                offer,
                price,
                sold: false,
            });
        }
        self.shop_totems = totems;
        self.shop_reroll_pos = Vec2::new(w - 3.5, y);
    }

    fn roll_shop_offer(&mut self) -> ShopOffer {
        // Weighted: mostly random spells/mods, occasional utilities / skills.
        let roll = self.rng.random_range(0..100);
        if roll < 62 {
            ShopOffer::Spell(self.lib.random_drop_id(&mut self.rng))
        } else if roll < 72 {
            let id = crate::skills::random_skill_id(&mut self.rng);
            if self.skills.owned.contains(&id) {
                ShopOffer::Spell(self.lib.random_drop_id(&mut self.rng))
            } else {
                ShopOffer::Skill(id)
            }
        } else if roll < 80 {
            ShopOffer::Credit
        } else if roll < 90 {
            ShopOffer::Heal
        } else {
            ShopOffer::SkillSlot
        }
    }

    fn price_for_offer(&self, offer: &ShopOffer) -> i32 {
        match offer {
            ShopOffer::Spell(id) => {
                let rarity = self
                    .lib
                    .get(id)
                    .map(|d| d.rarity)
                    .unwrap_or(crate::proj_logic::Rarity::Common);
                match rarity {
                    crate::proj_logic::Rarity::Common => 12,
                    crate::proj_logic::Rarity::Uncommon => 18,
                    crate::proj_logic::Rarity::Rare => 28,
                    crate::proj_logic::Rarity::Legendary => 45,
                    crate::proj_logic::Rarity::Mythical => 70,
                }
            }
            ShopOffer::Skill(_) => 22,
            ShopOffer::Credit => 12,
            ShopOffer::Heal => 10,
            ShopOffer::SkillSlot => self.skills.slot_upgrade_cost,
        }
    }

    /// Enter in shop: buy nearest totem / reroll, or hint.
    pub fn try_shop_interact(&mut self) {
        if self.phase != GamePhase::Shop {
            return;
        }
        if self.player.pos.dist(self.shop_reroll_pos) < 2.0 {
            self.reroll_shop_totems();
            return;
        }
        let Some(idx) = self
            .shop_totems
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.sold && self.player.pos.dist(t.pos) < 2.0)
            .min_by(|(_, a), (_, b)| {
                self.player
                    .pos
                    .dist(a.pos)
                    .partial_cmp(&self.player.pos.dist(b.pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
        else {
            self.toast("Walk up to a totem (or ⟳) · c to leave");
            return;
        };
        self.buy_shop_totem(idx);
    }

    fn reroll_shop_totems(&mut self) {
        let cost = self.shop_reroll_cost;
        if self.gold < cost {
            self.toast(format!("Need {cost}g to reroll"));
            return;
        }
        self.gold -= cost;
        let next = self.shop_reroll_cost.saturating_mul(2).max(cost + 1);
        self.stock_shop_totems();
        self.shop_reroll_cost = next;
        self.toast(format!("Rerolled · next reroll {}g", self.shop_reroll_cost));
        self.journal("shop reroll");
    }

    pub fn shop_offer_label(&self, offer: &ShopOffer) -> String {
        match offer {
            ShopOffer::Spell(id) => self
                .lib
                .get(id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| id.clone()),
            ShopOffer::Skill(id) => crate::skills::def(*id).name.to_string(),
            ShopOffer::Credit => "Re-anim+".into(),
            ShopOffer::Heal => "Heal 35".into(),
            ShopOffer::SkillSlot => "Skill+".into(),
        }
    }

    pub fn shop_offer_glyph(&self, offer: &ShopOffer) -> char {
        match offer {
            ShopOffer::Spell(id) => self
                .lib
                .get(id)
                .and_then(|d| d.glyph.chars().next())
                .unwrap_or('*'),
            ShopOffer::Skill(id) => crate::skills::def(*id).glyph,
            ShopOffer::Credit => '+',
            ShopOffer::Heal => '♥',
            ShopOffer::SkillSlot => '✦',
        }
    }

    pub fn shop_offer_color(&self, offer: &ShopOffer) -> Color {
        match offer {
            ShopOffer::Spell(id) => self
                .lib
                .get(id)
                .map(|d| d.rarity.color())
                .unwrap_or(Color::Gray),
            ShopOffer::Skill(id) => crate::skills::def(*id).color,
            ShopOffer::Credit => Color::LightGreen,
            ShopOffer::Heal => Color::LightRed,
            ShopOffer::SkillSlot => Color::LightMagenta,
        }
    }

    fn buy_shop_totem(&mut self, idx: usize) {
        let Some(totem) = self.shop_totems.get(idx).cloned() else {
            return;
        };
        if totem.sold {
            self.toast("Already sold");
            return;
        }
        if self.gold < totem.price {
            self.toast(format!("Need {}g", totem.price));
            return;
        }
        self.gold -= totem.price;
        match totem.offer {
            ShopOffer::Spell(id) => {
                let name = self
                    .lib
                    .get(&id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| id.clone());
                self.stash.push(id);
                self.toast(format!("Bought {name}"));
                self.journal(format!("shop {name}"));
            }
            ShopOffer::Skill(id) => {
                if self.skills.unlock(id) {
                    let def = crate::skills::def(id);
                    self.toast(format!("Skill: {}", def.name));
                    self.journal(format!("shop skill {}", def.name));
                } else {
                    self.gold += totem.price;
                    self.toast("Already know that skill — refunded");
                    return;
                }
            }
            ShopOffer::Credit => {
                self.credits += 1;
                self.toast("+1 re-animation credit");
            }
            ShopOffer::Heal => {
                self.player.hp = (self.player.hp + 35.0).min(self.player.max_hp);
                self.toast("Healed 35 HP");
            }
            ShopOffer::SkillSlot => {
                let paid = self.skills.buy_slot_upgrade();
                let _ = paid;
                self.toast(format!(
                    "Skill slots {} · next {}g",
                    self.skills.max_active, self.skills.slot_upgrade_cost
                ));
            }
        }
        if let Some(t) = self.shop_totems.get_mut(idx) {
            t.sold = true;
        }
    }

    fn spawn_wave_if_needed(&mut self) {
        if self.room.kind == RoomKind::Shop || self.room.cleared {
            return;
        }
        if self.room.wave >= self.room.waves_total {
            return;
        }
        if !self.pending_spawns.is_empty() {
            return;
        }
        if self.enemy_count() > 0 {
            return;
        }
        if self.room.spawn_timer > 0.0 {
            return;
        }
        self.room.wave += 1;
        if self.room.kind == RoomKind::Boss && self.room.wave == 1 {
            self.boss_mods = boss::roll_boss_mods(self.room.combat_index, &mut self.rng);
            // Serpent bosses never Multiply-split.
            if self.room.combat_index % 6 == 0 {
                self.boss_mods.retain(|m| *m != BossMod::Multiply);
            }
            self.wall_volley_cd = 0.9;
            self.tremor_cd = 1.2;
        }
        let mods = self.boss_mods.clone();
        let spawns = wave_enemies(&self.room, self.room.wave, &mut self.rng, &mods);
        if self.room.kind == RoomKind::Boss {
            // Bosses (and serpents) arrive together.
            for spawn in spawns {
                self.spawn_enemy_away(spawn);
            }
            self.wave_spawn_total = 0;
            self.wave_spawned = 0;
        } else {
            self.wave_spawn_total = spawns.len() as u32;
            self.wave_spawned = 0;
            self.pending_spawns.extend(spawns);
            self.spawn_next_in = 0.08;
        }
        if self.room.kind == RoomKind::Boss {
            let form = if self.room.combat_index % 6 == 0 {
                "SERPENT"
            } else {
                "BOSS"
            };
            self.toast(format!(
                "{form} · {}",
                boss::mods_label(&self.boss_mods)
            ));
        } else if self.room.mega {
            self.toast(format!(
                "Cathedral hall · wave {}/{} · break the columns",
                self.room.wave, self.room.waves_total
            ));
        } else {
            self.toast(format!(
                "Wave {}/{}",
                self.room.wave, self.room.waves_total
            ));
        }
    }

    /// Minimum spawn distance from the player (~30% of the crypt's longer side).
    fn spawn_min_player_dist(&self) -> f32 {
        0.30 * self.room.width.max(self.room.height)
    }

    fn spawn_enemy_away(&mut self, mut spawn: EnemySpawn) {
        let min_d = self.spawn_min_player_dist();
        let prefer_right = true;
        let mut best = spawn.pos;
        let mut best_d = best.dist(self.player.pos);
        for _ in 0..28 {
            let candidate = self.room.sample_footing(&mut self.rng, spawn.radius, prefer_right);
            let p = self.room.snap_to_footing(candidate, spawn.radius);
            let d = p.dist(self.player.pos);
            if d >= min_d {
                spawn.pos = p;
                let id = self.alloc_id();
                self.actors.push(make_actor(id, spawn));
                return;
            }
            if d > best_d {
                best_d = d;
                best = p;
            }
        }
        // Fallback: farthest sample, or push toward the far wall.
        if best_d < min_d {
            best = self.room.snap_to_footing(
                Vec2::new(self.room.width * 0.82, self.room.height * 0.5),
                spawn.radius,
            );
        }
        spawn.pos = best;
        let id = self.alloc_id();
        self.actors.push(make_actor(id, spawn));
    }

    fn update_pending_spawns(&mut self, dt: f32) {
        if self.pending_spawns.is_empty() {
            return;
        }
        self.spawn_next_in -= dt;
        while self.spawn_next_in <= 0.0 && !self.pending_spawns.is_empty() {
            if let Some(spawn) = self.pending_spawns.pop_front() {
                self.spawn_enemy_away(spawn);
                self.wave_spawned += 1;
            }
            // Exponentially faster arrivals as the wave fills in.
            let total = self.wave_spawn_total.max(1) as f32;
            let progress = (self.wave_spawned as f32 / total).clamp(0.0, 1.0);
            // Starts ~0.55s apart, drops toward ~0.05s.
            self.spawn_next_in = (0.55 * (0.12_f32).powf(progress)).clamp(0.05, 0.7);
        }
    }

    fn update_corpses(&mut self, dt: f32) {
        for c in &mut self.corpses {
            c.life -= dt;
        }
        self.corpses.retain(|c| c.life > 0.0);
    }

    pub fn update(&mut self, dt: f32, paused: bool) {
        if self.message_timer > 0.0 {
            self.message_timer -= dt;
        }
        if self.phase == GamePhase::Dead {
            return;
        }
        if paused {
            return;
        }

        self.anim_t += dt;
        self.shake = (self.shake - dt * 2.8).max(0.0);
        self.dash_cd = (self.dash_cd - dt).max(0.0);
        let dash_ready = self.dash_cd <= 0.0 && self.dash_time <= 0.0;
        if dash_ready && !self.dash_was_ready {
            self.dash_ready_pulse_t = 0.55;
        }
        self.dash_was_ready = dash_ready;
        self.dash_ready_pulse_t = (self.dash_ready_pulse_t - dt).max(0.0);
        self.invuln = (self.invuln - dt).max(0.0);
        self.player_flash = (self.player_flash - dt).max(0.0);
        self.proj_vuln_t = (self.proj_vuln_t - dt).max(0.0);
        self.update_explosions(dt);
        self.update_particles(dt);
        self.update_dash(dt);

        if self.phase == GamePhase::Shop {
            self.update_minion_follow(dt);
            self.update_orbit_blades(dt);
            self.update_damage_numbers(dt);
            if self.shop_dummy_active {
                self.update_pending(dt);
                self.update_autofire(dt);
                self.update_projectiles(dt);
                self.sample_dps_history();
                self.regen_shop_dummy();
            }
            return;
        }

        self.room.spawn_timer = (self.room.spawn_timer - dt).max(0.0);
        self.spawn_wave_if_needed();
        self.update_pending_spawns(dt);
        self.update_corpses(dt);
        self.update_pending(dt);
        self.update_minion_follow(dt);
        self.tick_temp_buffs(dt);
        self.update_temp_pickups(dt);
        self.update_autofire(dt);
        self.update_enemies(dt);
        self.update_boss_hazards(dt);
        self.update_level_mods(dt);
        self.update_projectiles(dt);
        self.update_orbit_blades(dt);
        self.update_daisies(dt);
        self.update_poison(dt);
        self.update_damage_numbers(dt);
        self.sample_dps_history();
        self.pickup_spells();
        self.update_reanimation();
        self.check_room_clear();

        if self.player.hp <= 0.0 {
            self.player.hp = 0.0;
            self.phase = GamePhase::Dead;
            self.toast("You died — permadeath. R to restart title.");
        }
    }

    fn update_pending(&mut self, dt: f32) {
        for p in &mut self.pending {
            p.delay -= dt;
        }
        let ready: Vec<PendingShot> = self
            .pending
            .extract_if(.., |p| p.delay <= 0.0)
            .collect();
        for pending in ready {
            let id = self.alloc_id();
            let mut proj = spawn_projectile(id, &pending);
            if pending.shot.special == "taze" {
                proj.lock_target = self.random_enemy_id();
                proj.homing = proj.homing.max(2.4);
                proj.glyph = 'z';
            }
            if proj.is_flock {
                proj.glyph = match (proj.orbit_angle * 3.0) as i32 % 3 {
                    0 => 'b',
                    1 => 'd',
                    _ => 'p',
                };
            }
            self.projectiles.push(proj);
        }
    }

    fn random_enemy_id(&mut self) -> Option<EntityId> {
        let ids: Vec<EntityId> = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .map(|a| a.id)
            .collect();
        if ids.is_empty() {
            None
        } else {
            Some(ids[self.rng.random_range(0..ids.len())])
        }
    }

    fn update_minion_follow(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let facing = self.player.facing;
        // Snap formation slots to current aim so the fire lane stays clear.
        for actor in &mut self.actors {
            if actor.kind != ActorKind::Minion {
                continue;
            }
            let target = player_pos + world_offset(actor.formation_local, facing);
            let delta = target - actor.pos;
            let dist = delta.length();
            if dist > 0.12 {
                // Faster catch-up when aim changes a lot so they leave the lane quickly.
                let speed = if dist > 2.5 { 16.0 } else { 11.0 };
                actor.pos += delta.normalized() * speed * dt;
            }
            actor.facing = facing;
            // Minions may cross void to keep formation after the player dashes.
            actor.pos = self.room.clamp(actor.pos, actor.radius);
        }
    }

    /// Autofire cycles projectile slots. Wait between slots = that slot's cooldown (1s × mods).
    fn update_autofire(&mut self, dt: f32) {
        if self.room.cleared && !(self.phase == GamePhase::Shop && self.shop_dummy_active) {
            return;
        }
        self.autofire_cd -= dt;
        if self.autofire_cd > 0.0 {
            return;
        }
        let Some(slot_idx) = self.next_autofire_slot() else {
            self.autofire_cd = 0.5;
            return;
        };
        self.autofire_slot = slot_idx;
        let plan = self.build_slot_plan(slot_idx);
        let mut interval = 1.0 * plan.fire_interval_mult;
        if self.skills.has_active(SkillId::MythicPulse) {
            interval *= 0.85;
        }
        if self.overclock_t > 0.0 {
            interval *= 0.5;
        }
        self.autofire_cd = interval;
        self.apply_nucleus_plan(&plan);
        // Advance cursor for next tick.
        self.autofire_slot = (slot_idx + 1) % self.nucleus.slot_count().max(1);
    }

    fn next_autofire_slot(&self) -> Option<usize> {
        let n = self.nucleus.slot_count();
        if n == 0 {
            return None;
        }
        for offset in 0..n {
            let i = (self.autofire_slot + offset) % n;
            if self.nucleus.slots[i].projectile.is_some() {
                return Some(i);
            }
        }
        None
    }

    /// Evaluate one projectile slot (its mods → projectile).
    fn build_slot_plan(&mut self, slot_idx: usize) -> NucleusPlan {
        let reanim_mult = self.reanim_damage_mult();
        let Some(slot) = self.nucleus.slots.get(slot_idx).cloned() else {
            return NucleusPlan {
                fire_interval_mult: 1.0,
                ..NucleusPlan::default()
            };
        };
        let mut mod_strengths = std::collections::HashMap::<String, f32>::new();
        for id in &slot.mods {
            mod_strengths
                .entry(id.clone())
                .or_insert_with(|| mod_strength(self.count_owned(id)));
        }
        let mut proj_dmgs = std::collections::HashMap::<String, f32>::new();
        if let Some(id) = &slot.projectile {
            proj_dmgs.insert(id.clone(), self.marks.get(id).damage_mult() * reanim_mult);
        }
        let loadout = self.nucleus.filled_projectile_ids();
        evaluate_slot(
            &slot,
            &self.lib,
            &|id| *mod_strengths.get(id).unwrap_or(&1.0),
            &|id| *proj_dmgs.get(id).unwrap_or(&1.0),
            &|id| loadout.iter().any(|p| p == id),
            &mut self.rng,
        )
    }

    fn update_explosions(&mut self, dt: f32) {
        for fx in &mut self.explosions {
            fx.life -= dt;
        }
        self.explosions.retain(|fx| fx.life > 0.0);
    }

    fn update_particles(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.pos += p.vel * dt;
            p.vel = p.vel * (1.0 - 3.5 * dt);
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    fn update_dash(&mut self, dt: f32) {
        if self.dash_time <= 0.0 {
            return;
        }
        self.dash_time -= dt;
        let speed = if self.fleet_dash { 76.0 } else { 38.0 };
        // Dash may cross void between platforms; room walls still clamp.
        self.player.pos += self.dash_dir * speed * dt;
        self.player.pos = self.room.resolve_columns(
            self.room.clamp(self.player.pos, self.player.radius),
            self.player.radius,
        );
        // Continuous streak particles.
        if self.rng.random_bool(0.7) {
            self.particles.push(Particle {
                pos: self.player.pos,
                vel: self.dash_dir * -4.0,
                life: 0.18,
                max_life: 0.18,
                glyph: '·',
                color: Color::White,
            });
        }
        if self.skills.has_active(SkillId::BlastDash) {
            for actor in &mut self.actors {
                if matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss)
                    && actor.pos.dist(self.player.pos) < actor.radius + 0.85
                {
                    actor.hp -= 28.0 * dt;
                }
            }
        }
        if self.dash_time <= 0.0 {
            self.player.pos = self
                .room
                .snap_to_footing(self.player.pos, self.player.radius);
        }
    }

    fn spawn_explosion(&mut self, pos: Vec2, radius: f32, color: Color) {
        if radius <= 0.05 {
            return;
        }
        self.explosions.push(ExplosionFx {
            pos,
            radius,
            life: 0.28,
            max_life: 0.28,
            color,
        });
    }

    fn update_orbit_blades(&mut self, dt: f32) {
        if self.orbit_blades.is_empty() {
            return;
        }

        let mut owner_pos: Vec<(EntityId, Vec2)> = vec![(self.player.id, self.player.pos)];
        owner_pos.extend(
            self.actors
                .iter()
                .filter(|a| a.kind == ActorKind::Minion)
                .map(|a| (a.id, a.pos)),
        );

        for blade in &mut self.orbit_blades {
            blade.lifetime -= dt;
            blade.angle += dt * 4.2;
            if let Some((_, pos)) = owner_pos.iter().find(|(id, _)| *id == blade.owner_id) {
                blade.pos = Vec2::new(
                    pos.x + blade.angle.cos() * blade.orbit_radius,
                    pos.y + blade.angle.sin() * blade.orbit_radius,
                );
            }
        }
        self.orbit_blades.retain(|b| {
            b.lifetime > 0.0 && owner_pos.iter().any(|(id, _)| *id == b.owner_id)
        });

        // Damage enemies only — never player/minions.
        let blades: Vec<(Vec2, f32)> = self
            .orbit_blades
            .iter()
            .map(|b| (b.pos, b.damage))
            .collect();
        let mut orbit_dealt = 0.0f32;
        for actor in &mut self.actors {
            if !matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss) {
                continue;
            }
            for (pos, dmg) in &blades {
                if actor.pos.dist(*pos) < actor.radius + 0.45 {
                    let dealt = *dmg * dt * 3.5;
                    actor.hp -= dealt;
                    orbit_dealt += dealt;
                }
            }
        }
        if orbit_dealt > 0.0 {
            self.record_damage(orbit_dealt);
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let dmg_scale = boss::boss_damage_scale(self.room.combat_index);
        // Side-shot: pos, vel_dir, damage, speed, homing, glyph
        let mut shots: Vec<(Vec2, Vec2, f32, f32, f32, char, bool)> = Vec::new();

        for actor in &mut self.actors {
            if !matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss) {
                continue;
            }
            if actor.is_dummy {
                continue;
            }
            if actor.boss_form == BossForm::SnakeBody {
                continue; // positioned from head trail below
            }

            let dir = (player_pos - actor.pos).normalized();
            actor.facing = dir;
            let snake = actor.boss_form == BossForm::SnakeHead;
            let speed = if snake {
                4.4 + (self.room.combat_index as f32 * 0.06).min(1.4)
            } else if actor.kind == ActorKind::Boss {
                3.0 + (self.room.combat_index as f32 * 0.08).min(1.8)
                    + actor.boss_gen as f32 * 0.35
            } else if actor.can_shoot {
                3.4
            } else {
                4.6
            };
            let next = actor.pos + dir * speed * dt;
            actor.pos = self.room.clamp_move(actor.pos, next, actor.radius, false);

            if snake {
                actor.trail.push(actor.pos);
                if actor.trail.len() > 96 {
                    let drain = actor.trail.len() - 96;
                    actor.trail.drain(0..drain);
                }
            }

            if actor.can_shoot {
                actor.shoot_cd -= dt;
                if actor.shoot_cd <= 0.0 {
                    if snake {
                        actor.shoot_cd = 0.85 + self.rng.random_range(0.0..0.3);
                        let side = Vec2::new(-dir.y, dir.x);
                        let damage = 10.0 * dmg_scale;
                        let spd = 7.5 + dmg_scale * 0.8;
                        shots.push((
                            actor.pos + side * 0.9,
                            side,
                            damage,
                            spd,
                            0.95,
                            '●',
                            true,
                        ));
                        shots.push((
                            actor.pos - side * 0.9,
                            side * -1.0,
                            damage,
                            spd,
                            0.95,
                            '●',
                            true,
                        ));
                    } else {
                        let interval = if actor.kind == ActorKind::Boss {
                            (0.7 - actor.boss_gen as f32 * 0.05).max(0.4)
                        } else {
                            1.15
                        };
                        actor.shoot_cd = interval + self.rng.random_range(0.0..0.35);
                        let spd = if actor.kind == ActorKind::Boss {
                            8.5 + dmg_scale * 0.9
                        } else {
                            13.5
                        };
                        let damage = if actor.kind == ActorKind::Boss {
                            14.0 * dmg_scale * (1.0 - actor.boss_gen as f32 * 0.12).max(0.7)
                        } else {
                            8.0
                        };
                        shots.push((
                            actor.pos + dir * 1.1,
                            dir,
                            damage,
                            spd,
                            if actor.kind == ActorKind::Boss {
                                0.25
                            } else {
                                0.05
                            },
                            if actor.kind == ActorKind::Boss {
                                '●'
                            } else {
                                '•'
                            },
                            actor.kind == ActorKind::Boss,
                        ));
                    }
                }
            }
        }

        // Snake body follows delayed samples of the head trail.
        if let Some(head) = self
            .actors
            .iter()
            .find(|a| a.boss_form == BossForm::SnakeHead)
        {
            let trail = head.trail.clone();
            let spacing = 4usize;
            let mut seg_i = 0usize;
            for actor in &mut self.actors {
                if actor.boss_form != BossForm::SnakeBody {
                    continue;
                }
                let idx = trail.len().saturating_sub(1 + (seg_i + 1) * spacing);
                if let Some(p) = trail.get(idx) {
                    actor.pos = self.room.resolve_columns(*p, actor.radius);
                    actor.facing = if seg_i + 1 < trail.len() {
                        (*trail.get(idx.saturating_sub(1)).unwrap_or(p) - *p).normalized()
                    } else {
                        actor.facing
                    };
                }
                seg_i += 1;
            }
        }

        // Contact damage pass (skipped while invulnerable / phasing).
        if self.invuln <= 0.0 {
            let mut contact = 0.0;
            let mut boss_hit = false;
            for actor in &self.actors {
                if matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss)
                    && actor.pos.dist(self.player.pos) < actor.radius + self.player.radius
                {
                    if actor.kind == ActorKind::Boss {
                        let mult = if actor.boss_form == BossForm::SnakeBody {
                            0.55
                        } else {
                            1.0
                        };
                        contact += 18.0 * dmg_scale * mult * dt;
                        boss_hit = true;
                    } else {
                        contact += 10.0 * dt;
                    }
                }
            }
            if self.skills.has_active(SkillId::IronWill) {
                contact *= 0.65;
            }
            if contact > 0.0 {
                if self.try_absorb_hit() {
                    // Guard ate the hit.
                } else if let Some(idx) = self.shield_minion_idx() {
                    self.actors[idx].hp -= contact;
                } else {
                    self.player.hp -= contact;
                    if boss_hit {
                        let amp = if self.boss_mods.contains(&BossMod::Tremor) {
                            0.55
                        } else {
                            0.22
                        };
                        self.shake = self.shake.max(amp);
                    }
                }
            }
        }

        if self.twin_volley_t > 0.0 && !shots.is_empty() {
            let mut twins = Vec::with_capacity(shots.len());
            for (pos, dir, damage, speed, homing, glyph, boss) in &shots {
                let side = Vec2::new(-dir.y, dir.x) * 0.45;
                twins.push((
                    *pos + side,
                    *dir,
                    *damage,
                    *speed,
                    *homing,
                    *glyph,
                    *boss,
                ));
            }
            shots.extend(twins);
        }

        for (pos, dir, damage, speed, homing, glyph, boss) in shots {
            let id = self.alloc_id();
            self.projectiles.push(Projectile {
                id,
                pos,
                vel: dir * speed,
                damage,
                radius: if boss { 0.72 } else { 0.32 },
                explosion_radius: 0.0,
                lifetime: if boss { 3.4 } else { 2.4 },
                max_lifetime: if boss { 3.4 } else { 2.4 },
                age: 0.0,
                bounces: 0,
                pierce: 0,
                pierced: Vec::new(),
                homing,
                poison: 0.0,
                chain: 0,
                glyph,
                color: if boss {
                    Color::Rgb(255, 90, 70)
                } else {
                    Color::LightYellow
                },
                owner_is_player_side: false,
                friendly_fire: false,
                chained: 0,
                trail: Vec::new(),
                returning: false,
                returned: false,
                trail_fx: false,
                trail_rainbow: false,
                trail_bright: 1.0,
                orbiting: false,
                orbit_angle: 0.0,
                orbit_radius: 0.0,
                gravity_well: false,
                arc: false,
                gated: false,
                gated_done: false,
                orbit_then_fire: false,
                orbit_launch_at: 0.7,
                homing_ramp: false,
                crit_bonus: 0.0,
                lock_target: None,
                is_flock: false,
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
spawn_offset: Vec2::ZERO,
            });
        }
    }

    fn update_boss_hazards(&mut self, dt: f32) {
        let boss_alive = self.actors.iter().any(|a| a.kind == ActorKind::Boss);
        if !boss_alive {
            return;
        }

        if self.boss_mods.contains(&BossMod::Tremor) {
            self.tremor_cd -= dt;
            if self.tremor_cd <= 0.0 {
                self.tremor_cd = (2.4 - self.room.combat_index as f32 * 0.06).max(1.2);
                self.shake = self.shake.max(0.45);
            }
        }

        if !self.boss_mods.contains(&BossMod::WallVolley) {
            return;
        }
        self.wall_volley_cd -= dt;
        if self.wall_volley_cd > 0.0 {
            return;
        }
        let scale = boss::boss_damage_scale(self.room.combat_index);
        self.wall_volley_cd = (1.4 - self.room.combat_index as f32 * 0.05).max(0.75);
        let n = 2 + self.rng.random_range(0..=2);
        let speed = 7.0 + scale * 1.2;
        let damage = 9.0 * scale;
        for _ in 0..n {
            let x = self.rng.random_range(3.0..(self.room.width - 3.0));
            let from_top = self.rng.random_bool(0.5);
            let pos = Vec2::new(
                x,
                if from_top {
                    1.6
                } else {
                    self.room.height - 1.6
                },
            );
            let vel = Vec2::new(0.0, if from_top { speed } else { -speed });
            let id = self.alloc_id();
            self.projectiles.push(Projectile {
                id,
                pos,
                vel,
                damage,
                radius: 0.55,
                explosion_radius: 0.0,
                lifetime: 3.6,
                max_lifetime: 3.6,
                age: 0.0,
                bounces: 0,
                pierce: 0,
                pierced: Vec::new(),
                homing: 0.0,
                poison: 0.0,
                chain: 0,
                glyph: '┃',
                color: Color::Rgb(255, 120, 80),
                owner_is_player_side: false,
                friendly_fire: false,
                chained: 0,
                trail: Vec::new(),
                returning: false,
                returned: false,
                trail_fx: false,
                trail_rainbow: false,
                trail_bright: 1.0,
                orbiting: false,
                orbit_angle: 0.0,
                orbit_radius: 0.0,
                gravity_well: false,
                arc: false,
                gated: false,
                gated_done: false,
                orbit_then_fire: false,
                orbit_launch_at: 0.7,
                homing_ramp: false,
                crit_bonus: 0.0,
                lock_target: None,
                is_flock: false,
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
spawn_offset: Vec2::ZERO,
            });
        }
    }

    fn push_damage_number(&mut self, pos: Vec2, amount: f32, crit: bool) {
        if amount < 0.5 {
            return;
        }
        let jitter = Vec2::new(
            self.rng.random_range(-0.35..0.35),
            self.rng.random_range(-0.55..-0.15),
        );
        self.damage_numbers.push(DamageNumber {
            pos: pos + jitter,
            amount,
            life: if crit { 1.1 } else { 0.85 },
            max_life: if crit { 1.1 } else { 0.85 },
            crit,
        });
        while self.damage_numbers.len() > 64 {
            self.damage_numbers.remove(0);
        }
    }

    fn update_damage_numbers(&mut self, dt: f32) {
        for n in &mut self.damage_numbers {
            n.life -= dt;
            n.pos.y -= 1.4 * dt;
        }
        self.damage_numbers.retain(|n| n.life > 0.0);
    }

    fn update_level_mods(&mut self, dt: f32) {
        if self.level_mods.is_empty() || self.room.cleared {
            // Still tick lingering hazards briefly after clear? Clear them.
            if self.room.cleared {
                self.gas_clouds.clear();
                self.stampede.clear();
            }
            return;
        }
        if self.level_mods.contains(&LevelMod::GasChamber) {
            self.update_gas_chamber(dt);
        }
        if self.level_mods.contains(&LevelMod::BulletHell) {
            self.update_bullet_hell(dt);
        }
        if self.level_mods.contains(&LevelMod::Stampede) {
            self.update_stampede(dt);
        }
        if self.level_mods.contains(&LevelMod::SomethingIsLava) {
            self.update_something_is_lava(dt);
        }
    }

    fn update_gas_chamber(&mut self, dt: f32) {
        self.gas_leak_cd -= dt;
        if self.gas_leak_cd <= 0.0 {
            self.gas_leak_cd = self.rng.random_range(2.2..3.8);
            let kind = match self.rng.random_range(0..3u8) {
                0 => GasKind::Heal,
                1 => GasKind::Poison,
                _ => GasKind::Lava,
            };
            let pos = Vec2::new(
                self.rng.random_range(4.0..(self.room.width - 4.0)),
                self.rng.random_range(3.0..(self.room.height - 3.0)),
            );
            let life = self.rng.random_range(3.5..5.5);
            self.gas_clouds.push(GasCloud {
                pos,
                radius: self.rng.random_range(2.2..3.4),
                kind,
                life,
                max_life: life,
            });
        }

        let mut heals: Vec<(EntityId, f32)> = Vec::new();
        let mut poisons: Vec<(EntityId, f32)> = Vec::new();
        let mut burns: Vec<(EntityId, f32, Vec2)> = Vec::new();
        let mut player_heal = 0.0f32;
        let mut player_poison = 0.0f32;
        let mut player_burn = 0.0f32;

        for cloud in &self.gas_clouds {
            let tick = match cloud.kind {
                GasKind::Heal => 8.0 * dt,
                GasKind::Poison => 0.0,
                GasKind::Lava => 22.0 * dt,
            };
            if self.player.pos.dist(cloud.pos) <= cloud.radius + self.player.radius {
                match cloud.kind {
                    GasKind::Heal => player_heal += tick,
                    GasKind::Poison => player_poison = player_poison.max(2.2),
                    GasKind::Lava => player_burn += tick,
                }
            }
            for a in &self.actors {
                if a.pos.dist(cloud.pos) > cloud.radius + a.radius {
                    continue;
                }
                match cloud.kind {
                    GasKind::Heal => heals.push((a.id, tick)),
                    GasKind::Poison => poisons.push((a.id, 2.4)),
                    GasKind::Lava => burns.push((a.id, tick, a.pos)),
                }
            }
        }

        if player_heal > 0.0 {
            self.player.hp = (self.player.hp + player_heal).min(self.player.max_hp);
        }
        if player_poison > 0.0 {
            self.player.poison_timer = self.player.poison_timer.max(player_poison);
        }
        if player_burn > 0.0 && self.invuln <= 0.0 && self.dash_time <= 0.0 {
            if !self.try_absorb_hit() {
                if let Some(idx) = self.shield_minion_idx() {
                    self.actors[idx].hp -= player_burn;
                } else {
                    self.player.hp -= player_burn;
                    self.shake = self.shake.max(0.12);
                }
            }
        }

        for (id, amt) in heals {
            if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
                a.hp = (a.hp + amt).min(a.max_hp);
            }
        }
        for (id, t) in poisons {
            if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
                a.poison_timer = a.poison_timer.max(t);
            }
        }
        for (id, amt, pos) in burns {
            if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
                // Gas lava hurts everyone — including enemies.
                a.hp -= amt;
                if a.is_dummy {
                    a.hp = a.hp.max(1.0);
                }
            }
            let _ = (id, pos);
        }

        for cloud in &mut self.gas_clouds {
            cloud.life -= dt;
        }
        self.gas_clouds.retain(|c| c.life > 0.0);
    }

    fn update_bullet_hell(&mut self, dt: f32) {
        self.bullet_hell_cd -= dt;
        if self.bullet_hell_cd > 0.0 {
            return;
        }
        self.bullet_hell_cd = self.rng.random_range(2.4..3.6);
        let pattern = self.rng.random_range(0..3u8);
        let w = self.room.width;
        let h = self.room.height;
        let damage = 7.0 + self.room.combat_index as f32 * 0.35;
        let speed = 5.5 + self.room.combat_index as f32 * 0.08;
        match pattern {
            0 => {
                // Horizontal wall riders with periodic gaps.
                let n = 10;
                let gap = self.rng.random_range(2..5);
                let from_left = self.rng.random_bool(0.5);
                for i in 0..n {
                    if i == gap || i == gap + 1 {
                        continue;
                    }
                    let y = 2.5 + i as f32 * ((h - 5.0) / (n - 1) as f32);
                    let pos = Vec2::new(if from_left { 2.2 } else { w - 2.2 }, y);
                    let dir = Vec2::new(if from_left { 1.0 } else { -1.0 }, 0.0);
                    self.spawn_hell_bullet(pos, dir, damage, speed, 8);
                }
            }
            1 => {
                // Vertical volleys hugging left/right with a center gap lane.
                let n = 12;
                let gap_a = 4;
                let gap_b = 5;
                let from_top = self.rng.random_bool(0.5);
                for i in 0..n {
                    if i == gap_a || i == gap_b {
                        continue;
                    }
                    let x = 3.0 + i as f32 * ((w - 6.0) / (n - 1) as f32);
                    let pos = Vec2::new(x, if from_top { 1.8 } else { h - 1.8 });
                    let dir = Vec2::new(0.0, if from_top { 1.0 } else { -1.0 });
                    self.spawn_hell_bullet(pos, dir, damage, speed, 10);
                }
            }
            _ => {
                // Diamond bounce from corners — leave one corner empty as escape.
                let corners = [
                    (Vec2::new(2.5, 2.5), Vec2::new(1.0, 0.55).normalized()),
                    (Vec2::new(w - 2.5, 2.5), Vec2::new(-1.0, 0.55).normalized()),
                    (Vec2::new(2.5, h - 2.5), Vec2::new(1.0, -0.55).normalized()),
                    (Vec2::new(w - 2.5, h - 2.5), Vec2::new(-1.0, -0.55).normalized()),
                ];
                let skip = self.rng.random_range(0..4);
                for (i, (pos, dir)) in corners.iter().enumerate() {
                    if i == skip {
                        continue;
                    }
                    for k in 0..3 {
                        let offset = Vec2::new(0.0, (k as f32 - 1.0) * 1.1);
                        self.spawn_hell_bullet(*pos + offset, *dir, damage, speed * 0.9, 14);
                    }
                }
            }
        }
    }

    fn spawn_hell_bullet(&mut self, pos: Vec2, dir: Vec2, damage: f32, speed: f32, bounces: u32) {
        let id = self.alloc_id();
        self.projectiles.push(Projectile {
            id,
            pos,
            vel: dir.normalized() * speed,
            damage,
            radius: 0.28,
            explosion_radius: 0.0,
            lifetime: 7.5,
            max_lifetime: 7.5,
            age: 0.0,
            bounces,
            pierce: 0,
            pierced: Vec::new(),
            homing: 0.0,
            poison: 0.0,
            chain: 0,
            glyph: '◦',
            color: Color::Rgb(255, 170, 90),
            owner_is_player_side: false,
            friendly_fire: false,
            chained: 0,
            trail: Vec::new(),
            returning: false,
            returned: false,
            trail_fx: false,
            trail_rainbow: false,
            trail_bright: 1.0,
            orbiting: false,
            orbit_angle: 0.0,
            orbit_radius: 0.0,
            gravity_well: false,
            arc: false,
            gated: false,
            gated_done: false,
            orbit_then_fire: false,
            orbit_launch_at: 0.7,
            homing_ramp: false,
            crit_bonus: 0.0,
            lock_target: None,
            is_flock: false,
            source_id: "bullet_hell".into(),
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
spawn_offset: Vec2::ZERO,
        });
    }

    fn update_stampede(&mut self, dt: f32) {
        self.stampede_cd -= dt;
        if self.stampede_cd <= 0.0 {
            self.stampede_cd = self.rng.random_range(3.5..5.5);
            let count = 4 + self.rng.random_range(0..=3);
            let damage = 14.0 + self.room.combat_index as f32 * 0.5;
            for i in 0..count {
                let x = self.rng.random_range(3.0..(self.room.width - 3.0));
                let stagger = i as f32 * 0.35;
                self.stampede.push(StampedeBeast {
                    pos: Vec2::new(x, -1.0 - stagger),
                    vel: Vec2::new(
                        self.rng.random_range(-0.8..0.8),
                        9.5 + self.rng.random_range(0.0..2.5),
                    ),
                    damage,
                    radius: 0.7,
                    life: 6.0,
                    hit: Vec::new(),
                });
            }
            self.toast("Stampede!");
            self.shake = self.shake.max(0.25);
        }

        let mut player_hits = Vec::new();
        let mut actor_hits: Vec<(EntityId, f32, Vec2)> = Vec::new();
        for beast in &mut self.stampede {
            beast.life -= dt;
            beast.pos += beast.vel * dt;
            if self.invuln <= 0.0
                && self.dash_time <= 0.0
                && beast.pos.dist(self.player.pos) < beast.radius + self.player.radius
                && !beast.hit.contains(&self.player.id)
            {
                beast.hit.push(self.player.id);
                player_hits.push(beast.damage);
            }
            for a in &self.actors {
                if beast.pos.dist(a.pos) < beast.radius + a.radius && !beast.hit.contains(&a.id)
                {
                    beast.hit.push(a.id);
                    actor_hits.push((a.id, beast.damage, a.pos));
                }
            }
        }
        self.stampede
            .retain(|b| b.life > 0.0 && b.pos.y < self.room.height + 2.0);

        for dmg in player_hits {
            if self.try_absorb_hit() {
                continue;
            }
            if let Some(idx) = self.shield_minion_idx() {
                self.actors[idx].hp -= dmg;
            } else {
                self.player.hp -= dmg;
                self.shake = self.shake.max(0.35);
            }
        }
        for (id, dmg, pos) in actor_hits {
            if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
                a.hp -= dmg;
                if a.is_dummy {
                    a.hp = a.hp.max(1.0);
                }
            }
            self.push_damage_number(pos, dmg, false);
        }
    }

    /// Top/bottom lava waveforms — orange, burns on contact.
    pub fn lava_top_y(&self, x: f32) -> f32 {
        let t = self.anim_t;
        1.35
            + 1.15 * (x * 0.38 + t * 2.1).sin()
            + 0.55 * (x * 0.72 - t * 1.4).sin()
    }

    pub fn lava_bottom_y(&self, x: f32) -> f32 {
        let t = self.anim_t;
        self.room.height
            - (1.35
                + 1.15 * (x * 0.38 + t * 2.1 + 1.7).sin()
                + 0.55 * (x * 0.72 - t * 1.4 + 0.9).sin())
    }

    fn in_lava_wave(&self, pos: Vec2, radius: f32) -> bool {
        if !self.level_mods.contains(&LevelMod::SomethingIsLava) {
            return false;
        }
        let top = self.lava_top_y(pos.x);
        let bot = self.lava_bottom_y(pos.x);
        pos.y - radius < top || pos.y + radius > bot
    }

    fn update_something_is_lava(&mut self, dt: f32) {
        let burn = (18.0 + self.room.combat_index as f32 * 0.4) * dt;
        if self.invuln <= 0.0
            && self.dash_time <= 0.0
            && self.in_lava_wave(self.player.pos, self.player.radius)
        {
            if !self.try_absorb_hit() {
                if let Some(idx) = self.shield_minion_idx() {
                    self.actors[idx].hp -= burn;
                } else {
                    self.player.hp -= burn;
                    self.shake = self.shake.max(0.1);
                }
            }
        }
        let mut hits = Vec::new();
        for a in &self.actors {
            if self.in_lava_wave(a.pos, a.radius) {
                hits.push((a.id, a.pos));
            }
        }
        for (id, pos) in hits {
            if let Some(a) = self.actors.iter_mut().find(|a| a.id == id) {
                a.hp -= burn;
                if a.is_dummy {
                    a.hp = a.hp.max(1.0);
                }
            }
            if self.anim_t.fract() < dt * 2.0 {
                self.push_damage_number(pos, burn / dt.max(0.01) * 0.15, false);
            }
        }
    }

    fn update_projectiles(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let player_facing = if self.player.facing.length() > 0.05 {
            self.player.facing.normalized()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let enemies: Vec<(u64, Vec2)> = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .map(|a| (a.id, a.pos))
            .collect();
        let mut gate_fx: Vec<Vec2> = Vec::new();

        // Gravity wells pull other projectiles toward their bolts.
        let wells: Vec<(usize, Vec2)> = self
            .projectiles
            .iter()
            .enumerate()
            .filter(|(_, p)| p.gravity_well && p.lifetime > 0.0)
            .map(|(i, p)| (i, p.pos))
            .collect();
        if !wells.is_empty() {
            for (i, proj) in self.projectiles.iter_mut().enumerate() {
                if proj.gravity_well || proj.orbiting {
                    continue;
                }
                let mut pull = Vec2::ZERO;
                for &(wi, wpos) in &wells {
                    if wi == i {
                        continue;
                    }
                    let delta = wpos - proj.pos;
                    let dist = delta.length().max(0.35);
                    if dist < 9.0 {
                        pull += delta.normalized() * (28.0 / dist);
                    }
                }
                if pull.length() > 0.0 {
                    let speed = proj.vel.length().max(6.0);
                    proj.vel = (proj.vel + pull * dt).normalized() * speed;
                }
            }
        }

        // Homing / return / orbit motion
        for proj in &mut self.projectiles {
            proj.age += dt;

            if proj.orbiting {
                proj.orbit_angle += 4.8 * dt;
                let center = player_pos;
                proj.pos = center
                    + Vec2::new(
                        proj.orbit_angle.cos() * proj.orbit_radius,
                        proj.orbit_angle.sin() * proj.orbit_radius * 0.85,
                    );
                proj.vel = Vec2::ZERO;
                if proj.orbit_then_fire && proj.age >= proj.orbit_launch_at {
                    proj.orbiting = false;
                    let out = Vec2::from_angle(proj.orbit_angle);
                    let dir = (out * 0.35 + player_facing * 0.65).normalized();
                    proj.vel = dir * 20.0;
                    proj.lifetime = proj.lifetime.max(1.2);
                }
                continue;
            }

            if proj.arc {
                proj.vel.y += 28.0 * dt; // gravity
            }

            if proj.gated && !proj.gated_done && proj.age >= 0.2 {
                proj.gated_done = true;
                if let Some((_, target)) = enemies.iter().min_by(|(ida, a), (idb, b)| {
                    let ja = a.dist(player_pos) + ((*ida as f32) * 0.13).sin() * 3.0;
                    let jb = b.dist(player_pos) + ((*idb as f32) * 0.13).sin() * 3.0;
                    ja.partial_cmp(&jb).unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    proj.pos = Vec2::new(target.x, target.y - 2.8);
                    proj.vel = Vec2::new(0.0, 22.0);
                    proj.homing = proj.homing.max(0.8);
                    gate_fx.push(proj.pos);
                }
            }

            if proj.homing_ramp {
                let t = (proj.age / 0.85).clamp(0.0, 1.0);
                proj.homing = 0.35 + t * 2.8;
                let speed = proj.vel.length().max(3.5);
                let target_speed = 5.0 + t * 16.0;
                if speed > 0.1 {
                    proj.vel = proj.vel.normalized() * (speed + (target_speed - speed) * 0.08);
                }
            }

            if proj.returning && !proj.returned && proj.age >= proj.max_lifetime * 0.42 {
                proj.returned = true;
                let speed = proj.vel.length().max(18.0);
                proj.vel = (player_pos - proj.pos).normalized() * speed;
                proj.homing = proj.homing.max(2.2);
            }
            if proj.returning {
                proj.glyph = match ((proj.age * 14.0) as i32) % 4 {
                    0 => '‹',
                    1 => '«',
                    2 => '›',
                    _ => '»',
                };
            }

            if proj.homing <= 0.0 {
                continue;
            }
            let target = if proj.returning && proj.returned {
                Some(player_pos)
            } else if let Some(lock) = proj.lock_target {
                enemies
                    .iter()
                    .find(|(id, _)| *id == lock)
                    .map(|(_, p)| *p)
                    .or_else(|| {
                        enemies
                            .iter()
                            .min_by(|(_, a), (_, b)| {
                                proj.pos
                                    .dist(*a)
                                    .partial_cmp(&proj.pos.dist(*b))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(_, p)| *p)
                    })
            } else if proj.owner_is_player_side {
                enemies
                    .iter()
                    .min_by(|(_, a), (_, b)| {
                        proj.pos
                            .dist(*a)
                            .partial_cmp(&proj.pos.dist(*b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(_, p)| *p)
            } else {
                Some(player_pos)
            };
            if let Some(t) = target {
                let speed = proj.vel.length().max(1.0);
                let desired = (t - proj.pos).normalized() * speed;
                proj.vel = (proj.vel + (desired - proj.vel) * proj.homing * dt * 4.0)
                    .normalized()
                    * speed;
            }
        }

        for pos in gate_fx {
            self.particles.push(Particle {
                pos,
                vel: Vec2::ZERO,
                life: 0.25,
                max_life: 0.25,
                glyph: '◎',
                color: Color::Magenta,
            });
        }

        // Flock birds deflect enemy projectiles, then dive at a foe.
        self.resolve_flock_deflects();


        // tuple: pos, blast_radius, damage, player_side, friendly_fire, poison, chain, color
        let mut explosions: Vec<(Vec2, f32, f32, bool, bool, f32, u32, Color)> = Vec::new();

        for proj in &mut self.projectiles {
            proj.trail.push(proj.pos);
            let max_trail = if proj.owner_is_player_side {
                if proj.trail_fx {
                    28
                } else {
                    22
                }
            } else if proj.radius >= 0.55 {
                14
            } else {
                6
            };
            if proj.trail.len() > max_trail {
                let drain = proj.trail.len() - max_trail;
                proj.trail.drain(0..drain);
            }
            if !proj.orbiting {
                proj.pos += proj.vel * dt;
            }
            proj.lifetime -= dt;

            // Wall bounce
            let mut bounced = false;
            if proj.pos.x < 1.2 || proj.pos.x > self.room.width - 1.2 {
                if proj.bounces > 0 {
                    proj.vel.x *= -1.0;
                    proj.bounces -= 1;
                    bounced = true;
                }
            }
            if proj.pos.y < 1.2 || proj.pos.y > self.room.height - 1.2 {
                if proj.bounces > 0 {
                    proj.vel.y *= -1.0;
                    proj.bounces -= 1;
                    bounced = true;
                }
            }
            if bounced {
                proj.pos = self.room.clamp(proj.pos, proj.radius);
            } else if !self.room.contains(proj.pos, 0.0) && !proj.orbiting {
                proj.lifetime = 0.0;
            }

            // Returning scythe despawns near player after turnaround
            if proj.returning && proj.returned && proj.pos.dist(player_pos) < 0.9 {
                proj.lifetime = 0.0;
            }
        }

        // Destructible columns.
        if self.room.has_columns() {
            let mut events: Vec<(usize, usize, f32)> = Vec::new(); // proj_i, col_i, dmg
            for (pi, proj) in self.projectiles.iter().enumerate() {
                if proj.lifetime <= 0.0 {
                    continue;
                }
                if let Some((ci, _)) = self
                    .room
                    .columns
                    .iter()
                    .enumerate()
                    .find(|(_, c)| proj.pos.dist(c.pos) < c.radius + proj.radius)
                {
                    events.push((pi, ci, proj.damage * 0.9));
                }
            }
            let mut fallen = Vec::new();
            for (pi, ci, dmg) in events {
                if let Some(col) = self.room.columns.get_mut(ci) {
                    col.hp -= dmg;
                    if col.hp <= 0.0 {
                        fallen.push(col.pos);
                    }
                }
                if let Some(proj) = self.projectiles.get_mut(pi) {
                    if proj.pierce == 0 {
                        proj.lifetime = 0.0;
                    } else {
                        proj.pierce -= 1;
                    }
                }
            }
            self.room.columns.retain(|c| c.hp > 0.0);
            for pos in fallen {
                self.spawn_column_rubble(pos);
                self.shake = self.shake.max(0.28);
            }
        }

        // Collisions — pierce carries through enemies when remaining.
        let mut hit_indices = Vec::new();
        // id, damage, poison_timer, crit, poison_stacks, fire_stacks, vuln, gold, xp, source
        let mut direct_hits: Vec<(EntityId, f32, f32, f32, f32, f32, f32, f32, f32, String)> =
            Vec::new();
        for (pi, proj) in self.projectiles.iter_mut().enumerate() {
            if proj.lifetime <= 0.0 {
                hit_indices.push(pi);
                explosions.push((
                    proj.pos,
                    proj.explosion_radius,
                    proj.damage,
                    proj.owner_is_player_side,
                    proj.friendly_fire,
                    proj.poison,
                    proj.chain.saturating_sub(proj.chained),
                    proj.color,
                ));
                continue;
            }

            let hit_player = self.invuln <= 0.0
                && !proj.owner_is_player_side
                && proj.pos.dist(self.player.pos) < proj.radius + self.player.radius;
            if hit_player {
                hit_indices.push(pi);
                explosions.push((
                    proj.pos,
                    proj.explosion_radius,
                    proj.damage,
                    proj.owner_is_player_side,
                    proj.friendly_fire,
                    proj.poison,
                    0,
                    proj.color,
                ));
                continue;
            }

            if !proj.owner_is_player_side {
                continue;
            }
            // Orbiting flock birds only deflect; they dive after a hit.
            if proj.is_flock && proj.orbiting {
                continue;
            }

            let mut consumed = false;
            for actor in &self.actors {
                if actor.kind == ActorKind::Minion {
                    continue;
                }
                if !matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss) {
                    continue;
                }
                if proj.pierced.contains(&actor.id) {
                    continue;
                }
                if proj.pos.dist(actor.pos) < proj.radius + actor.radius {
                    direct_hits.push((
                        actor.id,
                        proj.damage,
                        proj.poison,
                        proj.crit_bonus,
                        proj.poison_stacks,
                        proj.fire_stacks,
                        proj.vuln_bonus,
                        proj.gold_bonus,
                        proj.xp_bonus,
                        proj.source_id.clone(),
                    ));
                    proj.pierced.push(actor.id);
                    if proj.pierce > 0 {
                        proj.pierce -= 1;
                    } else {
                        hit_indices.push(pi);
                        explosions.push((
                            proj.pos,
                            proj.explosion_radius,
                            proj.damage,
                            proj.owner_is_player_side,
                            proj.friendly_fire,
                            proj.poison,
                            proj.chain.saturating_sub(proj.chained),
                            proj.color,
                        ));
                        consumed = true;
                        break;
                    }
                }
            }
            let _ = consumed;
        }

        for (
            id,
            damage,
            poison,
            crit_bonus,
            poison_stacks,
            fire_stacks,
            vuln,
            gold_bonus,
            xp_bonus,
            source_id,
        ) in direct_hits
        {
            let mut dmg = damage;
            let crit_chance = (0.01 + crit_bonus).clamp(0.0, 0.75);
            let crit = self.rng.random_bool(crit_chance as f64);
            if crit {
                dmg *= 10.0;
                self.journal(format!("CRIT {:.0}", dmg));
            }
            let mut hit_pos = None;
            if let Some(actor) = self.actors.iter_mut().find(|a| a.id == id) {
                let taken = dmg * (1.0 + actor.vuln_bonus);
                actor.hp -= taken;
                if actor.is_dummy {
                    actor.hp = actor.hp.max(1.0);
                }
                hit_pos = Some(actor.pos);
                if poison > 0.0 {
                    actor.poison_timer = actor.poison_timer.max(poison);
                }
                if poison_stacks > 0.0 {
                    actor.poison_stacks += poison_stacks;
                }
                if fire_stacks > 0.0 {
                    actor.fire_stacks += fire_stacks * 0.30;
                }
                if vuln > 0.0 {
                    actor.vuln_bonus = actor.vuln_bonus.max(vuln);
                }
                actor.kill_gold_bonus = gold_bonus;
                actor.kill_xp_bonus = xp_bonus;
                actor.kill_source = source_id;
                dmg = taken;
            }
            if let Some(pos) = hit_pos {
                self.push_damage_number(pos, dmg, crit);
            }
            self.record_damage(dmg);
        }

        // Remove hit/expired projectiles (high indices first)
        hit_indices.sort_unstable();
        hit_indices.dedup();
        for i in hit_indices.into_iter().rev() {
            if i < self.projectiles.len() {
                self.projectiles.remove(i);
            }
        }
        self.projectiles.retain(|p| p.lifetime > 0.0);

        for (pos, radius, damage, player_side, friendly_fire, poison, chain, color) in explosions {
            if radius > 0.05 {
                self.spawn_explosion(pos, radius, color);
                // Player blasts are weak splash; enemy shots keep full payload.
                // Wide electrify blasts (Taze) keep most of their damage; tiny P-Cannon blasts stay soft.
                let splash = if player_side {
                    if radius >= 2.0 {
                        damage * 0.8
                    } else {
                        damage * 0.35
                    }
                } else {
                    damage
                };
                if player_side {
                    self.record_damage(splash);
                }
                self.apply_blast(pos, radius, splash, player_side, friendly_fire, poison);
            } else if !player_side {
                // Enemy point hit (no blast) still hurts.
                self.apply_blast(pos, 0.35, damage, player_side, friendly_fire, poison);
            }
            if chain > 0 {
                self.spawn_chain(pos, damage * 0.75, player_side, chain);
            }
        }

        // Remove dead actors → corpses / loot
        let mut dead = Vec::new();
        let mut lost_minion = false;
        self.actors.retain(|a| {
            if a.hp <= 0.0 {
                if a.kind == ActorKind::Minion {
                    lost_minion = true;
                }
                dead.push(a.clone());
                false
            } else {
                true
            }
        });
        if lost_minion {
            self.reindex_minions();
        }
        for a in dead {
            if a.is_dummy {
                continue;
            }
            if a.boss_form == BossForm::SnakeHead {
                // Body collapses with the head.
                self.actors
                    .retain(|b| b.boss_form != BossForm::SnakeBody);
            }
            if a.kind == ActorKind::Boss
                && a.splits_left > 0
                && a.boss_form == BossForm::Classic
            {
                self.spawn_boss_splits(&a);
                if self.skills.has_active(SkillId::ManaSiphon) {
                    self.player.hp = (self.player.hp + 6.0).min(self.player.max_hp);
                }
                self.gold += 6;
                self.shake = self.shake.max(0.65);
                self.toast(format!(
                    "Multiply! gen {} → two smaller bosses",
                    a.boss_gen + 1
                ));
                continue;
            }
            if a.boss_form == BossForm::SnakeBody {
                self.gold += 2;
                continue;
            }
            let from_boss = a.kind == ActorKind::Boss;
            if matches!(a.kind, ActorKind::Enemy | ActorKind::Boss) {
                if self.skills.has_active(SkillId::ManaSiphon) {
                    self.player.hp = (self.player.hp + 10.0).min(self.player.max_hp);
                }
                let mut gold = if from_boss {
                    25 + a.boss_gen as i32 * 8
                } else {
                    3 + self.rng.random_range(0..=3)
                };
                if a.kill_gold_bonus > 0.0 {
                    gold = ((gold as f32) * (1.0 + a.kill_gold_bonus)).round() as i32;
                }
                self.gold += gold;
                if from_boss {
                    self.shake = self.shake.max(0.7);
                    self.journal("boss down");
                }
                // Drops: 10% chance of any pickup, with skills a rarer bonus (2%).
                if self.rng.random_bool(0.02) {
                    let skill_id = crate::skills::random_skill_id(&mut self.rng);
                    if !self.skills.owned.contains(&skill_id) {
                        let id = self.alloc_id();
                        self.pickups.push(Pickup {
                            id,
                            pos: a.pos,
                            kind: PickupKind::Skill { skill_id },
                            pulse: self.rng.random_range(0.0..std::f32::consts::TAU),
                        });
                    } else if self.rng.random_bool(0.10) {
                        self.spawn_spell_pickup(a.pos, from_boss);
                    }
                } else if self.rng.random_bool(0.10) {
                    self.spawn_spell_pickup(a.pos, from_boss);
                }
                // Projectile mark (MK) XP — every Payload currently slotted in the nucleus gains XP.
                let mut xp = if from_boss {
                    45.0 + self.room.combat_index as f32
                } else {
                    8.0 + self.room.combat_index as f32
                };
                if self.skills.has_active(SkillId::ScrapMetal) {
                    xp *= 1.5;
                }
                let glob_n = self.count_owned("glob_xp");
                if glob_n > 0 {
                    xp *= 1.0 + 0.10 * mod_strength(glob_n);
                }
                let killer = a.kill_source.clone();
                let killer_xp = a.kill_xp_bonus;
                let payload_ids = self.nucleus.filled_projectile_ids();
                for id in payload_ids {
                    let mut grant = xp;
                    if !killer.is_empty() && id == killer && killer_xp > 0.0 {
                        grant *= 1.0 + killer_xp;
                    }
                    let leveled = self.marks.progress(&id).add_xp(grant);
                    if leveled {
                        let mark = self.marks.get(&id).mark;
                        let name = self
                            .lib
                            .get(&id)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| id.clone());
                        self.journal(format!("{name} → {}", mark_label(mark)));
                    }
                }
                let corpse_id = self.alloc_id();
                self.corpses.push(Corpse {
                    id: corpse_id,
                    pos: a.pos,
                    glyph: a.glyph.to_ascii_lowercase(),
                    color: Color::DarkGray,
                    max_hp: a.max_hp,
                    from_boss,
                    life: 1.0,
                    max_life: 1.0,
                });
            }
        }
    }

    fn spawn_boss_splits(&mut self, parent: &Actor) {
        let left = parent.splits_left.saturating_sub(1);
        let hp = (parent.max_hp * 0.52).max(40.0);
        let radius = (parent.radius * 0.72).max(0.42);
        let glyph = if left == 0 { 'm' } else { 'W' };
        let color = if left == 0 {
            Color::Magenta
        } else {
            Color::LightMagenta
        };
        let offsets = [
            Vec2::new(-1.4, -0.7),
            Vec2::new(1.4, 0.7),
        ];
        for offset in offsets {
            let pos = self.room.snap_to_footing(parent.pos + offset, radius);
            let id = self.alloc_id();
            self.actors.push(Actor {
                id,
                kind: ActorKind::Boss,
                pos,
                vel: Vec2::ZERO,
                facing: parent.facing,
                hp,
                max_hp: hp,
                radius,
                glyph,
                color,
                formation_local: Vec2::ZERO,
                formation_index: 0,
                poison_timer: 0.0,
                poison_stacks: 0.0,
                fire_stacks: 0.0,
                vuln_bonus: 0.0,
                kill_gold_bonus: 0.0,
                kill_xp_bonus: 0.0,
                kill_source: String::new(),
                can_shoot: true,
                shoot_cd: 0.45,
                splits_left: left,
                boss_gen: parent.boss_gen.saturating_add(1),
                boss_form: BossForm::Classic,
                trail: Vec::new(),
                reanim_tier: 0,
                is_dummy: false,
            });
        }
    }

    fn spawn_column_rubble(&mut self, pos: Vec2) {
        for i in 0..8 {
            let a = i as f32 * 0.7;
            self.particles.push(Particle {
                pos,
                vel: Vec2::new(a.cos(), a.sin()) * (3.0 + (i % 3) as f32),
                life: 0.35 + (i % 3) as f32 * 0.08,
                max_life: 0.5,
                glyph: if i % 2 == 0 { '#' } else { '*' },
                color: Color::DarkGray,
            });
        }
    }

    fn apply_blast(
        &mut self,
        pos: Vec2,
        radius: f32,
        damage: f32,
        player_side: bool,
        friendly_fire: bool,
        poison: f32,
    ) {
        // Explosions from your army never hurt you.
        let hurt_player = self.invuln <= 0.0 && !player_side;
        let _ = friendly_fire;
        if hurt_player && self.player.pos.dist(pos) <= radius + self.player.radius {
            let dmg = damage * self.player_proj_vuln_mult();
            if self.try_absorb_hit() {
                // Guard absorbed.
            } else if let Some(idx) = self.shield_minion_idx() {
                self.actors[idx].hp -= dmg;
                if poison > 0.0 {
                    self.actors[idx].poison_timer = self.actors[idx].poison_timer.max(poison);
                }
            } else {
                self.player.hp -= dmg;
                if poison > 0.0 {
                    self.player.poison_timer = self.player.poison_timer.max(poison);
                }
                let amp = if self.boss_mods.contains(&BossMod::Tremor) {
                    0.4
                } else {
                    0.18
                };
                self.shake = self.shake.max(amp);
            }
        }
        let mut splash_hits: Vec<(Vec2, f32)> = Vec::new();
        for actor in &mut self.actors {
            // Never blast resurrected minions.
            if actor.kind == ActorKind::Minion {
                continue;
            }
            let is_enemy = matches!(actor.kind, ActorKind::Enemy | ActorKind::Boss);
            let can = if player_side {
                is_enemy
            } else {
                false
            };
            if can && actor.pos.dist(pos) <= radius + actor.radius {
                let taken = damage * (1.0 + actor.vuln_bonus);
                actor.hp -= taken;
                if actor.is_dummy {
                    actor.hp = actor.hp.max(1.0);
                }
                let hit_pos = actor.pos;
                if poison > 0.0 {
                    actor.poison_timer = actor.poison_timer.max(poison);
                }
                if player_side {
                    splash_hits.push((hit_pos, taken));
                }
            }
        }
        for (hit_pos, dmg) in splash_hits {
            self.push_damage_number(hit_pos, dmg, false);
        }
        // Weak splash also chips columns.
        if player_side && radius > 0.5 {
            let mut fallen = Vec::new();
            for col in &mut self.room.columns {
                if pos.dist(col.pos) <= radius + col.radius {
                    col.hp -= damage * 0.5;
                    if col.hp <= 0.0 {
                        fallen.push(col.pos);
                    }
                }
            }
            self.room.columns.retain(|c| c.hp > 0.0);
            for p in fallen {
                self.spawn_column_rubble(p);
            }
        }
    }

    fn spawn_chain(&mut self, from: Vec2, damage: f32, player_side: bool, remaining: u32) {
        let target = self
            .actors
            .iter()
            .filter(|a| {
                if player_side {
                    matches!(a.kind, ActorKind::Enemy | ActorKind::Boss)
                } else {
                    a.kind == ActorKind::Minion
                }
            })
            .filter(|a| a.pos.dist(from) > 0.4 && a.pos.dist(from) < 7.0)
            .min_by(|a, b| {
                a.pos
                    .dist(from)
                    .partial_cmp(&b.pos.dist(from))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|a| a.pos);
        let Some(t) = target else {
            return;
        };
        let dir = (t - from).normalized();
        let id = self.alloc_id();
        self.projectiles.push(Projectile {
            id,
            pos: from + dir * 0.5,
            vel: dir * 38.0,
            damage,
            radius: 0.3,
            explosion_radius: 0.0,
            lifetime: 0.45,
            max_lifetime: 0.45,
            age: 0.0,
            bounces: 0,
            pierce: 0,
            pierced: Vec::new(),
            homing: 1.2,
            poison: 0.0,
            chain: remaining,
            glyph: '/',
            color: Color::Cyan,
            owner_is_player_side: player_side,
            friendly_fire: false,
            chained: 1,
            trail: Vec::new(),
            returning: false,
            returned: false,
            trail_fx: false,
            trail_rainbow: false,
            trail_bright: 1.0,
            orbiting: false,
            orbit_angle: 0.0,
            orbit_radius: 0.0,
            gravity_well: false,
            arc: false,
            gated: false,
            gated_done: false,
            orbit_then_fire: false,
            orbit_launch_at: 0.7,
            homing_ramp: false,
            crit_bonus: 0.0,
            lock_target: None,
            is_flock: false,
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
spawn_offset: Vec2::ZERO,
        });
    }

    fn update_daisies(&mut self, dt: f32) {
        let mut blasts: Vec<(Vec2, f32, f32)> = Vec::new();
        let enemies: Vec<Vec2> = self
            .actors
            .iter()
            .filter(|a| matches!(a.kind, ActorKind::Enemy | ActorKind::Boss))
            .map(|a| a.pos)
            .collect();

        for daisy in &mut self.daisies {
            daisy.life -= dt;
            if let Some(target) = enemies
                .iter()
                .min_by(|a, b| {
                    daisy
                        .pos
                        .dist(**a)
                        .partial_cmp(&daisy.pos.dist(**b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
            {
                let dir = (target - daisy.pos).normalized();
                daisy.pos += dir * 2.4 * dt;
                daisy.pos = self.room.clamp(daisy.pos, 0.25);
                if daisy.pos.dist(target) < 0.7 {
                    daisy.life = 0.0;
                    blasts.push((daisy.pos, daisy.blast, daisy.damage));
                }
            }
        }
        self.daisies.retain(|d| d.life > 0.0);
        for (pos, radius, damage) in blasts {
            self.spawn_explosion(pos, radius, Color::LightYellow);
            self.record_damage(damage);
            self.apply_blast(pos, radius, damage, true, false, 0.0);
        }
    }

    fn update_poison(&mut self, dt: f32) {
        if self.player.poison_timer > 0.0 {
            self.player.poison_timer -= dt;
            self.player.hp -= 6.0 * dt;
        }
        if self.player.poison_stacks > 0.0 {
            self.player.hp -= self.player.poison_stacks * dt;
        }
        if self.player.fire_stacks > 0.0 {
            self.player.hp -= self.player.fire_stacks * dt;
        }
        let mut stack_dmg = 0.0f32;
        for a in &mut self.actors {
            if a.poison_timer > 0.0 {
                a.poison_timer -= dt;
                a.hp -= 6.0 * dt;
                stack_dmg += 6.0 * dt;
            }
            if a.poison_stacks > 0.0 {
                let d = a.poison_stacks * dt;
                a.hp -= d;
                if matches!(a.kind, ActorKind::Enemy | ActorKind::Boss) {
                    stack_dmg += d;
                }
            }
            if a.fire_stacks > 0.0 {
                let d = a.fire_stacks * dt;
                a.hp -= d;
                if matches!(a.kind, ActorKind::Enemy | ActorKind::Boss) {
                    stack_dmg += d;
                }
            }
            if a.is_dummy {
                a.hp = a.hp.max(1.0);
            }
        }
        if stack_dmg > 0.0 {
            self.record_damage(stack_dmg);
        }
    }

    fn spawn_spell_pickup(&mut self, pos: Vec2, from_boss: bool) {
        let spell = if from_boss {
            let mut id = self.lib.random_drop_id(&mut self.rng);
            for _ in 0..8 {
                let rarity = self
                    .lib
                    .get(&id)
                    .map(|s| s.rarity)
                    .unwrap_or(crate::proj_logic::Rarity::Common);
                if !matches!(
                    rarity,
                    crate::proj_logic::Rarity::Common | crate::proj_logic::Rarity::Uncommon
                ) {
                    break;
                }
                id = self.lib.random_drop_id(&mut self.rng);
            }
            id
        } else {
            self.lib.random_drop_id(&mut self.rng)
        };
        let rarity = self
            .lib
            .get(&spell)
            .map(|s| s.rarity)
            .unwrap_or(crate::proj_logic::Rarity::Common);
        let id = self.alloc_id();
        self.pickups.push(Pickup {
            id,
            pos,
            kind: PickupKind::Spell {
                spell_id: spell,
                rarity,
            },
            pulse: self.rng.random_range(0.0..std::f32::consts::TAU),
        });
    }

    fn pickup_spells(&mut self) {
        for p in &mut self.pickups {
            p.pulse += 0.12;
        }
        let mut got = Vec::new();
        self.pickups.retain(|p| {
            if self.player.pos.dist(p.pos) < 1.2 {
                got.push(p.kind.clone());
                false
            } else {
                true
            }
        });
        for kind in got {
            match kind {
                PickupKind::Spell { spell_id, rarity } => {
                    let name = self
                        .lib
                        .get(&spell_id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| spell_id.clone());
                    self.stash.push(spell_id);
                    self.journal_pickup(&name, rarity);
                    self.toast(format!("★ {} [{}] ★", name, rarity.label()));
                }
                PickupKind::Skill { skill_id } => {
                    if self.skills.unlock(skill_id) {
                        let def = crate::skills::def(skill_id);
                        self.journal(format!("skill {}", def.name));
                        self.toast(format!("✦ Skill: {} ✦", def.name));
                    } else {
                        self.toast("Already know that skill");
                    }
                }
                PickupKind::Temp(boost) => {
                    self.apply_temp_boost(boost);
                }
            }
        }
    }

    fn check_room_clear(&mut self) {
        if self.room.cleared || self.room.kind == RoomKind::Shop {
            return;
        }
        if self.enemy_count() == 0
            && self.pending_spawns.is_empty()
            && self.room.wave >= self.room.waves_total
        {
            self.room.cleared = true;
            self.room.doors_open = true;
            self.fleet_dash = false;
            self.toast("Room cleared — reach the door (Enter)");
        } else if self.enemy_count() == 0
            && self.pending_spawns.is_empty()
            && self.room.wave > 0
            && self.room.wave < self.room.waves_total
            && self.room.spawn_timer <= 0.0
        {
            // Arm inter-wave delay once — do not reset every frame (that blocked wave 1 forever).
            self.room.spawn_timer = 0.8;
        }
    }

    pub fn living_bosses(&self) -> impl Iterator<Item = &Actor> {
        self.actors.iter().filter(|a| a.kind == ActorKind::Boss)
    }

    /// Combined HP across all living boss fragments.
    pub fn boss_hp_totals(&self) -> Option<(f32, f32)> {
        let mut cur = 0.0;
        let mut max = 0.0;
        let mut any = false;
        for a in self.living_bosses() {
            any = true;
            cur += a.hp.max(0.0);
            max += a.max_hp.max(1.0);
        }
        any.then_some((cur, max))
    }

    pub fn dash_ready_ratio(&self) -> f32 {
        if self.dash_time > 0.0 {
            return 0.15;
        }
        if self.dash_cd <= 0.0 {
            return 1.0;
        }
        let max = self.dash_cd_max.max(0.01);
        (1.0 - self.dash_cd / max).clamp(0.0, 1.0)
    }

    pub fn shake_offset(&self) -> Vec2 {
        if self.shake <= 0.01 {
            return Vec2::ZERO;
        }
        let t = self.anim_t * 38.0;
        let m = self.shake;
        Vec2::new(t.sin() * m * 0.55, (t * 1.37).cos() * m * 0.4)
    }

    // Inventory helpers
    pub fn stash_pick_open(&self) -> bool {
        matches!(
            self.inv_overlay,
            InvOverlay::PickProjectile { .. } | InvOverlay::PickMod { .. }
        )
    }

    pub fn mod_menu_open(&self) -> bool {
        matches!(self.inv_overlay, InvOverlay::ModMenu { .. })
    }

    /// Rows in the mod-attach menu: capacity mod slots + Change projectile + Done.
    pub fn mod_menu_row_count(&self) -> usize {
        self.nucleus.mod_capacity + 2
    }

    /// Unique stash spells matching the active `stash_filter`, grouped by id.
    pub fn grouped_stash(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for id in &self.stash {
            let matches_filter = match self.lib.get(id).map(|s| s.kind) {
                Some(SpellKind::Payload) => self.stash_filter == StashFilter::Projectiles,
                Some(SpellKind::Modifier) | Some(SpellKind::Chaos) => {
                    self.stash_filter == StashFilter::Mods
                }
                None => true,
            };
            if !matches_filter {
                continue;
            }
            if let Some((_, n)) = counts.iter_mut().find(|(k, _)| k == id) {
                *n += 1;
            } else {
                counts.push((id.clone(), 1));
            }
        }
        counts.sort_by(|(a, _), (b, _)| {
            let da = self.lib.get(a);
            let db = self.lib.get(b);
            let ra = da.map(|s| s.rarity.sort_rank()).unwrap_or(0);
            let rb = db.map(|s| s.rarity.sort_rank()).unwrap_or(0);
            rb.cmp(&ra).then_with(|| {
                let na = da.map(|s| s.name.as_str()).unwrap_or(a.as_str());
                let nb = db.map(|s| s.name.as_str()).unwrap_or(b.as_str());
                na.cmp(nb)
            })
        });
        counts
    }

    pub fn inv_move(&mut self, delta: isize) {
        match self.inv_overlay {
            InvOverlay::PickProjectile { .. } | InvOverlay::PickMod { .. } => {
                let len = (1 + self.grouped_stash().len()) as isize;
                if len <= 0 {
                    return;
                }
                self.stash_cursor =
                    ((self.stash_cursor as isize + delta).rem_euclid(len)) as usize;
            }
            InvOverlay::ModMenu { .. } => {
                let len = self.mod_menu_row_count() as isize;
                if len == 0 {
                    return;
                }
                self.mod_menu_cursor =
                    ((self.mod_menu_cursor as isize + delta).rem_euclid(len)) as usize;
            }
            InvOverlay::None => match self.inv_focus {
                1 => self.skills.move_cursor(delta),
                _ => {
                    let len = self.nucleus.slot_count() as isize;
                    if len == 0 {
                        return;
                    }
                    self.inv_cursor =
                        ((self.inv_cursor as isize + delta).rem_euclid(len)) as usize;
                }
            },
        }
    }

    pub fn inv_toggle_focus(&mut self) {
        if self.inv_overlay != InvOverlay::None {
            return;
        }
        self.inv_focus = (self.inv_focus + 1) % 2;
    }

    /// Reorder projectile slots (or unused while picking).
    pub fn inv_swap_adjacent(&mut self, dir: isize) {
        if self.inv_overlay != InvOverlay::None {
            return;
        }
        if self.inv_focus != 0 {
            return;
        }
        let a = self.inv_cursor;
        let b = a as isize + dir;
        if b < 0 || b >= self.nucleus.slot_count() as isize {
            return;
        }
        self.nucleus.swap(a, b as usize);
        self.inv_cursor = b as usize;
    }

    /// Close the innermost overlay (stash pick → mod menu → none).
    pub fn inv_close_stash(&mut self) {
        match self.inv_overlay {
            InvOverlay::PickProjectile { slot } => {
                // Closing projectile pick without a choice: if slot still empty, stay closed.
                self.inv_overlay = if self
                    .nucleus
                    .slots
                    .get(slot)
                    .is_some_and(|s| s.projectile.is_some())
                {
                    InvOverlay::ModMenu { slot }
                } else {
                    InvOverlay::None
                };
                self.stash_cursor = 0;
            }
            InvOverlay::PickMod { slot, .. } => {
                self.inv_overlay = InvOverlay::ModMenu { slot };
                self.stash_cursor = 0;
                self.mod_menu_cursor = 0;
            }
            InvOverlay::ModMenu { .. } => {
                self.inv_overlay = InvOverlay::None;
                self.mod_menu_cursor = 0;
            }
            InvOverlay::None => {}
        }
    }

    pub fn inv_confirm(&mut self) {
        match self.inv_overlay {
            InvOverlay::PickProjectile { slot } => {
                self.apply_projectile_pick(slot);
            }
            InvOverlay::PickMod { slot, mod_idx } => {
                self.apply_mod_pick(slot, mod_idx);
            }
            InvOverlay::ModMenu { slot } => {
                self.confirm_mod_menu(slot);
            }
            InvOverlay::None => match self.inv_focus {
                1 => {
                    if self.skills.owned.is_empty() {
                        self.toast("No skills yet — find pickups");
                        return;
                    }
                    self.skills.toggle_at_cursor();
                    if let Some(id) = self.skills.owned.get(self.skills.cursor).copied() {
                        let def = crate::skills::def(id);
                        let on = self.skills.has_active(id);
                        self.toast(format!(
                            "{} {} · slots {}/{}",
                            def.name,
                            if on { "equipped" } else { "unequipped" },
                            self.skills.active.len(),
                            self.skills.max_active
                        ));
                    }
                }
                _ => {
                    let slot = self.inv_cursor;
                    if self
                        .nucleus
                        .slots
                        .get(slot)
                        .is_some_and(|s| s.projectile.is_some())
                    {
                        self.inv_overlay = InvOverlay::ModMenu { slot };
                        self.mod_menu_cursor = 0;
                    } else {
                        self.open_projectile_pick(slot);
                    }
                }
            },
        }
    }

    fn open_projectile_pick(&mut self, slot: usize) {
        self.stash_filter = StashFilter::Projectiles;
        self.stash_cursor = 0;
        self.inv_overlay = InvOverlay::PickProjectile { slot };
    }

    fn open_mod_pick(&mut self, slot: usize, mod_idx: usize) {
        self.stash_filter = StashFilter::Mods;
        self.stash_cursor = 0;
        self.inv_overlay = InvOverlay::PickMod { slot, mod_idx };
    }

    fn confirm_mod_menu(&mut self, slot: usize) {
        let cap = self.nucleus.mod_capacity;
        if self.mod_menu_cursor < cap {
            self.open_mod_pick(slot, self.mod_menu_cursor);
            return;
        }
        if self.mod_menu_cursor == cap {
            // Change projectile
            self.open_projectile_pick(slot);
            return;
        }
        // Done
        self.inv_overlay = InvOverlay::None;
        self.mod_menu_cursor = 0;
    }

    fn apply_projectile_pick(&mut self, slot: usize) {
        if self.stash_cursor == 0 {
            // [none] — clear projectile + return its mods to stash
            for m in self.nucleus.take_mods(slot) {
                self.stash.push(m);
            }
            if let Some(prev) = self.nucleus.clear_projectile(slot) {
                self.stash.push(prev);
            }
            self.inv_overlay = InvOverlay::None;
            self.stash_cursor = 0;
            return;
        }
        let groups = self.grouped_stash();
        let pick_i = self.stash_cursor - 1;
        let Some((spell_id, _)) = groups.get(pick_i).cloned() else {
            self.inv_overlay = InvOverlay::None;
            return;
        };
        if !matches!(
            self.lib.get(&spell_id).map(|d| d.kind),
            Some(SpellKind::Payload)
        ) {
            self.toast("Slots only accept projectiles");
            return;
        }
        let Some(stash_idx) = self.stash.iter().position(|s| s == &spell_id) else {
            self.inv_overlay = InvOverlay::None;
            return;
        };
        let spell = self.stash.remove(stash_idx);
        if let Some(displaced) = self.nucleus.set_projectile(slot, spell) {
            self.stash.push(displaced);
        }
        // Secondary menu: attach mods.
        self.inv_overlay = InvOverlay::ModMenu { slot };
        self.mod_menu_cursor = 0;
        self.stash_cursor = 0;
    }

    fn apply_mod_pick(&mut self, slot: usize, mod_idx: usize) {
        if self.stash_cursor == 0 {
            if let Some(prev) = self.nucleus.clear_mod(slot, mod_idx) {
                self.stash.push(prev);
            }
            self.inv_overlay = InvOverlay::ModMenu { slot };
            self.stash_cursor = 0;
            return;
        }
        let groups = self.grouped_stash();
        let pick_i = self.stash_cursor - 1;
        let Some((spell_id, _)) = groups.get(pick_i).cloned() else {
            self.inv_overlay = InvOverlay::ModMenu { slot };
            return;
        };
        if !matches!(
            self.lib.get(&spell_id).map(|d| d.kind),
            Some(SpellKind::Modifier) | Some(SpellKind::Chaos)
        ) {
            self.toast("Only mods can be attached");
            return;
        }
        let Some(stash_idx) = self.stash.iter().position(|s| s == &spell_id) else {
            self.inv_overlay = InvOverlay::ModMenu { slot };
            return;
        };
        let spell = self.stash.remove(stash_idx);
        // Ensure mods vec has room at mod_idx: clear/replace or attach.
        let attached = self
            .nucleus
            .slots
            .get(slot)
            .map(|s| s.mods.len())
            .unwrap_or(0);
        if mod_idx < attached {
            if let Some(displaced) = self.nucleus.set_mod(slot, mod_idx, spell) {
                self.stash.push(displaced);
            }
        } else if attached < self.nucleus.mod_capacity {
            if let Err(spell) = self.nucleus.attach_mod(slot, spell) {
                self.stash.push(spell);
                self.toast("Mod capacity full");
            }
        } else {
            self.stash.push(spell);
            self.toast("Mod capacity full");
        }
        self.inv_overlay = InvOverlay::ModMenu { slot };
        self.stash_cursor = 0;
    }
}
