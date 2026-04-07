package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.databind.JsonNode;

public record WorkerRequest(
    String id,
    String method,
    JsonNode params
) {
}

