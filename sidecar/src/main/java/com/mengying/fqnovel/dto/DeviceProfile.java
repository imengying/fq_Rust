package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record DeviceProfile(
    String name,
    @JsonProperty("user_agent") String userAgent,
    String cookie,
    UpstreamDevice device
) {
}

