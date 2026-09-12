use sha2::{Digest, Sha256};

pub const HASH_SIZE: usize = 32;
pub const TREE_DEPTH: usize = 64;
pub const EMPTY: [u8; HASH_SIZE] = [0u8; HASH_SIZE];

pub fn key_index(key: &[u8]) -> u64 {
    let h = sha256(key);

    u64::from_be_bytes(h[..8].try_into().expect("sha256 has 32 bytes"))
}

pub fn leaf_hash(key_hash: [u8; HASH_SIZE], value_hash: [u8; HASH_SIZE]) -> [u8; HASH_SIZE] {
    let mut h = Sha256::new();

    h.update([0x00]);
    h.update(key_hash);
    h.update(value_hash);
    h.finalize().into()
}

pub fn inner_hash(left: [u8; HASH_SIZE], right: [u8; HASH_SIZE]) -> [u8; HASH_SIZE] {
    if left == EMPTY && right == EMPTY {
        return EMPTY;
    }

    let mut h = Sha256::new();

    h.update([0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

pub fn sha256(data: &[u8]) -> [u8; HASH_SIZE] {
    Sha256::digest(data).into()
}

pub fn fold_siblings(
    idx: u64,
    leaf: [u8; HASH_SIZE],
    siblings: &[[u8; HASH_SIZE]],
) -> [u8; HASH_SIZE] {
    let mut current = leaf;
    let mut sub_idx = idx;

    for sibling in siblings {
        current = if sub_idx & 1 == 0 {
            inner_hash(current, *sibling)
        } else {
            inner_hash(*sibling, current)
        };

        sub_idx >>= 1;
    }

    current
}

pub fn verify_membership_raw(
    root: &[u8; HASH_SIZE],
    key: &[u8],
    value: &[u8],
    siblings: &[[u8; HASH_SIZE]],
) -> bool {
    if value.is_empty() || siblings.len() != TREE_DEPTH {
        return false;
    }

    let key_hash = sha256(key);
    let value_hash = sha256(value);
    let idx = key_index(key);

    fold_siblings(idx, leaf_hash(key_hash, value_hash), siblings) == *root
}

pub fn verify_non_membership_raw(
    root: &[u8; HASH_SIZE],
    key: &[u8],
    siblings: &[[u8; HASH_SIZE]],
) -> bool {
    if siblings.len() != TREE_DEPTH {
        return false;
    }

    let idx = key_index(key);

    fold_siblings(idx, EMPTY, siblings) == *root
}
