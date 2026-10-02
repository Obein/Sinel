# syntax=docker/dockerfile:1

# ------------------------------------------------------------------------------
# Build Stage: Compile statically linked binary using musl
# ------------------------------------------------------------------------------
FROM rust:alpine AS builder

WORKDIR /usr/src/app

# Install compilation essentials
RUN apk add --no-cache musl-dev

# 1. Pre-build dependencies for Docker layer caching
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 2. Build application with real source code
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ------------------------------------------------------------------------------
# Runtime Stage: Ultra-lightweight Alpine container (< 15MB total)
# ------------------------------------------------------------------------------
FROM alpine:3.20

# Install ca-certificates for upstream HTTPS/TLS SNI resolution
RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/signal-tls-proxy-fly /app/signal-tls-proxy-fly

# Security: Run as dedicated non-root user
RUN adduser -D -u 10001 -g 10001 proxyuser
USER proxyuser:proxyuser

# Fly.io defaults
ENV PORT=8080
ENV HOST=0.0.0.0
ENV RUST_LOG=info

EXPOSE 8080

ENTRYPOINT ["/app/signal-tls-proxy-fly"]
