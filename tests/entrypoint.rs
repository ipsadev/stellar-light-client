use cosmwasm_std::{
    testing::{message_info, mock_dependencies, mock_env},
    Binary, Response,
};
use ed25519_dalek::{Signer, SigningKey};
use prost::Message;
use sha2::{Digest, Sha256};
use stellar_light_client::{
    entrypoint::{instantiate, query, sudo},
    error::ContractError,
    msg::{
        CheckForMisbehaviourMsg, CheckForMisbehaviourResult, ClientStatus, Height as MsgHeight,
        InstantiateMsg, LatestHeightResult, MerklePath, QueryMsg, StatusResult, SudoMsg,
        TimestampAtHeightResult, UpdateStateMsg, UpdateStateOnMisbehaviourMsg, UpdateStateResult,
        VerifyMembershipMsg, VerifyNonMembershipMsg,
    },
    types::{
        ClientState, ConsensusState, Height as WireHeight, Misbehaviour, QuorumConfig,
        StellarHeader,
    },
};

const CHAIN_ID: &str = "stellar-testnet";
const ROOT_INIT: [u8; 32] = [0x11; 32];
const LEDGER_HASH_INIT: [u8; 32] = [0xaa; 32];

fn network_id() -> [u8; 32] {
    Sha256::digest(b"Test SDF Network ; September 2015").into()
}

fn u32be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn var_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = u32be(data.len() as u32).to_vec();

    out.extend_from_slice(data);
    out.extend(core::iter::repeat(0u8).take((4 - (data.len() % 4)) % 4));
    out
}

fn node_id_xdr(public: &[u8; 32]) -> Vec<u8> {
    let mut out = u32be(0).to_vec();

    out.extend_from_slice(public);
    out
}

fn validator(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn quorum_set_xdr(publics: &[[u8; 32]], threshold: u32) -> Vec<u8> {
    let mut out = u32be(threshold).to_vec();

    out.extend_from_slice(&u32be(publics.len() as u32));

    for p in publics {
        out.extend_from_slice(&node_id_xdr(p));
    }

    out.extend_from_slice(&u32be(0));
    out
}

fn stellar_value_xdr(close_time: u64) -> Vec<u8> {
    let mut out = [7u8; 32].to_vec();

    out.extend_from_slice(&close_time.to_be_bytes());
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&u32be(0));
    out
}

fn ledger_header_with_results(seq: u32, value: &[u8], tx_set_result_hash: [u8; 32]) -> Vec<u8> {
    ledger_header_full(seq, value, tx_set_result_hash, [0u8; 32])
}

fn ledger_header_full(
    seq: u32,
    value: &[u8],
    tx_set_result_hash: [u8; 32],
    previous_ledger_hash: [u8; 32],
) -> Vec<u8> {
    let mut out = u32be(23).to_vec();

    out.extend_from_slice(&previous_ledger_hash);
    out.extend_from_slice(value);
    out.extend_from_slice(&tx_set_result_hash);
    out.extend_from_slice(&[2u8; 32]);
    out.extend_from_slice(&u32be(seq));
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&u32be(0));
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&u32be(100));
    out.extend_from_slice(&u32be(5_000_000));
    out.extend_from_slice(&u32be(1000));

    for _ in 0..4 {
        out.extend_from_slice(&[0u8; 32]);
    }

    out.extend_from_slice(&u32be(0));
    out
}

fn envelope_xdr(key: &SigningKey, slot: u64, value: &[u8], qset_hash: [u8; 32]) -> Vec<u8> {
    let public = key.verifying_key().to_bytes();
    let mut statement = node_id_xdr(&public);

    statement.extend_from_slice(&slot.to_be_bytes());
    statement.extend_from_slice(&u32be(2));
    statement.extend_from_slice(&u32be(1));
    statement.extend_from_slice(&var_bytes(value));
    statement.extend_from_slice(&u32be(1));
    statement.extend_from_slice(&qset_hash);
    let mut payload = network_id().to_vec();

    payload.extend_from_slice(&[0, 0, 0, 1]);
    payload.extend_from_slice(&statement);
    let signature = key.sign(&payload).to_bytes();
    let mut out = statement;

    out.extend_from_slice(&var_bytes(&signature));
    out
}

fn validators() -> Vec<SigningKey> {
    (1..=4).map(validator).collect()
}

