package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record SignRequest(
    String url,
    @JsonProperty("headers_text") String headersText
) {
}
