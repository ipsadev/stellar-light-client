use cosmwasm_std::{
    testing::{mock_dependencies, mock_env},
    OwnedDeps,
};
use stellar_light_client::{
    entrypoint::sudo,
    error::ContractError,
    msg::{MigrateClientStoreMsg, SudoMsg, VerifyUpgradeAndUpdateStateMsg},
    store::{
        client_state_prefixed, set_client_state_prefixed, set_consensus_state_prefixed,
        SUBJECT_PREFIX, SUBSTITUTE_PREFIX,
    },
    types::{ClientState, ConsensusState, Height, QuorumConfig},
};

mod common;

const CHAIN_ID: &str = "stellar-testnet";
const SUBJECT_HEIGHT: u64 = 100;
const SUBSTITUTE_HEIGHT: u64 = 900;

fn height(h: u64) -> Height {
    Height {
        revision_number: 0,
        revision_height: h,
    }
}

fn quorum(valid_from: u64, seed: u8) -> QuorumConfig {
    let validators: Vec<[u8; 32]> = (0..3)
        .map(|i| common::public_of(&common::signing_key(seed.wrapping_add(i))))
        .collect();

    QuorumConfig {
        quorum_set_xdr: common::quorum_set(&validators, 2),
        valid_from,
    }
}

fn client_state(latest: u64, quorum_seed: u8) -> ClientState {
    ClientState {
        chain_id: CHAIN_ID.into(),
        latest_height: Some(height(latest)),
        frozen_height: None,
        quorum_configs: vec![quorum(0, quorum_seed)],
        proof_specs: vec![vec![0x01, 0x02]],
        network_id: vec![0xab; 32],
        max_consensus_age: 1_000_000,
        router_contract_id: vec![0xcd; 32],
        root_event_topic: b"ibc_root".to_vec(),
        generation: 0,
    }
}

fn consensus(seconds_ago: u64, seed: u8) -> ConsensusState {
    ConsensusState {
        timestamp: mock_env().block.time.seconds().saturating_sub(seconds_ago),
        ledger_hash: vec![seed; 32],
        root: vec![seed ^ 0xff; 32],
        generation: 0,
    }
}

type Deps = OwnedDeps<
    cosmwasm_std::testing::MockStorage,
    cosmwasm_std::testing::MockApi,
    cosmwasm_std::testing::MockQuerier,
>;

fn staged(subject: ClientState, substitute: ClientState) -> Deps {
    let mut deps = mock_dependencies();

    set_client_state_prefixed(&mut deps.storage, SUBJECT_PREFIX, &subject);
    set_consensus_state_prefixed(
        &mut deps.storage,
        SUBJECT_PREFIX,
        SUBJECT_HEIGHT,
        &consensus(50_000, 0x11),
    );
    let substitute_height = substitute
        .latest_height
        .as_ref()
        .map(|h| h.revision_height)
        .unwrap_or_default();

    set_client_state_prefixed(&mut deps.storage, SUBSTITUTE_PREFIX, &substitute);
    set_consensus_state_prefixed(
        &mut deps.storage,
        SUBSTITUTE_PREFIX,
        substitute_height,
        &consensus(100, 0x22),
    );
    deps
}

fn migrate(deps: &mut Deps) -> Result<(), ContractError> {
    sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::MigrateClientStore(MigrateClientStoreMsg {}),
    )
    .map(|_| ())
}

#[test]
fn an_expired_subject_takes_the_substitutes_height_and_quorum() {
    let mut subject = client_state(SUBJECT_HEIGHT, 0x01);

    subject.frozen_height = Some(height(SUBJECT_HEIGHT));
    let mut deps = staged(subject, client_state(SUBSTITUTE_HEIGHT, 0x02));

    migrate(&mut deps).expect("migration should succeed");
    let migrated = client_state_prefixed(&deps.storage, SUBJECT_PREFIX).expect("subject present");

    assert_eq!(
        migrated.latest_height.as_ref().map(|h| h.revision_height),
        Some(SUBSTITUTE_HEIGHT)
    );
    assert_eq!(migrated.quorum_configs, vec![quorum(0, 0x02)]);
    assert!(
        migrated.frozen_height.is_none(),
        "migration must unfreeze the subject"
    );
}

#[test]
fn the_subjects_chain_identity_survives_the_migration() {
    let mut deps = staged(
        client_state(SUBJECT_HEIGHT, 0x01),
        client_state(SUBSTITUTE_HEIGHT, 0x02),
    );

    migrate(&mut deps).expect("migration should succeed");
    let migrated = client_state_prefixed(&deps.storage, SUBJECT_PREFIX).expect("subject present");

    assert_eq!(migrated.chain_id, CHAIN_ID);
    assert_eq!(migrated.network_id, vec![0xab; 32]);
    assert_eq!(migrated.router_contract_id, vec![0xcd; 32]);
    assert_eq!(migrated.root_event_topic, b"ibc_root".to_vec());
}

#[test]
fn the_substitutes_consensus_state_is_copied_to_the_subject() {
    let mut deps = staged(
        client_state(SUBJECT_HEIGHT, 0x01),
        client_state(SUBSTITUTE_HEIGHT, 0x02),
    );

    migrate(&mut deps).expect("migration should succeed");
    let copied = stellar_light_client::store::consensus_state_prefixed(
        &deps.storage,
        SUBJECT_PREFIX,
        SUBSTITUTE_HEIGHT,
    )
    .expect("consensus state copied to the subject");

    assert_eq!(
        copied,
        ConsensusState {
            generation: 1,
            ..consensus(100, 0x22)
        }
    );
}

#[test]
fn a_substitute_tracking_another_chain_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.chain_id = "stellar-pubnet".into();
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::SubstituteMismatch { field: "chain_id" })
    ));
}

