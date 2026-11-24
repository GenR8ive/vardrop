# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS panel
WORKDIR /src
COPY apps/admin-web/package.json apps/admin-web/package-lock.json ./apps/admin-web/
RUN cd apps/admin-web && npm ci
COPY apps/admin-web ./apps/admin-web
RUN cd apps/admin-web && npm run build

FROM rust:1-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY config.example.toml config.host.example.toml ./
COPY crates ./crates
RUN cargo build --release -p vardrop-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data --shell /usr/sbin/nologin vardrop \
 && mkdir -p /data /app/panel /etc/vardrop \
 && chown -R vardrop:vardrop /data

COPY --from=server /src/target/release/vardrop-server /usr/local/bin/vardrop-server
COPY --from=panel /src/apps/admin-web/dist /app/panel
COPY deploy/config.docker.toml /etc/vardrop/config.toml
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN sed -i 's/\r$//' /usr/local/bin/docker-entrypoint.sh \
 && chmod 755 /usr/local/bin/docker-entrypoint.sh /usr/local/bin/vardrop-server

EXPOSE 8080
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/api/health || exit 1
ENTRYPOINT ["docker-entrypoint.sh"]
