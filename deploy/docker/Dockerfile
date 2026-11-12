# The image of the Arauto hub (docs/15-mvp.md, section 6).
#
# Build it from the root of the repository:
#   docker build -f deploy/docker/Dockerfile -t arauto-hub .
#
# It carries the hub and, to be served at /v1/download/, the connector for Linux x86_64. The
# WhatsApp engine is never in the image: whoever installs brings their own.

# ---------------------------------------------------------------------------- build
FROM rust:1.98-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock clippy.toml ./
COPY crates ./crates
# `--locked`: the versions are the ones of Cargo.lock, or the build fails.
# The connector is built with TLS (`wss://`): outside the machine of the hub there is no other way.
RUN cargo build --release --locked -p arauto-hub \
 && cargo build --release --locked -p arauto-connector --features tls \
 && mkdir -p /out/download \
 && cp target/release/arauto-hub /out/arauto-hub \
 && cp target/release/arauto-connector /out/download/arauto-connector-linux-x86_64

# ---------------------------------------------------------------------------- run
FROM debian:bookworm-slim
# An unprivileged user with a fixed number, so the volume of the data folder can be given to it.
RUN groupadd --system --gid 10001 arauto \
 && useradd --system --uid 10001 --gid arauto --home-dir /var/lib/arauto --shell /usr/sbin/nologin arauto \
 && mkdir -p /var/lib/arauto /usr/share/arauto/download \
 && chown arauto:arauto /var/lib/arauto \
 && chmod 700 /var/lib/arauto
COPY --from=build /out/arauto-hub /usr/local/bin/arauto-hub
COPY --from=build /out/download/ /usr/share/arauto/download/

# The master key and the audit mark: a volume of its own, apart from the volume of the database
# and out of its backup.
ENV ARAUTO_DATA_DIR=/var/lib/arauto \
    ARAUTO_DOWNLOAD_DIR=/usr/share/arauto/download \
    ARAUTO_LISTEN=0.0.0.0:8443 \
    ARAUTO_LISTEN_INTERNAL=0.0.0.0:8444
VOLUME /var/lib/arauto

USER arauto
# Only the port of the connectors is published. The internal one (health, events of the
# engine) is reached by the network of the composition, never from outside.
EXPOSE 8443
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
  CMD ["arauto-hub", "health"]
ENTRYPOINT ["arauto-hub"]
CMD ["serve"]
