use stellar_light_client::scp::{
    envelope::Envelope,
    quorum::{is_quorum, is_quorum_slice, QuorumSet},
    verify::{verify, Inputs},
};

use crate::common::{build_ledger, nested_quorum_set, public_of, quorum_set, sha256, signing_key};

fn run(l: &crate::common::Ledger, local: &QuorumSet, slot: u64) -> Result<(), String> {
    verify(Inputs {
        network_id: &l.network_id,
        quorum_set_for_slot: local,
        quorum_set_for_next_slot: local,
        slot_index: slot,
        ledger_header_xdr: &l.header_xdr,
        envelopes_xdr: &l.envelopes,
        next_envelopes_xdr: &l.next_envelopes,
        next_tx_set_xdr: &l.next_tx_set,
        quorum_sets_xdr: &l.quorum_sets,
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

fn local_of(l: &crate::common::Ledger) -> QuorumSet {
    QuorumSet::decode(&l.quorum_sets[0]).unwrap()
}

#[test]
fn verifies_a_synthetic_quorum() {
    let l = build_ledger(100, 4, 3);

    run(&l, &local_of(&l), 100).expect("verifies");
}

#[test]
fn a_tampered_ballot_counter_is_refused_before_the_signature_check() {
    let l = build_ledger(101, 4, 3);
    let mut envelopes = l.envelopes.clone();
    let counter_offset = 4 + 32 + 8 + 4;

    envelopes[0][counter_offset..counter_offset + 4].copy_from_slice(&9u32.to_be_bytes());
    let err = verify(Inputs {
        network_id: &l.network_id,
        quorum_set_for_slot: &local_of(&l),
        quorum_set_for_next_slot: &local_of(&l),
        slot_index: 101,
        ledger_header_xdr: &l.header_xdr,
        envelopes_xdr: &envelopes,
        next_envelopes_xdr: &l.next_envelopes,
        next_tx_set_xdr: &l.next_tx_set,
        quorum_sets_xdr: &l.quorum_sets,
    })
    .unwrap_err();

    assert!(
        format!("{err}").contains("below the commit counter"),
        "stellar-core requires nH >= commit.counter, so this never reaches L1: {err}"
    );
}

#[test]
fn a_tampered_highest_confirmed_counter_fails_the_signature_check() {
    let l = build_ledger(101, 4, 3);
    let mut envelopes = l.envelopes.clone();
    let counter_offset = 4 + 32 + 8 + 4;
    let value_len = u32::from_be_bytes(
        envelopes[0][counter_offset + 4..counter_offset + 8]
            .try_into()
            .unwrap(),
    ) as usize;
    let highest_offset = counter_offset + 8 + value_len;

    envelopes[0][highest_offset..highest_offset + 4].copy_from_slice(&9u32.to_be_bytes());
    let err = verify(Inputs {
        network_id: &l.network_id,
        quorum_set_for_slot: &local_of(&l),
        quorum_set_for_next_slot: &local_of(&l),
        slot_index: 101,
        ledger_header_xdr: &l.header_xdr,
        envelopes_xdr: &envelopes,
        next_envelopes_xdr: &l.next_envelopes,
        next_tx_set_xdr: &l.next_tx_set,
        quorum_sets_xdr: &l.quorum_sets,
    })
    .unwrap_err();

    assert!(
        format!("{err}").contains("does not verify"),
        "the counter is inside the signed statement, so tampering must fail L1: {err}"
    );
}

#[test]
fn a_prepare_carries_no_commit_evidence() {
    let l = build_ledger(102, 4, 3);
    let value = Envelope::decode(&l.envelopes[0])
        .expect("decodes")
        .externalize()
        .expect("is an EXTERNALIZE")
        .commit
        .value
        .clone();
    let prepare = |index: usize| {
        crate::common::prepare_envelope(
            &l.keys[index],
            &l.network_id,
            102,
            &value,
            sha256(&l.quorum_sets[0]),
        )
    };
    let mut one_replaced = l.envelopes.clone();

    one_replaced[0] = prepare(0);
    run(
        &crate::common::Ledger {
            envelopes: one_replaced,
            ..rebuild(&l)
        },
        &local_of(&l),
        102,
    )
    .expect("the remaining three still reach confirm commit");

    let mut two_replaced = l.envelopes.clone();

    two_replaced[0] = prepare(0);
    two_replaced[1] = prepare(1);
    let err = run(
        &crate::common::Ledger {
            envelopes: two_replaced,
            ..rebuild(&l)
        },
        &local_of(&l),
        102,
    )
    .unwrap_err();

    assert!(err.contains("confirm commit"), "unexpected: {err}");
}

fn rebuild(l: &crate::common::Ledger) -> crate::common::Ledger {
    crate::common::Ledger {
        slot: l.slot,
        header_xdr: l.header_xdr.clone(),
        envelopes: l.envelopes.clone(),
        next_envelopes: l.next_envelopes.clone(),
        next_tx_set: l.next_tx_set.clone(),
        quorum_sets: l.quorum_sets.clone(),
        network_id: l.network_id,
        keys: l.keys.to_vec(),
    }
}

#[test]
fn rejects_signers_outside_the_configured_set() {
    let l = build_ledger(103, 4, 3);
    let strangers: Vec<[u8; 32]> = (10..14).map(|i| public_of(&signing_key(i))).collect();
    let other = QuorumSet::decode(&quorum_set(&strangers, 3)).unwrap();
    let err = run(&l, &other, 103).unwrap_err();

    assert!(err.contains("confirm commit"), "unexpected: {err}");
}

#[test]
fn rejects_trailing_bytes_in_a_quorum_set() {
    let mut raw = quorum_set(&[public_of(&signing_key(1))], 1);

    raw.push(0);
    assert!(QuorumSet::decode(&raw).is_err());
}

#[test]
fn rejects_a_quorum_set_nested_too_deeply() {
    let leaf = quorum_set(&[public_of(&signing_key(1))], 1);
    let mut nested = leaf;

    for _ in 0..6 {
        let mut outer = 1u32.to_be_bytes().to_vec();

        outer.extend_from_slice(&0u32.to_be_bytes());
        outer.extend_from_slice(&1u32.to_be_bytes());
        outer.extend_from_slice(&nested);
        nested = outer;
    }

    assert!(QuorumSet::decode(&nested).is_err());
}

#[test]
fn sanity_rejects_a_zero_threshold() {
    let raw = quorum_set(&[public_of(&signing_key(1))], 0);
    let qset = QuorumSet::decode(&raw).unwrap();

    assert!(qset.check_sane(false).is_err());
}

#[test]
fn sanity_rejects_a_threshold_above_the_entry_count() {
    let raw = quorum_set(&[public_of(&signing_key(1))], 2);
    let qset = QuorumSet::decode(&raw).unwrap();

    assert!(qset.check_sane(false).is_err());
}

#[test]
fn sanity_rejects_a_duplicate_validator() {
    let k = public_of(&signing_key(1));
    let raw = quorum_set(&[k, k], 2);
    let qset = QuorumSet::decode(&raw).unwrap();

    assert!(qset.check_sane(false).is_err());
}

#[test]
fn strict_majority_applies_only_to_the_trust_root() {
    let validators: Vec<[u8; 32]> = (1..=4).map(|i| public_of(&signing_key(i))).collect();
    let qset = QuorumSet::decode(&quorum_set(&validators, 2)).unwrap();

    assert!(qset.check_sane(false).is_ok());
    assert!(qset.check_sane(true).is_err());
}

#[test]
fn quorum_slice_walks_nested_sets() {
    let groups: Vec<Vec<[u8; 32]>> = (0..3)
        .map(|g| {
            (0..3)
                .map(|i| public_of(&signing_key(g * 3 + i + 1)))
                .collect()
        })
        .collect();
    let qset = QuorumSet::decode(&nested_quorum_set(&groups, 2, 2)).unwrap();
    let two_groups: Vec<[u8; 32]> = groups[0]
        .iter()
        .take(2)
        .chain(groups[1].iter().take(2))
        .copied()
        .collect();

    assert!(is_quorum_slice(&qset, &two_groups));
    let one_group: Vec<[u8; 32]> = groups[0].iter().take(2).copied().collect();

    assert!(!is_quorum_slice(&qset, &one_group));
}

#[test]
fn is_quorum_discards_signers_whose_own_set_is_unsatisfied() {
    let validators: Vec<[u8; 32]> = (1..=3).map(|i| public_of(&signing_key(i))).collect();
    let local = QuorumSet::decode(&quorum_set(&validators, 2)).unwrap();
    let strangers: Vec<[u8; 32]> = (20..23).map(|i| public_of(&signing_key(i))).collect();
    let unsatisfiable = QuorumSet::decode(&quorum_set(&strangers, 3)).unwrap();
    let signers: Vec<([u8; 32], QuorumSet)> = validators
        .iter()
        .map(|v| (*v, unsatisfiable.clone()))
        .collect();

    assert!(
        !is_quorum(&local, &signers),
        "signers whose own quorum set cannot be satisfied are not a quorum"
    );
    let satisfiable: Vec<([u8; 32], QuorumSet)> =
        validators.iter().map(|v| (*v, local.clone())).collect();

    assert!(is_quorum(&local, &satisfiable));
}

#[test]
fn quorum_set_hash_matches_sha256_of_the_encoding() {
    let raw = quorum_set(&[public_of(&signing_key(1))], 1);

    assert_eq!(QuorumSet::hash(&raw), sha256(&raw));
}
