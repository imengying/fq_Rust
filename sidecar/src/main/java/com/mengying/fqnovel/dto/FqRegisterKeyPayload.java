package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * FQNovel 注册密钥请求载荷。
 */
public record FqRegisterKeyPayload(
    @JsonProperty("content") String content,
    @JsonProperty("keyver") long keyver
) {
}
