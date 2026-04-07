package com.mengying.fqnovel.service;

import com.mengying.fqnovel.dto.SignResult;
import org.springframework.stereotype.Service;

import java.util.LinkedHashMap;
import java.util.Map;

@Service
public class SignerService {

    private final FQEncryptServiceWorker worker;

    public SignerService(FQEncryptServiceWorker worker) {
        this.worker = worker;
    }

    public SignResult sign(String url, Map<String, String> headers) {
        if (url == null || url.isBlank()) {
            throw new IllegalArgumentException("url 不能为空");
        }
        if (headers == null) {
            throw new IllegalArgumentException("headers 不能为空");
        }
        Map<String, String> signed = worker.generateSignatureHeadersSync(url, headers);
        if (signed == null || signed.isEmpty()) {
            throw new SignerUnavailableException("signer unavailable");
        }
        return new SignResult(new LinkedHashMap<>(signed), FQEncryptServiceWorker.currentEpoch());
    }

    public SignerResetDecision reset(String reason) {
        if (reason == null || reason.isBlank()) {
            throw new IllegalArgumentException("reason 不能为空");
        }
        return FQEncryptServiceWorker.requestGlobalReset(reason);
    }
}