fn quorum_config() -> Vec<u8> {
    let publics: Vec<[u8; 32]> = validators()
        .iter()
        .map(|k| k.verifying_key().to_bytes())
        .collect();

    quorum_set_xdr(&publics, 3)
}

fn fresh_client_state(latest_height: u64) -> ClientState {
    ClientState {
        chain_id: CHAIN_ID.to_string(),
        latest_height: Some(WireHeight {
            revision_number: 0,
            revision_height: latest_height,
        }),
        frozen_height: None,
        quorum_configs: vec![QuorumConfig {
            quorum_set_xdr: quorum_config(),
            valid_from: 0,
        }],
        proof_specs: vec![],
        network_id: network_id().to_vec(),
        max_consensus_age: 0,
        router_contract_id: Vec::new(),
        root_event_topic: Vec::new(),
    }
}

fn fresh_consensus_state(ts: u64, ledger_hash: [u8; 32], root: [u8; 32]) -> ConsensusState {
    ConsensusState {
        timestamp: ts,
        ledger_hash: ledger_hash.to_vec(),
        root: root.to_vec(),
    }
}

fn encode<T: Message>(m: &T) -> Binary {
    Binary::new(m.encode_to_vec())
}

fn tx_set_xdr(previous_ledger_hash: [u8; 32]) -> Vec<u8> {
    let mut out = u32be(1).to_vec();

    out.extend_from_slice(&previous_ledger_hash);
    out.extend_from_slice(&u32be(0));
    out
}

fn header(slot: u64) -> StellarHeader {
    header_at(slot, 1_700_000_000 + slot)
}

fn fork_evidence(slot: u64) -> Misbehaviour {
    Misbehaviour {
        client_id: "08-wasm-0".to_string(),
        header_1: Some(header_at(slot, 1_700_000_000 + slot)),
        header_2: Some(header_at(slot, 1_800_000_000 + slot)),
    }
}

fn freeze_with_fork(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
    slot: u64,
) {
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
            client_message: encode(&fork_evidence(slot)),
        }),
    )
    .expect("valid fork evidence freezes the client");
}

fn do_instantiate(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
) {
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let cs = fresh_client_state(100);
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };

    instantiate(deps.as_mut(), env, info, msg).expect("instantiate");
}

#[test]
fn instantiate_stores_state_and_consensus() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let latest: LatestHeightResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::LatestHeight {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(latest.height.revision_height, 100);
    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(status.status, ClientStatus::Active);
    let ts: TimestampAtHeightResult = serde_json::from_slice(
        query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::TimestampAtHeight {
                height: MsgHeight {
                    revision_number: 0,
                    revision_height: 100,
                },
            },
        )
        .unwrap()
        .as_slice(),
    )
    .unwrap();

    assert_eq!(ts.timestamp, 1_000_000);
}

#[test]
fn instantiate_rejects_double_instantiation() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let cs = fresh_client_state(100);
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };

    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let err = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();

    assert!(matches!(err, ContractError::AlreadyInitialised));
}

fn instantiate_with(cs: ClientState) -> Result<Response, ContractError> {
    let mut deps = mock_dependencies();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let cons = fresh_consensus_state(1_000_000, LEDGER_HASH_INIT, ROOT_INIT);
    let msg = InstantiateMsg {
        client_state: encode(&cs),
        consensus_state: encode(&cons),
        checksum: Binary::default(),
    };

    instantiate(deps.as_mut(), mock_env(), info, msg)
}

#[test]
fn instantiate_rejects_an_empty_quorum_configuration() {
    let mut cs = fresh_client_state(100);

    cs.quorum_configs.clear();
    let err = instantiate_with(cs).unwrap_err();

    assert!(matches!(err, ContractError::NoQuorumConfigs));
}

#[test]
fn instantiate_rejects_two_configurations_sharing_a_valid_from() {
    let mut cs = fresh_client_state(100);

    cs.quorum_configs.push(QuorumConfig {
        quorum_set_xdr: quorum_config(),
        valid_from: 0,
    });
    let err = instantiate_with(cs).unwrap_err();

    assert!(
        matches!(err, ContractError::AmbiguousQuorumConfig { valid_from: 0 }),
        "a tie must be rejected rather than resolved by list order, got {err:?}"
    );
}

#[test]
fn instantiate_accepts_a_rotation_with_distinct_ranges() {
    let mut cs = fresh_client_state(100);

    cs.quorum_configs.push(QuorumConfig {
        quorum_set_xdr: quorum_config(),
        valid_from: 50,
    });
    assert!(instantiate_with(cs).is_ok());
}

