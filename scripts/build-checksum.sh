#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/dist}"

CRATE="stellar-light-client"
ARTIFACT="stellar_light_client.wasm"
BINARYEN="${BINARYEN_VERSION:-129}"

if ! command -v wasm-opt >/dev/null 2>&1; then
  echo "error: wasm-opt not found. The release artifact is defined by this step;" >&2
  echo "       building without it produces a different checksum. Install binaryen $BINARYEN." >&2
  exit 1
fi

have="$(wasm-opt --version | grep -oE '[0-9]+' | head -1)"
if [ "$have" != "$BINARYEN" ]; then
  echo "error: wasm-opt version $have, expected $BINARYEN." >&2
  echo "       Different binaryen versions produce different bytes." >&2
  exit 1
fi

CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

export SOURCE_DATE_EPOCH=0
export RUSTFLAGS="--remap-path-prefix=$ROOT=/build --remap-path-prefix=$CARGO_HOME_DIR=/cargo"

cd "$ROOT"
cargo build --locked --release --target wasm32-unknown-unknown -p "$CRATE"

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
BUILT="$TARGET_DIR/wasm32-unknown-unknown/release/$ARTIFACT"

if [ ! -f "$BUILT" ]; then
  echo "error: no artifact at $BUILT" >&2
  exit 1
fi

mkdir -p "$OUT"
cp "$BUILT" "$OUT/$ARTIFACT"

wasm-opt \
  --enable-bulk-memory \
  --llvm-memory-copy-fill-lowering \
  -O1 \
  --strip-debug \
  --strip-producers \
  "$OUT/$ARTIFACT" \
  -o "$OUT/$ARTIFACT"

gzip -9 -n -k -f "$OUT/$ARTIFACT"

cd "$OUT"
{
  echo "$(shasum -a 256 "$ARTIFACT" | cut -d' ' -f1)  $ARTIFACT"
  echo "$(shasum -a 256 "$ARTIFACT.gz" | cut -d' ' -f1)  $ARTIFACT.gz"
} > checksums.txt

echo
echo "artifacts in $OUT"
cat checksums.txt
echo
echo "on-chain checksum is the uncompressed one:"
echo "  gunzip -c $ARTIFACT.gz | shasum -a 256"
