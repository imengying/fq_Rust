FROM maven:3.9.9-eclipse-temurin-21 AS builder

WORKDIR /app
COPY pom.xml pom.xml
COPY src src
RUN mvn -B -DskipTests package

FROM eclipse-temurin:21-jre

WORKDIR /app
COPY --from=builder /app/target/fq-sidecar.jar /app/fq-sidecar.jar

EXPOSE 18080
CMD ["java", "--enable-native-access=ALL-UNNAMED", "-jar", "/app/fq-sidecar.jar"]

