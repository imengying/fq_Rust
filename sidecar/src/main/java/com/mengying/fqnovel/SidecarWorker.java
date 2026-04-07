package com.mengying.fqnovel;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.mengying.fqnovel.config.UnidbgProperties;
import com.mengying.fqnovel.dto.*;
import com.mengying.fqnovel.service.*;
import com.mengying.fqnovel.utils.ProcessLifecycle;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.UUID;

public final class SidecarWorker {

    private static final Logger log = LoggerFactory.getLogger(SidecarWorker.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private SidecarWorker() {
    }

    public static void main(String[] args) throws Exception {
        UnidbgProperties unidbgProperties = UnidbgProperties.fromEnv();
        FQEncryptServiceWorker encryptWorker = new FQEncryptServiceWorker(unidbgProperties);
        SignerService signerService = new SignerService(encryptWorker);

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            ProcessLifecycle.markShuttingDown("worker-shutdown");
            encryptWorker.destroy();
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
                WorkerResponse<?> response = handle(line, signerService);
                writer.write(MAPPER.writeValueAsString(response));
                writer.newLine();
                writer.flush();
            }
        } finally {
            encryptWorker.destroy();
        }
    }

    private static WorkerResponse<?> handle(
        String line,
        SignerService signerService
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
                case "sign" -> WorkerResponse.success(requestId, handleSign(request.params(), signerService));
                case "signer-reset" -> WorkerResponse.success(requestId, handleSignerReset(request.params(), signerService));
                default -> WorkerResponse.error(requestId, 1001, "invalid request");
            };
        } catch (SignerUnavailableException e) {
            return WorkerResponse.error(requestId, 1003, e.getMessage());
        } catch (IllegalArgumentException e) {
            return WorkerResponse.error(requestId, 1001, e.getMessage());
        } catch (Exception e) {
            log.error("sidecar worker request failed", e);
            return WorkerResponse.error(requestId, 1500, "internal sidecar error");
        }
    }

    private static SignResult handleSign(JsonNode params, SignerService signerService) {
        SignRequest request = MAPPER.convertValue(params, SignRequest.class);
        return signerService.sign(request.url(), request.headers());
    }

    private static SignerResetResult handleSignerReset(JsonNode params, SignerService signerService) {
        SignerResetRequest request = MAPPER.convertValue(params, SignerResetRequest.class);
        SignerResetDecision decision = signerService.reset(request.reason());
        return new SignerResetResult(true, decision.signerEpoch(), decision.cooldownApplied());
    }
}
