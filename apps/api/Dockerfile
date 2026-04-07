FROM rust:1.82-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/api/src apps/api/src
RUN cargo build --workspace --release

FROM debian:bookworm-slim

WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY configs/api.example.yaml /app/configs/api.example.yaml

EXPOSE 9999
CMD ["fq-api"]

