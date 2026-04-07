package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record RegisterKeyResolveResult(
    @JsonProperty("device_fingerprint") String deviceFingerprint,
    long keyver,
    @JsonProperty("real_key_hex") String realKeyHex,
    @JsonProperty("expires_at_ms") long expiresAtMs,
    String source
) {

    public RegisterKeyResolveResult withSource(String value) {
        return new RegisterKeyResolveResult(deviceFingerprint, keyver, realKeyHex, expiresAtMs, value);
    }
}

