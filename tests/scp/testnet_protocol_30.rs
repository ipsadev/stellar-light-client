use serde::Deserialize;
use stellar_light_client::scp::{
    quorum::QuorumSet,
    verify::{verify, Inputs, VerifiedHeader},
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
    serde_json::from_str(include_str!("../fixtures/testnet-5541000-protocol-30.json"))
        .expect("fixture")
}

fn run(f: &Fixture, header: &[u8]) -> Result<VerifiedHeader, String> {
    let network_id: [u8; 32] = unhex(&f.network_id).try_into().unwrap();
    let local = QuorumSet::decode(&unhex(&f.quorum_sets_xdr[0])).unwrap();
    let envelopes: Vec<Vec<u8>> = f.envelopes_xdr.iter().map(|e| unhex(e)).collect();
    let next_envelopes: Vec<Vec<u8>> = f.next_envelopes_xdr.iter().map(|e| unhex(e)).collect();
    let quorum_sets: Vec<Vec<u8>> = f.quorum_sets_xdr.iter().map(|q| unhex(q)).collect();
    let tx_set = unhex(&f.next_tx_set_xdr);

    verify(Inputs {
        network_id: &network_id,
        quorum_set_for_slot: &local,
        quorum_set_for_next_slot: &local,
        slot_index: f.ledger_seq,
        ledger_header_xdr: header,
        envelopes_xdr: &envelopes,
        next_envelopes_xdr: &next_envelopes,
        next_tx_set_xdr: &tx_set,
        quorum_sets_xdr: &quorum_sets,
    })
    .map_err(|e| format!("{e}"))
}

#[test]
fn authenticates_a_protocol_30_ledger_with_a_millisecond_close_time() {
    let f = fixture();
    let verified = run(&f, &unhex(&f.ledger_header_xdr)).expect("verifies");

    assert_eq!(verified.slot_index, f.ledger_seq);
    assert_eq!(verified.timestamp, f.close_time);
    assert_eq!(verified.ledger_hash.to_vec(), unhex(&f.ledger_hash));
    assert_eq!(
        verified.tx_set_result_hash.to_vec(),
        unhex(&f.tx_set_result_hash)
    );
}

#[test]
fn rejects_a_protocol_30_header_whose_millisecond_close_time_was_changed() {
    let f = fixture();
    let mut header = unhex(&f.ledger_header_xdr);
    let marker = 3i32.to_be_bytes();
    let kind = header
        .windows(4)
        .enumerate()
        .skip(4 + 32 + 32 + 8)
        .find(|(_, w)| *w == marker)
        .map(|(i, _)| i)
        .expect("the header carries a millisecond value");

    header[kind + 4 + 7] ^= 0x01;

    assert!(run(&f, &header).is_err());
}
