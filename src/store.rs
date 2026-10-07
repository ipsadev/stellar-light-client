use cosmwasm_std::Storage;
use prost::Message;

use crate::types::{Any, ClientState, ConsensusState, WasmClientState, WasmConsensusState};

pub const SUBJECT_PREFIX: &[u8] = b"subject/";
pub const SUBSTITUTE_PREFIX: &[u8] = b"substitute/";

const CLIENT_STATE_KEY: &[u8] = b"clientState";
const STELLAR_REVISION_NUMBER: u64 = 0;
const WASM_CLIENT_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ClientState";
const WASM_CONSENSUS_STATE_TYPE_URL: &str = "/ibc.lightclients.wasm.v1.ConsensusState";

fn consensus_state_key(height: u64) -> alloc::vec::Vec<u8> {
    alloc::format!("consensusStates/{STELLAR_REVISION_NUMBER}-{height}").into_bytes()
}

fn prefixed(prefix: &[u8], key: &[u8]) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::with_capacity(prefix.len() + key.len());

    out.extend_from_slice(prefix);
    out.extend_from_slice(key);
    out
}

fn wasm_client_state_prefixed(storage: &dyn Storage, prefix: &[u8]) -> Option<WasmClientState> {
    let raw = storage.get(prefixed(prefix, CLIENT_STATE_KEY).as_slice())?;
    let any = Any::decode(raw.as_slice()).ok()?;

    WasmClientState::decode(any.value.as_slice()).ok()
}

pub fn checksum(storage: &dyn Storage) -> alloc::vec::Vec<u8> {
    checksum_prefixed(storage, &[])
}

pub fn checksum_prefixed(storage: &dyn Storage, prefix: &[u8]) -> alloc::vec::Vec<u8> {
    wasm_client_state_prefixed(storage, prefix)
        .map(|wasm| wasm.checksum)
        .unwrap_or_default()
}

pub fn init_client_state(storage: &mut dyn Storage, client_state: &ClientState, checksum: &[u8]) {
    write_client_state(storage, &[], client_state, checksum.to_vec());
}

pub fn set_client_state(storage: &mut dyn Storage, client_state: &ClientState) {
    set_client_state_prefixed(storage, &[], client_state);
}

pub fn set_client_state_prefixed(
    storage: &mut dyn Storage,
    prefix: &[u8],
    client_state: &ClientState,
) {
    let checksum = checksum_prefixed(storage, prefix);

    write_client_state(storage, prefix, client_state, checksum);
}

fn write_client_state(
    storage: &mut dyn Storage,
    prefix: &[u8],
    client_state: &ClientState,
    checksum: alloc::vec::Vec<u8>,
) {
    let wasm = WasmClientState {
        data: client_state.encode_to_vec(),
        checksum,
        latest_height: client_state.latest_height.clone(),
    };

    let any = Any {
        type_url: WASM_CLIENT_STATE_TYPE_URL.into(),
        value: wasm.encode_to_vec(),
    };

    storage.set(
        prefixed(prefix, CLIENT_STATE_KEY).as_slice(),
        &any.encode_to_vec(),
    );
}

pub fn client_state(storage: &dyn Storage) -> Option<ClientState> {
    client_state_prefixed(storage, &[])
}

pub fn client_state_prefixed(storage: &dyn Storage, prefix: &[u8]) -> Option<ClientState> {
    let wasm = wasm_client_state_prefixed(storage, prefix)?;

    ClientState::decode(wasm.data.as_slice()).ok()
}

pub fn set_consensus_state(
    storage: &mut dyn Storage,
    height: u64,
    consensus_state: &ConsensusState,
) {
    set_consensus_state_prefixed(storage, &[], height, consensus_state);
}

pub fn set_consensus_state_prefixed(
    storage: &mut dyn Storage,
    prefix: &[u8],
    height: u64,
    consensus_state: &ConsensusState,
) {
    let wasm = WasmConsensusState {
        data: consensus_state.encode_to_vec(),
    };

    let any = Any {
        type_url: WASM_CONSENSUS_STATE_TYPE_URL.into(),
        value: wasm.encode_to_vec(),
    };

    storage.set(
        prefixed(prefix, consensus_state_key(height).as_slice()).as_slice(),
        &any.encode_to_vec(),
    );
}

pub fn consensus_state_ro(storage: &dyn Storage, height: u64) -> Option<ConsensusState> {
    consensus_state(storage, height)
}

pub fn consensus_state(storage: &dyn Storage, height: u64) -> Option<ConsensusState> {
    consensus_state_prefixed(storage, &[], height)
}

pub fn consensus_state_prefixed(
    storage: &dyn Storage,
    prefix: &[u8],
    height: u64,
) -> Option<ConsensusState> {
    let raw = storage.get(prefixed(prefix, consensus_state_key(height).as_slice()).as_slice())?;
    let any = Any::decode(raw.as_slice()).ok()?;
    let wasm = WasmConsensusState::decode(any.value.as_slice()).ok()?;

    ConsensusState::decode(wasm.data.as_slice()).ok()
}

extern crate alloc;
