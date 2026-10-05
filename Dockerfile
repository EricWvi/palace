FROM node:24-alpine AS frontend

WORKDIR /build
COPY package.json package-lock.json ./
COPY apps/palace-web/package.json apps/palace-web/package.json
RUN npm ci
COPY apps/palace-web apps/palace-web
# The app imports the shared design tokens and ornament geometry by path.
COPY design/tokens.css design/ornament.js design/ornament.d.ts design/
RUN npm run build -w apps/palace-web

FROM rust:1.95-alpine AS server

WORKDIR /build
RUN apk add --no-cache build-base perl
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
COPY apps/palace-server apps/palace-server
RUN cargo build --release --locked -p palace-server

FROM alpine:latest AS runtime

RUN apk add --no-cache ca-certificates \
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
