use crate::world::entity::Vec2;

/// Local-space slot: +x is forward (aim), +y is right.
/// Compact cluster — raised corpses stay tight on the player.
pub fn formation_local(index: usize) -> Vec2 {
    const SLOTS: &[(f32, f32)] = &[
        (-0.52, -0.62),
        (-0.52, 0.62),
        (-0.88, -0.38),
        (-0.88, 0.38),
        (-0.52, -0.98),
        (-0.52, 0.98),
        (-1.18, -0.62),
        (-1.18, 0.62),
        (-0.88, -0.98),
        (-0.88, 0.98),
        (-1.18, -1.0),
        (-1.18, 1.0),
    ];
    let (back, lateral) = SLOTS[index % SLOTS.len()];
    let ring = (index / SLOTS.len()) as f32;
    Vec2::new(back - ring * 0.4, lateral)
}

/// Rotate a local formation offset into world space using the player's aim.
pub fn world_offset(local: Vec2, facing: Vec2) -> Vec2 {
    let forward = facing.normalized();
    let right = Vec2::new(-forward.y, forward.x);
    forward * local.x + right * local.y
}
