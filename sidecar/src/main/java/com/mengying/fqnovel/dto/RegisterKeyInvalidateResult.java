package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record RegisterKeyInvalidateResult(
    @JsonProperty("device_fingerprint") String deviceFingerprint,
    boolean invalidated
) {
}