#[test]
fn instantiate_rejects_an_unsound_quorum_set() {
    let mut cs = fresh_client_state(100);

    cs.quorum_configs[0].quorum_set_xdr = quorum_set_xdr(&[[1u8; 32], [2u8; 32]], 0);
    let err = instantiate_with(cs).unwrap_err();

    assert!(matches!(err, ContractError::InvalidWire(_)), "got {err:?}");
}

#[test]
fn update_state_advances_height_when_chain_intact() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let hdr = header(105);
    let msg = SudoMsg::UpdateState(UpdateStateMsg {
        client_message: encode(&hdr),
    });
    let resp = sudo(deps.as_mut(), mock_env(), msg).expect("update_state");
    let result: UpdateStateResult = serde_json::from_slice(resp.data.unwrap().as_slice()).unwrap();

    assert_eq!(result.heights.len(), 1);
    assert_eq!(result.heights[0].revision_height, 105);
    let latest: LatestHeightResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::LatestHeight {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(latest.height.revision_height, 105);
}

fn header_at(slot: u64, close_time: u64) -> StellarHeader {
    header_after(slot, close_time, [0u8; 32])
}

fn header_after(slot: u64, close_time: u64, previous_ledger_hash: [u8; 32]) -> StellarHeader {
    let qset = quorum_config();
    let qset_hash: [u8; 32] = Sha256::digest(&qset).into();
    let value = stellar_value_xdr(close_time);
    let header_xdr = ledger_header_full(slot as u32, &value, [9u8; 32], previous_ledger_hash);
    let ledger_hash: [u8; 32] = Sha256::digest(&header_xdr).into();
    let next_tx_set = tx_set_xdr(ledger_hash);
    let next_tx_set_hash: [u8; 32] = Sha256::digest(&next_tx_set).into();
    let mut next_value = next_tx_set_hash.to_vec();

    next_value.extend_from_slice(&(close_time + 5).to_be_bytes());
    next_value.extend_from_slice(&u32be(0));
    next_value.extend_from_slice(&u32be(0));
    StellarHeader {
        slot_index: slot,
        ledger_header_xdr: header_xdr,
        scp_envelopes: validators()
            .iter()
            .map(|k| envelope_xdr(k, slot, &value, qset_hash))
            .collect(),
        quorum_sets_xdr: vec![qset],
        next_scp_envelopes: validators()
            .iter()
            .map(|k| envelope_xdr(k, slot + 1, &next_value, qset_hash))
            .collect(),
        next_tx_set_xdr: next_tx_set,
        state_root_proof: None,
    }
}

#[test]
fn update_state_rejects_a_header_that_breaks_the_chain() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    push_header(&mut deps, &header(105)).expect("first update");
    let err = push_header(&mut deps, &header(106)).unwrap_err();

    assert!(
        matches!(
            err,
            ContractError::PreviousLedgerMismatch {
                slot: 106,
                previous: 105,
                ..
            }
        ),
        "a header whose predecessor is not the trusted ledger at N-1 must be refused, got {err:?}"
    );
}

#[test]
fn update_state_accepts_a_header_that_names_the_trusted_predecessor() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let first = header(105);
    let first_hash: [u8; 32] = Sha256::digest(&first.ledger_header_xdr).into();

    push_header(&mut deps, &first).expect("first update");
    push_header(&mut deps, &header_after(106, 1_700_000_106, first_hash))
        .expect("a header naming the trusted predecessor must be accepted");
}

#[test]
fn update_state_accepts_a_gap_in_stored_heights() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    push_header(&mut deps, &header(105)).expect("first update");
    push_header(&mut deps, &header(110))
        .expect("with nothing stored at 109 there is no predecessor to contradict");
}

#[test]
fn check_for_misbehaviour_detects_two_quorum_backed_ledgers_at_one_slot() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let slot = 105u64;
    let first = header_at(slot, 1_700_000_000 + slot);

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&first),
        }),
    )
    .expect("first update");
    let conflicting = header_at(slot, 1_900_000_000);
    let res = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&conflicting),
        }),
    )
    .expect("misbehaviour check");
    let parsed: CheckForMisbehaviourResult = cosmwasm_std::from_json(res.data.unwrap()).unwrap();

    assert!(
        parsed.found_misbehaviour,
        "two quorum-backed ledgers for one slot is fork evidence"
    );
}

