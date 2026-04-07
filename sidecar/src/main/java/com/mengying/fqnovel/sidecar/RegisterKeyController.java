package com.mengying.fqnovel.sidecar;

import com.mengying.fqnovel.dto.ApiEnvelope;
import com.mengying.fqnovel.dto.RegisterKeyInvalidateRequest;
import com.mengying.fqnovel.dto.RegisterKeyInvalidateResult;
import com.mengying.fqnovel.dto.RegisterKeyResolveRequest;
import com.mengying.fqnovel.dto.RegisterKeyResolveResult;
import com.mengying.fqnovel.service.RegisterKeyService;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class RegisterKeyController {

    private final RegisterKeyService registerKeyService;

    public RegisterKeyController(RegisterKeyService registerKeyService) {
        this.registerKeyService = registerKeyService;
    }

    @PostMapping("/internal/v1/register-key/resolve")
    public ApiEnvelope<RegisterKeyResolveResult> resolve(@RequestBody RegisterKeyResolveRequest request) throws Exception {
        if (request == null) {
            throw new IllegalArgumentException("请求不能为空");
        }
        return ApiEnvelope.success(
            registerKeyService.resolve(request.deviceProfile(), request.requiredKeyver())
        );
    }

    @PostMapping("/internal/v1/register-key/invalidate")
    public ApiEnvelope<RegisterKeyInvalidateResult> invalidate(@RequestBody RegisterKeyInvalidateRequest request) {
        if (request == null || request.deviceFingerprint() == null || request.deviceFingerprint().isBlank()) {
            throw new IllegalArgumentException("device_fingerprint 不能为空");
        }
        boolean invalidated = registerKeyService.invalidate(request.deviceFingerprint());
        return ApiEnvelope.success(new RegisterKeyInvalidateResult(request.deviceFingerprint(), invalidated));
    }
}
