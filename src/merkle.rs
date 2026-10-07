pub use ics23::{
    commitment_proof::Proof, CommitmentProof, ExistenceProof, InnerOp, LeafOp, NonExistenceProof,
};
use prost::Message;

use crate::{error::ContractError, smt::HASH_SIZE};

pub type Siblings = Vec<[u8; HASH_SIZE]>;
pub type DecodedMembership = (Vec<u8>, Vec<u8>, Siblings);
pub type DecodedNonMembership = (Vec<u8>, Siblings);

#[derive(Clone, PartialEq, Message)]
pub struct MerkleProof {
    #[prost(message, repeated, tag = "1")]
    pub proofs: Vec<CommitmentProof>,
}

pub fn decode_membership_proof(bytes: &[u8]) -> Result<DecodedMembership, ContractError> {
    let merkle = MerkleProof::decode(bytes)
        .map_err(|e| ContractError::InvalidWire(format!("MerkleProof: {e}")))?;
    let first = merkle
        .proofs
        .into_iter()
        .next()
        .ok_or(ContractError::MerkleVerificationFailed)?;
    let existence = match first.proof.ok_or(ContractError::MerkleVerificationFailed)? {
        Proof::Exist(e) => e,
        _ => return Err(ContractError::MerkleVerificationFailed),
    };

    let siblings = extract_siblings(&existence.path)?;

    Ok((existence.key, existence.value, siblings))
}

pub fn decode_non_membership_proof(bytes: &[u8]) -> Result<DecodedNonMembership, ContractError> {
    let merkle = MerkleProof::decode(bytes)
        .map_err(|e| ContractError::InvalidWire(format!("MerkleProof: {e}")))?;
    let first = merkle
        .proofs
        .into_iter()
        .next()
        .ok_or(ContractError::MerkleVerificationFailed)?;
    let nonexist = match first.proof.ok_or(ContractError::MerkleVerificationFailed)? {
        Proof::Nonexist(n) => n,
        _ => return Err(ContractError::MerkleVerificationFailed),
    };

    let inner = nonexist
        .left
        .ok_or(ContractError::MerkleVerificationFailed)?;

    if !inner.value.is_empty() {
        return Err(ContractError::MerkleVerificationFailed);
    }

    let siblings = extract_siblings(&inner.path)?;

    Ok((nonexist.key, siblings))
}

fn extract_siblings(ops: &[InnerOp]) -> Result<Siblings, ContractError> {
    ops.iter()
        .map(extract_sibling)
        .collect::<Result<Siblings, _>>()
}

pub fn extract_sibling(op: &InnerOp) -> Result<[u8; HASH_SIZE], ContractError> {
    if !op.suffix.is_empty()
        && op.suffix.len() == HASH_SIZE
        && op.prefix.len() == 1
        && op.prefix[0] == 0x01
    {
        op.suffix
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::MerkleVerificationFailed)
    } else if op.suffix.is_empty() && op.prefix.len() == 1 + HASH_SIZE && op.prefix[0] == 0x01 {
        op.prefix[1..]
            .try_into()
            .map_err(|_| ContractError::MerkleVerificationFailed)
    } else {
        Err(ContractError::MerkleVerificationFailed)
    }
}
