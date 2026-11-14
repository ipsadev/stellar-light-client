use alloc::vec::Vec;

use stellar_consensus_verifier::verify::{
    bind_header, bind_successor, verify_consensus, AuthenticatedHeader, TrustRoot,
};

use super::quorum::QuorumSet;
use crate::error::ContractError;

#[derive(Clone, Debug)]
pub struct VerifiedHeader {
    pub slot_index: u64,
    pub timestamp: u64,
    pub ledger_hash: [u8; 32],
    pub previous_ledger_hash: [u8; 32],
    pub tx_set_result_hash: [u8; 32],
    pub bucket_list_hash: [u8; 32],
    pub authenticated: AuthenticatedHeader,
}

pub struct Inputs<'a> {
    pub network_id: &'a [u8; 32],
    pub trust_root_for_slot: &'a TrustRoot,
    pub trust_root_for_next_slot: &'a TrustRoot,
    pub slot_index: u64,
    pub ledger_header_xdr: &'a [u8],
    pub envelopes_xdr: &'a [Vec<u8>],
    pub next_envelopes_xdr: &'a [Vec<u8>],
    pub next_tx_set_xdr: &'a [u8],
    pub quorum_sets_xdr: &'a [Vec<u8>],
}

pub fn trust_root(quorum_set_xdr: &[u8]) -> Result<TrustRoot, ContractError> {
    Ok(TrustRoot::from_pinned(
        quorum_set_xdr,
        &QuorumSet::hash(quorum_set_xdr),
    )?)
}

pub fn verify(input: Inputs<'_>) -> Result<VerifiedHeader, ContractError> {
    let next_slot = input
        .slot_index
        .checked_add(1)
        .ok_or(ContractError::SlotOverflow {
            slot: input.slot_index,
        })?;
    let value = verify_consensus(
        input.network_id,
        input.trust_root_for_slot,
        input.slot_index,
        input.envelopes_xdr,
        input.quorum_sets_xdr,
    )?;
    let bound = bind_header(&value, input.ledger_header_xdr)?;
    let successor = verify_consensus(
        input.network_id,
        input.trust_root_for_next_slot,
        next_slot,
        input.next_envelopes_xdr,
        input.quorum_sets_xdr,
    )?;
    let authenticated = bind_successor(&bound, &successor, input.next_tx_set_xdr)?;
    let header = authenticated.header();

    Ok(VerifiedHeader {
        slot_index: input.slot_index,
        timestamp: header.close_time,
        ledger_hash: header.ledger_hash,
        previous_ledger_hash: header.previous_ledger_hash,
        tx_set_result_hash: header.tx_set_result_hash,
        bucket_list_hash: header.bucket_list_hash,
        authenticated,
    })
}

extern crate alloc;
