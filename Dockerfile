FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends cmake pkg-config && rm -rf /var/lib/apt/lists/*
COPY .cargo .cargo
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY api/Cargo.toml api/Cargo.toml
COPY signer-native/Cargo.toml signer-native/Cargo.toml
COPY resources resources
COPY third_party/rnidbg third_party/rnidbg
COPY api/src api/src
COPY signer-native/src signer-native/src
RUN cargo build --workspace --release

FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

ENV FQ_SIGNER_RESOURCE_ROOT=/app/resources
ENV RNIDBG_BASE_PATH=/app/rnidbg-sdk

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY third_party/rnidbg/android/sdk23 /app/rnidbg-sdk
COPY resources /app/resources
COPY configs/config.yaml /app/configs/config.yaml

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/fq-api"]
