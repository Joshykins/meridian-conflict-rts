# meridian-server in a container. From the repository root:
#
#   docker build -f deploy/Dockerfile -t meridian-server .
#   docker run -d --name meridian -p 7777:7777 -v meridian-data:/data \
#       --restart unless-stopped meridian-server
#
# Arguments after the image name are passed to the server (see --help). Names and
# replays live in the /data volume. docs/SERVER.md has the whole guide.

# The toolchain pinned in rust-toolchain.toml; rustup inside the image reads it too.
FROM rust:1.98.0-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p mc-server --bin meridian-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin meridian \
    && mkdir /data && chown meridian:meridian /data
COPY --from=build /src/target/release/meridian-server /usr/local/bin/meridian-server
USER meridian
# Mount /data explicitly with Docker -v or a Railway Volume. Railway rejects
# Dockerfile VOLUME instructions; the directory still exists in the image.
EXPOSE 7777
# SIGTERM (docker stop) ends running games cleanly so their replays are kept.
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/meridian-server", "--data-dir", "/data"]
