package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonInclude;

@JsonInclude(JsonInclude.Include.NON_NULL)
public record WorkerResponse<T>(
    String id,
    int code,
    String message,
    T data,
    long serverTime
) {

    public static <T> WorkerResponse<T> success(String id, T data) {
        return new WorkerResponse<>(id, 0, "success", data, System.currentTimeMillis());
    }

    public static <T> WorkerResponse<T> error(String id, int code, String message) {
        return new WorkerResponse<>(id, code, message, null, System.currentTimeMillis());
    }
}

