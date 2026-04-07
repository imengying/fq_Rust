package com.mengying.fqnovel;

import com.mengying.fqnovel.unidbg.IdleFQ;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.Base64;

public final class SignerWorker {

    private static final String DEFAULT_RESOURCE_ROOT = "/app/unidbg";
    private static final Base64.Decoder DECODER = Base64.getUrlDecoder();
    private static final Base64.Encoder ENCODER = Base64.getUrlEncoder().withoutPadding();

    private SignerWorker() {
    }

    public static void main(String[] args) throws Exception {
        ConsoleNoiseFilter.install();

        IdleFQ signer = new IdleFQ(
            Boolean.parseBoolean(System.getenv().getOrDefault("UNIDBG_VERBOSE", "false")),
            trimToNull(System.getenv("UNIDBG_APK_PATH")),
            defaultIfNull(trimToNull(System.getenv("UNIDBG_RESOURCE_ROOT")), DEFAULT_RESOURCE_ROOT)
        );

        Runtime.getRuntime().addShutdownHook(new Thread(signer::destroy));

        try (
            BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
            BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8))
        ) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isBlank()) {
                    continue;
                }
                writer.write(handle(line, signer));
                writer.newLine();
                writer.flush();
            }
        } finally {
            signer.destroy();
        }
    }

    private static String handle(String line, IdleFQ signer) {
        try {
            String[] parts = line.split("\t", -1);
            if (parts.length != 3 || !"sign".equals(parts[0])) {
                return encodeError(1001, "invalid request");
            }

            String url = decodeField(parts[1], "url");
            String headersText = decodeField(parts[2], "headers_text");
            if (url.isBlank()) {
                throw new IllegalArgumentException("url 不能为空");
            }

            String raw = signer.generateSignature(url, headersText);
            if (raw == null || raw.isBlank()) {
                return encodeError(1003, "signer unavailable");
            }

            return "ok\t" + encodeField(raw);
        } catch (IllegalArgumentException e) {
            return encodeError(1001, e.getMessage());
        } catch (Exception e) {
            System.err.printf("signer worker request failed: %s%n", e.getMessage());
            e.printStackTrace(System.err);
            return encodeError(1500, "internal signer error");
        }
    }

    private static String encodeError(int code, String message) {
        return "err\t" + code + "\t" + encodeField(message);
    }

    private static String encodeField(String value) {
        return ENCODER.encodeToString(value.getBytes(StandardCharsets.UTF_8));
    }

    private static String decodeField(String value, String fieldName) {
        try {
            byte[] decoded = DECODER.decode(value);
            return new String(decoded, StandardCharsets.UTF_8);
        } catch (IllegalArgumentException e) {
            throw new IllegalArgumentException(fieldName + " 非法");
        }
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
}
