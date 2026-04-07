FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/api/src apps/api/src
RUN cargo build --workspace --release

FROM maven:3.9.13-eclipse-temurin-25 AS sidecar-builder

WORKDIR /build/sidecar
COPY sidecar/pom.xml pom.xml
COPY sidecar/src src
RUN mvn -B -DskipTests package

FROM gcr.io/distroless/java25-debian13:nonroot

WORKDIR /app

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY --from=rust-builder /app/target/release/container-launcher /usr/local/bin/container-launcher
COPY --from=sidecar-builder /build/sidecar/target/fq-sidecar.jar /app/fq-sidecar.jar
COPY configs/api.example.yaml /app/configs/api.example.yaml

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/container-launcher"]
