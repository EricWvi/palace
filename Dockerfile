FROM node:24-alpine AS frontend

WORKDIR /build
COPY package.json package-lock.json ./
COPY apps/palace-web/package.json apps/palace-web/package.json
# npm ci installs every workspace the lockfile names; the extension itself is never built here.
COPY apps/palace-extension/package.json apps/palace-extension/package.json
RUN npm ci
COPY apps/palace-web apps/palace-web
# The app imports the shared design tokens and ornament geometry by path.
COPY design/tokens.css design/ornament.js design/ornament.d.ts design/
RUN npm run build -w apps/palace-web

FROM rust:1.95-alpine AS chef

WORKDIR /build
RUN apk add --no-cache build-base perl
# The pinned toolchain is installed here, once, rather than by the first cargo call of each build.
COPY rust-toolchain.toml ./
RUN cargo install cargo-chef --locked --version 0.1.78

# Every release changes the workspace sources, which invalidates the layer that compiles them. The
# recipe keeps only what decides the dependency build, so that layer stays cached until it changes.
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps/pal apps/pal
COPY apps/palace-server apps/palace-server
COPY xtask xtask
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS server

COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked -p palace-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps/pal apps/pal
COPY apps/palace-server apps/palace-server
COPY xtask xtask
RUN cargo build --release --locked -p palace-server

FROM alpine:latest AS runtime

# ripgrep searches the mounted 摘星 notes folder.
RUN apk add --no-cache ca-certificates ripgrep \
    && adduser -D --uid 10001 palace

WORKDIR /app
COPY --from=server /build/target/release/palace-server /usr/local/bin/palace-server
COPY --from=frontend /build/apps/palace-web/dist /app/dist

ARG VERSION=unknown
ARG BUILDTIME=unknown
LABEL org.opencontainers.image.title="Palace" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.created="$BUILDTIME"

ENV PALACE_LISTEN=0.0.0.0:8080 \
    PALACE_WEB_DIST=/app/dist
EXPOSE 8080
USER palace
ENTRYPOINT ["/usr/local/bin/palace-server"]
