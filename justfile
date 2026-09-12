# Stellar 08-wasm light client

crate      := "stellar-light-client"
artifact   := "stellar_light_client.wasm"

# clippy allowances, defined once so CI and local runs cannot drift
lints := "-D warnings -A clippy::manual_is_multiple_of -A clippy::too_many_arguments -A clippy::result_large_err"

default:
    @just --list

# everything CI runs, in CI's order
ci: fmt-check lint test audit

fmt:
    cargo +nightly fmt --all

# what CI checks, on the pinned stable toolchain
fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --locked --all-targets -- {{lints}}

test:
    cargo test --locked

# one test binary, e.g. `just test-one scp` or `just test-one entrypoint`
test-one name:
    cargo test --locked --test {{name}}

# a single test by name, across every binary
test-filter pattern:
    cargo test --locked -- {{pattern}}

audit:
    cargo audit --file Cargo.lock

# compile to wasm without the reproducibility pins; NOT the release artifact
build-wasm:
    cargo build --locked --release --target wasm32-unknown-unknown -p {{crate}}

# the reproducible release build, in a pinned container
release:
    ./scripts/release-build.sh dist

# prove the release build reproduces from a cold cache
verify-reproducible:
    #!/usr/bin/env bash
    set -euo pipefail
    ./scripts/release-build.sh dist
    cp dist/checksums.txt /tmp/first.txt
    docker volume rm -f stellar-light-client-target >/dev/null
    rm -rf dist
    ./scripts/release-build.sh dist >/dev/null
    diff -u /tmp/first.txt dist/checksums.txt
    echo "rebuilt byte-identically"
    cat dist/checksums.txt

# the on-chain checksum of the built artifact
checksum:
    @gunzip -c dist/{{artifact}}.gz | shasum -a 256 | cut -d' ' -f1

clean:
    cargo clean
    rm -rf dist