#[test]
fn check_for_misbehaviour_ignores_a_replay_of_the_same_ledger() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let hdr = header_at(105, 1_700_000_105);

    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .expect("update");
    let res = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&hdr),
        }),
    )
    .expect("misbehaviour check");
    let parsed: CheckForMisbehaviourResult = cosmwasm_std::from_json(res.data.unwrap()).unwrap();

    assert!(!parsed.found_misbehaviour);
}

#[test]
fn update_state_on_misbehaviour_freezes_client() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    freeze_with_fork(&mut deps, 105);
    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(status.status, ClientStatus::Frozen);
}

#[test]
fn update_state_rejects_when_frozen() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    freeze_with_fork(&mut deps, 105);
    let hdr = header(105);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::Frozen { .. }));
}

#[test]
fn verify_membership_rejects_when_consensus_state_missing() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 999,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
            value: Binary::default(),
        }),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        ContractError::ConsensusStateMissing { height: 999 }
    ));
}

#[test]
fn verify_membership_rejects_when_proof_bytes_are_empty() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
            value: Binary::default(),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::MerkleVerificationFailed));
}

#[test]
fn verify_membership_accepts_valid_proof_against_matching_root() {
    use stellar_light_client::{
        merkle::{CommitmentProof, ExistenceProof, InnerOp, LeafOp, MerkleProof, Proof},
        smt::{fold_siblings, key_index, leaf_hash, sha256, HASH_SIZE, TREE_DEPTH},
    };

    let key = b"10-stellar-0\x01\x00\x00\x00\x00\x00\x00\x00\x07";
    let value = b"committed-bytes";
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [0x40u8.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let leaf = leaf_hash(sha256(key), sha256(value));
    let root = fold_siblings(key_index(key), leaf, &siblings);
    let idx = key_index(key);
    let mut path_ops = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = idx;

    for sibling in &siblings {
        let is_left_child = sub_idx & 1 == 0;

        path_ops.push(if is_left_child {
            InnerOp {
                hash: 1,
                prefix: vec![0x01],
                suffix: sibling.to_vec(),
            }
        } else {
            let mut prefix = Vec::with_capacity(1 + HASH_SIZE);

            prefix.push(0x01);
            prefix.extend_from_slice(sibling);
            InnerOp {
                hash: 1,
                prefix,
                suffix: Vec::new(),
            }
        });
        sub_idx >>= 1;
    }

    let proof_bytes = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: key.to_vec(),
                value: sha256(value).to_vec(),
                leaf: Some(LeafOp::default()),
                path: path_ops,
            })),
        }],
    }
    .encode_to_vec();
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let cs = fresh_client_state(100);
    let cons = ConsensusState {
        timestamp: 1_000_000,
        ledger_hash: LEDGER_HASH_INIT.to_vec(),
        root: root.to_vec(),
    };

    instantiate(
        deps.as_mut(),
        env,
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&cons),
            checksum: Binary::default(),
        },
    )
    .unwrap();
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::new(proof_bytes),
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key.to_vec())],
            },
            value: Binary::new(value.to_vec()),
        }),
    )
    .expect("verify_membership accepts a valid proof");
}

#[test]
fn verify_non_membership_rejects_when_frozen() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    freeze_with_fork(&mut deps, 105);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyNonMembership(VerifyNonMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 100,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::Frozen { .. }));
}

