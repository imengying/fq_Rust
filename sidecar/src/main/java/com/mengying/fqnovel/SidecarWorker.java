package com.mengying.fqnovel;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mengying.fqnovel.config.UnidbgProperties;
import com.mengying.fqnovel.dto.*;
import com.mengying.fqnovel.service.FQEncryptService;
import com.mengying.fqnovel.utils.ProcessLifecycle;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.UUID;

public final class SidecarWorker {

    private static final Logger log = LoggerFactory.getLogger(SidecarWorker.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private SidecarWorker() {
    }

    public static void main(String[] args) throws Exception {
        UnidbgProperties unidbgProperties = UnidbgProperties.fromEnv();
        FQEncryptService signer = new FQEncryptService(unidbgProperties);

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            ProcessLifecycle.markShuttingDown("worker-shutdown");
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
        FQEncryptService signer
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
            log.error("sidecar worker request failed", e);
            return WorkerResponse.error(requestId, 1500, "internal sidecar error");
        }
    }

    private static WorkerResponse<?> handleSign(String requestId, JsonNode params, FQEncryptService signer) {
        SignRequest request = MAPPER.convertValue(params, SignRequest.class);
        if (request.url() == null || request.url().isBlank()) {
            throw new IllegalArgumentException("url 不能为空");
        }
        if (request.headers() == null) {
            throw new IllegalArgumentException("headers 不能为空");
        }
        var signed = signer.generateSignatureHeaders(request.url(), request.headers());
        if (signed == null || signed.isEmpty()) {
            return WorkerResponse.error(requestId, 1003, "signer unavailable");
        }
        return WorkerResponse.success(requestId, new SignResult(new LinkedHashMap<>(signed)));
    }
}
