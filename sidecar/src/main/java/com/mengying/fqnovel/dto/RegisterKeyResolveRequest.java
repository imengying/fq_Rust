package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record RegisterKeyResolveRequest(
    @JsonProperty("device_profile") DeviceProfile deviceProfile,
    @JsonProperty("required_keyver") Long requiredKeyver
) {
}

