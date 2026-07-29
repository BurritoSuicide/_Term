use rand::RngExt;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossMod {
    /// Top/bottom walls fire vertical volleys while the boss lives.
    WallVolley,
    /// Periodic screen tremor + heavier hit shake.
    Tremor,
    /// On death, splits into two smaller bosses — twice.
    Multiply,
}

impl BossMod {
    pub fn label(self) -> &'static str {
        match self {
            Self::WallVolley => "Wallfire",
            Self::Tremor => "Tremor",
            Self::Multiply => "Multiply",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::WallVolley => "Wall",
            Self::Tremor => "Shake",
            Self::Multiply => "×",
        }
    }
}

pub fn roll_boss_mods(combat_index: u32, rng: &mut ChaCha8Rng) -> Vec<BossMod> {
    let mut pool = vec![
        BossMod::WallVolley,
        BossMod::Tremor,
        BossMod::Multiply,
    ];
    // Deeper bosses stack more modifiers.
    let want = if combat_index >= 12 {
        3
    } else if combat_index >= 9 {
        2
    } else if combat_index >= 6 {
        1 + usize::from(rng.random_bool(0.55))
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

pub fn mods_label(mods: &[BossMod]) -> String {
    if mods.is_empty() {
        "none".into()
    } else {
        mods.iter()
            .map(|m| m.label())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Soft-exponential boss HP at generation 0.
pub fn scaled_boss_hp(combat_index: u32) -> f32 {
    let idx = combat_index.max(1) as f32;
    200.0 + (1.48_f32).powf(idx) * 32.0
}

pub fn boss_damage_scale(combat_index: u32) -> f32 {
    let idx = combat_index.max(1) as f32;
    1.0 + (idx - 3.0).max(0.0) * 0.11 + (1.12_f32).powf(idx * 0.35) * 0.08
}
