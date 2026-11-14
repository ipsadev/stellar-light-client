# Verifying the artifact

A governance vote approves a checksum. If that checksum cannot be reproduced from source, the vote approves a binary nobody has read. This page is how a reviewer checks; it takes about ten minutes.

## The short version

```sh
git clone --branch v<version> https://github.com/ipsadev/stellar-light-client
cd stellar-light-client
./scripts/release-build.sh dist
cat dist/checksums.txt
```

Compare against `checksums.txt` on the release. They must match exactly. Docker is required; the script refuses to run without it, because a host build produces different bytes.

## Why the build runs in a container

Reproducibility is not determinism. An earlier host build produced byte-identical output when repeated **on the same machine**, which proved nothing: a macOS arm64 host and a Linux x86_64 runner disagreed by one byte, because their `wasm-opt` binaries differ despite reporting the same version.

The container makes the platform part of the pin. `scripts/release.Dockerfile` fixes:

| Pinned | Otherwise |
|---|---|
| `rust:1.91.1-bookworm`, `--platform linux/amd64` | Host OS and architecture change codegen |
| binaryen 129, by tarball sha256 | Same version number, different binary, different bytes |
| `--locked` | Dependencies resolve differently over time |
| `--remap-path-prefix` | Absolute build paths are embedded in the binary |
| `SOURCE_DATE_EPOCH=0` | Build timestamps |
| `gzip -n` | The gzip header carries an mtime |

The tarball hash is checked during the image build, so a substituted binaryen fails the build rather than silently changing the output.

The build runs as root in the container (changing the build user would move `HOME` and `CARGO_HOME`, not worth adding to a reproducibility pin). On a native Linux Docker daemon that leaves `dist/` owned by `root`, so `release-build.sh` follows the build with a throwaway container that chowns `dist/` back to you. Without it, a second run fails to overwrite `dist/checksums.txt` with `Permission denied`. macOS does not show this, because Docker Desktop maps ownership back to the invoking user.

## Checking what a chain actually stored

```sh
gaiad q ibc-wasm checksums --node https://rpc.provider-sentry-01.hub-testnet.polypore.xyz
```

Compare against the uncompressed hash. To confirm a specific client instance runs that code:

```sh
gaiad q ibc client state 08-wasm-<n> --node <rpc>
```

The client state carries the checksum the client runs: the one it was instantiated against, or the one a later `migrate-contract` moved it to.

## If the checksums differ

Do not work around it. A mismatch means the release does not describe its source; report it rather than patching locally. Include your platform, your Docker version, and both hashes.
