# Phalus - Multi-stage Rust build with cargo-chef for dependency caching
FROM rust:1.88-slim-bookworm AS chef

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

RUN cargo install cargo-chef
WORKDIR /app

# --- Planner: generate dependency recipe ---
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder: cook deps then build app ---
FROM chef AS builder

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_RELEASE_STRIP=true \
    CARGO_PROFILE_RELEASE_LTO=true \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_OPT_LEVEL=3

# Cook dependencies (cached when only source changes)
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --recipe-path recipe.json -j2

# Copy source and build
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release -j2 && \
    cp target/release/phalus /tmp/phalus

# --- Runtime stage - minimal image ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user
RUN groupadd -r phalus && useradd -r -g phalus -u 1000 -d /var/lib/phalus phalus

# Create directories for data and configuration
RUN mkdir -p /var/lib/phalus /etc/phalus && \
    chown -R phalus:phalus /var/lib/phalus /etc/phalus

# Copy binary from builder
COPY --from=builder /tmp/phalus /usr/local/bin/phalus

# Set proper ownership and permissions
RUN chown phalus:phalus /usr/local/bin/phalus && \
    chmod +x /usr/local/bin/phalus

# Switch to non-root user
USER phalus

ENV HOME=/var/lib/phalus
WORKDIR /var/lib/phalus

# Expose web UI port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

ENTRYPOINT ["/usr/local/bin/phalus"]
CMD ["serve"]

LABEL org.opencontainers.image.title="Phalus" \
      org.opencontainers.image.description="Private Headless Automated License Uncoupling System" \
      org.opencontainers.image.licenses="0BSD" \
      org.opencontainers.image.source="https://github.com/phalus-sh/phalus"
