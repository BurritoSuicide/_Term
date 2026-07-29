use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// Room-wide hazard modifiers rolled when entering combat rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelMod {
    /// Colored gas leaks that linger (heal / poison / lava).
    GasChamber,
    /// Bouncing wall patterns with dash gaps — play the center.
    BulletHell,
    /// Bison stampede from the top of the room.
    Stampede,
    /// Animated lava waveforms along the top and bottom edges.
    SomethingIsLava,
}

impl LevelMod {
    pub fn label(self) -> &'static str {
        match self {
            Self::GasChamber => "Gas Chamber",
            Self::BulletHell => "Bullet Hell",
            Self::Stampede => "Stampede!",
            Self::SomethingIsLava => "The Something Is Lava",
        }
    }
}

/// Roll 0–2 level mods for a combat room (never shop). Chance scales with depth.
pub fn roll_level_mods(combat_index: u32, is_boss: bool, rng: &mut ChaCha8Rng) -> Vec<LevelMod> {
    if combat_index <= 1 {
        return Vec::new();
    }
    let chance = if is_boss {
        0.55
    } else if combat_index >= 8 {
        0.48
    } else if combat_index >= 4 {
        0.36
    } else {
        0.22
    };
    if !rng.random_bool(chance) {
        return Vec::new();
    }

    let mut pool = vec![
        LevelMod::GasChamber,
        LevelMod::BulletHell,
        LevelMod::Stampede,
        LevelMod::SomethingIsLava,
    ];
    let want = if is_boss || combat_index >= 10 {
        1 + usize::from(rng.random_bool(0.35))
    } else {
        1
    };
    let mut out = Vec::with_capacity(want);
    for _ in 0..want {
        if pool.is_empty() {
            break;
        }
        let i = rng.random_range(0..pool.len());
        out.push(pool.swap_remove(i));
    }
    out
}

pub fn mods_label(mods: &[LevelMod]) -> String {
    if mods.is_empty() {
        "none".into()
    } else {
        mods.iter()
            .map(|m| m.label())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
