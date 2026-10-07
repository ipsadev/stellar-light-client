# Operating the Stellar light client

Everything an operator or a governance reviewer needs to verify, store, instantiate and migrate the
Stellar light client on a Cosmos SDK chain.

The client is an `08-wasm` contract. A chain stores it by checksum, so every claim below reduces to
one question: **does the binary on chain correspond to source anyone can read?** These pages exist so
that question can be answered without trusting the person who built it.

| Page | Answers |
|---|---|
| [verify.md](verify.md) | Is the published artifact the one this source produces? |
| [store.md](store.md) | How does the wasm get onto a chain? |
| [migrate.md](migrate.md) | How is a stored client moved to a new version, or recovered? |
| [incidents.md](incidents.md) | What breaks, how it is detected, and what to do about it |

## Releases

Each `v*` tag publishes `stellar_light_client.wasm`, `stellar_light_client.wasm.gz` and
`checksums.txt` to a [GitHub release](https://github.com/ipsadev/stellar-light-client/releases),
whose notes carry the on-chain checksum. The tag always equals the `version` in `Cargo.toml`.

The `.wasm` hash is the one recorded on chain. `08-wasm` gunzips submitted bytes before hashing them,
so the gzipped file is transport only and its own hash never appears anywhere on chain.

## What this client verifies

Stellar has no global validator set and no fixed threshold. Each node names the quorum slices it
trusts, so the client is instantiated with the quorum sets it trusts and accepts a ledger only when a
quorum containing one of their slices confirmed it. The [README](../../README.md#what-the-client-checks)
lists every check, in order, and what the client refuses.
