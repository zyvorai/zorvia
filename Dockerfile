# Multi-stage build for minimal production image
FROM rust:1.76-slim-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY tests/ tests/
COPY examples/ examples/

RUN cargo build --release --locked && strip target/release/zorvia

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/zorvia /usr/local/bin/zorvia

RUN useradd -m zorvia
USER zorvia

ENTRYPOINT ["zorvia"]
CMD ["--help"]
