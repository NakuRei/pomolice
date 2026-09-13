# syntax=docker/dockerfile:1
FROM node:24.21.0-bookworm-slim AS node

FROM ubuntu:22.04 AS build
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl file pkg-config xz-utils \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    libssl-dev libxdo-dev libx11-dev libxi-dev libxtst-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/ /usr/local/
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs -o /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain 1.92.0 \
    && rm /tmp/rustup-init.sh

WORKDIR /workspace
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm HUSKY=0 npm ci
COPY . .
RUN --mount=type=cache,target=/opt/cargo/registry,sharing=locked \
    --mount=type=cache,target=/opt/cargo/git,sharing=locked \
    --mount=type=cache,target=/workspace/src-tauri/target,sharing=locked \
    npm run tauri -- build --bundles deb -- --locked \
    && mkdir /packages \
    && cp src-tauri/target/release/bundle/deb/*.deb /packages/

FROM ubuntu:22.04 AS verify
ENV DEBIAN_FRONTEND=noninteractive
COPY --from=build /packages/ /packages/
RUN apt-get update \
    && apt-get install -y --no-install-recommends /packages/*.deb \
    && dpkg-query -W pomolice \
    && ldd /usr/bin/pomolice > /packages/shared-libraries.txt \
    && ! grep -q 'not found' /packages/shared-libraries.txt \
    && cd /packages && sha256sum *.deb > SHA256SUMS

FROM scratch AS artifacts
COPY --from=verify /packages/ /
