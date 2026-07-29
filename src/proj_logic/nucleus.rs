use super::{SpellId, SpellLibrary};

/// One nucleus fire-slot: a projectile plus attached mods (up to `Nucleus::mod_capacity`).
#[derive(Debug, Clone, Default)]
pub struct NucleusSlot {
    pub projectile: Option<SpellId>,
    pub mods: Vec<SpellId>,
}

impl NucleusSlot {
    pub fn has_projectile(&self) -> bool {
        self.projectile.is_some()
    }
}

/// Programmable Nucleus: ordered projectile slots with per-slot mod attachments.
#[derive(Debug, Clone)]
pub struct Nucleus {
    pub slots: Vec<NucleusSlot>,
    /// How many mods may be attached to each projectile.
    pub mod_capacity: usize,
}

impl Nucleus {
    pub fn new(slot_count: usize, starter_projectile: Option<SpellId>, mod_capacity: usize) -> Self {
        let mut slots = vec![NucleusSlot::default(); slot_count.max(1)];
        if let Some(id) = starter_projectile {
            slots[0].projectile = Some(id);
        }
        Self {
            slots,
            mod_capacity: mod_capacity.max(1),
        }
    }

    pub fn starter(lib: &SpellLibrary) -> Self {
        let _ = lib;
        // One slot, P-Cannon, one mod attachment allowed.
        Self::new(1, Some("p_cannon".into()), 1)
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn occupied_projectiles(&self) -> usize {
        self.slots.iter().filter(|s| s.has_projectile()).count()
    }

    /// +1 empty projectile slot (boss reward).
    pub fn expand_slot(&mut self) {
        self.slots.push(NucleusSlot::default());
    }

    /// +1 mod attachment per projectile (every 5 bosses).
    pub fn expand_mod_capacity(&mut self) {
        self.mod_capacity += 1;
    }

    pub fn filled_projectile_ids(&self) -> Vec<SpellId> {
        self.slots
            .iter()
            .filter_map(|s| s.projectile.clone())
            .collect()
    }

    /// All spell ids currently equipped (projectiles + attached mods).
    pub fn filled_ids(&self) -> Vec<SpellId> {
        let mut out = Vec::new();
        for slot in &self.slots {
            if let Some(p) = &slot.projectile {
                out.push(p.clone());
            }
            out.extend(slot.mods.iter().cloned());
        }
        out
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() {
            self.slots.swap(a, b);
        }
    }

    pub fn set_projectile(&mut self, slot: usize, spell: SpellId) -> Option<SpellId> {
        let Some(s) = self.slots.get_mut(slot) else {
            return Some(spell);
        };
        s.projectile.replace(spell)
    }

    pub fn clear_projectile(&mut self, slot: usize) -> Option<SpellId> {
        self.slots.get_mut(slot)?.projectile.take()
    }

    /// Detach all mods from a slot (caller should return them to stash).
    pub fn take_mods(&mut self, slot: usize) -> Vec<SpellId> {
        self.slots
            .get_mut(slot)
            .map(|s| std::mem::take(&mut s.mods))
            .unwrap_or_default()
    }

    pub fn set_mod(&mut self, slot: usize, mod_idx: usize, spell: SpellId) -> Option<SpellId> {
        let Some(s) = self.slots.get_mut(slot) else {
            return Some(spell);
        };
        if mod_idx >= self.mod_capacity {
            return Some(spell);
        }
        while s.mods.len() <= mod_idx {
            // Shouldn't place into sparse holes — use attach_mod instead.
            break;
        }
        if mod_idx < s.mods.len() {
            Some(std::mem::replace(&mut s.mods[mod_idx], spell))
        } else if s.mods.len() < self.mod_capacity {
            s.mods.push(spell);
            None
        } else {
            Some(spell)
        }
    }

    pub fn clear_mod(&mut self, slot: usize, mod_idx: usize) -> Option<SpellId> {
        let s = self.slots.get_mut(slot)?;
        if mod_idx >= s.mods.len() {
            return None;
        }
        Some(s.mods.remove(mod_idx))
    }

    pub fn attach_mod(&mut self, slot: usize, spell: SpellId) -> Result<(), SpellId> {
        let Some(s) = self.slots.get_mut(slot) else {
            return Err(spell);
        };
        if s.mods.len() >= self.mod_capacity {
            return Err(spell);
        }
        s.mods.push(spell);
        Ok(())
    }
}

