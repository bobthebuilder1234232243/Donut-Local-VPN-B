# smoltcp 0.14 needs Rust 1.91, so track the current stable toolchain, the same
# one the release workflow uses (dtolnay/rust-toolchain@stable).
FROM rust:bookworm

# iproute2 brings `ip`, which the engine uses to install and remove routes.
# ca-certificates is needed for DNS over HTTPS.
RUN apt-get update \
    && apt-get install -y --no-install-recommends iproute2 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# The harness includes the real sources through #[path], so both trees are
# copied. The Tauri-only files are never compiled by this crate.
COPY src-tauri/tunnel-harness /app/src-tauri/tunnel-harness
COPY src-tauri/src /app/src-tauri/src

WORKDIR /app/src-tauri/tunnel-harness

# Build the test binary while the image is built so both DPI runs reuse it.
RUN cargo test --release --no-run

CMD ["cargo", "test", "--release", "--", "--nocapture", "--test-threads=1"]
