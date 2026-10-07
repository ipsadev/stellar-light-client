use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("client state already initialised")]
    AlreadyInitialised,
    #[error("client state not initialised")]
    NotInitialised,
    #[error(
        "stellar has no ibc client upgrade path, so an upgrade cannot be verified; recover this \
         client with migrate_client_store against a fresh substitute instead"
    )]
    UpgradeNotSupported,
    #[error("no substitute client state found under the substitute prefix")]
    SubstituteMissing,
    #[error("the substitute client is frozen at height {height} and cannot be migrated from")]
    SubstituteFrozen { height: u64 },
    #[error(
        "the substitute client is itself expired; its trust root is no longer usable for recovery"
    )]
    SubstituteExpired,
    #[error(
        "subject and substitute disagree on {field}; migrating would repoint the client at a \
         different chain or contract"
    )]
    SubstituteMismatch { field: &'static str },
    #[error("client is frozen at height {height}")]
    Frozen { height: u64 },
    #[error("invalid wire bytes: {0}")]
    InvalidWire(String),
    #[error("the EXTERNALIZE commit ballot has counter 0")]
    CommitCounterZero,
    #[error("the EXTERNALIZE commit ballot has an empty value")]
    EmptyCommitValue,
    #[error("the ballot has counter 0")]
    BallotCounterZero,
    #[error("the PREPARE statement is malformed")]
    MalformedPrepare,
    #[error("the CONFIRM statement is malformed")]
    MalformedConfirm,
    #[error("the statement is a nomination, not a ballot-protocol statement")]
    NominationStatement,
    #[error("the statement is not EXTERNALIZE")]
    NotExternalize,
    #[error("the signature on the envelope from node {node} does not verify")]
    UnauthenticatedEnvelope { node: String },
    #[error("a supplied quorum set fails the sanity rules: {0}")]
    InsaneQuorumSet(String),
    #[error("no quorum set preimage hashes to {hash}, named by the statement from {node}")]
    QuorumSetPreimageMissingFor { node: String, hash: String },
    #[error("the envelope is for slot {found}, not the slot {expected} being proven")]
    EnvelopeSlotMismatch { expected: u64, found: u64 },
    #[error("no value reaches confirm commit for the trust root")]
    NoConfirmedCommit,
    #[error("two different values each reach confirm commit — this is fork evidence")]
    ForkedConfirmCommit,
    #[error("the highest confirmed ballot counter {highest} is below the commit counter {commit}")]
    HighestConfirmedBelowCommit { highest: u32, commit: u32 },
    #[error("consensus state missing at height {height}")]
    ConsensusStateMissing { height: u64 },
    #[error("conflicting consensus state already stored at height {height}")]
    ConsensusStateConflict { height: u64 },
    #[error(
        "ledger {slot} names predecessor {found}, but the trusted consensus state at {previous} \
         has ledger hash {expected}"
    )]
    PreviousLedgerMismatch {
        slot: u64,
        previous: u64,
        expected: String,
        found: String,
    },
    #[error(
        "ledger {slot} closed at {close_time}, not after the trusted consensus state at {latest} \
         (closed {previous})"
    )]
    NonMonotonicCloseTime {
        slot: u64,
        close_time: u64,
        latest: u64,
        previous: u64,
    },
    #[error(
        "misbehaviour headers are for slots {first} and {second}; fork evidence needs one slot"
    )]
    MisbehaviourSlotMismatch { first: u64, second: u64 },
    #[error("misbehaviour headers are identical; that is not evidence of a fork")]
    MisbehaviourNotAFork,
    #[error("fork evidence cannot advance the client; route it to UpdateStateOnMisbehaviour")]
    ForkEvidenceInUpdate,
    #[error(
        "the trusted consensus state is older than max_consensus_age, so the trust root can no \
         longer be relied on; the client must be recovered rather than advanced"
    )]
    ClientExpired,
    #[error("slot {claimed} does not match {header}")]
    SlotMismatch { header: u64, claimed: u64 },
    #[error("no envelopes supplied")]
    NoEnvelopes,
    #[error("the same signer appears twice")]
    DuplicateSigner,
    #[error("quorum set preimage missing for a signer")]
    QuorumSetPreimageMissing,
    #[error("signers externalized different values")]
    SignersDisagree,
    #[error("{signers} signers do not form a quorum")]
    NotAQuorum { signers: usize },
    #[error("externalized value is not this ledger's scpValue")]
    ValueNotThisLedger,
    #[error("no quorum configuration covers slot {slot}")]
    NoApplicableQuorumConfig { slot: u64 },
    #[error("client state carries no quorum configuration")]
    NoQuorumConfigs,
    #[error(
        "two quorum configurations claim valid_from {valid_from}; the trust root would be chosen \
         by list order"
    )]
    AmbiguousQuorumConfig { valid_from: u64 },
    #[error("network_id must be 32 bytes")]
    NetworkIdInvalid,
    #[error("state root at height {height} is not bound to a Stellar ledger")]
    StateRootNotBound { height: u64 },
    #[error("transaction set does not hash to the value agreed for the next slot")]
    TxSetHashMismatch,
    #[error("next slot's transaction set does not follow this ledger")]
    LedgerNotPrevious,
    #[error("supplied transaction results do not hash to the header's txSetResultHash")]
    TxResultSetMismatch,
    #[error("result_index {index} is out of range for {len} result(s)")]
    ResultIndexOutOfRange { index: u32, len: usize },
    #[error("the transaction carrying the state root did not succeed (code {code})")]
    TransactionNotSuccessful { code: i32 },
    #[error("the referenced result is not a single-operation Soroban invocation")]
    NotASorobanInvocation,
    #[error("the host-function invocation failed (code {code})")]
    InvokeHostFunctionFailed { code: i32 },
    #[error("the success preimage does not hash to the value the result committed to")]
    SuccessPreimageMismatch,
    #[error("no event from the configured router contract carries the state root")]
    RouterEventMissing,
    #[error("more than one router event carries a state root")]
    AmbiguousRouterEvent,
    #[error("the router's state-root event data is not 32 bytes")]
    StateRootMalformed,

    #[error("ledger {target} is not an ancestor of ledger {anchor}")]
    ChainNotDescending { anchor: u32, target: u32 },

    #[error("walking {anchor} back to {target} needs {expected} header(s), {found} supplied")]
    ChainLengthMismatch {
        anchor: u32,
        target: u32,
        expected: usize,
        found: usize,
    },

    #[error("expected ledger {expected} in the chain, found {found}")]
    ChainGap { expected: u32, found: u32 },

    #[error("ledger {sequence} does not hash to the previousLedgerHash of ledger {child}")]
    ChainBroken { sequence: u32, child: u32 },
    #[error("client state is missing the router contract id needed to bind the state root")]
    RouterContractIdMissing,
    #[error("invalid client state: {0}")]
    InvalidClientState(&'static str),
    #[error("invalid consensus state: {0}")]
    InvalidConsensusState(&'static str),
    #[error("stellar has a single revision; revision number {revision} is not 0")]
    RevisionNumberUnsupported { revision: u64 },
    #[error(
        "delay periods are not enforced by this client (time {time}, blocks {blocks}); use a \
         connection or path with no delay"
    )]
    DelayPeriodUnsupported { time: u64, blocks: u64 },
    #[error("slot {slot} has no successor slot")]
    SlotOverflow { slot: u64 },
    #[error("close time {seconds}s does not fit in nanoseconds")]
    TimestampOverflow { seconds: u64 },
    #[error("recovery would overflow the client generation")]
    GenerationOverflow,
    #[error("header carries no state-root proof")]
    StateRootProofMissing,
    #[error("merkle proof verification failed")]
    MerkleVerificationFailed,
    #[error(
        "merkle membership mismatch (key_match={key_match}, value_match={value_match}, siblings={siblings}, height={height}, req_key={req_key}, proof_key={proof_key}, value_len={value_len}/{proof_value_len}, stored_root={stored_root}, computed_root={computed_root})"
    )]
    MembershipMismatch {
        key_match: bool,
        value_match: bool,
        siblings: usize,
        height: u64,
        req_key: String,
        proof_key: String,
        value_len: usize,
        proof_value_len: usize,
        stored_root: String,
        computed_root: String,
    },
}

