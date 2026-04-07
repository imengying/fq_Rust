package com.mengying.fqnovel.dto;

import java.util.Map;

public record SignRequest(
    String url,
    Map<String, String> headers
) {
}