#[test]
fn update_state_rejects_envelope_signed_against_different_network() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let slot = 105u64;
    let qset = quorum_config();
    let qset_hash: [u8; 32] = Sha256::digest(&qset).into();
    let value = stellar_value_xdr(1_700_000_000 + slot);
    let foreign_network = [0x55u8; 32];
    let mut hdr = header(slot);

    hdr.scp_envelopes = validators()
        .iter()
        .map(|key| {
            let public = key.verifying_key().to_bytes();
            let mut statement = node_id_xdr(&public);

            statement.extend_from_slice(&slot.to_be_bytes());
            statement.extend_from_slice(&u32be(2));
            statement.extend_from_slice(&u32be(1));
            statement.extend_from_slice(&var_bytes(&value));
            statement.extend_from_slice(&u32be(1));
            statement.extend_from_slice(&qset_hash);
            let mut payload = foreign_network.to_vec();

            payload.extend_from_slice(&[0, 0, 0, 1]);
            payload.extend_from_slice(&statement);
            let signature = key.sign(&payload).to_bytes();
            let mut out = statement;

            out.extend_from_slice(&var_bytes(&signature));
            out
        })
        .collect();
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();

    assert!(
        matches!(err, ContractError::UnauthenticatedEnvelope { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn unverified_evidence_cannot_freeze_the_client() {
    for evidence in [
        Binary::default(),
        Binary::new(vec![0xde, 0xad, 0xbe, 0xef]),
        encode(&header(105)), // valid header, but no consensus state to contradict
    ] {
        let mut deps = mock_dependencies();

        do_instantiate(&mut deps);
        let res = sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg {
                client_message: evidence,
            }),
        );

        assert!(
            res.is_err(),
            "evidence without a proven fork must not freeze"
        );
        let status: StatusResult = serde_json::from_slice(
            query(deps.as_ref(), mock_env(), QueryMsg::Status {})
                .unwrap()
                .as_slice(),
        )
        .unwrap();

        assert_eq!(status.status, ClientStatus::Active, "client must stay live");
    }
}

#[test]
fn fork_evidence_is_detected_as_misbehaviour() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let res = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&fork_evidence(105)),
        }),
    )
    .unwrap();
    let parsed: CheckForMisbehaviourResult = cosmwasm_std::from_json(res.data.unwrap()).unwrap();

    assert!(
        parsed.found_misbehaviour,
        "two valid headers for one slot is equivocation"
    );
}

#[test]
fn freezing_records_the_slot_the_fork_happened_at() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    freeze_with_fork(&mut deps, 107);
    let cs = stellar_light_client::store::client_state(deps.as_ref().storage).unwrap();

    assert_eq!(cs.frozen_height.unwrap().revision_height, 107);
}

#[test]
fn misbehaviour_across_two_slots_is_not_a_fork() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let evidence = Misbehaviour {
        client_id: "08-wasm-0".to_string(),
        header_1: Some(header(105)),
        header_2: Some(header(106)),
    };

    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&evidence),
        }),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        ContractError::MisbehaviourSlotMismatch { .. }
    ));
}

#[test]
fn identical_headers_are_not_a_fork() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let evidence = Misbehaviour {
        client_id: "08-wasm-0".to_string(),
        header_1: Some(header(105)),
        header_2: Some(header(105)),
    };

    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::CheckForMisbehaviour(CheckForMisbehaviourMsg {
            client_message: encode(&evidence),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::MisbehaviourNotAFork));
}

#[test]
fn fork_evidence_cannot_advance_the_client() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&fork_evidence(105)),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::ForkEvidenceInUpdate));
}

#[test]
fn status_reports_expired_once_the_consensus_state_is_too_old() {
    let mut deps = mock_dependencies();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let mut cs = fresh_client_state(100);

    cs.max_consensus_age = 3600;
    instantiate(
        deps.as_mut(),
        mock_env(),
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&fresh_consensus_state(
                1_000_000,
                LEDGER_HASH_INIT,
                ROOT_INIT,
            )),
            checksum: Binary::default(),
        },
    )
    .unwrap();
    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(status.status, ClientStatus::Expired);
}

#[test]
fn an_expired_client_cannot_be_advanced() {
    let mut deps = mock_dependencies();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let mut cs = fresh_client_state(100);

    cs.max_consensus_age = 3600;
    instantiate(
        deps.as_mut(),
        mock_env(),
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&fresh_consensus_state(
                1_000_000,
                LEDGER_HASH_INIT,
                ROOT_INIT,
            )),
            checksum: Binary::default(),
        },
    )
    .unwrap();
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&header(101)),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::ClientExpired), "got {err:?}");
}

#[test]
fn a_live_client_is_still_advanced_while_max_consensus_age_is_set() {
    let mut deps = mock_dependencies();
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let mut cs = fresh_client_state(100);

    cs.max_consensus_age = 3600;
    instantiate(
        deps.as_mut(),
        mock_env(),
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&fresh_consensus_state(
                mock_env().block.time.seconds(),
                LEDGER_HASH_INIT,
                ROOT_INIT,
            )),
            checksum: Binary::default(),
        },
    )
    .unwrap();
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&header(105)),
        }),
    )
    .expect("a client inside max_consensus_age must still advance");
}

#[test]
fn max_consensus_age_of_zero_disables_expiry() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps); // fresh_client_state uses max_consensus_age = 0
    let status: StatusResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::Status {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    assert_eq!(status.status, ClientStatus::Active);
}

