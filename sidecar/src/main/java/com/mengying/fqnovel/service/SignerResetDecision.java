package com.mengying.fqnovel.service;

public record SignerResetDecision(
    long signerEpoch,
    boolean cooldownApplied
) {
}