impl From<stellar_consensus_verifier::error::ScpError> for ContractError {
    fn from(e: stellar_consensus_verifier::error::ScpError) -> Self {
        use stellar_consensus_verifier::error::ScpError as E;

        match e {
            E::InvalidWire(w) => Self::InvalidWire(w),
            E::CommitCounterZero => Self::CommitCounterZero,
            E::EmptyCommitValue => Self::EmptyCommitValue,
            E::BallotCounterZero => Self::BallotCounterZero,
            E::MalformedPrepare => Self::MalformedPrepare,
            E::MalformedConfirm => Self::MalformedConfirm,
            E::NominationStatement => Self::NominationStatement,
            E::NotExternalize => Self::NotExternalize,
            E::NoConfirmedCommit => Self::NoConfirmedCommit,
            E::UnauthenticatedEnvelope { node } => Self::UnauthenticatedEnvelope { node },
            E::EnvelopeSlotMismatch { expected, found } => {
                Self::EnvelopeSlotMismatch { expected, found }
            }
            E::ForkedConfirmCommit => Self::ForkedConfirmCommit,
            E::HighestConfirmedBelowCommit { highest, commit } => {
                Self::HighestConfirmedBelowCommit { highest, commit }
            }
            E::TxResultSetMismatch => Self::TxResultSetMismatch,
            E::ResultIndexOutOfRange { index, len } => Self::ResultIndexOutOfRange { index, len },
            E::SuccessPreimageMismatch => Self::SuccessPreimageMismatch,
            E::TransactionNotSuccessful { code } => Self::TransactionNotSuccessful { code },
            E::NotASorobanInvocation => Self::NotASorobanInvocation,
            E::InvokeHostFunctionFailed { code } => Self::InvokeHostFunctionFailed { code },
            E::RouterEventMissing => Self::RouterEventMissing,
            E::AmbiguousRouterEvent => Self::AmbiguousRouterEvent,
            E::StateRootMalformed => Self::StateRootMalformed,
            E::ChainNotDescending { anchor, target } => Self::ChainNotDescending { anchor, target },
            E::ChainLengthMismatch {
                anchor,
                target,
                expected,
                found,
            } => Self::ChainLengthMismatch {
                anchor,
                target,
                expected,
                found,
            },
            E::ChainGap { expected, found } => Self::ChainGap { expected, found },
            E::ChainBroken { seq, child } => Self::ChainBroken {
                sequence: seq,
                child,
            },
        }
    }
}
