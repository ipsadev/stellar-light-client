# Stellar Light Client for `08-wasm`

The Stellar light client, compiled to `wasm32-unknown-unknown` and loaded by a
Cosmos SDK chain through [`08-wasm`](https://github.com/cosmos/ibc-go/tree/main/modules/light-clients/08-wasm).
It verifies that a quorum of a trusted set of Stellar validators confirmed a
ledger, binds that ledger to the state root published by an IBC router contract,
and checks membership and non-membership proofs against that root, inside the
consuming chain's own state machine.

The consensus checks live in
[`stellar-consensus-verifier`](https://crates.io/crates/stellar-consensus-verifier)
([source](https://github.com/ipsadev/stellar-consensus-verifier)); this crate is the
`08-wasm` contract around them.

> [!WARNING]
> **Early, under active development. Not production-ready.** No third-party
> security audit has been done. The client trusts exactly the quorum sets it is
> instantiated with: the testnet fixtures use a 2-of-3 set, which two
> compromised keys can forge and one equivocating validator can fork.

## What the client checks

A header is accepted only when all of these hold:

1. every SCP envelope is signed by its node over this network's id;
2. the statements confirm a commit for one value, by a quorum that contains a
   slice of the configured trust root for that slot;
3. that value is byte-identical to the ledger header's `scpValue`;
4. the next slot is confirmed the same way, and its transaction set names this
   ledger's hash as its predecessor, which binds every other header field;
5. the ledger follows the stored consensus state at the slot before it, when
   there is one;
6. its close time is later than the latest trusted ledger's, when it advances
   the client;
7. when a state root is supplied, the transaction results hash to the header's
   `txSetResultHash`, the chosen transaction is a successful contract call, and
   it carries exactly one root event from the configured router.

Proofs are checked against a 64-level sparse Merkle tree keyed by the first 8
bytes of `sha256(path)`, the tree the router maintains. The proofs travel in
ICS-23 messages, but the tree is not an ICS-23 spec, and `proof_specs` in the
client state is carried and compared but not used.

The client refuses rather than ignores what it cannot honour:

| Input | Behaviour |
|---|---|
| a revision number other than 0 | refused; Stellar has one revision |
| a non-zero delay period | refused; delays are not enforced |
| `max_consensus_age` of 0 | refused at instantiate and recovery; the trust root must expire |
| a header with no state root, for a ledger already stored with one | accepted as a no-op |
| the same ledger with a root, after a root-less update | the root is added |
| a different ledger, or a different root, for a stored slot | refused |

`TimestampAtHeight` returns nanoseconds, as ibc-go expects. A recovery starts a
new generation: consensus states stored before it are no longer trusted.

## Operations

`docs/ops` covers verifying, storing, migrating and recovering the client on a
chain.

| Page | Answers |
|---|---|
| [verify.md](docs/ops/verify.md) | is the published artifact the one this source produces |
| [store.md](docs/ops/store.md) | how does the wasm get onto a chain |
| [migrate.md](docs/ops/migrate.md) | how is a stored client moved to a new version, or recovered |
| [incidents.md](docs/ops/incidents.md) | what breaks, how it is detected, what to do |

## Working on it

Recipes live in the `justfile`; `just --list` shows them all.

```sh
just ci              # exactly what CI runs: fmt-check, lint, test, audit
just test            # the whole suite
just test-one scp    # one binary: scp, entrypoint, merkle, migrate, ics20_transfer_flow, proof_conformance
just fmt             # needs nightly, for imports_granularity
```

Tests are grouped by the surface they cover. `tests/scp/` is one binary with
`synthetic` checks and the `mainnet` / `testnet_chain` fixture replays; the rest
are one binary each.
Recorded ledgers live in `tests/fixtures/`.

## Build

```sh
just release              # the reproducible containerised build
just verify-reproducible  # build twice from a cold cache and diff the checksums
just checksum             # the on-chain (uncompressed) hash
```

Docker is required. A host build produces different bytes, so
`scripts/release-build.sh` refuses to run without it, and `just build-wasm`
compiles the contract but does **not** reproduce the published checksum.

Three things fix the output bytes: the Rust toolchain (1.91.1), binaryen
(`wasm-opt` 129, verified by tarball sha256 during the image build), and
`Cargo.lock` under `--locked`. `SOURCE_DATE_EPOCH=0` and `--remap-path-prefix`
remove the build timestamp and the absolute dependency paths.

Every pull request builds the artifact twice from a cold cache and fails if the
checksums differ. Pushing a `v*` tag runs the same proof, then publishes the
artifact, its `.gz`, and `checksums.txt` to a GitHub release whose notes carry
the on-chain checksum and the commands to reproduce it. The tag must match the
`version` in `Cargo.toml`.

The crate's own sources are passed to `rustc` as paths relative to the crate
root, so they are embedded as `src/entrypoint.rs` and so on. This is why the
crate sits at the root of its own repository: the embedded paths, and therefore
the checksum, no longer depend on where the tree is checked out.

## Security

Report vulnerabilities privately; see [SECURITY.md](SECURITY.md).

## License

Licensed under the [Apache License 2.0](LICENSE).
