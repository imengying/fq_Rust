package com.mengying.fqnovel.sidecar;

import com.mengying.fqnovel.dto.ApiEnvelope;
import com.mengying.fqnovel.dto.SignRequest;
import com.mengying.fqnovel.dto.SignResult;
import com.mengying.fqnovel.dto.SignerResetRequest;
import com.mengying.fqnovel.dto.SignerResetResult;
import com.mengying.fqnovel.service.SignerResetDecision;
import com.mengying.fqnovel.service.SignerService;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class SignController {

    private final SignerService signerService;

    public SignController(SignerService signerService) {
        this.signerService = signerService;
    }

    @PostMapping("/internal/v1/sign")
    public ApiEnvelope<SignResult> sign(@RequestBody SignRequest request) {
        if (request == null) {
            throw new IllegalArgumentException("请求不能为空");
        }
        return ApiEnvelope.success(signerService.sign(request.url(), request.headers()));
    }

    @PostMapping("/internal/v1/signer/reset")
    public ApiEnvelope<SignerResetResult> reset(@RequestBody SignerResetRequest request) {
        if (request == null || request.reason() == null || request.reason().isBlank()) {
            throw new IllegalArgumentException("reason 不能为空");
        }
        SignerResetDecision decision = signerService.reset(request.reason());
        return ApiEnvelope.success(new SignerResetResult(true, decision.signerEpoch(), decision.cooldownApplied()));
    }
}

