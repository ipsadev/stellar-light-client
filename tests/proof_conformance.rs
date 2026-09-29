use ipsa_ibc_core::{
    proof::{serialize_membership_proof, serialize_non_membership_proof},
    smt::{MembershipProof, NonMembershipProof, Smt},
};
use sha2::{Digest, Sha256};
use stellar_light_client::{
    merkle::{decode_membership_proof, decode_non_membership_proof},
    smt::{verify_membership_raw, verify_non_membership_raw, TREE_DEPTH},
};

fn populated() -> Smt {
    let mut tree = Smt::new();

    for position in 0u8..16 {
        tree.insert(&[b'k', position], &[b'v', position]);
    }

    tree
}

fn contract_accepts_membership(
    root: &[u8; 32],
    serialized: &[u8],
    key: &[u8],
    value: &[u8],
) -> bool {
    let (proof_key, proof_value, siblings) =
        decode_membership_proof(serialized).expect("the contract decodes");
    let value_hash: [u8; 32] = Sha256::digest(value).into();

    proof_key == key
        && proof_value == value_hash
        && verify_membership_raw(root, key, value, &siblings)
}

fn contract_accepts_non_membership(root: &[u8; 32], serialized: &[u8], key: &[u8]) -> bool {
    let (proof_key, siblings) =
        decode_non_membership_proof(serialized).expect("the contract decodes");

    proof_key == key && verify_non_membership_raw(root, key, &siblings)
}

#[test]
fn a_membership_proof_the_core_builds_is_one_the_contract_accepts() {
    let tree = populated();
    let root = tree.root();

    for position in 0u8..16 {
        let key = [b'k', position];
        let proof = tree
            .generate_membership_proof(&key)
            .expect("the key is in the tree");

        assert!(
            contract_accepts_membership(
                &root,
                &serialize_membership_proof(&proof),
                &key,
                &[b'v', position]
            ),
            "the contract rejected a proof the core built for key {position}"
        );
    }
}

#[test]
fn a_non_membership_proof_the_core_builds_is_one_the_contract_accepts() {
    let tree = populated();
    let root = tree.root();

    for position in 200u8..216 {
        let key = [b'k', position];
        let proof = tree
            .generate_non_membership_proof(&key)
            .expect("the key is absent");

        assert!(
            contract_accepts_non_membership(&root, &serialize_non_membership_proof(&proof), &key),
            "the contract rejected an absence proof the core built for key {position}"
        );
    }
}

#[test]
fn both_sides_agree_on_the_root() {
    let tree = populated();
    let key = [b'k', 3];
    let proof = tree.generate_membership_proof(&key).expect("present");
    let (_, _, siblings) =
        decode_membership_proof(&serialize_membership_proof(&proof)).expect("decodes");

    assert_eq!(
        siblings.len(),
        TREE_DEPTH,
        "the contract requires a sibling per level"
    );
    assert!(verify_membership_raw(
        &tree.root(),
        &key,
        &[b'v', 3],
        &siblings
    ));
}

#[test]
fn a_relayer_claiming_another_value_is_refused_by_the_contract() {
    let tree = populated();
    let key = [b'k', 5];
    let proof = tree.generate_membership_proof(&key).expect("present");

    assert!(
        !contract_accepts_membership(
            &tree.root(),
            &serialize_membership_proof(&proof),
            &key,
            b"not the stored value"
        ),
        "an honest proof must not verify a value the tree does not hold"
    );
}

#[test]
fn a_forged_value_hash_is_refused_by_the_contract() {
    let tree = populated();
    let key = [b'k', 5];
    let claimed = b"not the stored value";
    let honest = tree.generate_membership_proof(&key).expect("present");
    let forged = MembershipProof {
        value_hash: Sha256::digest(claimed).into(),
        ..honest
    };

    assert!(
        !contract_accepts_membership(
            &tree.root(),
            &serialize_membership_proof(&forged),
            &key,
            claimed
        ),
        "a proof whose value hash was swapped must not verify"
    );
}

#[test]
fn a_proof_against_another_root_is_refused_by_the_contract() {
    let tree = populated();
    let mut other = populated();

    other.insert(b"one more", b"entry");

    let key = [b'k', 7];
    let proof = tree.generate_membership_proof(&key).expect("present");

    assert!(
        !contract_accepts_membership(
            &other.root(),
            &serialize_membership_proof(&proof),
            &key,
            &[b'v', 7]
        ),
        "a proof must only verify against the root it was built from"
    );
}

#[test]
fn an_absence_proof_for_a_present_key_is_refused_by_the_contract() {
    let tree = populated();
    let key = [b'k', 9];
    let present = tree.generate_membership_proof(&key).expect("present");
    let forged = NonMembershipProof {
        key: present.key,
        key_hash: present.key_hash,
        siblings: present.siblings,
    };

    assert!(
        !contract_accepts_non_membership(
            &tree.root(),
            &serialize_non_membership_proof(&forged),
            &key
        ),
        "a key that is present cannot be proven absent"
    );
}

#[test]
fn removing_a_key_makes_its_absence_provable() {
    let mut tree = populated();
    let key = [b'k', 11];

    tree.remove(&key);

    let proof = tree
        .generate_non_membership_proof(&key)
        .expect("absent after removal");

    assert!(contract_accepts_non_membership(
        &tree.root(),
        &serialize_non_membership_proof(&proof),
        &key
    ));
}
