use prost::Message;
use stellar_light_client::{
    merkle::*,
    smt::{fold_siblings, key_index, leaf_hash, sha256, EMPTY, HASH_SIZE, TREE_DEPTH},
    ContractError,
};

fn build_membership_fixture(
    key: &[u8],
    value: &[u8],
    sibling_byte: u8,
) -> ([u8; HASH_SIZE], Vec<u8>) {
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [sibling_byte.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let key_hash = sha256(key);
    let value_hash = sha256(value);
    let leaf = leaf_hash(key_hash, value_hash);
    let root = fold_siblings(key_index(key), leaf, &siblings);
    let idx = key_index(key);
    let mut path = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = idx;

    for sibling in &siblings {
        let is_left_child = sub_idx & 1 == 0;

        path.push(if is_left_child {
            InnerOp {
                hash: 1,
                prefix: vec![0x01],
                suffix: sibling.to_vec(),
            }
        } else {
            let mut prefix = Vec::with_capacity(1 + HASH_SIZE);

            prefix.push(0x01);
            prefix.extend_from_slice(sibling);
            InnerOp {
                hash: 1,
                prefix,
                suffix: Vec::new(),
            }
        });
        sub_idx >>= 1;
    }

    let merkle = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: key.to_vec(),
                value: value.to_vec(),
                leaf: Some(LeafOp {
                    hash: 1,
                    prehash_key: 0,
                    prehash_value: 0,
                    length: 0,
                    prefix: Vec::new(),
                }),
                path,
            })),
        }],
    };

    (root, merkle.encode_to_vec())
}

fn build_non_membership_fixture(key: &[u8], sibling_byte: u8) -> ([u8; HASH_SIZE], Vec<u8>) {
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [sibling_byte.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let root = fold_siblings(key_index(key), EMPTY, &siblings);
    let idx = key_index(key);
    let mut path = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = idx;

    for sibling in &siblings {
        let is_left_child = sub_idx & 1 == 0;

        path.push(if is_left_child {
            InnerOp {
                hash: 1,
                prefix: vec![0x01],
                suffix: sibling.to_vec(),
            }
        } else {
            let mut prefix = Vec::with_capacity(1 + HASH_SIZE);

            prefix.push(0x01);
            prefix.extend_from_slice(sibling);
            InnerOp {
                hash: 1,
                prefix,
                suffix: Vec::new(),
            }
        });
        sub_idx >>= 1;
    }

    let merkle = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Nonexist(NonExistenceProof {
                key: key.to_vec(),
                left: Some(ExistenceProof {
                    key: key.to_vec(),
                    value: Vec::new(),
                    leaf: Some(LeafOp::default()),
                    path,
                }),
                right: None,
            })),
        }],
    };

    (root, merkle.encode_to_vec())
}

#[test]
fn membership_round_trip_recovers_siblings_and_value() {
    let key = b"10-stellar-0\x01\x00\x00\x00\x00\x00\x00\x00\x42";
    let value = b"committed-bytes";
    let (root, bytes) = build_membership_fixture(key, value, 0xA0);
    let (decoded_key, decoded_value, siblings) = decode_membership_proof(&bytes).expect("decode");

    assert_eq!(decoded_key, key);
    assert_eq!(decoded_value, value);
    assert_eq!(siblings.len(), TREE_DEPTH);
    assert!(stellar_light_client::smt::verify_membership_raw(
        &root, key, value, &siblings
    ));
}

#[test]
fn non_membership_round_trip_recovers_siblings() {
    let key = b"absent-key";
    let (root, bytes) = build_non_membership_fixture(key, 0xB0);
    let (decoded_key, siblings) = decode_non_membership_proof(&bytes).expect("decode");

    assert_eq!(decoded_key, key);
    assert_eq!(siblings.len(), TREE_DEPTH);
    assert!(stellar_light_client::smt::verify_non_membership_raw(
        &root, key, &siblings
    ));
}

#[test]
fn membership_proof_with_wrong_value_fails_smt_check() {
    let key = b"k";
    let value = b"v";
    let (root, bytes) = build_membership_fixture(key, value, 0xCC);
    let (decoded_key, _, siblings) = decode_membership_proof(&bytes).expect("decode");

    assert!(!stellar_light_client::smt::verify_membership_raw(
        &root,
        &decoded_key,
        b"wrong",
        &siblings
    ));
}

#[test]
fn malformed_inner_op_returns_error() {
    let bad = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: vec![1],
                value: vec![2],
                leaf: None,
                path: vec![InnerOp {
                    hash: 1,
                    prefix: vec![0xFF; 5],
                    suffix: vec![0xAA; 32],
                }],
            })),
        }],
    }
    .encode_to_vec();
    let err = decode_membership_proof(&bad).unwrap_err();

    assert!(matches!(err, ContractError::MerkleVerificationFailed));
}

#[test]
fn empty_proofs_vec_returns_error() {
    let bytes = MerkleProof { proofs: vec![] }.encode_to_vec();
    let err = decode_membership_proof(&bytes).unwrap_err();

    assert!(matches!(err, ContractError::MerkleVerificationFailed));
}

#[test]
fn membership_proof_decoded_as_non_membership_rejected() {
    let (_, bytes) = build_membership_fixture(b"k", b"v", 0xDD);
    let err = decode_non_membership_proof(&bytes).unwrap_err();

    assert!(matches!(err, ContractError::MerkleVerificationFailed));
}

#[test]
fn extract_sibling_left_child_picks_suffix() {
    let s = [0xAB; HASH_SIZE];
    let op = InnerOp {
        hash: 1,
        prefix: vec![0x01],
        suffix: s.to_vec(),
    };

    assert_eq!(extract_sibling(&op).unwrap(), s);
}

#[test]
fn extract_sibling_right_child_picks_prefix_tail() {
    let s = [0xCD; HASH_SIZE];
    let mut prefix = vec![0x01];

    prefix.extend_from_slice(&s);
    let op = InnerOp {
        hash: 1,
        prefix,
        suffix: Vec::new(),
    };

    assert_eq!(extract_sibling(&op).unwrap(), s);
}
