//! Projectile / Mod mark (MK) progression.

use std::collections::HashMap;

use super::SpellId;

#[derive(Debug, Clone)]
pub struct MarkProgress {
    pub mark: u32,
    pub xp: f32,
    pub xp_to_next: f32,
}

impl Default for MarkProgress {
    fn default() -> Self {
        Self {
            mark: 1,
            xp: 0.0,
            xp_to_next: 40.0,
        }
    }
}

impl MarkProgress {
    pub fn xp_needed(mark: u32) -> f32 {
        40.0 * 1.4f32.powi(mark.saturating_sub(1) as i32)
    }

    /// Returns true if a mark-up occurred.
    pub fn add_xp(&mut self, amount: f32) -> bool {
        if amount <= 0.0 {
            return false;
        }
        self.xp += amount;
        let mut leveled = false;
        while self.xp >= self.xp_to_next {
            self.xp -= self.xp_to_next;
            self.mark += 1;
            self.xp_to_next = Self::xp_needed(self.mark);
            leveled = true;
        }
        leveled
    }

    /// Damage multiplier from projectile marks: +20% per mark above 1.
    pub fn damage_mult(&self) -> f32 {
        1.0 + 0.20 * (self.mark.saturating_sub(1) as f32)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkBook {
    pub by_id: HashMap<SpellId, MarkProgress>,
}

impl MarkBook {
    pub fn progress(&mut self, id: &str) -> &mut MarkProgress {
        self.by_id
            .entry(id.to_string())
            .or_insert_with(MarkProgress::default)
    }

    pub fn get(&self, id: &str) -> MarkProgress {
        self.by_id.get(id).cloned().unwrap_or_default()
    }
}

/// Mod MK label from total copies owned (nucleus + stash). Display only.
/// 1 copy → MK1; 6 copies → MK2; 11 → MK3; …
pub fn mod_mark_from_count(total: usize) -> u32 {
    if total == 0 {
        1
    } else {
        1 + ((total - 1) / 5) as u32
    }
}

/// Effect scale for stacked mods.
/// 1 copy → 1.0 (applies the 10% base on the mod def);
/// each extra owned copy → +0.5 scale (= +5% absolute when base is 10%).
/// Stacks infinitely: n copies → 1.0 + 0.5*(n-1).
pub fn mod_strength(owned_copies: usize) -> f32 {
    let n = owned_copies.max(1) as f32;
    1.0 + 0.5 * (n - 1.0)
}

/// Animated stash color by mark level.
/// MK1 normal → blue@10 → yellow@20 → red@50 → purple@100.
pub fn mark_color(mark: u32, anim_t: f32) -> ratatui::style::Color {
    use ratatui::style::Color;
    let m = mark as f32;
    let (r0, g0, b0, r1, g1, b1, t) = if m <= 1.0 {
        (200, 200, 200, 200, 200, 200, 0.0)
    } else if m < 10.0 {
        let t = (m - 1.0) / 9.0;
        (200, 200, 200, 80, 160, 255, t)
    } else if m < 20.0 {
        let t = (m - 10.0) / 10.0;
        (80, 160, 255, 240, 210, 60, t)
    } else if m < 50.0 {
        let t = (m - 20.0) / 30.0;
        (240, 210, 60, 255, 70, 70, t)
    } else {
        let t = ((m - 50.0) / 50.0).min(1.0);
        (255, 70, 70, 190, 90, 255, t)
    };
    let pulse = 0.85 + 0.15 * (anim_t * 2.4 + mark as f32 * 0.3).sin();
    let lerp = |a: u8, b: u8| ((a as f32 + (b as f32 - a as f32) * t) * pulse).clamp(0.0, 255.0) as u8;
    Color::Rgb(lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
}

/// Format a mark level as `MKI`, `MKII`, `MKIV`, …
pub fn mark_label(mark: u32) -> String {
    format!("MK{}", to_roman(mark.max(1)))
}

fn to_roman(mut n: u32) -> String {
    if n == 0 {
        return "N".into();
    }
    n = n.min(3999);
    const TABLE: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for &(v, s) in TABLE {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}
