FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends cmake pkg-config && rm -rf /var/lib/apt/lists/*
COPY .cargo .cargo
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY crates/api/Cargo.toml crates/api/Cargo.toml
COPY crates/signer-native/Cargo.toml crates/signer-native/Cargo.toml
COPY assets/fq-signer assets/fq-signer
COPY vendor/rnidbg vendor/rnidbg
COPY crates/api/src crates/api/src
COPY crates/signer-native/src crates/signer-native/src
RUN cargo build --workspace --release

FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY configs/config.yaml /app/configs/config.yaml

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/fq-api"]
