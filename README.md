# Stellar Light Client for `08-wasm`

The Stellar light client, compiled to `wasm32-unknown-unknown` and loaded by a
Cosmos SDK chain through [`08-wasm`](https://github.com/cosmos/ibc-go/tree/main/modules/light-clients/08-wasm).
It verifies Stellar SCP `EXTERNALIZE` consensus and ICS-23 membership and
non-membership proofs against the Stellar state SMT root, inside the consuming
chain's own state machine.

> [!WARNING]
> **Early, under active development. A test implementation, not production-ready.**
> No third-party security audit has been done, and the pinned testnet trust root
> tolerates zero Byzantine faults.

## Working on it

Recipes live in the `justfile`; `just --list` shows them all.

```sh
just ci              # exactly what CI runs: fmt-check, lint, test, audit
just test            # the whole suite
just test-one scp    # one binary: scp, entrypoint, merkle, migrate, ics20_transfer_flow
just fmt             # needs nightly, for imports_granularity
```

Tests are grouped by the surface they cover. `tests/scp/` is one binary with a
module per check (`envelope`, `ledger`, `quorum`, `txset`, `xdr`, and the
`mainnet` / `testnet_chain` fixture replays); the rest are one binary each.
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

## License

Licensed under the [Apache License 2.0](LICENSE).
