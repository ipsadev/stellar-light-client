# Migrating and recovering a client

Two different operations share the word "migrate", and confusing them is expensive.

| | What moves | Who can do it |
|---|---|---|
| `MsgMigrateContract` | The **code** a client runs — new wasm, same client id | The authority, or an authz holder |
| `migrate_client_store` | The **state** — a stuck client adopts a healthy one's | Triggered by a client recovery proposal |

The first is an upgrade. The second is a recovery, and it is what governance uses when a client has
expired or frozen and there are channels depending on its id.

## Upgrading the code

Store the new wasm first, exactly as in [store.md](store.md), then point the existing client at it:

```sh
gaiad tx ibc-wasm migrate-contract <client-id> <new-checksum> '{}' --from <grantee>
```

The client keeps its id, so counterparties and channels are unaffected. Verify:

```sh
gaiad q ibc client state <client-id> --node <rpc>
```

## Recovering a stuck client

A client stops being usable in two ways: it **freezes** on misbehaviour evidence, or it **expires**
because no header arrived inside its trusting window. Neither can be undone by submitting more
headers — the trust root is gone.

Recovery creates a fresh client tracking the same chain, then asks governance to merge it into the
broken one. The chain calls `migrate_client_store` on the contract, which is handed both stores: the
broken client under `subject/` and the healthy one under `substitute/`. Only `subject/` writes take
effect.

This client copies from the substitute:

- `latest_height`
- `quorum_configs`
- `max_consensus_age`
- the consensus state at the substitute's height

and clears `frozen_height`. It keeps from the subject:

- `chain_id`
- `network_id`
- `router_contract_id`
- `root_event_topic`
- `proof_specs`

**The split is deliberate and is the security property to review.** Quorum configuration must be able
to change, because validator rotation is a reason a client needs recovery in the first place. Chain
identity and the router contract must not, because allowing them to move would let a recovery
silently repoint a live client — and every channel trusting it — at a different chain or a different
contract. The contract refuses the migration and names the offending field if any of the five differ.

It also refuses a substitute that is frozen, expired, or carries no quorum configuration.

## What is deliberately not supported

`verify_upgrade_and_update_state` returns an error. It exists for chains that perform IBC-breaking
upgrades and publish upgrade proofs; Stellar has no such mechanism, so there is nothing to verify.
Recovery goes through `migrate_client_store` instead. The refusal is explicit rather than absent, so
a caller gets a stated reason rather than a decoding failure.
