FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/api/src apps/api/src
RUN cargo build --workspace --release

FROM maven:3.9.13-eclipse-temurin-25 AS signer-builder

WORKDIR /build/signer
COPY signer/pom.xml pom.xml
COPY signer/src src
RUN mvn -B -DskipTests package

FROM gcr.io/distroless/java25-debian13:nonroot

WORKDIR /app

ENV UNIDBG_RESOURCE_ROOT=/app/unidbg

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY --from=signer-builder /build/signer/target/fq-signer.jar /app/fq-signer.jar
COPY signer/src/main/resources /app/unidbg
COPY configs/api.example.yaml /app/configs/api.example.yaml

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/fq-api"]
