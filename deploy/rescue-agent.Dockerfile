# Rescue mode's Job entrypoint image. GuestKit's disk-mount path shells out
# to losetup/qemu-nbd/mount/chroot (see src/kube/rescue.rs and
# docs/RESCUE.md) -- this needs the same package set GuestKit's own worker
# image ships (qemu-utils, util-linux, kmod), and the container must run
# with SYS_ADMIN + SYS_RESOURCE (set in the Job's pod spec, not here).
FROM rust:1.98-slim-bookworm@sha256:dacc9e51f252243eb59d2fb4cb4ad8b0d3f607b6a82c398cf8a321e59ff778a7 AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release --locked --no-default-features --features rescue-agent --bin rescue-agent \
    && strip target/release/rescue-agent

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251

RUN apt-get update && apt-get install -y --no-install-recommends \
    qemu-utils \
    util-linux \
    kmod \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/rescue-agent /usr/local/bin/rescue-agent

# Must run as root: losetup/qemu-nbd/mount/chroot need real privilege, same
# as GuestKit's own worker image (crates/guestkit-worker/Dockerfile in
# ../guestkit) and its k8s/daemonset.yaml's SYS_ADMIN+SYS_RESOURCE pod spec.
ENTRYPOINT ["rescue-agent"]
