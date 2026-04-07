package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.Map;

public record SignResult(
    Map<String, String> headers,
    @JsonProperty("signer_epoch") long signerEpoch
) {
}

