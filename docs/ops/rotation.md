# Rotating the trust root

The client trusts the quorum sets in its client state's `quorum_configs`. Each entry is a quorum set, the slot it takes effect from, and a pinned fingerprint:

| Field | Meaning |
|---|---|
| `quorum_set_xdr` | the `SCPQuorumSet` XDR the client checks confirm-commit against |
| `valid_from` | the first slot this set is in force for |
| `quorum_set_fingerprint` | `sha256(quorum_set_xdr)`, 32 bytes, committed out of band |

## How a set is chosen

For a slot, the entry with the highest `valid_from` at or below it is in force. A ledger at slot `N` is checked in two parts: its own SCP statements against the set in force at `N`, and its successor's statements against the set in force at `N + 1`. So with a rotation at slot `R`:

| Ledger | Its statements are checked against | Its successor's against |
|---|---|---|
| below `R - 1` | the old set | the old set |
| `R - 1` | the old set | the new set |
| `R` and later | the new set | the new set |

Once the client's latest height reaches `R`, the old set is retired: a header below `R` is refused, as an update or as misbehaviour evidence, even though the old set once signed that range.

## The fingerprint

The fingerprint is the commitment a reviewer checks. A quorum set is several hundred bytes of XDR; its SHA-256 is 32 bytes anyone can recompute from the validators' published configuration and compare with what a proposal carries. The client refuses a configuration whose set does not hash to its fingerprint, or whose fingerprint is not 32 bytes:

| Where | What is checked |
|---|---|
| instantiate | every configuration, including scheduled rotations |
| recovery (`migrate_client_store`) | every configuration the substitute carries |
| code migration (`MsgMigrateContract`) | every configuration in the stored state |
| every update and misbehaviour check | the configuration chosen for the slot and for its successor |

A client stored by 0.3.0 or earlier has no fingerprints. Migrating it to this code pins each one from the quorum set it already holds, so the trust it extends does not change; a fingerprint that is present and does not match is refused. ipsa tooling's `PINNED_QUORUM_SET_HASHES` are the same values, hex encoded.

## When a rotation can be added

`quorum_configs` changes in two places only: at instantiate, and in a recovery, which copies the substitute's configurations. There is no message that edits them on a live client, and ibc-go refuses to recover a client that is still active. So:

**A rotation known in advance** goes into the client's configuration when it is created: the current set from its slot, and the next set from the slot it takes over. The client crosses the boundary on its own.

**A rotation that was not scheduled** stops the client: once the new set signs, no header verifies against the old one, and the client expires after `max_consensus_age`. Recover it as in [migrate.md](migrate.md#recovering-a-stuck-client), with a substitute whose configuration carries the new set from the rotation slot. Channels stall from the rotation until the recovery passes, so watch the validators' published configuration and schedule rotations before they happen.

## Procedure

1. Get the new set's `SCPQuorumSet` XDR from the validators' published configuration, and the slot it takes effect from. Compute its fingerprint: `sha256sum` of the raw XDR bytes.
2. Check the set is sound: every level's threshold is a strict majority of its members. The client refuses any other set, so a set that fails here cannot be recovered to.
3. Build the substitute (or the new client) with both entries: the current set and its `valid_from`, and the new set with `valid_from` set to the rotation slot, each with its fingerprint. Put both fingerprints in the proposal text.
4. Before submitting, instantiate the configuration on a test chain and update it across the boundary: the old set must sign up to `R - 1`, and the new set the successor of `R - 1` and every ledger from `R`. A ledger signed by the wrong set for its slot is refused with "no quorum confirmed a commit".
5. Submit the client creation or the recovery proposal. After it passes, query the client state and compare every fingerprint with the proposal:

```sh
gaiad q ibc client state <client-id> --node <rpc>
```

## Tests

`tests/entrypoint.rs` covers each boundary: a ledger before `R` against the old set and refused when the new set signs it; `R - 1` with its successor from the new set, refused when the old set signs that successor; `R` against the new set, refused when the old set signs it; the old set retired once the client crosses `R`; and fingerprint mismatches at instantiate and on update. `tests/migrate.rs` covers the same refusals in a recovery and in a code migration, and the pinning of fingerprints missing from older state.