#[test]
fn advancing_the_client_requires_time_to_move_forward() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    let hdr = header_after(101, 999_999, LEDGER_HASH_INIT);
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&hdr),
        }),
    )
    .unwrap_err();

    assert!(
        matches!(err, ContractError::NonMonotonicCloseTime { .. }),
        "unexpected error: {err}"
    );
}

#[test]
fn backfilling_an_older_slot_is_still_allowed() {
    let mut deps = mock_dependencies();

    do_instantiate(&mut deps);
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&header(110)),
        }),
    )
    .expect("advance to 110");
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(&header(105)),
        }),
    )
    .expect("backfilling 105 must still work");
    let latest: LatestHeightResult = serde_json::from_slice(
        query(deps.as_ref(), mock_env(), QueryMsg::LatestHeight {})
            .unwrap()
            .as_slice(),
    )
    .unwrap();

    assert_eq!(
        latest.height.revision_height, 110,
        "latest must not regress"
    );
}

const ROUTER_ID: [u8; 32] = [0x5a; 32];
const ROOT_TOPIC: &[u8] = b"ibc_root";

fn scval_void() -> Vec<u8> {
    u32be(1).to_vec()
}

fn scval_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = u32be(13).to_vec(); // SCV_BYTES
    out.extend_from_slice(&var_bytes(data));

    out
}

fn scval_symbol(sym: &[u8]) -> Vec<u8> {
    let mut out = u32be(15).to_vec(); // SCV_SYMBOL
    out.extend_from_slice(&var_bytes(sym));

    out
}

fn contract_event(contract_id: &[u8; 32], topic: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = u32be(0).to_vec(); // ExtensionPoint v = 0
    out.extend_from_slice(&u32be(1)); // contractID present
    out.extend_from_slice(contract_id);

    out.extend_from_slice(&u32be(1)); // ContractEventType::CONTRACT
    out.extend_from_slice(&u32be(0)); // body v = 0
    out.extend_from_slice(&u32be(1)); // topics.len()
    out.extend_from_slice(&scval_symbol(topic));
    out.extend_from_slice(data);
    out
}

fn success_preimage(events: &[Vec<u8>]) -> Vec<u8> {
    let mut out = scval_void(); // returnValue — deliberately not the root
    out.extend_from_slice(&u32be(events.len() as u32));

    for e in events {
        out.extend_from_slice(e);
    }

    out
}

fn invoke_result_pair(tx_hash: [u8; 32], success_hash: [u8; 32]) -> Vec<u8> {
    let mut out = tx_hash.to_vec();

    out.extend_from_slice(&0u64.to_be_bytes()); // feeCharged
    out.extend_from_slice(&u32be(0)); // txSUCCESS
    out.extend_from_slice(&u32be(1)); // one operation result
    out.extend_from_slice(&u32be(0)); // opINNER
    out.extend_from_slice(&u32be(24)); // INVOKE_HOST_FUNCTION
    out.extend_from_slice(&u32be(0)); // INVOKE_HOST_FUNCTION_SUCCESS
    out.extend_from_slice(&success_hash);
    out.extend_from_slice(&u32be(0)); // TransactionResult ext
    out
}

fn state_root_proof(events: &[Vec<u8>]) -> (stellar_light_client::types::StateRootProof, [u8; 32]) {
    let preimage = success_preimage(events);
    let success_hash: [u8; 32] = Sha256::digest(&preimage).into();
    let pairs = vec![
        invoke_result_pair([0x11; 32], [0x99; 32]),
        invoke_result_pair([0x22; 32], success_hash),
    ];
    let mut hasher = Sha256::new();

    hasher.update(u32be(pairs.len() as u32));

    for p in &pairs {
        hasher.update(p);
    }

    let tx_set_result_hash: [u8; 32] = hasher.finalize().into();

    (
        stellar_light_client::types::StateRootProof {
            result_pairs: pairs,
            result_index: 1,
            success_preimage_xdr: preimage,
        },
        tx_set_result_hash,
    )
}

fn header_with_root(slot: u64, root: [u8; 32]) -> StellarHeader {
    header_with_events(
        slot,
        &[contract_event(&ROUTER_ID, ROOT_TOPIC, &scval_bytes(&root))],
    )
}

