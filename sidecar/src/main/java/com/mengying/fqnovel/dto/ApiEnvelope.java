package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonInclude(JsonInclude.Include.NON_NULL)
public record ApiEnvelope<T>(
    int code,
    String message,
    T data,
    @JsonProperty("server_time") Long serverTime
) {

    public static <T> ApiEnvelope<T> success(T data) {
        return new ApiEnvelope<>(0, "success", data, System.currentTimeMillis());
    }

    public static <T> ApiEnvelope<T> error(int code, String message) {
        return new ApiEnvelope<>(code, message, null, System.currentTimeMillis());
    }
}

