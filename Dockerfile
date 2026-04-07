FROM rust:1.82-bookworm AS rust-builder

WORKDIR /app
COPY Cargo.toml Cargo.toml
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY apps/api/src apps/api/src
RUN cargo build --workspace --release

FROM maven:3.9.9-eclipse-temurin-21 AS sidecar-builder

WORKDIR /build/sidecar
COPY sidecar/pom.xml pom.xml
COPY sidecar/src src
RUN mvn -B -DskipTests package

FROM eclipse-temurin:21-jre

WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends bash ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /app/target/release/fq-api /usr/local/bin/fq-api
COPY --from=sidecar-builder /build/sidecar/target/fq-sidecar.jar /app/fq-sidecar.jar
COPY configs/api.example.yaml /app/configs/api.example.yaml
COPY --chmod=755 scripts/container-entrypoint.sh /usr/local/bin/container-entrypoint.sh

EXPOSE 9999

ENTRYPOINT ["/usr/local/bin/container-entrypoint.sh"]

