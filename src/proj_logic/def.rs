use std::collections::HashMap;

use rand::RngExt;
use ratatui::style::Color;
use serde::Deserialize;

pub type SpellId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SpellKind {
    Payload,
    Modifier,
    Chaos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
    Mythical,
}

impl Rarity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Rare => "Rare",
            Self::Legendary => "Legendary",
            Self::Mythical => "Mythical",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Common => Color::Gray,
            Self::Uncommon => Color::LightGreen,
            Self::Rare => Color::LightBlue,
            Self::Legendary => Color::Yellow,
            Self::Mythical => Color::LightMagenta,
        }
    }

    pub fn pickup_glyph(self) -> char {
        match self {
            Self::Common => '*',
            Self::Uncommon => '◆',
            Self::Rare => '◇',
            Self::Legendary => '★',
            Self::Mythical => '✦',
        }
    }

    /// Relative drop weights (sum = 975 ≈ 97.5%).
    /// Common 50% · Uncommon 30% · Rare 15% · Legendary 2% · Mythical 0.5%.
    pub fn drop_weight(self) -> u32 {
        match self {
            Self::Common => 500,
            Self::Uncommon => 300,
            Self::Rare => 150,
            Self::Legendary => 20,
            Self::Mythical => 5,
        }
    }

    /// Higher = rarer (for sort: rarest first).
    pub fn sort_rank(self) -> u8 {
        match self {
            Self::Common => 0,
            Self::Uncommon => 1,
            Self::Rare => 2,
            Self::Legendary => 3,
            Self::Mythical => 4,
        }
    }
}

impl Default for Rarity {
    fn default() -> Self {
        Self::Common
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SpellColor {
    Yellow,
    Red,
    Cyan,
    Green,
    White,
    Blue,
    Magenta,
    Gray,
    DarkGray,
    LightBlue,
    LightRed,
    LightMagenta,
    LightYellow,
    LightGreen,
}

impl SpellColor {
    #[allow(dead_code)]
    pub fn to_color(self) -> Color {
        match self {
            Self::Yellow => Color::Yellow,
            Self::Red => Color::Red,
            Self::Cyan => Color::Cyan,
            Self::Green => Color::Green,
            Self::White => Color::White,
            Self::Blue => Color::Blue,
            Self::Magenta => Color::Magenta,
            Self::Gray => Color::Gray,
            Self::DarkGray => Color::DarkGray,
            Self::LightBlue => Color::LightBlue,
            Self::LightRed => Color::LightRed,
            Self::LightMagenta => Color::LightMagenta,
            Self::LightYellow => Color::LightYellow,
            Self::LightGreen => Color::LightGreen,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SpellDef {
    pub id: SpellId,
    pub name: String,
    pub kind: SpellKind,
    #[serde(default)]
    pub rarity: Rarity,
    pub description: String,
    pub mana_cost: i32,
    #[serde(default = "default_glyph")]
    pub glyph: String,
    #[serde(default = "default_spell_color")]
    pub color: SpellColor,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub lifetime: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub explosion_radius: f32,
    #[serde(default)]
    pub spread: f32,
    #[serde(default)]
    pub bounces: u32,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default)]
    pub chain: u32,
    #[serde(default)]
    pub poison: f32,
    #[serde(default)]
    pub speed_mult: f32,
    #[serde(default)]
    pub lifetime_mult: f32,
    #[serde(default)]
    pub damage_mult: f32,
    #[serde(default)]
    pub radius_mult: f32,
    #[serde(default)]
    pub add_bounces: u32,
    #[serde(default)]
    pub add_spread: f32,
    #[serde(default)]
    pub add_count: u32,
    #[serde(default)]
    pub add_pierce: u32,
    #[serde(default)]
    pub pierce: u32,
    #[serde(default)]
    pub add_explosion: f32,
    #[serde(default)]
    pub remove_explosion: bool,
    #[serde(default)]
    pub homing: f32,
    #[serde(default)]
    pub echo: bool,
    #[serde(default)]
    pub wild: bool,
    #[serde(default)]
    pub orbit: bool,
    #[serde(default = "default_orbit_radius")]
    pub orbit_radius: f32,
    #[serde(default = "default_orbit_duration")]
    pub orbit_duration: f32,
    /// Behavior tag: chadical, droplet_storm, scythe, trail_fx, rainbow_trail,
    /// rain_bow, stone_column, orbit_next, daisymania, …
    #[serde(default)]
    pub special: String,
}

fn default_glyph() -> String {
    "?".into()
}

fn default_spell_color() -> SpellColor {
    SpellColor::White
}

fn one() -> u32 {
    1
}

fn default_orbit_radius() -> f32 {
    2.0
}

fn default_orbit_duration() -> f32 {
    12.0
}

#[derive(Debug, Clone)]
pub struct SpellLibrary {
    by_id: HashMap<SpellId, SpellDef>,
    order: Vec<SpellId>,
    by_rarity: HashMap<Rarity, Vec<SpellId>>,
}

impl SpellLibrary {
    pub fn load_embedded() -> Self {
        let raw = include_str!("../../data/spells.json");
        let defs: Vec<SpellDef> = serde_json::from_str(raw).expect("spells.json");
        let mut by_id = HashMap::new();
        let mut order = Vec::new();
        let mut by_rarity: HashMap<Rarity, Vec<SpellId>> = HashMap::new();
        for def in defs {
            by_rarity.entry(def.rarity).or_default().push(def.id.clone());
            order.push(def.id.clone());
            by_id.insert(def.id.clone(), def);
        }
        Self {
            by_id,
            order,
            by_rarity,
        }
    }

    pub fn get(&self, id: &str) -> Option<&SpellDef> {
        self.by_id.get(id)
    }

    pub fn random_drop_id(&self, rng: &mut impl rand::Rng) -> SpellId {
        let rarity = Self::roll_rarity(rng);
        if let Some(pool) = self.by_rarity.get(&rarity).filter(|p| !p.is_empty()) {
            let idx = rng.random_range(0..pool.len());
            return pool[idx].clone();
        }
        // Fallback if a tier is empty.
        let idx = rng.random_range(0..self.order.len());
        self.order[idx].clone()
    }

    pub fn roll_rarity(rng: &mut impl rand::Rng) -> Rarity {
        let tiers = [
            Rarity::Common,
            Rarity::Uncommon,
            Rarity::Rare,
            Rarity::Legendary,
            Rarity::Mythical,
        ];
        let total: u32 = tiers.iter().map(|r| r.drop_weight()).sum();
        let roll = rng.random_range(0..total.max(1));
        let mut acc = 0u32;
        for rarity in tiers {
            acc += rarity.drop_weight();
            if roll < acc {
                return rarity;
            }
        }
        Rarity::Common
    }
}
