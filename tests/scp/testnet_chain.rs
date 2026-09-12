use serde::Deserialize;
use stellar_light_client::scp::{
    quorum::QuorumSet,
    verify::{verify, Inputs},
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
    next_envelopes_xdr: Vec<String>,
    next_tx_set_xdr: String,
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/testnet-4160279.json")).expect("fixture")
}

struct Case {
    network_id: [u8; 32],
    local: QuorumSet,
    header: Vec<u8>,
    envelopes: Vec<Vec<u8>>,
    next_envelopes: Vec<Vec<u8>>,
    tx_set: Vec<u8>,
    qsets: Vec<Vec<u8>>,
}

fn case(f: &Fixture) -> Case {
    Case {
        network_id: unhex(&f.network_id).try_into().unwrap(),
        local: QuorumSet::decode(&unhex(&f.quorum_sets_xdr[0])).unwrap(),
        header: unhex(&f.ledger_header_xdr),
        envelopes: f.envelopes_xdr.iter().map(|e| unhex(e)).collect(),
        next_envelopes: f.next_envelopes_xdr.iter().map(|e| unhex(e)).collect(),
        tx_set: unhex(&f.next_tx_set_xdr),
        qsets: f.quorum_sets_xdr.iter().map(|q| unhex(q)).collect(),
    }
}

fn run(c: &Case, slot: u64) -> Result<stellar_light_client::scp::verify::VerifiedHeader, String> {
    verify(Inputs {
        network_id: &c.network_id,
        quorum_set_for_slot: &c.local,
        quorum_set_for_next_slot: &c.local,
        slot_index: slot,
        ledger_header_xdr: &c.header,
        envelopes_xdr: &c.envelopes,
        next_envelopes_xdr: &c.next_envelopes,
        next_tx_set_xdr: &c.tx_set,
        quorum_sets_xdr: &c.qsets,
    })
    .map_err(|e| format!("{e}"))
}

#[test]
fn authenticates_a_real_ledger_through_the_next_slots_tx_set() {
    let f = fixture();
    let verified = run(&case(&f), f.ledger_seq).expect("verifies");

    assert_eq!(verified.slot_index, f.ledger_seq);
    assert_eq!(verified.timestamp, f.close_time);
    assert_eq!(verified.ledger_hash.to_vec(), unhex(&f.ledger_hash));
    assert_eq!(
        verified.tx_set_result_hash.to_vec(),
        unhex(&f.tx_set_result_hash),
        "the result hash is authenticated once the header is"
    );
}

#[test]
fn rejects_a_forged_tx_set_result_hash() {
    let f = fixture();
    let mut c = case(&f);
    let offset = 4 + 32 + c.header.len() - c.header.len();
    let _ = offset;
    let pos = c
        .header
        .windows(32)
        .position(|w| w == unhex(&f.tx_set_result_hash).as_slice())
        .expect("result hash appears in the header");

    c.header[pos] ^= 0x01;
    let err = run(&c, f.ledger_seq).unwrap_err();

    assert!(
        err.contains("does not follow this ledger"),
        "forging an unsigned header field must break the tx-set link: {err}"
    );
}

#[test]
fn rejects_a_tampered_tx_set() {
    let f = fixture();
    let mut c = case(&f);
    let last = c.tx_set.len() - 1;

    c.tx_set[last] ^= 0x01;
    let err = run(&c, f.ledger_seq).unwrap_err();

    assert!(
        err.contains("does not hash to the value"),
        "unexpected: {err}"
    );
}

#[test]
fn rejects_when_the_next_slot_is_not_backed_by_a_quorum() {
    let f = fixture();
    let mut c = case(&f);

    c.next_envelopes.truncate(1);
    let err = run(&c, f.ledger_seq).unwrap_err();

    assert!(err.contains("confirm commit"), "unexpected: {err}");
}

#[test]
fn rejects_a_missing_next_slot() {
    let f = fixture();
    let mut c = case(&f);

    c.next_envelopes.clear();
    let err = run(&c, f.ledger_seq).unwrap_err();

    assert!(err.contains("confirm commit"), "unexpected: {err}");
}
