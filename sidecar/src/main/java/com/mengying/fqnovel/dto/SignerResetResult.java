package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record SignerResetResult(
    boolean accepted,
    @JsonProperty("signer_epoch") long signerEpoch,
    @JsonProperty("cooldown_applied") boolean cooldownApplied
) {
}

