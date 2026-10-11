# syntax=docker/dockerfile:1
FROM node:24.21.0-bookworm-slim AS node

FROM ubuntu:26.04 AS check
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl file pkg-config xz-utils \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    libssl-dev libxdo-dev libx11-dev libxi-dev libxtst-dev dbus \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/ /usr/local/
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH="/opt/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs -o /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain 1.92.0 --component rustfmt --component clippy \
    && rm /tmp/rustup-init.sh

WORKDIR /workspace
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm HUSKY=0 npm ci
COPY . .
# Files copied from a Windows checkout arrive with the executable bit set,
# and the bundler keeps those modes for resources and deb files.
RUN find . -path ./node_modules -prune -o -type f -exec chmod 644 {} +
ARG RELEASE_TAG
RUN --mount=type=cache,target=/opt/cargo/registry,sharing=locked \
    --mount=type=cache,target=/opt/cargo/git,sharing=locked \
    --mount=type=cache,id=pomolice-ubuntu26-target,target=/workspace/target,sharing=locked \
    if [ -n "$RELEASE_TAG" ]; then npm run release:check -- "$RELEASE_TAG"; fi \
    && npm run verify

FROM check AS build
RUN --mount=type=cache,target=/opt/cargo/registry,sharing=locked \
    --mount=type=cache,target=/opt/cargo/git,sharing=locked \
    --mount=type=cache,id=pomolice-ubuntu26-target,target=/workspace/target,sharing=locked \
    rm -f target/release/bundle/deb/*.deb \
    && npm run tauri -- build --bundles deb -- --locked --bin pomolice \
    && mkdir /packages \
    && node scripts/check-release-version.mjs > /packages/VERSION \
    && cp target/release/bundle/deb/*.deb /packages/ \
    && test "$(find /packages -name '*.deb' | wc -l)" -eq 1

FROM ubuntu:26.04 AS verify
ENV DEBIAN_FRONTEND=noninteractive
COPY --from=build /packages/ /packages/
COPY src-tauri/resources/sounds/*.wav THIRD-PARTY-NOTICES.md /expected/
COPY scripts/verify-deb.sh docker/verify-portal-registration.sh /checks/
RUN apt-get update && apt-get install -y --no-install-recommends \
    file desktop-file-utils dbus xdg-desktop-portal libglib2.0-bin \
    && sh /checks/verify-deb.sh /packages/*.deb "$(cat /packages/VERSION)" \
    && cd /packages && sha256sum *.deb > SHA256SUMS

FROM scratch AS artifacts
COPY --from=verify /packages/ /
