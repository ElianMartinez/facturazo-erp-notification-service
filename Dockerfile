# ===========================================
# Stage 1: Chef - Prepare recipe (Alpine)
# ===========================================
FROM rust:1.86-alpine AS chef
RUN apk add --no-cache musl-dev
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
RUN apk add --no-cache \
    bash \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    librdkafka-dev \
    cyrus-sasl-dev \
    cmake \
    make \
    gcc \
    g++ \
    git \
    zlib-dev \
    zlib-static \
    perl \
    python3

WORKDIR /app

# Configure cargo for better network reliability and dynamic linking
ENV CARGO_NET_RETRY=10
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV RUSTFLAGS="-C target-feature=-crt-static"

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and build application
COPY . .

# Build only the api binary (includes both HTTP + Kafka worker)
RUN cargo build --release --bin api

# Strip binary for smaller size
RUN strip target/release/api

# ===========================================
# Stage 4: Runtime (minimal Alpine)
# ===========================================
FROM alpine:3.21 AS runtime

# Install minimal runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libssl3 \
    librdkafka \
    libsasl \
    libgcc \
    xz \
    curl

# Install Typst from GitHub releases
ARG TYPST_VERSION=0.12.0
ARG TARGETARCH
RUN cd /tmp && \
    if [ "$TARGETARCH" = "arm64" ]; then \
    ARCH="aarch64"; \
    else \
    ARCH="x86_64"; \
    fi && \
    wget -q https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-${ARCH}-unknown-linux-musl.tar.xz && \
    tar -xf typst-${ARCH}-unknown-linux-musl.tar.xz && \
    mv typst-${ARCH}-unknown-linux-musl/typst /usr/local/bin/ && \
    rm -rf typst-* && \
    typst --version

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/api /app/pdf-service

# Copy custom fonts for PDF generation
COPY fonts/ /app/fonts/

# Create necessary directories
RUN mkdir -p /app/work /app/storage /app/data

# Create non-root user and set permissions
RUN adduser -D -H -s /sbin/nologin appuser && \
    chown -R appuser:appuser /app

USER appuser

# Default environment variables
ENV RUST_LOG=info
ENV HOST=0.0.0.0
ENV PORT=8080
ENV PDF_FONTS_DIR=/app/fonts

# Kafka defaults (can be overridden)
ENV KAFKA_ENABLED=true
ENV KAFKA_BROKERS=kafka:9092
ENV KAFKA_GROUP_ID=pdf-service-worker
ENV KAFKA_TOPICS=document-generate-request,notification-dispatch-request
ENV KAFKA_RESPONSE_TOPIC=document-events

EXPOSE 8080

# Run the combined HTTP + Kafka service
CMD ["./pdf-service"]
