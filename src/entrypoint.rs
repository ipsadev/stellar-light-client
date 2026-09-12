use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
};
use prost::Message;

use crate::{
    error::ContractError,
    merkle::{decode_membership_proof, decode_non_membership_proof},
    msg::{
        CheckForMisbehaviourMsg, CheckForMisbehaviourResult, ClientStatus, Height as MsgHeight,
        InstantiateMsg, LatestHeightResult, MigrateMsg, QueryMsg, StatusResult, SudoMsg,
        TimestampAtHeightResult, UpdateStateMsg, UpdateStateOnMisbehaviourMsg, UpdateStateResult,
        VerifyMembershipMsg, VerifyNonMembershipMsg,
    },
    scp::{
        quorum::QuorumSet,
        results,
        verify::{self, Inputs, VerifiedHeader},
    },
    smt::{
        fold_siblings, key_index, leaf_hash, sha256, verify_non_membership_raw, HASH_SIZE,
        TREE_DEPTH,
    },
    store,
    types::{ClientState, ConsensusState, Height as WireHeight, Misbehaviour, StellarHeader},
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    if store::client_state(deps.storage).is_some() {
        return Err(ContractError::AlreadyInitialised);
    }

    let client_state = ClientState::decode(msg.client_state.as_slice())
        .map_err(|e| ContractError::InvalidWire(format!("client_state: {e}")))?;
    let consensus_state = ConsensusState::decode(msg.consensus_state.as_slice())
        .map_err(|e| ContractError::InvalidWire(format!("consensus_state: {e}")))?;
    let height = client_state
        .latest_height
        .as_ref()
        .ok_or_else(|| ContractError::InvalidWire("client_state.latest_height".into()))?
        .revision_height;

    validate_quorum_configs(&client_state)?;
    store::set_checksum(deps.storage, msg.checksum.as_slice());
    store::set_client_state(deps.storage, &client_state);
    store::set_consensus_state(deps.storage, height, &consensus_state);

    Ok(Response::default())
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let data = match msg {
        SudoMsg::UpdateState(m) => to_json(&update_state(deps, env, m)?)?,
        SudoMsg::UpdateStateOnMisbehaviour(m) => {
            update_state_on_misbehaviour(deps, env, m)?;
            Binary::default()
        }
        SudoMsg::CheckForMisbehaviour(m) => to_json(&check_for_misbehaviour(deps, env, m)?)?,
        SudoMsg::VerifyMembership(m) => {
            verify_membership(deps, env, m)?;
            Binary::default()
        }
        SudoMsg::VerifyNonMembership(m) => {
            verify_non_membership(deps, env, m)?;
            Binary::default()
        }
        SudoMsg::VerifyUpgradeAndUpdateState(_) => {
            return Err(ContractError::UpgradeNotSupported);
        }
        SudoMsg::MigrateClientStore(_) => {
            migrate_client_store(deps, env)?;
            Binary::default()
        }
    };

    Ok(Response::default().set_data(data))
}

#[entry_point]
pub fn migrate(deps: DepsMut<'_>, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let client_state = store::client_state(deps.storage).ok_or(ContractError::NotInitialised)?;

    validate_quorum_configs(&client_state)?;
    let height = client_state
        .latest_height
        .as_ref()
        .ok_or_else(|| ContractError::InvalidWire("client_state.latest_height".into()))?
        .revision_height;

    if store::consensus_state(deps.storage, height).is_none() {
        return Err(ContractError::ConsensusStateMissing { height });
    }

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::ClientState {} => {
            let cs = require_client_state(deps)?;

            Ok(Binary::new(cs.encode_to_vec()))
        }
        QueryMsg::ConsensusState { height } => {
            let cons = require_consensus_state(deps, height.revision_height)?;

            Ok(Binary::new(cons.encode_to_vec()))
        }
        QueryMsg::LatestHeight {} => {
            let cs = require_client_state(deps)?;
            let h = cs.latest_height.unwrap_or_default();

            to_json(&LatestHeightResult {
                height: MsgHeight {
                    revision_number: h.revision_number,
                    revision_height: h.revision_height,
                },
            })
        }
        QueryMsg::Status {} => {
            let cs = require_client_state(deps)?;
            let status = if cs.frozen_height.is_some() {
                ClientStatus::Frozen
            } else if is_expired(deps, &env, &cs) {
                ClientStatus::Expired
            } else {
                ClientStatus::Active
            };

            to_json(&StatusResult { status })
        }
        QueryMsg::TimestampAtHeight { height } => {
            let cons = require_consensus_state(deps, height.revision_height)?;

            to_json(&TimestampAtHeightResult {
                timestamp: cons.timestamp,
            })
        }
        QueryMsg::VerifyClientMessage { client_message } => {
            verify_client_message(deps, client_message.as_slice())?;
            to_json(&cosmwasm_std::Empty {})
        }
        QueryMsg::CheckForMisbehaviour { client_message } => {
            let verified = verify_client_message(deps, client_message.as_slice())?;

            to_json(&CheckForMisbehaviourResult {
                found_misbehaviour: detect_misbehaviour(deps, &verified),
            })
        }
    }
}

