FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends cmake pkg-config && rm -rf /var/lib/apt/lists/*
COPY .cargo .cargo
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY api/Cargo.toml api/Cargo.toml
COPY signer-native/Cargo.toml signer-native/Cargo.toml
COPY third_party/rnidbg third_party/rnidbg
COPY api/src api/src
COPY signer-native/src signer-native/src
RUN cargo build --workspace --release

FROM maven:3.9.13-eclipse-temurin-25 AS signer-builder

WORKDIR /build/signer
COPY signer/pom.xml pom.xml
COPY signer/src src
RUN mvn -B -DskipTests package

FROM gcr.io/distroless/java25-debian13:nonroot

WORKDIR /app

ENV UNIDBG_RESOURCE_ROOT=/app/unidbg
ENV RNIDBG_BASE_PATH=/app/rnidbg-sdk

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY --from=rust-builder /app/target/release/fq-signer-native /app/fq-signer-native
COPY --from=signer-builder /build/signer/target/fq-signer.jar /app/fq-signer.jar
COPY third_party/rnidbg/android/sdk23 /app/rnidbg-sdk
COPY signer/src/main/resources /app/unidbg
COPY configs/config.yaml /app/configs/config.yaml

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/fq-api"]
