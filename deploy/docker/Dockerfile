# The image of the Arauto hub (docs/15-mvp.md, section 6).
#
# Build it from the root of the repository:
#   docker build -f deploy/docker/Dockerfile -t arauto-hub .
#
# It carries the hub and, to be served at /v1/download/, the connector for Linux x86_64, Windows
# x86_64 and macOS (arm64 and x86_64). The WhatsApp engine is never in the image: whoever installs
# brings their own.
#
# The connectors are built first. Their build number and the SHA-256 of each file are then
# compiled into the hub, which at start compares the files it serves with those digests: the hub
# knows the build of the connectors it serves (decision 0053, proposal).
#
# Fact stated by the maintainer's assistant on 14/11/2026: the Windows and macOS builds below
# compile and link in this build image. Fact of 24/11/2026: the Windows file of this image runs a
# real Hermes. The macOS files were never run.

# ---------------------------------------------------------------------------- build
FROM rust:1.98-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock clippy.toml ./
COPY crates ./crates
# `--locked`: the versions are the ones of Cargo.lock, or the build fails.
# The connector is built with TLS (`wss://`): outside the machine of the hub there is no other way.
RUN cargo build --release --locked -p arauto-connector --features tls \
 && mkdir -p /out/download \
 && cp target/release/arauto-connector /out/download/arauto-connector-linux-x86_64
# The connector for Windows x86_64: the GNU target, linked by mingw-w64. Two things made two
# builds of the same sources give two files with two SHA-256 digests, so a rebuild of the image for
# the hub alone changed the digest it serves for Windows and a connector installed before it showed
# as "Other build":
# - `--no-insert-timestamp`: by default this linker writes the time of the link into the PE header
#   (TimeDateStamp, and the CheckSum changes with it). With the option the field is zero; Windows
#   does not read it to run a program.
# - `-C strip=symbols`: the file carried a table of symbols whose generated names (`.llvm.` and a
#   number) and order change from one build from scratch to the next. Without the table the program
#   runs the same; a panic on Windows names fewer functions.
# Cargo reads the variable for this target only: the Linux file above is built without it. The
# last lines read the two fields and fail the build when the time is not zero or the table is
# there, for instance if a RUSTFLAGS set elsewhere made Cargo ignore the variable. Checked on
# 24/11/2026: two builds from scratch with both are byte-identical (referencia/build-fatos.md).
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add x86_64-pc-windows-gnu \
 && CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS='-C link-arg=-Wl,--no-insert-timestamp -C strip=symbols' \
    cargo build --release --locked -p arauto-connector --features tls --target x86_64-pc-windows-gnu \
 && cp target/x86_64-pc-windows-gnu/release/arauto-connector.exe /out/download/arauto-connector-windows-x86_64.exe \
 && pe="$(od -An -tu4 -j60 -N4 /out/download/arauto-connector-windows-x86_64.exe | tr -d ' ')" \
 && test "$(od -An -tx4 -j"$((pe + 8))" -N4 /out/download/arauto-connector-windows-x86_64.exe | tr -d ' ')" = 00000000 \
 && test "$(od -An -tu4 -j"$((pe + 16))" -N4 /out/download/arauto-connector-windows-x86_64.exe | tr -d ' ')" = 0
# The connector for macOS (arm64 and x86_64): linked by zig through cargo-zigbuild. The linker
# warns that no Apple SDK was found and still links.
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3-pip \
 && rm -rf /var/lib/apt/lists/* \
 && pip3 install --break-system-packages ziglang==0.17.1 cargo-zigbuild==0.24.1 \
 && rustup target add aarch64-apple-darwin x86_64-apple-darwin \
 && cargo zigbuild --release --locked -p arauto-connector --features tls --target aarch64-apple-darwin \
 && cargo zigbuild --release --locked -p arauto-connector --features tls --target x86_64-apple-darwin \
 && cp target/aarch64-apple-darwin/release/arauto-connector /out/download/arauto-connector-macos-aarch64 \
 && cp target/x86_64-apple-darwin/release/arauto-connector /out/download/arauto-connector-macos-x86_64
# The hub, last: it is built with the number and the digests of the four connectors above.
RUN export ARAUTO_BUNDLE_CONNECTOR_VERSION="$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)"$/\1/p' crates/arauto-connector/Cargo.toml | head -n 1)" \
 && export ARAUTO_BUNDLE_CONNECTOR_SHA256="$(cd /out/download && for f in arauto-connector-*; do printf '%s=%s,' "$f" "$(sha256sum "$f" | cut -d ' ' -f 1)"; done | sed 's/,$//')" \
 && test -n "$ARAUTO_BUNDLE_CONNECTOR_VERSION" && test -n "$ARAUTO_BUNDLE_CONNECTOR_SHA256" \
 && cargo build --release --locked -p arauto-hub \
 && cp target/release/arauto-hub /out/arauto-hub

# ---------------------------------------------------------------------------- run
FROM debian:bookworm-slim
# An unprivileged user with a fixed number, so the volume of the data folder can be given to it.
# /run/arauto-engine: the mount point of the volume shared with the engine, which mounts it
# read-only. It holds the digest of the engine key, never the key.
RUN groupadd --system --gid 10001 arauto \
 && useradd --system --uid 10001 --gid arauto --home-dir /var/lib/arauto --shell /usr/sbin/nologin arauto \
 && mkdir -p /var/lib/arauto /usr/share/arauto/download /run/arauto-engine \
 && chown arauto:arauto /var/lib/arauto /run/arauto-engine \
 && chmod 700 /var/lib/arauto \
 && chmod 755 /run/arauto-engine
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
# The port of the connectors is the one published to the proxy. The internal one (health,
# events of the engine) is reached by the network of the composition, never from outside.
# The panel is off in the image: its port only exists when the composition sets
# ARAUTO_LISTEN_PANEL and ARAUTO_PANEL_URL.
EXPOSE 8443
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
  CMD ["arauto-hub", "health"]
ENTRYPOINT ["arauto-hub"]
CMD ["serve"]
