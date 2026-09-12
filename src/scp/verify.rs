use alloc::vec::Vec;

use super::{
    ballot::{authenticate, confirmed_value},
    ledger::{tx_set_hash_of_value, LedgerHeader},
    quorum::QuorumSet,
    txset,
};
use crate::error::ContractError;

#[derive(Clone, Debug)]
pub struct VerifiedHeader {
    pub slot_index: u64,
    pub timestamp: u64,
    pub ledger_hash: [u8; 32],
    pub previous_ledger_hash: [u8; 32],
    pub tx_set_result_hash: [u8; 32],
    pub bucket_list_hash: [u8; 32],
}

pub struct Inputs<'a> {
    pub network_id: &'a [u8; 32],
    pub quorum_set_for_slot: &'a QuorumSet,
    pub quorum_set_for_next_slot: &'a QuorumSet,
    pub slot_index: u64,
    pub ledger_header_xdr: &'a [u8],
    pub envelopes_xdr: &'a [Vec<u8>],
    pub next_envelopes_xdr: &'a [Vec<u8>],
    pub next_tx_set_xdr: &'a [u8],
    pub quorum_sets_xdr: &'a [Vec<u8>],
}

fn agreed_value_for_slot(
    network_id: &[u8; 32],
    local: &QuorumSet,
    slot: u64,
    envelopes_xdr: &[Vec<u8>],
    quorum_sets_xdr: &[Vec<u8>],
) -> Result<Vec<u8>, ContractError> {
    let evidence = authenticate(network_id, slot, envelopes_xdr, quorum_sets_xdr)?;

    Ok(confirmed_value(local, &evidence)?)
}

pub fn verify(input: Inputs<'_>) -> Result<VerifiedHeader, ContractError> {
    let header = LedgerHeader::decode(input.ledger_header_xdr)?;

    if header.ledger_seq as u64 != input.slot_index {
        return Err(ContractError::SlotMismatch {
            header: header.ledger_seq as u64,
            claimed: input.slot_index,
        });
    }

    let value = agreed_value_for_slot(
        input.network_id,
        input.quorum_set_for_slot,
        input.slot_index,
        input.envelopes_xdr,
        input.quorum_sets_xdr,
    )?;

    if value != header.scp_value_bytes {
        return Err(ContractError::ValueNotThisLedger);
    }

    let next_slot = input.slot_index + 1;
    let next_value = agreed_value_for_slot(
        input.network_id,
        input.quorum_set_for_next_slot,
        next_slot,
        input.next_envelopes_xdr,
        input.quorum_sets_xdr,
    )?;

    if txset::hash(input.next_tx_set_xdr) != tx_set_hash_of_value(&next_value)? {
        return Err(ContractError::TxSetHashMismatch);
    }

    if txset::previous_ledger_hash(input.next_tx_set_xdr)? != header.ledger_hash {
        return Err(ContractError::LedgerNotPrevious);
    }

    Ok(VerifiedHeader {
        slot_index: input.slot_index,
        timestamp: header.close_time,
        ledger_hash: header.ledger_hash,
        previous_ledger_hash: header.previous_ledger_hash,
        tx_set_result_hash: header.tx_set_result_hash,
        bucket_list_hash: header.bucket_list_hash,
    })
}

extern crate alloc;
