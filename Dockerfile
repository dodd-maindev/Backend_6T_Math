# Build stage
FROM rust:slim AS builder
WORKDIR /app

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Install system dependencies needed for compiling SQLx (TLS/SSL support)
RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the source code
COPY . .

# Build the release binary
RUN cargo build --release

# Run stage
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled binary and necessary assets from builder
COPY --from=builder /app/target/release/backend-6t-math /app/backend-6t-math
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/.env /app/.env

EXPOSE 8000

CMD ["./backend-6t-math"]
