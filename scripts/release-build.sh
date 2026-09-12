#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/dist}"
IMAGE="stellar-light-client-release:129-1.91.1"

if ! docker info >/dev/null 2>&1; then
  echo "error: docker is required. The published checksum is defined by the build" >&2
  echo "       container, so a host build would produce different bytes." >&2
  exit 1
fi

docker build \
  --platform linux/amd64 \
  -f "$ROOT/scripts/release.Dockerfile" \
  -t "$IMAGE" \
  "$ROOT/scripts" >/dev/null

mkdir -p "$OUT"

docker run --rm \
  --platform linux/amd64 \
  -v "$ROOT:/src" \
  -v "stellar-light-client-target:/target" \
  -v "stellar-light-client-registry:/usr/local/cargo/registry" \
  -w /src \
  "$IMAGE" \
  ./scripts/build-checksum.sh /src/dist

docker run --rm \
  --platform linux/amd64 \
  -v "$ROOT/dist:/out" \
  "$IMAGE" \
  chown -R "$(id -u):$(id -g)" /out

OUT_ABS="$(cd "$OUT" && pwd)"

if [ "$OUT_ABS" != "$ROOT/dist" ]; then
  cp "$ROOT/dist"/* "$OUT_ABS/"
fi

echo
echo "built in $IMAGE (linux/amd64)"
cat "$OUT_ABS/checksums.txt"