fn migrate_client_store(deps: DepsMut<'_>, env: Env) -> Result<(), ContractError> {
    let subject = store::client_state_prefixed(deps.storage, store::SUBJECT_PREFIX)
        .ok_or(ContractError::NotInitialised)?;
    let substitute = store::client_state_prefixed(deps.storage, store::SUBSTITUTE_PREFIX)
        .ok_or(ContractError::SubstituteMissing)?;

    ensure_same_target(&subject, &substitute)?;

    if let Some(frozen) = substitute.frozen_height.as_ref() {
        return Err(ContractError::SubstituteFrozen {
            height: frozen.revision_height,
        });
    }

    validate_quorum_configs(&substitute)?;
    let height = substitute
        .latest_height
        .as_ref()
        .ok_or_else(|| ContractError::InvalidWire("substitute.latest_height".into()))?
        .revision_height;
    let consensus = store::consensus_state_prefixed(deps.storage, store::SUBSTITUTE_PREFIX, height)
        .ok_or(ContractError::ConsensusStateMissing { height })?;

    if substitute.max_consensus_age != 0
        && env.block.time.seconds()
            > consensus
                .timestamp
                .saturating_add(substitute.max_consensus_age)
    {
        return Err(ContractError::SubstituteExpired);
    }

    let migrated = ClientState {
        chain_id: subject.chain_id.clone(),
        latest_height: substitute.latest_height.clone(),
        frozen_height: None,
        quorum_configs: substitute.quorum_configs.clone(),
        proof_specs: subject.proof_specs.clone(),
        network_id: subject.network_id.clone(),
        max_consensus_age: substitute.max_consensus_age,
        router_contract_id: subject.router_contract_id.clone(),
        root_event_topic: subject.root_event_topic.clone(),
    };

    store::set_consensus_state_prefixed(deps.storage, store::SUBJECT_PREFIX, height, &consensus);
    store::set_client_state_prefixed(deps.storage, store::SUBJECT_PREFIX, &migrated);

    Ok(())
}

