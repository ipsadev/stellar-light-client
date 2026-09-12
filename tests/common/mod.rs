#![allow(dead_code)]

use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

pub const ENVELOPE_TYPE_SCP: [u8; 4] = [0, 0, 0, 1];

pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn u32be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn u64be(v: u64) -> [u8; 8] {
    v.to_be_bytes()
}

fn pad(out: &mut Vec<u8>, len: usize) {
    let n = (4 - (len % 4)) % 4;

    out.extend(core::iter::repeat(0u8).take(n));
}

pub fn var_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = u32be(data.len() as u32).to_vec();

    out.extend_from_slice(data);
    pad(&mut out, data.len());
    out
}

pub fn node_id(public: &[u8; 32]) -> Vec<u8> {
    let mut out = u32be(0).to_vec();

    out.extend_from_slice(public);
    out
}

pub fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

pub fn public_of(key: &SigningKey) -> [u8; 32] {
    key.verifying_key().to_bytes()
}

pub fn quorum_set(validators: &[[u8; 32]], threshold: u32) -> Vec<u8> {
    let mut out = u32be(threshold).to_vec();

    out.extend_from_slice(&u32be(validators.len() as u32));

    for v in validators {
        out.extend_from_slice(&node_id(v));
    }

    out.extend_from_slice(&u32be(0));
    out
}

pub fn nested_quorum_set(groups: &[Vec<[u8; 32]>], outer: u32, inner: u32) -> Vec<u8> {
    let mut out = u32be(outer).to_vec();

    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(groups.len() as u32));

    for g in groups {
        out.extend_from_slice(&quorum_set(g, inner));
    }

    out
}

pub fn stellar_value(tx_set_hash: [u8; 32], close_time: u64) -> Vec<u8> {
    let mut out = tx_set_hash.to_vec();

    out.extend_from_slice(&u64be(close_time));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(0));
    out
}

pub fn ledger_header(seq: u32, scp_value: &[u8], tx_set_result_hash: [u8; 32]) -> Vec<u8> {
    let mut out = u32be(23).to_vec();

    out.extend_from_slice(&[0u8; 32]);
    out.extend_from_slice(scp_value);
    out.extend_from_slice(&tx_set_result_hash);
    out.extend_from_slice(&[2u8; 32]);
    out.extend_from_slice(&u32be(seq));
    out.extend_from_slice(&u64be(0));
    out.extend_from_slice(&u64be(0));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u64be(0));
    out.extend_from_slice(&u32be(100));
    out.extend_from_slice(&u32be(5_000_000));
    out.extend_from_slice(&u32be(1000));

    for _ in 0..4 {
        out.extend_from_slice(&[0u8; 32]);
    }

    out.extend_from_slice(&u32be(0));
    out
}

pub fn externalize_statement(
    public: &[u8; 32],
    slot: u64,
    commit_value: &[u8],
    qset_hash: [u8; 32],
) -> Vec<u8> {
    let mut out = node_id(public);

    out.extend_from_slice(&u64be(slot));
    out.extend_from_slice(&u32be(2));
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&var_bytes(commit_value));
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&qset_hash);
    out
}

pub fn prepare_statement(
    public: &[u8; 32],
    slot: u64,
    ballot_value: &[u8],
    qset_hash: [u8; 32],
) -> Vec<u8> {
    let mut out = node_id(public);

    out.extend_from_slice(&u64be(slot));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&qset_hash);
    out.extend_from_slice(&u32be(1));
    out.extend_from_slice(&var_bytes(ballot_value));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(0));
    out
}

pub fn prepare_envelope(
    key: &SigningKey,
    network_id: &[u8; 32],
    slot: u64,
    ballot_value: &[u8],
    qset_hash: [u8; 32],
) -> Vec<u8> {
    let public = public_of(key);
    let statement = prepare_statement(&public, slot, ballot_value, qset_hash);
    let mut payload = network_id.to_vec();

    payload.extend_from_slice(&ENVELOPE_TYPE_SCP);
    payload.extend_from_slice(&statement);
    let signature = key.sign(&payload).to_bytes();
    let mut out = statement;

    out.extend_from_slice(&var_bytes(&signature));
    out
}

pub fn envelope(
    key: &SigningKey,
    network_id: &[u8; 32],
    slot: u64,
    commit_value: &[u8],
    qset_hash: [u8; 32],
) -> Vec<u8> {
    let public = public_of(key);
    let statement = externalize_statement(&public, slot, commit_value, qset_hash);
    let mut payload = network_id.to_vec();

    payload.extend_from_slice(&ENVELOPE_TYPE_SCP);
    payload.extend_from_slice(&statement);
    let signature = key.sign(&payload).to_bytes();
    let mut out = statement;

    out.extend_from_slice(&var_bytes(&signature));
    out
}

pub struct Ledger {
    pub slot: u64,
    pub header_xdr: Vec<u8>,
    pub envelopes: Vec<Vec<u8>>,
    pub next_envelopes: Vec<Vec<u8>>,
    pub next_tx_set: Vec<u8>,
    pub quorum_sets: Vec<Vec<u8>>,
    pub network_id: [u8; 32],
    pub keys: Vec<SigningKey>,
}

pub fn tx_set(previous_ledger_hash: [u8; 32]) -> Vec<u8> {
    let mut out = 1u32.to_be_bytes().to_vec();

    out.extend_from_slice(&previous_ledger_hash);
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

pub fn build_ledger(slot: u64, validators: usize, threshold: u32) -> Ledger {
    let network_id = sha256(b"Test SDF Network ; September 2015");
    let keys: Vec<SigningKey> = (0..validators).map(|i| signing_key(i as u8 + 1)).collect();
    let publics: Vec<[u8; 32]> = keys.iter().map(public_of).collect();
    let qset = quorum_set(&publics, threshold);
    let qset_hash = sha256(&qset);
    let value = stellar_value([7u8; 32], 1_700_000_000 + slot);
    let header_xdr = ledger_header(slot as u32, &value, [9u8; 32]);
    let ledger_hash = sha256(&header_xdr);
    let envelopes = keys
        .iter()
        .map(|k| envelope(k, &network_id, slot, &value, qset_hash))
        .collect();
    let next_tx_set = tx_set(ledger_hash);
    let next_value = stellar_value(sha256(&next_tx_set), 1_700_000_001 + slot);
    let next_envelopes = keys
        .iter()
        .map(|k| envelope(k, &network_id, slot + 1, &next_value, qset_hash))
        .collect();

    Ledger {
        slot,
        header_xdr,
        envelopes,
        next_envelopes,
        next_tx_set,
        quorum_sets: vec![qset],
        network_id,
        keys,
    }
}
