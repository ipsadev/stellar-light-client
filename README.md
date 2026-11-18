# Stellar Light Client for `08-wasm`

A light client that lets a Cosmos SDK chain verify Stellar, compiled to `wasm32-unknown-unknown` and loaded through [`08-wasm`](https://github.com/cosmos/ibc-go/tree/main/modules/light-clients/08-wasm).

Running inside the consuming chain, it does three things: checks that a quorum of a trusted set of Stellar validators confirmed a ledger, binds that ledger to the state root published by an IBC router contract, and answers membership and non-membership proofs against that root. The consensus checks themselves live in [`stellar-consensus-verifier`](https://crates.io/crates/stellar-consensus-verifier) ([source](https://github.com/ipsadev/stellar-consensus-verifier)); this crate is the `08-wasm` contract around them.

> [!WARNING]
> **Early, under active development. Not production-ready.** No third-party security audit has been done. The client trusts exactly the quorum sets it is instantiated with: the testnet fixtures use a 2-of-3 set, which two compromised keys can forge and one equivocating validator can fork.

## What a header must satisfy

A *header* carries a Stellar ledger and the SCP messages that closed it. It is accepted only when all of these hold:

1. its slot is not below the `valid_from` of the quorum configuration in force at the latest height, and the configurations in force for its slot and the next hash to their pinned fingerprints;
2. every SCP envelope is signed by its node over this network's id;
3. the statements confirm a commit for one value, by a quorum containing a slice of the configured trust root for that slot;
4. that value is byte-identical to the ledger header's `scpValue`;
5. the next slot is confirmed the same way, and its transaction set names this ledger's hash as predecessor, binding every other header field;
6. the ledger follows the stored consensus state at the slot before it, when there is one;
7. advancing the client: its close time is not earlier than the latest trusted ledger's (equal seconds allowed; CAP-88, Protocol 30);
8. filling a slot below the latest: its close time is not later than the latest trusted ledger's.

When a header also supplies a state root, three more checks apply: the transaction results hash to the header's `txSetResultHash`; the chosen transaction is a successful contract call, directly or as a fee-bump's inner transaction; and it carries at least one root event from the configured router. The **last** such event is bound: the success preimage holds only events from calls that succeeded, in emission order, so the last root is the router's state when the invocation ended.

## Roots within a ledger

A ledger can contain several router transactions, each publishing a fresh root over the same sparse Merkle tree, so each root is cumulative. The canonical root is the one the **last** router transaction published: the only root covering every commitment the ledger made, and the one a relayer must prove.

The client stores one root per ledger height with the index of the result that published it, and lets it move forward within the ledger: a later result replaces the stored root; an earlier one is refused as superseded. Result order is apply order: stellar-core applies result stages in sequence and refuses parallel clusters that share a read-write key, which every root-publishing call does. A root stored before result indexes were kept has no known position, so a different root for that ledger is a conflict.

Proofs are checked against a 64-level sparse Merkle tree keyed by the first 8 bytes of `sha256(path)`. They travel in ICS-23 messages, but the tree is not an ICS-23 spec, and `proof_specs` is carried and compared but not used.

## What the client refuses

| Input | Behaviour |
|---|---|
| a revision number other than 0 | refused; Stellar has one revision |
| a non-zero delay period | refused; delays are not enforced |
| `max_consensus_age` of 0 | refused at instantiate and recovery; the trust root must expire |
| a root in the initial consensus state | dropped; the first verified header for that slot binds it |
| an initial close time more than 600 seconds ahead of the block time | refused |
| a header below the `valid_from` of the configuration in force at the latest height | refused, including as misbehaviour evidence |
| a quorum set that does not hash to its `quorum_set_fingerprint`, or a fingerprint that is not 32 bytes | refused at instantiate, recovery and code migration, and on every update; see [rotation.md](docs/ops/rotation.md) |
| a header with no state root, for a ledger already stored with one | accepted as a no-op |
| the same ledger with a root, after a root-less update | the root is added |
| a root from a later transaction result in a ledger already stored with a root | the root moves forward to it |
| a different root from an earlier transaction result in that ledger | refused as superseded |
| a different ledger for a stored slot | refused |

The initial consensus state is the one part of the trust root no SCP evidence backs, so instantiate trusts as little of it as it can: it drops any root (the first verified header binds one) and refuses a close time more than 600 seconds ahead of the block time. Its ledger hash is taken as given: if it is wrong, the real header for that slot is misbehaviour evidence, and anyone can freeze the client with it. `TimestampAtHeight` returns nanoseconds, as ibc-go expects. A recovery starts a new generation: consensus states stored before it are no longer trusted.

## Operations

`docs/ops` covers verifying, storing, migrating and recovering the client on a chain.

| Page | Answers |
|---|---|
| [verify.md](docs/ops/verify.md) | is the published artifact the one this source produces |
| [store.md](docs/ops/store.md) | how does the wasm get onto a chain |
| [migrate.md](docs/ops/migrate.md) | how is a stored client moved to a new version, or recovered |
| [incidents.md](docs/ops/incidents.md) | what breaks, how it is detected, what to do |

## Working on it

Recipes live in the `justfile` (`just --list`):

```sh
just ci              # what CI runs: fmt-check, lint, test, audit
just test            # the whole suite
just test-one scp    # one binary: scp, entrypoint, merkle, migrate, ics20_transfer_flow, proof_conformance
just fmt             # needs nightly, for imports_granularity
```

`tests/scp/` is one binary with `synthetic` checks plus the `mainnet` / `testnet_chain` fixture replays (including Protocol 30 testnet ledger 5541000, whose value records its close time in milliseconds); the rest are one binary each, with recorded ledgers in `tests/fixtures/`. `just audit` ignores RUSTSEC-2026-0387 (`crypto-bigint` 0.5): it is reachable only through `cosmwasm-std`'s secp256k1 code, which this client never calls. Stellar's signatures are Ed25519, checked with `ed25519-dalek`.

## Build

```sh
just release              # the reproducible containerised build
just verify-reproducible  # build twice from a cold cache and diff the checksums
just checksum             # the on-chain (uncompressed) hash
```

Docker is required; a host build produces different bytes, so `scripts/release-build.sh` refuses to run without it, and `just build-wasm` compiles the contract but does **not** reproduce the published checksum. Three things fix the output: the Rust toolchain (1.91.1), binaryen (`wasm-opt` 129, sha256-verified during the image build), and `Cargo.lock` under `--locked`; `SOURCE_DATE_EPOCH=0` and `--remap-path-prefix` strip the timestamp and absolute paths.

Every pull request builds the artifact twice from a cold cache and fails if the checksums differ. Pushing a `v*` tag (which must match `Cargo.toml`'s `version`) runs the same proof, then publishes the artifact, its `.gz`, and `checksums.txt` to a GitHub release whose notes carry the on-chain checksum and the reproduce commands. Sources are passed to `rustc` relative to the crate root, so the crate sits at the root of its own repository to keep the checksum independent of checkout location.

## Security & License

Report vulnerabilities privately; see [SECURITY.md](SECURITY.md). Copyright 2026 Amanda Gonsalves. Licensed under the [Apache License 2.0](LICENSE).
