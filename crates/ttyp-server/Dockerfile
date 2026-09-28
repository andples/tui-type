# Build context is the repo root (the server depends on crates/ttyp-core):
#   docker build -f crates/ttyp-server/Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p ttyp-server --locked

# Runtime: slim Debian, the server binary, the sqlite3 CLI for backups and
# inspection, and gzip for the backup sidecar. Runs as an unprivileged user.
FROM debian:stable-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends sqlite3 gzip ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /data --shell /usr/sbin/nologin ttyp \
 && mkdir -p /data /backups && chown ttyp:ttyp /data /backups
COPY --from=build /src/target/release/ttyp-server /usr/local/bin/ttyp-server
COPY --chmod=755 crates/ttyp-server/backup.sh /usr/local/bin/ttyp-backup
USER ttyp
ENV TTYP_BIND=0.0.0.0:8080 TTYP_DB=/data/ttyp.db RUST_LOG=info
EXPOSE 8080
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["ttyp-server", "healthcheck"]
CMD ["ttyp-server"]