fn ensure_same_target(
    subject: &ClientState,
    substitute: &ClientState,
) -> Result<(), ContractError> {
    let checks: [(&'static str, bool); 5] = [
        ("chain_id", subject.chain_id == substitute.chain_id),
        ("network_id", subject.network_id == substitute.network_id),
        (
            "router_contract_id",
            subject.router_contract_id == substitute.router_contract_id,
        ),
        (
            "root_event_topic",
            subject.root_event_topic == substitute.root_event_topic,
        ),
        ("proof_specs", subject.proof_specs == substitute.proof_specs),
    ];

    for (field, matches) in checks {
        if !matches {
            return Err(ContractError::SubstituteMismatch { field });
        }
    }

    Ok(())
}

fn is_expired(deps: Deps<'_>, env: &Env, cs: &ClientState) -> bool {
    if cs.max_consensus_age == 0 {
        return false;
    }

    let Some(latest) = cs.latest_height.as_ref() else {
        return true;
    };

    let Some(consensus) = store::consensus_state(deps.storage, latest.revision_height) else {
        return true;
    };

    env.block.time.seconds() > consensus.timestamp.saturating_add(cs.max_consensus_age)
}

enum ClientMessage {
    Header(Box<StellarHeader>),
    Fork(Box<StellarHeader>, Box<StellarHeader>),
}

pub struct VerifiedLedger {
    pub header: VerifiedHeader,
    pub root: Option<[u8; 32]>,
}

pub enum Verified {
    Header(Box<VerifiedLedger>),
    Fork {
        first: Box<VerifiedLedger>,
        second: Box<VerifiedLedger>,
    },
}

fn decode_client_message(bytes: &[u8]) -> Result<ClientMessage, ContractError> {
    if let Ok(m) = Misbehaviour::decode(bytes) {
        if let (Some(a), Some(b)) = (m.header_1, m.header_2) {
            if plausible_header(&a) && plausible_header(&b) {
                return Ok(ClientMessage::Fork(Box::new(a), Box::new(b)));
            }
        }
    }

    Ok(ClientMessage::Header(Box::new(decode_header(bytes)?)))
}

fn plausible_header(h: &StellarHeader) -> bool {
    !h.ledger_header_xdr.is_empty() && !h.scp_envelopes.is_empty()
}

fn verify_client_message(deps: Deps<'_>, client_message: &[u8]) -> Result<Verified, ContractError> {
    let cs = require_client_state(deps)?;

    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }

    match decode_client_message(client_message)? {
        ClientMessage::Header(h) => Ok(Verified::Header(Box::new(verify_header(&cs, &h)?))),
        ClientMessage::Fork(a, b) => {
            let first = verify_header(&cs, &a)?;
            let second = verify_header(&cs, &b)?;

            if first.header.slot_index != second.header.slot_index {
                return Err(ContractError::MisbehaviourSlotMismatch {
                    first: first.header.slot_index,
                    second: second.header.slot_index,
                });
            }

            if first.header.ledger_hash == second.header.ledger_hash {
                return Err(ContractError::MisbehaviourNotAFork);
            }

            Ok(Verified::Fork {
                first: Box::new(first),
                second: Box::new(second),
            })
        }
    }
}

fn verify_header(
    cs: &ClientState,
    header: &StellarHeader,
) -> Result<VerifiedLedger, ContractError> {
    let network_id: [u8; 32] = cs
        .network_id
        .as_slice()
        .try_into()
        .map_err(|_| ContractError::NetworkIdInvalid)?;
    let local = applicable_quorum_set(cs, header.slot_index)?;
    let local_next = applicable_quorum_set(cs, header.slot_index + 1)?;
    let verified = verify::verify(Inputs {
        network_id: &network_id,
        quorum_set_for_slot: &local,
        quorum_set_for_next_slot: &local_next,
        slot_index: header.slot_index,
        ledger_header_xdr: &header.ledger_header_xdr,
        envelopes_xdr: &header.scp_envelopes,
        next_envelopes_xdr: &header.next_scp_envelopes,
        next_tx_set_xdr: &header.next_tx_set_xdr,
        quorum_sets_xdr: &header.quorum_sets_xdr,
    })?;
    let root = match header.state_root_proof.as_ref() {
        None => None,
        Some(proof) => {
            let router: [u8; 32] = cs
                .router_contract_id
                .as_slice()
                .try_into()
                .map_err(|_| ContractError::RouterContractIdMissing)?;

            Some(results::verify_state_root(&results::Inputs {
                tx_set_result_hash: &verified.tx_set_result_hash,
                result_pairs: &proof.result_pairs,
                result_index: proof.result_index,
                success_preimage_xdr: &proof.success_preimage_xdr,
                router_contract_id: &router,
                root_event_topic: &cs.root_event_topic,
            })?)
        }
    };

    Ok(VerifiedLedger {
        header: verified,
        root,
    })
}

fn detect_misbehaviour(deps: Deps<'_>, verified: &Verified) -> bool {
    match verified {
        Verified::Fork { .. } => true,
        Verified::Header(h) => match store::consensus_state_ro(deps.storage, h.header.slot_index) {
            Some(existing) => existing.ledger_hash != h.header.ledger_hash.to_vec(),
            None => false,
        },
    }
}

fn validate_quorum_configs(cs: &ClientState) -> Result<(), ContractError> {
    if cs.quorum_configs.is_empty() {
        return Err(ContractError::NoQuorumConfigs);
    }

    for (i, config) in cs.quorum_configs.iter().enumerate() {
        if cs.quorum_configs[..i]
            .iter()
            .any(|other| other.valid_from == config.valid_from)
        {
            return Err(ContractError::AmbiguousQuorumConfig {
                valid_from: config.valid_from,
            });
        }

        QuorumSet::decode(&config.quorum_set_xdr)?.check_sane(true)?;
    }

    Ok(())
}

