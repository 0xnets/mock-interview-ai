# syntax=docker/dockerfile:1.7

# ─── Stage 1: build the Vite SPA ─────────────────────────────────────────────
# node 22 LTS — pdfjs-dist@5.7+ requires >=22.13.
FROM node:22-alpine AS frontend-build
WORKDIR /app

COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build


# ─── Stage 2: build the Rust API ─────────────────────────────────────────────
FROM rust:1.89-bookworm AS backend-build
WORKDIR /src

ENV CARGO_TERM_COLOR=always

COPY backend/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p api -p transcript-verify \
 && mkdir -p /out \
 && cp target/release/api /out/api \
 && cp target/release/transcript-verify /out/transcript-verify


# ─── Stage 3: runtime ────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# TARGETARCH is set by buildx (amd64 / arm64). For Mac dev → EC2 amd64,
# build with: docker build --platform=linux/amd64 -t mock-interview .
ARG TARGETARCH
ARG S6_OVERLAY_VERSION=3.2.0.0
ARG CADDY_VERSION=2.8.4

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        xz-utils \
 && rm -rf /var/lib/apt/lists/*

# Caddy: single static binary, picked per arch.
RUN set -eux; \
    case "${TARGETARCH}" in \
        amd64) caddy_arch=amd64 ;; \
        arm64) caddy_arch=arm64 ;; \
        *) echo "unsupported TARGETARCH: ${TARGETARCH}" && exit 1 ;; \
    esac; \
    curl -fsSL "https://github.com/caddyserver/caddy/releases/download/v${CADDY_VERSION}/caddy_${CADDY_VERSION}_linux_${caddy_arch}.tar.gz" \
        | tar -xz -C /usr/local/bin caddy; \
    chmod +x /usr/local/bin/caddy

# s6-overlay v3 — noarch tarball + arch-specific tarball.
RUN set -eux; \
    case "${TARGETARCH}" in \
        amd64) s6_arch=x86_64 ;; \
        arm64) s6_arch=aarch64 ;; \
        *) echo "unsupported TARGETARCH: ${TARGETARCH}" && exit 1 ;; \
    esac; \
    curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-noarch.tar.xz" -o /tmp/s6-noarch.tar.xz; \
    curl -fsSL "https://github.com/just-containers/s6-overlay/releases/download/v${S6_OVERLAY_VERSION}/s6-overlay-${s6_arch}.tar.xz" -o /tmp/s6-arch.tar.xz; \
    tar -C / -Jxpf /tmp/s6-noarch.tar.xz; \
    tar -C / -Jxpf /tmp/s6-arch.tar.xz; \
    rm /tmp/s6-noarch.tar.xz /tmp/s6-arch.tar.xz

# Binaries
COPY --from=backend-build /out/api /usr/local/bin/api
COPY --from=backend-build /out/transcript-verify /usr/local/bin/transcript-verify

# Frontend dist
COPY --from=frontend-build /app/dist /srv/dist

# Caddyfile + s6 services
COPY Caddyfile /etc/caddy/Caddyfile
COPY deploy/s6/ /etc/s6-overlay/s6-rc.d/
# Enable both services in the default bundle
RUN mkdir -p /etc/s6-overlay/s6-rc.d/user/contents.d \
 && touch /etc/s6-overlay/s6-rc.d/user/contents.d/caddy \
 && touch /etc/s6-overlay/s6-rc.d/user/contents.d/api \
 && chmod +x /etc/s6-overlay/s6-rc.d/caddy/run \
              /etc/s6-overlay/s6-rc.d/api/run

EXPOSE 80 443

# If either supervised service dies, exit the container so compose restarts it.
ENV S6_BEHAVIOUR_IF_STAGE2_FAILS=2

ENTRYPOINT ["/init"]
