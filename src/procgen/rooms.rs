use rand_chacha::ChaCha8Rng;

use crate::world::room::RoomState;

pub fn next_combat_room(combat_index: u32, rng: &mut ChaCha8Rng) -> RoomState {
    RoomState::new_combat(combat_index, rng)
}

pub fn shop_after_boss(combat_index: u32) -> RoomState {
    RoomState::new_shop(combat_index)
}

pub fn rooms_until_boss(combat_index: u32) -> u32 {
    if combat_index == 0 {
        return 3;
    }
    let rem = combat_index % 3;
    if rem == 0 {
        3
    } else {
        3 - rem
    }
}

