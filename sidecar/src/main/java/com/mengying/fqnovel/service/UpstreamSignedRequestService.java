package com.mengying.fqnovel.service;

/**
 * 仅保留 signer reset 判定所需的错误原因常量。
 */
public final class UpstreamSignedRequestService {

    public static final String REASON_UPSTREAM_EMPTY = "UPSTREAM_EMPTY";
    public static final String REASON_CHAPTER_EMPTY_OR_SHORT = "CHAPTER_EMPTY_OR_SHORT";

    private UpstreamSignedRequestService() {
    }
}
