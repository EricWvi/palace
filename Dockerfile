FROM node:24-bookworm-slim AS frontend

WORKDIR /build
COPY package.json package-lock.json ./
COPY apps/palace-web/package.json apps/palace-web/package.json
RUN npm ci
COPY apps/palace-web apps/palace-web
RUN npm run build -w apps/palace-web

FROM rust:1.95-bookworm AS server

WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY apps/palace-server apps/palace-server
RUN cargo build --release --locked -p palace-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 palace

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
