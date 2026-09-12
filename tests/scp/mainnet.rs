use serde::Deserialize;
use sha2::{Digest, Sha256};
use stellar_light_client::scp::{
    envelope::Envelope,
    ledger::LedgerHeader,
    quorum::{is_quorum, QuorumSet},
};

#[derive(Deserialize)]
struct Fixture {
    ledger_seq: u64,
    network_id: String,
    ledger_hash: String,
    close_time: u64,
    tx_set_result_hash: String,
    ledger_header_xdr: String,
    quorum_sets_xdr: Vec<String>,
    envelopes_xdr: Vec<String>,
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/mainnet-63907880.json")).expect("fixture")
}

/// The fixture, decoded out of hex.
struct Parts {
    #[allow(dead_code)]
    network_id: [u8; 32],
    envelopes: Vec<Vec<u8>>,
    qsets: Vec<Vec<u8>>,
    header: Vec<u8>,
}

fn parts(f: &Fixture) -> Parts {
    Parts {
        network_id: unhex(&f.network_id).try_into().unwrap(),
        envelopes: f.envelopes_xdr.iter().map(|e| unhex(e)).collect(),
        qsets: f.quorum_sets_xdr.iter().map(|q| unhex(q)).collect(),
        header: unhex(&f.ledger_header_xdr),
    }
}

fn local_set(f: &Fixture) -> QuorumSet {
    QuorumSet::decode(&unhex(&f.quorum_sets_xdr[0])).expect("quorum set decodes")
}

#[test]
fn mainnet_quorum_set_is_five_of_seven_nested() {
    let f = fixture();
    let qset = local_set(&f);

    assert_eq!(qset.threshold, 5);
    assert_eq!(qset.validators.len(), 0);
    assert_eq!(qset.inner_sets.len(), 7);

    for inner in &qset.inner_sets {
        assert_eq!(inner.threshold, 2);
        assert_eq!(inner.validators.len(), 3);
    }

    qset.check_sane(true).expect("tier 1 set is sane");
}

#[test]
fn every_mainnet_envelope_decodes_and_agrees() {
    let f = fixture();
    let Parts {
        envelopes, qsets, ..
    } = parts(&f);
    let qset_hash = QuorumSet::hash(&qsets[0]);
    let mut values = Vec::new();
    let mut signers = Vec::new();

    for raw in &envelopes {
        let env = Envelope::decode(raw).expect("real envelope decodes");

        assert_eq!(env.slot_index, f.ledger_seq);
        assert_eq!(env.externalize().unwrap().commit_quorum_set_hash, qset_hash);
        values.push(env.externalize().unwrap().commit.value.clone());
        signers.push(env.node_id);
    }

    assert_eq!(envelopes.len(), 21);
    assert!(values.windows(2).all(|w| w[0] == w[1]), "one agreed value");
    signers.sort_unstable();
    signers.dedup();
    assert_eq!(signers.len(), 21, "21 distinct signers");
}

#[test]
fn twenty_one_mainnet_signers_form_a_quorum_but_five_do_not() {
    let f = fixture();
    let Parts { envelopes, .. } = parts(&f);
    let local = local_set(&f);
    let all: Vec<([u8; 32], QuorumSet)> = envelopes
        .iter()
        .map(|raw| (Envelope::decode(raw).unwrap().node_id, local.clone()))
        .collect();

    assert!(is_quorum(&local, &all));
    let five: Vec<([u8; 32], QuorumSet)> = all.into_iter().take(5).collect();

    assert!(!is_quorum(&local, &five));
}

#[test]
fn the_agreed_value_is_this_ledgers_scp_value() {
    let f = fixture();
    let Parts {
        envelopes, header, ..
    } = parts(&f);
    let value = Envelope::decode(&envelopes[0])
        .unwrap()
        .externalize()
        .unwrap()
        .commit
        .value
        .clone();

    assert!(
        header.windows(value.len()).any(|w| w == value.as_slice()),
        "the externalized value appears verbatim in the ledger header"
    );
}

/// Deriving the ledger hash from the header bytes is what turns "a quorum
/// agreed on a value" into "this is the ledger". The rest of this file stops at
/// the value, because the mainnet vector carries no slot N+1 and so cannot run
/// the full `verify`.
#[test]
fn the_header_hashes_to_the_recorded_ledger_hash() {
    let f = fixture();
    let Parts { header, .. } = parts(&f);
    let hash: [u8; 32] = Sha256::digest(&header).into();

    assert_eq!(hash.to_vec(), unhex(&f.ledger_hash), "sha256(LedgerHeader)");
    let decoded = LedgerHeader::decode(&header).expect("the header decodes");

    assert_eq!(decoded.ledger_seq as u64, f.ledger_seq);
    assert_eq!(decoded.close_time, f.close_time);
    assert_eq!(
        decoded.tx_set_result_hash.to_vec(),
        unhex(&f.tx_set_result_hash)
    );
}
