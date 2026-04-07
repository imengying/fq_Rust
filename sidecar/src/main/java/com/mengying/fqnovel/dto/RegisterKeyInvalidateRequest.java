package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record RegisterKeyInvalidateRequest(
    @JsonProperty("device_fingerprint") String deviceFingerprint
) {
}