fn header_with_events(slot: u64, events: &[Vec<u8>]) -> StellarHeader {
    let (proof, tx_set_result_hash) = state_root_proof(events);
    let close_time = 1_700_000_000 + slot;
    let qset = quorum_config();
    let qset_hash: [u8; 32] = Sha256::digest(&qset).into();
    let value = stellar_value_xdr(close_time);
    let header_xdr = ledger_header_with_results(slot as u32, &value, tx_set_result_hash);
    let ledger_hash: [u8; 32] = Sha256::digest(&header_xdr).into();
    let next_tx_set = tx_set_xdr(ledger_hash);
    let next_tx_set_hash: [u8; 32] = Sha256::digest(&next_tx_set).into();
    let mut next_value = next_tx_set_hash.to_vec();

    next_value.extend_from_slice(&(close_time + 5).to_be_bytes());
    next_value.extend_from_slice(&u32be(0));
    next_value.extend_from_slice(&u32be(0));
    StellarHeader {
        slot_index: slot,
        ledger_header_xdr: header_xdr,
        scp_envelopes: validators()
            .iter()
            .map(|k| envelope_xdr(k, slot, &value, qset_hash))
            .collect(),
        quorum_sets_xdr: vec![qset],
        next_scp_envelopes: validators()
            .iter()
            .map(|k| envelope_xdr(k, slot + 1, &next_value, qset_hash))
            .collect(),
        next_tx_set_xdr: next_tx_set,
        state_root_proof: Some(proof),
    }
}

fn instantiate_with_router(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
) {
    let info = message_info(&deps.api.addr_make("creator"), &[]);
    let mut cs = fresh_client_state(100);

    cs.router_contract_id = ROUTER_ID.to_vec();
    cs.root_event_topic = ROOT_TOPIC.to_vec();
    instantiate(
        deps.as_mut(),
        mock_env(),
        info,
        InstantiateMsg {
            client_state: encode(&cs),
            consensus_state: encode(&fresh_consensus_state(
                1_000_000,
                LEDGER_HASH_INIT,
                ROOT_INIT,
            )),
            checksum: Binary::default(),
        },
    )
    .expect("instantiate");
}

fn push_header(
    deps: &mut cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        cosmwasm_std::testing::MockApi,
        cosmwasm_std::testing::MockQuerier,
    >,
    hdr: &StellarHeader,
) -> Result<(), ContractError> {
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::UpdateState(UpdateStateMsg {
            client_message: encode(hdr),
        }),
    )
    .map(|_| ())
}

#[test]
fn state_root_is_bound_from_the_routers_event() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let root = [0x7c; 32];

    push_header(&mut deps, &header_with_root(105, root)).expect("verifies");
    let cons = stellar_light_client::store::consensus_state(deps.as_ref().storage, 105).unwrap();

    assert_eq!(cons.root, root.to_vec(), "the proven root must be stored");
}

#[test]
fn a_root_emitted_by_another_contract_is_rejected() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let impostor = [0xee; 32];
    let hdr = header_with_events(
        105,
        &[contract_event(
            &impostor,
            ROOT_TOPIC,
            &scval_bytes(&[0xff; 32]),
        )],
    );
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(
        matches!(err, ContractError::RouterEventMissing),
        "unexpected: {err}"
    );
}

#[test]
fn the_routers_event_is_still_honoured_inside_another_contracts_invocation() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let root = [0x4d; 32];
    let hdr = header_with_events(
        105,
        &[
            contract_event(&[0xee; 32], ROOT_TOPIC, &scval_bytes(&[0xff; 32])),
            contract_event(&ROUTER_ID, ROOT_TOPIC, &scval_bytes(&root)),
        ],
    );

    push_header(&mut deps, &hdr).expect("verifies");
    let cons = stellar_light_client::store::consensus_state(deps.as_ref().storage, 105).unwrap();

    assert_eq!(cons.root, root.to_vec());
}

#[test]
fn two_router_roots_in_one_invocation_are_ambiguous() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let hdr = header_with_events(
        105,
        &[
            contract_event(&ROUTER_ID, ROOT_TOPIC, &scval_bytes(&[0x01; 32])),
            contract_event(&ROUTER_ID, ROOT_TOPIC, &scval_bytes(&[0x02; 32])),
        ],
    );
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::AmbiguousRouterEvent));
}

