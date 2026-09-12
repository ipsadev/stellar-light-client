FROM --platform=linux/amd64 rust:1.91.1-bookworm

ARG BINARYEN_VERSION=129
ARG BINARYEN_SHA256=50b9fa62b9abea752da92ec57e0c555fee578760cd237c40107957715d2976ba

RUN curl -sSL "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz" \
      -o /tmp/binaryen.tar.gz \
 && echo "${BINARYEN_SHA256}  /tmp/binaryen.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/binaryen.tar.gz -C /opt \
 && rm /tmp/binaryen.tar.gz

ENV PATH="/opt/binaryen-version_129/bin:${PATH}"
ENV CARGO_TARGET_DIR=/target

RUN rustup target add wasm32-unknown-unknown

WORKDIR /src
