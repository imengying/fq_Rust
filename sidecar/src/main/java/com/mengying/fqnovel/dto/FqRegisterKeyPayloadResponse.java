package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * FQNovel 注册密钥响应载荷。
 */
public record FqRegisterKeyPayloadResponse(
    @JsonProperty("key") String key,
    @JsonProperty("keyver") long keyver
) {
}