#[test]
fn a_tampered_result_set_breaks_the_binding() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let mut hdr = header_with_root(105, [0x7c; 32]);
    let proof = hdr.state_root_proof.as_mut().unwrap();

    *proof.result_pairs[0].last_mut().unwrap() ^= 0x01;
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::TxResultSetMismatch));
}

#[test]
fn a_substituted_success_preimage_is_rejected() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let mut hdr = header_with_root(105, [0x7c; 32]);
    let proof = hdr.state_root_proof.as_mut().unwrap();

    proof.success_preimage_xdr = success_preimage(&[contract_event(
        &ROUTER_ID,
        ROOT_TOPIC,
        &scval_bytes(&[0xab; 32]),
    )]);
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::SuccessPreimageMismatch));
}

#[test]
fn pointing_at_the_wrong_result_is_rejected() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let mut hdr = header_with_root(105, [0x7c; 32]);

    hdr.state_root_proof.as_mut().unwrap().result_index = 0;
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::SuccessPreimageMismatch));
    let mut hdr = header_with_root(106, [0x7c; 32]);

    hdr.state_root_proof.as_mut().unwrap().result_index = 9;
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::ResultIndexOutOfRange { .. }));
}

#[test]
fn a_root_that_is_not_32_bytes_is_rejected() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let hdr = header_with_events(
        105,
        &[contract_event(
            &ROUTER_ID,
            ROOT_TOPIC,
            &scval_bytes(b"short"),
        )],
    );
    let err = push_header(&mut deps, &hdr).unwrap_err();

    assert!(matches!(err, ContractError::StateRootMalformed));
}

#[test]
fn a_header_without_a_proof_leaves_the_root_unbound() {
    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    push_header(&mut deps, &header(105)).expect("consensus still verifies");
    let cons = stellar_light_client::store::consensus_state(deps.as_ref().storage, 105).unwrap();

    assert!(cons.root.is_empty());
    let err = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 105,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::default(),
            merkle_path: MerklePath { key_path: vec![] },
            value: Binary::default(),
        }),
    )
    .unwrap_err();

    assert!(matches!(err, ContractError::StateRootNotBound { .. }));
}

#[test]
fn a_packet_commitment_verifies_against_the_bound_root() {
    use stellar_light_client::{
        merkle::{CommitmentProof, ExistenceProof, InnerOp, LeafOp, MerkleProof, Proof},
        smt::{fold_siblings, key_index, leaf_hash, sha256 as smt_sha256, HASH_SIZE, TREE_DEPTH},
    };

    let mut deps = mock_dependencies();

    instantiate_with_router(&mut deps);
    let mut key = b"10-stellar-0".to_vec();

    key.push(0x01);
    key.extend_from_slice(&7u64.to_be_bytes());
    let value = b"packet-commitment".to_vec();
    let siblings: Vec<[u8; HASH_SIZE]> = (0..TREE_DEPTH)
        .map(|i| [0x40u8.wrapping_add(i as u8); HASH_SIZE])
        .collect();
    let leaf = leaf_hash(smt_sha256(&key), smt_sha256(&value));
    let root = fold_siblings(key_index(&key), leaf, &siblings);
    let mut path = Vec::with_capacity(TREE_DEPTH);
    let mut sub_idx = key_index(&key);

    for sibling in &siblings {
        path.push(if sub_idx & 1 == 0 {
            InnerOp {
                hash: 1,
                prefix: vec![0x01],
                suffix: sibling.to_vec(),
            }
        } else {
            let mut prefix = vec![0x01];

            prefix.extend_from_slice(sibling);
            InnerOp {
                hash: 1,
                prefix,
                suffix: Vec::new(),
            }
        });
        sub_idx >>= 1;
    }

    let proof = MerkleProof {
        proofs: vec![CommitmentProof {
            proof: Some(Proof::Exist(ExistenceProof {
                key: key.clone(),
                value: smt_sha256(&value).to_vec(),
                leaf: Some(LeafOp::default()),
                path,
            })),
        }],
    }
    .encode_to_vec();

    push_header(&mut deps, &header_with_root(105, root)).expect("header verifies");
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyMembership(VerifyMembershipMsg {
            height: MsgHeight {
                revision_number: 0,
                revision_height: 105,
            },
            delay_time_period: 0,
            delay_block_period: 0,
            proof: Binary::new(proof),
            merkle_path: MerklePath {
                key_path: vec![Binary::new(key)],
            },
            value: Binary::new(value),
        }),
    )
    .expect("membership verifies against the bound root");
}
