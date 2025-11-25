# ===========================================
# Stage 1: Chef - Prepare recipe
# ===========================================
FROM rust:1.86-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ===========================================
# Stage 2: Planner - Analyze dependencies
# ===========================================
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ===========================================
# Stage 3: Builder - Build dependencies and app
# ===========================================
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    librdkafka-dev \
    libsasl2-dev \
    cmake \
    git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Configure cargo for better network reliability
ENV CARGO_NET_RETRY=10

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build application
COPY . .
RUN cargo build --release --bin api --bin kafka-worker

# ===========================================
# Stage 4: Runtime
# ===========================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    librdkafka1 \
    libsasl2-2 \
    curl \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install Typst from GitHub releases
ARG TYPST_VERSION=0.12.0
RUN cd /tmp && \
    curl -fsSLO https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-x86_64-unknown-linux-musl.tar.xz && \
    tar -xf typst-x86_64-unknown-linux-musl.tar.xz && \
    mv typst-x86_64-unknown-linux-musl/typst /usr/local/bin/ && \
    rm -rf typst-x86_64-unknown-linux-musl*

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/target/release/kafka-worker /app/kafka-worker

# Create non-root user
RUN useradd -r -s /bin/false appuser && \
    chown -R appuser:appuser /app

USER appuser

# Default environment variables
ENV RUST_LOG=info
ENV PDF_SERVER_HOST=0.0.0.0
ENV PDF_SERVER_PORT=8080

EXPOSE 8080

# Default command (can be overridden)
CMD ["./api"]
