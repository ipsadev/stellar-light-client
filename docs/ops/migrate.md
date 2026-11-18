# Migrating and recovering a client

Two operations share the word "migrate", and confusing them is expensive.

| | What moves | Who can do it |
|---|---|---|
| `MsgMigrateContract` | The **code** a client runs (new wasm, same client id) | The authority, or an authz holder |
| `migrate_client_store` | The **state**: a stuck client adopts a healthy one's | Triggered by a client recovery proposal |

The first is an upgrade. The second is a recovery: what governance uses when a client has expired or frozen and channels depend on its id.

## Upgrading the code

Store the new wasm first, as in [store.md](store.md), then point the existing client at it:

```sh
gaiad tx ibc-wasm migrate-contract <client-id> <new-checksum> '{}' --from <grantee>
```

The client keeps its id, so counterparties and channels are unaffected. A client stored by 0.3.0 or earlier carries no quorum-set fingerprints; the migration pins each from the set already stored and refuses one that is present and wrong (see [rotation.md](rotation.md#the-fingerprint)). The contract reads its checksum from the stored client state on every write, so after the migration it keeps the new checksum and goes on updating, freezing and recovering normally. Verify:

```sh
gaiad q ibc client state <client-id> --node <rpc>
```

## Recovering a stuck client

A client becomes unusable in two ways: it **freezes** on misbehaviour evidence, or it **expires** when no header arrives inside its trusting window. Neither can be undone by submitting more headers. The trust root is gone.

Recovery creates a fresh client tracking the same chain, then asks governance to merge it into the broken one. The chain calls `migrate_client_store` on the contract with both stores: the broken client under `subject/`, the healthy one under `substitute/`. Only `subject/` writes take effect.

The contract copies from the substitute (`latest_height`, `quorum_configs`, `max_consensus_age`, and the consensus state at the substitute's height), clears `frozen_height`, and starts a new generation. Consensus states the subject stored before the recovery stay in storage but are no longer trusted: proofs, updates and misbehaviour checks use only the current generation, so nothing from a forked or stale branch survives. It keeps from the subject: `chain_id`, `network_id`, `router_contract_id`, `root_event_topic`, `proof_specs`.

Only the substitute's **latest** consensus state is copied, not its history, the same as ibc-go's `07-tendermint`, whose `CheckSubstituteAndUpdateState` copies the single state at `LatestHeight`. So after a recovery there is no trusted consensus state below the substitute's latest height, and a packet proof built against a pre-recovery height must be re-proven at or after it (a relayer reaches that by updating the recovered client forward). This is liveness, not safety: the source chain still holds the commitment, so the proof can be rebuilt.

**The split is the security property to review.** Quorum configuration must be able to change, because validator rotation is a reason a client needs recovery at all. Chain identity and the router contract must not: allowing them to move would let a recovery silently repoint a live client, and every channel trusting it, at a different chain or contract. The contract refuses the migration and names the offending field if any of the five differ. It also refuses a substitute that is frozen, expired, never expires (`max_consensus_age` of 0), or carries no quorum configuration.

## What is deliberately not supported

`verify_upgrade_and_update_state` returns an error. It exists for chains that perform IBC-breaking upgrades and publish upgrade proofs; Stellar has no such mechanism, so there is nothing to verify. Recovery goes through `migrate_client_store` instead. The refusal is explicit rather than absent, so a caller gets a stated reason rather than a decoding failure.
