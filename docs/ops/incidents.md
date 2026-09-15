# Incidents

What actually goes wrong, in rough order of likelihood.

## The client is about to expire

**Symptom.** `gaiad q ibc client status <client-id>` returns anything but `Active`, or the latest
consensus height stops advancing.

**Cause.** Client updates ride inside packet relaying, so a corridor with no traffic does not advance
its clients. An idle corridor can let one expire.

**Response.** Relay anything, or submit a header directly, before the trusting period closes. Once it
has expired, the only route is recovery — see [migrate.md](migrate.md) — which needs a governance
action and is far more expensive than avoiding it.

**Prevention.** A scheduled refresh worker. This is a known gap, not a solved problem.

## The client froze

**Symptom.** Status `Frozen`, and `frozen_height` is set in the client state.

**Cause.** Misbehaviour evidence was accepted: two distinct headers for one slot, both individually
valid. On Stellar that means a genuine quorum-level fork, or a bug.

**Response.** Do not rush to recover it. Freezing is the client working correctly, and the evidence
is worth understanding before the state is replaced. Recovery is the same path as expiry, and it
discards the frozen state.

## A proof fails to verify

**Symptom.** `verify_membership` returns false, or a relayer reports a packet it cannot prove.

**Usual cause.** A race, not a cryptographic failure — the proof was read at a height the client has
not reached, or before the transaction carrying the state root was included. Confirm the height is
present with `gaiad q ibc client consensus-state <client-id> <height>` before treating it as a
verification bug.

## The stored checksum does not match the release

**Symptom.** `gaiad q ibc-wasm checksums` disagrees with the release's `checksums.txt`.

**This is the serious one.** It means the code running on chain is not the code that was published.
Do not migrate over it and do not assume a build glitch. Establish which artifact is on chain, from
whom, and when — the migration path is only safe once that is known.

## Archive evidence is unavailable

**Symptom.** Header submission fails because SCP evidence cannot be assembled.

**Cause.** The watcher is behind or absent, and the history archives have not yet published the
checkpoint containing the ledger.

**Response.** None needed. The corridor degrades to the checkpoint cadence — roughly 5.5 minutes —
rather than failing. Investigate only if it persists beyond a checkpoint interval.
