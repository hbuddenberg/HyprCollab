# Stage 1: Build
FROM rust:1.95-slim-bookworm AS builder
WORKDIR /app

# Cache deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p crates/hyprcollab-core/src && \
    echo "fn main(){}" > crates/hyprcollab-core/src/main.rs && \
    cargo build --release -p hyprcollab-core 2>/dev/null || true

# Full build
COPY . .
RUN cargo build --release --workspace

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/hyprcollab-server /usr/local/bin/

EXPOSE 3000
CMD ["hyprcollab-server"]
