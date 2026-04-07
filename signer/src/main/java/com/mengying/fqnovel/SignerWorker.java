package com.mengying.fqnovel;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mengying.fqnovel.unidbg.IdleFQ;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

public final class SignerWorker {

    private static final Logger log = LoggerFactory.getLogger(SignerWorker.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private SignerWorker() {
    }

    public static void main(String[] args) throws Exception {
        IdleFQ signer = new IdleFQ(
            Boolean.parseBoolean(System.getenv().getOrDefault("UNIDBG_VERBOSE", "false")),
            trimToNull(System.getenv("UNIDBG_APK_PATH")),
            defaultIfNull(
                trimToNull(System.getenv("UNIDBG_APK_CLASSPATH")),
                "com/dragon/read/oversea/gp/apk/base.apk"
            )
        );

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            signer.destroy();
        }));

        try (
            BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
            BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8))
        ) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isBlank()) {
                    continue;
                }
                WorkerResponse<?> response = handle(line, signer);
                writer.write(MAPPER.writeValueAsString(response));
                writer.newLine();
                writer.flush();
            }
        } finally {
            signer.destroy();
        }
    }

    private static WorkerResponse<?> handle(
        String line,
        IdleFQ signer
    ) {
        WorkerRequest request;
        try {
            request = MAPPER.readValue(line, WorkerRequest.class);
        } catch (Exception e) {
            return WorkerResponse.error("", 1001, "invalid request");
        }

        String requestId = request.id() == null || request.id().isBlank()
            ? UUID.randomUUID().toString()
            : request.id();

        try {
            String method = request.method();
            if (method == null || method.isBlank()) {
                throw new IllegalArgumentException("method 不能为空");
            }
            return switch (method) {
                case "sign" -> handleSign(requestId, request.params(), signer);
                default -> WorkerResponse.error(requestId, 1001, "invalid request");
            };
        } catch (IllegalArgumentException e) {
            return WorkerResponse.error(requestId, 1001, e.getMessage());
        } catch (Exception e) {
            log.error("signer worker request failed", e);
            return WorkerResponse.error(requestId, 1500, "internal signer error");
        }
    }

    private static WorkerResponse<?> handleSign(String requestId, JsonNode params, IdleFQ signer) {
        SignRequest request = MAPPER.convertValue(params, SignRequest.class);
        if (request.url() == null || request.url().isBlank()) {
            throw new IllegalArgumentException("url 不能为空");
        }
        if (request.headersText() == null) {
            throw new IllegalArgumentException("headers_text 不能为空");
        }
        String raw = signer.generateSignature(request.url(), request.headersText());
        if (raw == null || raw.isBlank()) {
            return WorkerResponse.error(requestId, 1003, "signer unavailable");
        }
        return WorkerResponse.success(requestId, new SignResult(raw));
    }

    private static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }

    private static String defaultIfNull(String value, String defaultValue) {
        return value == null ? defaultValue : value;
    }

    private record WorkerRequest(
        String id,
        String method,
        JsonNode params
    ) {
    }

    @JsonInclude(JsonInclude.Include.NON_NULL)
    private record WorkerResponse<T>(
        String id,
        int code,
        String message,
        T data,
        long serverTime
    ) {

        private static <T> WorkerResponse<T> success(String id, T data) {
            return new WorkerResponse<>(id, 0, "success", data, System.currentTimeMillis());
        }

        private static <T> WorkerResponse<T> error(String id, int code, String message) {
            return new WorkerResponse<>(id, code, message, null, System.currentTimeMillis());
        }
    }

    private record SignResult(
        String raw
    ) {
    }

    private record SignRequest(
        String url,
        @JsonProperty("headers_text") String headersText
    ) {
    }
}
