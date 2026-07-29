use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn parse_seed(input: &str) -> u64 {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return random_seed_value();
    }
    if let Ok(n) = trimmed.parse::<u64>() {
        return n;
    }
    let mut hash: u64 = 1469598103934665603;
    for b in trimmed.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

pub fn random_seed_value() -> u64 {
    rand::rng().random()
}

pub fn random_seed_string() -> String {
    format!("{:08X}", random_seed_value() & 0xFFFF_FFFF)
}

pub fn rng_from_seed(seed: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed)
}