#[test]
fn a_substitute_pointing_at_another_router_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.router_contract_id = vec![0xee; 32];
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::SubstituteMismatch {
            field: "router_contract_id"
        })
    ));
}

#[test]
fn a_frozen_substitute_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.frozen_height = Some(height(500));
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::SubstituteFrozen { height: 500 })
    ));
}

#[test]
fn an_expired_substitute_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.max_consensus_age = 1;
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::SubstituteExpired)
    ));
}

#[test]
fn a_missing_substitute_is_refused() {
    let mut deps = mock_dependencies();

    set_client_state_prefixed(
        &mut deps.storage,
        SUBJECT_PREFIX,
        &client_state(SUBJECT_HEIGHT, 0x01),
    );
    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::SubstituteMissing)
    ));
}

#[test]
fn a_substitute_with_no_quorum_configuration_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.quorum_configs = vec![];
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::NoQuorumConfigs)
    ));
}

#[test]
fn an_upgrade_is_refused_rather_than_unhandled() {
    let mut deps = staged(
        client_state(SUBJECT_HEIGHT, 0x01),
        client_state(SUBSTITUTE_HEIGHT, 0x02),
    );
    let result = sudo(
        deps.as_mut(),
        mock_env(),
        SudoMsg::VerifyUpgradeAndUpdateState(VerifyUpgradeAndUpdateStateMsg {
            upgrade_client_state: Default::default(),
            upgrade_consensus_state: Default::default(),
            proof_upgrade_client: Default::default(),
            proof_upgrade_consensus_state: Default::default(),
        }),
    );

    assert!(matches!(result, Err(ContractError::UpgradeNotSupported)));
}

#[test]
fn the_wire_format_matches_what_ibc_go_sends() {
    let migrate: SudoMsg = cosmwasm_std::from_json(br#"{"migrate_client_store":{}}"#)
        .expect("migrate_client_store should decode");

    assert!(matches!(migrate, SudoMsg::MigrateClientStore(_)));
    let upgrade: SudoMsg = cosmwasm_std::from_json(
        br#"{"verify_upgrade_and_update_state":{
            "upgrade_client_state":"AQI=",
            "upgrade_consensus_state":"AwQ=",
            "proof_upgrade_client":"BQY=",
            "proof_upgrade_consensus_state":"Bwg="
        }}"#,
    )
    .expect("verify_upgrade_and_update_state should decode");

    match upgrade {
        SudoMsg::VerifyUpgradeAndUpdateState(m) => {
            assert_eq!(m.upgrade_client_state.as_slice(), &[1, 2]);
            assert_eq!(m.proof_upgrade_consensus_state.as_slice(), &[7, 8]);
        }
        other => panic!("decoded as {other:?}"),
    }
}

#[test]
fn the_contract_migrate_entry_point_accepts_a_healthy_client() {
    use stellar_light_client::{
        entrypoint::migrate,
        msg::MigrateMsg,
        store::{set_client_state, set_consensus_state},
    };

    let mut deps = mock_dependencies();

    set_client_state(&mut deps.storage, &client_state(SUBJECT_HEIGHT, 0x01));
    set_consensus_state(&mut deps.storage, SUBJECT_HEIGHT, &consensus(100, 0x11));
    let response =
        migrate(deps.as_mut(), mock_env(), MigrateMsg {}).expect("migrate should succeed");

    assert!(
        response.messages.is_empty(),
        "submessages are rejected by 08-wasm"
    );
    assert!(response.events.is_empty(), "events are rejected by 08-wasm");
    assert!(
        response.attributes.is_empty(),
        "attributes are rejected by 08-wasm"
    );
}

#[test]
fn migrating_an_uninitialised_contract_is_refused() {
    use stellar_light_client::{entrypoint::migrate, msg::MigrateMsg};

    let mut deps = mock_dependencies();

    assert!(matches!(
        migrate(deps.as_mut(), mock_env(), MigrateMsg {}),
        Err(ContractError::NotInitialised)
    ));
}

#[test]
fn migrating_without_the_consensus_state_at_latest_height_is_refused() {
    use stellar_light_client::{entrypoint::migrate, msg::MigrateMsg, store::set_client_state};

    let mut deps = mock_dependencies();

    set_client_state(&mut deps.storage, &client_state(SUBJECT_HEIGHT, 0x01));
    assert!(matches!(
        migrate(deps.as_mut(), mock_env(), MigrateMsg {}),
        Err(ContractError::ConsensusStateMissing { height }) if height == SUBJECT_HEIGHT
    ));
}

#[test]
fn recovery_starts_a_new_generation() {
    let mut deps = staged(
        client_state(SUBJECT_HEIGHT, 0x01),
        client_state(SUBSTITUTE_HEIGHT, 0x02),
    );

    migrate(&mut deps).expect("migration should succeed");
    let migrated = client_state_prefixed(&deps.storage, SUBJECT_PREFIX).expect("subject present");
    let stale = stellar_light_client::store::consensus_state_prefixed(
        &deps.storage,
        SUBJECT_PREFIX,
        SUBJECT_HEIGHT,
    )
    .expect("the old state is still in storage");

    assert_eq!(migrated.generation, 1);
    assert_ne!(
        stale.generation, migrated.generation,
        "the subject's pre-recovery states must not count as trusted"
    );
}

#[test]
fn a_substitute_that_never_expires_is_refused() {
    let mut substitute = client_state(SUBSTITUTE_HEIGHT, 0x02);

    substitute.max_consensus_age = 0;
    let mut deps = staged(client_state(SUBJECT_HEIGHT, 0x01), substitute);

    assert!(matches!(
        migrate(&mut deps),
        Err(ContractError::InvalidClientState(_))
    ));
}
