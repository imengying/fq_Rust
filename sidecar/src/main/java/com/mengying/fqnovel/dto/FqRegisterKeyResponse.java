package com.mengying.fqnovel.dto;

/**
 * FQNovel 注册密钥响应。
 */
public record FqRegisterKeyResponse(
    long code,
    String message,
    FqRegisterKeyPayloadResponse data
) {
}
