# Storing the client on a chain

`MsgStoreCode` is authority-gated. `08-wasm`'s keeper compares the message signer against a single
configured authority and rejects anything else, so there are exactly two ways for the wasm to land:
the authority signs it, or someone holding an authz grant from the authority signs it on its behalf.

Nothing about this is specific to this client. It is the same path the Ethereum light client took
onto the Cosmos Hub.

## Find out which door applies

```sh
REST=https://rest.provider-sentry-01.hub-testnet.polypore.xyz

curl -s $REST/cosmos/auth/v1beta1/module_accounts/gov
curl -s $REST/cosmos/authz/v1beta1/grants/granter/<gov address>
curl -s $REST/ibc/core/client/v1/params
```

The first gives the authority. The second lists who may act for it. The third confirms `08-wasm` is
in `allowed_clients` at all — without that, storing the code achieves nothing because no client of
that type can be created.

As of 2026-09-01 on `provider` and on Cosmos Hub mainnet, store-code is **delegated by authz**, not
gated on a vote. The grant holders are the people to talk to; a proposal is the fallback, not the
default.

## Route A — an authz holder signs

The holder runs, against a file containing the `MsgStoreCode`:

```sh
gaiad tx authz exec store-code.json --from <grantee> --node <rpc> --chain-id <id>
```

Submit the **gzipped** wasm in `wasm_byte_code`. It is roughly a third the size, the keeper gunzips
it before hashing, and the recorded checksum is unaffected.

## Route B — a governance proposal

```json
{
  "messages": [{
    "@type": "/ibc.lightclients.wasm.v1.MsgStoreCode",
    "signer": "<gov module authority>",
    "wasm_byte_code": "<base64 of stellar_light_client.wasm.gz>"
  }],
  "metadata": "<link to the forum post>",
  "deposit": "<at least min_deposit>uatom",
  "title": "Store the Stellar (SCP) light client",
  "summary": "..."
}
```

```sh
gaiad tx gov submit-proposal proposal.json --from <key> --node <rpc> --chain-id <id>
```

**Check the deposit against the chain before submitting.** `provider` requires 500 ATOM with a
`min_initial_deposit_ratio` of 0.1, so a proposal needs at least 50 ATOM up front or it is rejected
at submission:

```sh
gaiad q gov params --node <rpc>
```

Convention on the Hub is a forum post first, with `metadata` pointing at it. A proposal that appears
without discussion is rejected on process regardless of merit.

## What a reviewer will ask for

- The artifact and its checksum, reproducible from source — see [verify.md](verify.md)
- What the client verifies, and what it assumes
- How it is migrated or recovered if it breaks — see [migrate.md](migrate.md)
- Whether it has been audited, and by whom

## After it is stored

```sh
gaiad q ibc-wasm checksums --node <rpc>
```

The checksum should appear in that list. Storing code does not create a client; instantiation happens
separately, through the relayer, with the client state naming the checksum to run.
