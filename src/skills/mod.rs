use rand::RngExt;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillId {
    MythicPulse,
    MomentumDash,
    PhaseStep,
    BlastDash,
    ManaSiphon,
    IronWill,
    GraveBond,
    DataCharm,
    FleetBoots,
    ScrapMetal,
    StarterChest,
}

#[derive(Debug, Clone, Copy)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: &'static str,
    pub description: &'static str,
    pub glyph: char,
    pub color: Color,
}

pub const SKILL_DEFS: &[SkillDef] = &[
    SkillDef {
        id: SkillId::MythicPulse,
        name: "Mythic Pulse",
        description: "Nucleus fires 15% faster. Stack with T_Happy for rapid MK farming.",
        glyph: '✦',
        color: Color::LightMagenta,
    },
    SkillDef {
        id: SkillId::MomentumDash,
        name: "Momentum Dash",
        description: "Dash cooldown reduced by 40%. Chain evasions in dense rooms.",
        glyph: '»',
        color: Color::LightCyan,
    },
    SkillDef {
        id: SkillId::PhaseStep,
        name: "Phase Step",
        description: "Brief invulnerability while dashing. Slip through bullet hell.",
        glyph: '◌',
        color: Color::LightBlue,
    },
    SkillDef {
        id: SkillId::BlastDash,
        name: "Blast Dash",
        description: "Dashing through enemies deals burst damage.",
        glyph: '↯',
        color: Color::LightYellow,
    },
    SkillDef {
        id: SkillId::ManaSiphon,
        name: "Mana Siphon",
        description: "Recover a little HP whenever an enemy dies.",
        glyph: '✧',
        color: Color::Cyan,
    },
    SkillDef {
        id: SkillId::IronWill,
        name: "Iron Will",
        description: "Take 35% less contact damage from enemies.",
        glyph: '▣',
        color: Color::Gray,
    },
    SkillDef {
        id: SkillId::GraveBond,
        name: "Grave Bond",
        description: "Minions inherit +25% max HP when raised.",
        glyph: '†',
        color: Color::LightGreen,
    },
    SkillDef {
        id: SkillId::DataCharm,
        name: "Data Charm",
        description: "Reduces Nucleus fire interval by 50%.",
        glyph: '⚡',
        color: Color::LightCyan,
    },
    SkillDef {
        id: SkillId::FleetBoots,
        name: "Fleet Boots",
        description: "Move 15% faster.",
        glyph: '≫',
        color: Color::LightGreen,
    },
    SkillDef {
        id: SkillId::ScrapMetal,
        name: "Scrap_Metal",
        description: "Projectile mark XP gain increased by 50%.",
        glyph: '⚙',
        color: Color::Yellow,
    },
    SkillDef {
        id: SkillId::StarterChest,
        name: "_Starter_Chest",
        description: "Temp pickups spawn 2× faster; one appears in front of you each room.",
        glyph: '▣',
        color: Color::LightYellow,
    },
];

/// Future skill ideas (design backlog, not coded yet):
/// - Mirror Veil: first hit each room is negated
/// - Blood Price: spend HP instead of mana when low
/// - Necro Magnet: corpses drift toward you
/// - Overcharge: every 5th cast deals double
/// - Frost Trail: dash leaves slowing frost
/// - Soul Radar: briefly reveal shooters/elites
/// - Second Wind: once per shop cycle, survive lethal at 1 HP
pub fn def(id: SkillId) -> &'static SkillDef {
    SKILL_DEFS
        .iter()
        .find(|s| s.id == id)
        .expect("skill def")
}

pub fn random_skill_id(rng: &mut impl rand::Rng) -> SkillId {
    SKILL_DEFS[rng.random_range(0..SKILL_DEFS.len())].id
}

#[derive(Debug, Clone)]
pub struct SkillLoadout {
    pub owned: Vec<SkillId>,
    pub active: Vec<SkillId>,
    pub cursor: usize,
    /// How many skills may be equipped at once (starts at 1).
    pub max_active: usize,
    /// Gold cost of the next slot upgrade (starts at 1000, grows exponentially).
    pub slot_upgrade_cost: i32,
}

impl SkillLoadout {
    pub fn new_empty() -> Self {
        Self {
            owned: Vec::new(),
            active: Vec::new(),
            cursor: 0,
            max_active: 1,
            slot_upgrade_cost: 1000,
        }
    }

    pub fn has_active(&self, id: SkillId) -> bool {
        self.active.contains(&id)
    }

    pub fn unlock(&mut self, id: SkillId) -> bool {
        if self.owned.contains(&id) {
            return false;
        }
        self.owned.push(id);
        true
    }

    pub fn buy_slot_upgrade(&mut self) -> i32 {
        let paid = self.slot_upgrade_cost;
        self.max_active += 1;
        // Exponential: 1000 → 2000 → 4000 → …
        self.slot_upgrade_cost = self.slot_upgrade_cost.saturating_mul(2);
        paid
    }

    /// Toggle equip. Respects max_active — equipping at capacity replaces the oldest active.
    pub fn toggle_owned(&mut self, id: SkillId) {
        if !self.owned.contains(&id) {
            return;
        }
        if let Some(i) = self.active.iter().position(|s| *s == id) {
            self.active.remove(i);
            return;
        }
        while self.active.len() >= self.max_active {
            self.active.remove(0);
        }
        self.active.push(id);
    }

    pub fn toggle_at_cursor(&mut self) {
        if let Some(id) = self.owned.get(self.cursor).copied() {
            self.toggle_owned(id);
        }
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.owned.is_empty() {
            return;
        }
        let len = self.owned.len() as isize;
        self.cursor = ((self.cursor as isize + delta).rem_euclid(len)) as usize;
    }
}
