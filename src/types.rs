use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct Height {
    #[prost(uint64, tag = "1")]
    pub revision_number: u64,
    #[prost(uint64, tag = "2")]
    pub revision_height: u64,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct QuorumConfig {
    #[prost(bytes = "vec", tag = "1")]
    pub quorum_set_xdr: alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub valid_from: u64,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct ClientState {
    #[prost(string, tag = "1")]
    pub chain_id: alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub latest_height: ::core::option::Option<Height>,
    #[prost(message, optional, tag = "3")]
    pub frozen_height: ::core::option::Option<Height>,
    #[prost(message, repeated, tag = "4")]
    pub quorum_configs: alloc::vec::Vec<QuorumConfig>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub proof_specs: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", tag = "6")]
    pub network_id: alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "7")]
    pub max_consensus_age: u64,
    #[prost(bytes = "vec", tag = "8")]
    pub router_contract_id: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "9")]
    pub root_event_topic: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct StateRootProof {
    #[prost(bytes = "vec", repeated, tag = "1")]
    pub result_pairs: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(uint32, tag = "2")]
    pub result_index: u32,
    #[prost(bytes = "vec", tag = "3")]
    pub success_preimage_xdr: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct ConsensusState {
    #[prost(uint64, tag = "1")]
    pub timestamp: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub ledger_hash: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub root: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct StellarHeader {
    #[prost(uint64, tag = "1")]
    pub slot_index: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub ledger_header_xdr: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub scp_envelopes: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    pub quorum_sets_xdr: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub next_scp_envelopes: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", tag = "6")]
    pub next_tx_set_xdr: alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "7")]
    pub state_root_proof: ::core::option::Option<StateRootProof>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct Any {
    #[prost(string, tag = "1")]
    pub type_url: alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub value: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct WasmClientState {
    #[prost(bytes = "vec", tag = "1")]
    pub data: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub checksum: alloc::vec::Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub latest_height: ::core::option::Option<Height>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct WasmConsensusState {
    #[prost(bytes = "vec", tag = "1")]
    pub data: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct Misbehaviour {
    #[prost(string, tag = "1")]
    pub client_id: alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub header_1: ::core::option::Option<StellarHeader>,
    #[prost(message, optional, tag = "3")]
    pub header_2: ::core::option::Option<StellarHeader>,
}

extern crate alloc;
