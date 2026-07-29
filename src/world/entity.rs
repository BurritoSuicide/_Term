use ratatui::style::Color;

pub type EntityId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            Self::ZERO
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }

    pub fn dist(self, other: Self) -> f32 {
        (self - other).length()
    }

    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn from_angle(angle: f32) -> Self {
        Self {
            x: angle.cos(),
            y: angle.sin(),
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Player,
    Minion,
    Enemy,
    Boss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BossForm {
    #[default]
    None,
    Classic,
    SnakeHead,
    SnakeBody,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Actor {
    pub id: EntityId,
    pub kind: ActorKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub radius: f32,
    pub glyph: char,
    pub color: Color,
    /// Local-space rear slot; rotated by aim each frame.
    pub formation_local: Vec2,
    pub formation_index: usize,
    pub poison_timer: f32,
    /// Stacking poison DoT (1 DPS/stack); cleared when the room ends.
    pub poison_stacks: f32,
    /// Stacking fire DoT (1 DPS/stack); cleared when the room ends.
    pub fire_stacks: f32,
    /// Extra damage taken from all sources (0.05 = +5%).
    pub vuln_bonus: f32,
    /// Kill attribution from the last player hit (gold / XP / source).
    pub kill_gold_bonus: f32,
    pub kill_xp_bonus: f32,
    pub kill_source: String,
    pub can_shoot: bool,
    pub shoot_cd: f32,
    /// Boss split generations remaining (2 → splits twice). Non-bosses: 0.
    pub splits_left: u8,
    /// 0 = original boss, increases each split.
    pub boss_gen: u8,
    pub boss_form: BossForm,
    /// Snake head trail of recent positions for body follow.
    pub trail: Vec<Vec2>,
    /// Re-animation merge tier (0 = fresh raise).
    pub reanim_tier: u8,
    /// Shop target dummy — no loot, regenerates, ignored for room clear.
    pub is_dummy: bool,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub pos: Vec2,
    pub radius: f32,
    pub hp: f32,
    pub max_hp: f32,
}

/// Walking flower from Daisymania — drifts toward enemies and explodes.
#[derive(Debug, Clone)]
pub struct Daisy {
    pub pos: Vec2,
    pub life: f32,
    pub damage: f32,
    pub blast: f32,
}

#[derive(Debug, Clone)]
pub struct Corpse {
    pub id: EntityId,
    pub pos: Vec2,
    pub glyph: char,
    pub color: Color,
    pub max_hp: f32,
    pub from_boss: bool,
    /// Seconds remaining before the body fades into the floor.
    pub life: f32,
    pub max_life: f32,
}

/// One line in the journalctl panel.
#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub t: f32,
    pub text: String,
    /// Inventory pickup — body pulses with rarity color; "pickup" fades yellow.
    pub pickup_rarity: Option<crate::proj_logic::Rarity>,
}

/// Timed floor powerups that spawn during combat rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempBoost {
    /// Nucleus fires at 50% interval for 5 seconds.
    Overclock,
    /// Dash: −80% CD, 2× distance & speed until the room is cleared.
    FleetDash,
    /// Double projectiles from all sources for 20 seconds.
    TwinVolley,
    /// Absorb the next hit that would damage the player.
    Guard,
    /// Player projectiles gain strong homing until the room is cleared.
    Homing,
}

impl TempBoost {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overclock => "Overclock",
            Self::FleetDash => "Fleet Dash",
            Self::TwinVolley => "Twin Volley",
            Self::Guard => "Guard",
            Self::Homing => "Homing",
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Self::Overclock => '!',
            Self::FleetDash => '»',
            Self::TwinVolley => '‡',
            Self::Guard => '◈',
            Self::Homing => '◎',
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Overclock => Color::LightYellow,
            Self::FleetDash => Color::Cyan,
            Self::TwinVolley => Color::LightMagenta,
            Self::Guard => Color::LightBlue,
            Self::Homing => Color::LightGreen,
        }
    }

    pub fn all() -> [TempBoost; 5] {
        [
            Self::Overclock,
            Self::FleetDash,
            Self::TwinVolley,
            Self::Guard,
            Self::Homing,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum PickupKind {
    Spell {
        spell_id: String,
        rarity: crate::proj_logic::Rarity,
    },
    Skill {
        skill_id: crate::skills::SkillId,
    },
    Temp(TempBoost),
}

#[derive(Debug, Clone)]
pub enum ShopOffer {
    Spell(String),
    Skill(crate::skills::SkillId),
    Credit,
    Heal,
    SkillSlot,
}

#[derive(Debug, Clone)]
pub struct ShopTotem {
    pub pos: Vec2,
    pub offer: ShopOffer,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Pickup {
    pub id: EntityId,
    pub pos: Vec2,
    pub kind: PickupKind,
    pub pulse: f32,
}

#[derive(Debug, Clone)]
pub struct OrbitBlade {
    pub owner_id: EntityId,
    pub angle: f32,
    pub orbit_radius: f32,
    pub damage: f32,
    pub lifetime: f32,
    pub pos: Vec2,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Projectile {
    pub id: EntityId,
    pub pos: Vec2,
    pub vel: Vec2,
    pub damage: f32,
    pub radius: f32,
    pub explosion_radius: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub age: f32,
    pub bounces: u32,
    pub pierce: u32,
    pub pierced: Vec<EntityId>,
    pub homing: f32,
    pub poison: f32,
    pub chain: u32,
    pub glyph: char,
    pub color: Color,
    pub owner_is_player_side: bool,
    pub friendly_fire: bool,
    pub chained: u32,
    pub trail: Vec<Vec2>,
    pub returning: bool,
    pub returned: bool,
    pub trail_fx: bool,
    pub trail_rainbow: bool,
    pub trail_bright: f32,
    pub orbiting: bool,
    pub orbit_angle: f32,
    pub orbit_radius: f32,
    /// Pulls nearby projectiles toward this bolt.
    pub gravity_well: bool,
    /// Parabola rock — gravity applied each frame.
    pub arc: bool,
    /// Gated mod — teleport above a foe after 0.2s.
    pub gated: bool,
    pub gated_done: bool,
    /// Thorn orbit then launch.
    pub orbit_then_fire: bool,
    pub orbit_launch_at: f32,
    /// Plague wasps — homing strengthens with age.
    pub homing_ramp: bool,
    pub crit_bonus: f32,
    /// Lock onto a specific actor (Taze random target).
    pub lock_target: Option<EntityId>,
    /// Flock bird — orbit until it deflects a shot, then dive.
    pub is_flock: bool,
    pub source_id: String,
    /// Extra gold on kill (0.2 = +20%).
    pub gold_bonus: f32,
    /// Extra MK XP for this projectile's mark on kill (0.3 = +30%).
    pub xp_bonus: f32,
    /// Poison stacks applied on hit (full).
    pub poison_stacks: f32,
    /// Fire stacks on projectile; 30% applied on hit.
    pub fire_stacks: f32,
    /// Vuln bonus applied on hit (0.05 = +5% damage taken).
    pub vuln_bonus: f32,
    /// Soft green/red glow around the bolt.
    pub glow_green: bool,
    pub glow_red: bool,
    /// Bright halo ring (vuln).
    pub glow_halo: bool,
    /// Spawn at angle × spawn_radius from origin (splash rings).
    pub ring_spawn: bool,
    pub spawn_radius: f32,
    /// Absolute spawn offset from origin (link lines); if nonzero, overrides ring.
    pub spawn_offset: Vec2,
}

#[derive(Debug, Clone)]
pub struct ExplosionFx {
    pub pos: Vec2,
    pub radius: f32,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max_life: f32,
    pub glyph: char,
    pub color: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct Torch {
    pub pos: Vec2,
}

#[derive(Debug, Clone)]
pub struct PendingShot {
    pub delay: f32,
    pub origin: Vec2,
    pub facing: Vec2,
    pub shot: crate::proj_logic::PlannedShot,
    pub owner_is_player_side: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasKind {
    /// Red — heals entities in the cloud.
    Heal,
    /// Green — applies poison.
    Poison,
    /// Orange — heavy fire damage.
    Lava,
}

impl GasKind {
    pub fn color(self) -> Color {
        match self {
            Self::Heal => Color::LightRed,
            Self::Poison => Color::LightGreen,
            Self::Lava => Color::Rgb(255, 140, 40),
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Self::Heal => '≈',
            Self::Poison => '~',
            Self::Lava => '░',
        }
    }
}

#[derive(Debug, Clone)]
pub struct GasCloud {
    pub pos: Vec2,
    pub radius: f32,
    pub kind: GasKind,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Debug, Clone)]
pub struct StampedeBeast {
    pub pos: Vec2,
    pub vel: Vec2,
    pub damage: f32,
    pub radius: f32,
    pub life: f32,
    /// Entities already trampled this charge.
    pub hit: Vec<EntityId>,
}

#[derive(Debug, Clone)]
pub struct DamageNumber {
    pub pos: Vec2,
    pub amount: f32,
    pub life: f32,
    pub max_life: f32,
    pub crit: bool,
}
