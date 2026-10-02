# syntax=docker/dockerfile:1

# ------------------------------------------------------------------------------
# Build Stage: Compile cleanly with musl
# ------------------------------------------------------------------------------
FROM rust:alpine AS builder

WORKDIR /usr/src/app

# Install compilation essentials
RUN apk add --no-cache musl-dev

# Copy all source files and compile directly (guarantees real code is compiled)
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY web ./web

RUN cargo build --release

# ------------------------------------------------------------------------------
# Runtime Stage: Ultra-lightweight Alpine container (< 15MB total)
# ------------------------------------------------------------------------------
FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/sinel /app/sinel

# Security: Run as dedicated non-root user
RUN adduser -D -u 10001 -g 10001 proxyuser
USER proxyuser:proxyuser

ENV PORT=8080
ENV HOST=0.0.0.0
ENV RUST_LOG=info

EXPOSE 8080

ENTRYPOINT ["/app/sinel"]