fn applicable_quorum_set(cs: &ClientState, slot: u64) -> Result<QuorumSet, ContractError> {
    let mut applicable = cs.quorum_configs.iter().filter(|c| c.valid_from <= slot);
    let mut chosen = applicable
        .next()
        .ok_or(ContractError::NoApplicableQuorumConfig { slot })?;

    for config in applicable {
        if config.valid_from == chosen.valid_from {
            return Err(ContractError::AmbiguousQuorumConfig {
                valid_from: config.valid_from,
            });
        }

        if config.valid_from > chosen.valid_from {
            chosen = config;
        }
    }

    let qset = QuorumSet::decode(&chosen.quorum_set_xdr)?;

    qset.check_sane(true)?;

    Ok(qset)
}

fn update_state(
    deps: DepsMut<'_>,
    env: Env,
    msg: UpdateStateMsg,
) -> Result<UpdateStateResult, ContractError> {
    let verified = match verify_client_message(deps.as_ref(), &msg.client_message)? {
        Verified::Header(h) => h,
        Verified::Fork { .. } => return Err(ContractError::ForkEvidenceInUpdate),
    };

    let mut cs = require_client_state(deps.as_ref())?;

    if is_expired(deps.as_ref(), &env, &cs) {
        return Err(ContractError::ClientExpired);
    }

    let new_consensus = ConsensusState {
        timestamp: verified.header.timestamp,
        ledger_hash: verified.header.ledger_hash.to_vec(),
        root: verified.root.map(|r| r.to_vec()).unwrap_or_default(),
    };

    if let Some(existing) = store::consensus_state(deps.storage, verified.header.slot_index) {
        if existing != new_consensus {
            return Err(ContractError::ConsensusStateConflict {
                height: verified.header.slot_index,
            });
        }
    }

    if let Some(previous_slot) = verified.header.slot_index.checked_sub(1) {
        if let Some(previous) = store::consensus_state(deps.storage, previous_slot) {
            if previous.ledger_hash != verified.header.previous_ledger_hash {
                return Err(ContractError::PreviousLedgerMismatch {
                    slot: verified.header.slot_index,
                    previous: previous_slot,
                    expected: hex_encode(&previous.ledger_hash),
                    found: hex_encode(&verified.header.previous_ledger_hash),
                });
            }
        }
    }

    let latest = cs
        .latest_height
        .as_ref()
        .map(|h| h.revision_height)
        .unwrap_or(0);

    if verified.header.slot_index > latest {
        if let Some(previous) = store::consensus_state(deps.storage, latest) {
            if verified.header.timestamp <= previous.timestamp {
                return Err(ContractError::NonMonotonicCloseTime {
                    slot: verified.header.slot_index,
                    close_time: verified.header.timestamp,
                    latest,
                    previous: previous.timestamp,
                });
            }
        }
    }

    store::set_consensus_state(deps.storage, verified.header.slot_index, &new_consensus);

    if verified.header.slot_index > latest {
        cs.latest_height = Some(WireHeight {
            revision_number: 0,
            revision_height: verified.header.slot_index,
        });
        store::set_client_state(deps.storage, &cs);
    }

    Ok(UpdateStateResult {
        heights: vec![MsgHeight {
            revision_number: 0,
            revision_height: verified.header.slot_index,
        }],
    })
}

fn update_state_on_misbehaviour(
    deps: DepsMut<'_>,
    _env: Env,
    msg: UpdateStateOnMisbehaviourMsg,
) -> Result<(), ContractError> {
    let verified = verify_client_message(deps.as_ref(), &msg.client_message)?;

    if !detect_misbehaviour(deps.as_ref(), &verified) {
        return Err(ContractError::MisbehaviourNotAFork);
    }

    let at = match &verified {
        Verified::Fork { first, .. } => first.header.slot_index,
        Verified::Header(h) => h.header.slot_index,
    };

    let mut cs = require_client_state_mut(deps.as_ref())?;

    cs.frozen_height = Some(WireHeight {
        revision_number: 0,
        revision_height: at.max(1),
    });
    store::set_client_state(deps.storage, &cs);

    Ok(())
}

