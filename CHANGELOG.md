# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each released version corresponds to a `v*` tag and a reproducible artifact; see [docs/ops/verify.md](docs/ops/verify.md).

## [Unreleased]

### Added

- Pin a fingerprint for every quorum configuration: `QuorumConfig.quorum_set_fingerprint` (field 3) holds `sha256(quorum_set_xdr)`, and a set that does not match it, or a fingerprint that is not 32 bytes, is refused at instantiate, recovery, code migration and on every update. Previously the fingerprint was derived from the set itself, so the check could not fail.
- Migrating a client stored by 0.3.0 or earlier pins its missing fingerprints from the quorum sets it already holds.
- Tests for each trust-root rotation boundary and for fingerprint mismatches.

### Documentation

- Add [docs/ops/rotation.md](docs/ops/rotation.md): how a quorum set is chosen per slot, the fingerprint, and the rotation procedure.

## [0.3.0] - 2026-11-14

### Changed

- Verify consensus with `stellar-consensus-verifier` 2.3.0: a fee-bumped router transaction is accepted (its inner transaction's result is used), and when one invocation emits several root events the last is bound.
- A ledger's bound root may now move forward within the ledger: a root from a later transaction result replaces an earlier one, and a different root from an earlier result is refused as superseded. The transaction result index is stored alongside the root.

### Fixed

- Refuse a header below the `valid_from` of the trust root in force at the latest height, for updates and misbehaviour evidence alike, so a superseded trust root is never reused.
- Stop trusting the root carried in the initial consensus state; the first verified header for that slot binds one.
- Reject proof verification against an expired client.

### Documentation

- Define the canonical root for a ledger as the one its last router transaction published.
- State that recovery copies only the substitute's latest consensus state.
- Rewrite the README and `docs/ops` pages for clarity and concision.

## [0.2.0] - 2026-11-03

### Added

- Protocol 30 support: accept two ledgers that close within the same second (CAP-88), verified with `stellar-consensus-verifier` 1.1.0.

### Changed

- Take the proof primitives from the `ipsa` crate.

## [0.1.0] - 2026-10-22

### Added

- Initial Stellar `08-wasm` light client: SCP consensus verification, state-root binding to the IBC router contract, and ICS-23 membership and non-membership proofs against the router's 64-level sparse Merkle tree.
- Operations runbook under `docs/ops` (verify, store, migrate, incidents).

[Unreleased]: https://github.com/ipsadev/stellar-light-client/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/ipsadev/stellar-light-client/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ipsadev/stellar-light-client/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ipsadev/stellar-light-client/releases/tag/v0.1.0
