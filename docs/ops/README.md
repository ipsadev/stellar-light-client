# ipsa light client operations

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

## Current release

`v0.1.1` — https://github.com/ipsadev/stellar-light-client/releases/tag/v0.1.1

| Artifact | sha256 |
|---|---|
| `stellar_light_client.wasm` | `729e451c7545a6399b9bd69f3182e520f257fa8343294bd08e586482653a3cc2` |
| `stellar_light_client.wasm.gz` | `3c938d433a2e07fc62960e9d78fd2521180772c5408089f9e9b14ad5151a15b1` |

The first value is the one recorded on chain. `08-wasm` gunzips submitted bytes before hashing them,
so the gzipped file is transport only and its own hash never appears anywhere on chain.

## What this client verifies

Stellar has no validator set and no threshold. Agreement is federated: each node names the quorum
slices it trusts, and a value is externalized when the trusted set is quorum-intersecting. The client
therefore checks eight things, in order, and the on-chain contract is the same code the standalone
verifier runs.

A reviewer wanting the protocol argument rather than the operations should read the technical
deep-dive; this directory assumes the design is settled and covers only running it.