fn check_for_misbehaviour(
    deps: DepsMut<'_>,
    _env: Env,
    msg: CheckForMisbehaviourMsg,
) -> Result<CheckForMisbehaviourResult, ContractError> {
    let cs = require_client_state(deps.as_ref())?;

    if cs.frozen_height.is_some() {
        return Ok(CheckForMisbehaviourResult {
            found_misbehaviour: false,
        });
    }

    let verified = verify_client_message(deps.as_ref(), &msg.client_message)?;

    Ok(CheckForMisbehaviourResult {
        found_misbehaviour: detect_misbehaviour(deps.as_ref(), &verified),
    })
}

fn verify_membership(
    deps: DepsMut<'_>,
    _env: Env,
    msg: VerifyMembershipMsg,
) -> Result<(), ContractError> {
    let cs = require_client_state_mut(deps.as_ref())?;

    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }

    let consensus = require_consensus_state(deps.as_ref(), msg.height.revision_height)?;
    let root: [u8; HASH_SIZE] =
        consensus
            .root
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::StateRootNotBound {
                height: msg.height.revision_height,
            })?;
    let key = concat_path(&msg.merkle_path.key_path);
    let (proof_key, proof_value, siblings) = decode_membership_proof(msg.proof.as_slice())?;
    let key_match = proof_key == key;
    let value_match = proof_value.as_slice() == sha256(msg.value.as_slice()).as_slice();
    let computed_root = if siblings.len() == TREE_DEPTH && !msg.value.is_empty() {
        let leaf = leaf_hash(sha256(&key), sha256(msg.value.as_slice()));

        Some(fold_siblings(key_index(&key), leaf, &siblings))
    } else {
        None
    };

    let root_match = computed_root.map(|r| r == root).unwrap_or(false);

    if key_match && value_match && root_match {
        return Ok(());
    }

    Err(ContractError::MembershipMismatch {
        key_match,
        value_match,
        siblings: siblings.len(),
        height: msg.height.revision_height,
        req_key: hex_encode(&key),
        proof_key: hex_encode(&proof_key),
        value_len: msg.value.len(),
        proof_value_len: proof_value.len(),
        stored_root: hex_encode(&root),
        computed_root: computed_root.map(|r| hex_encode(&r)).unwrap_or_default(),
    })
}

fn verify_non_membership(
    deps: DepsMut<'_>,
    _env: Env,
    msg: VerifyNonMembershipMsg,
) -> Result<(), ContractError> {
    let cs = require_client_state_mut(deps.as_ref())?;

    if let Some(h) = cs.frozen_height.as_ref() {
        return Err(ContractError::Frozen {
            height: h.revision_height,
        });
    }

    let consensus = require_consensus_state(deps.as_ref(), msg.height.revision_height)?;
    let root: [u8; HASH_SIZE] =
        consensus
            .root
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::StateRootNotBound {
                height: msg.height.revision_height,
            })?;
    let key = concat_path(&msg.merkle_path.key_path);
    let (proof_key, siblings) = decode_non_membership_proof(msg.proof.as_slice())?;

    if proof_key != key {
        return Err(ContractError::MerkleVerificationFailed);
    }

    if !verify_non_membership_raw(&root, &key, &siblings) {
        return Err(ContractError::MerkleVerificationFailed);
    }

    Ok(())
}

fn concat_path(path: &[cosmwasm_std::Binary]) -> Vec<u8> {
    let total: usize = path.iter().map(|b| b.len()).sum();
    let mut out = Vec::with_capacity(total);

    for chunk in path {
        out.extend_from_slice(chunk.as_slice());
    }

    out
}

fn decode_header(bytes: &[u8]) -> Result<StellarHeader, ContractError> {
    StellarHeader::decode(bytes).map_err(|e| ContractError::InvalidWire(format!("header: {e}")))
}

fn require_client_state(deps: Deps<'_>) -> Result<ClientState, ContractError> {
    store::client_state(deps.storage).ok_or(ContractError::NotInitialised)
}

fn require_client_state_mut(deps: Deps<'_>) -> Result<ClientState, ContractError> {
    require_client_state(deps)
}

fn require_consensus_state(deps: Deps<'_>, height: u64) -> Result<ConsensusState, ContractError> {
    store::consensus_state(deps.storage, height)
        .ok_or(ContractError::ConsensusStateMissing { height })
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<Binary, ContractError> {
    to_json_binary(value).map_err(|e| ContractError::Std(StdError::generic_err(e.to_string())))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);

    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }

    s
}
