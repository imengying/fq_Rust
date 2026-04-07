package com.mengying.fqnovel.service;

import com.mengying.fqnovel.config.SidecarUpstreamProperties;
import com.mengying.fqnovel.dto.DeviceProfile;
import com.mengying.fqnovel.dto.RegisterKeyResolveResult;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public class RegisterKeyService {

    private final RegisterKeyUpstreamClient upstreamClient;
    private final SidecarUpstreamProperties properties;
    private final ConcurrentHashMap<String, CacheEntry> cacheByFingerprintAndKeyver = new ConcurrentHashMap<>();
    private final ConcurrentHashMap<String, CacheEntry> currentByFingerprint = new ConcurrentHashMap<>();

    public RegisterKeyService(RegisterKeyUpstreamClient upstreamClient, SidecarUpstreamProperties properties) {
        this.upstreamClient = upstreamClient;
        this.properties = properties;
    }

    public synchronized RegisterKeyResolveResult resolve(DeviceProfile profile, Long requiredKeyver) throws Exception {
        if (profile == null || profile.device() == null) {
            throw new IllegalArgumentException("device_profile 不能为空");
        }

        String fingerprint = fingerprint(profile);
        Long normalizedKeyver = normalizeKeyver(requiredKeyver);

        if (normalizedKeyver != null) {
            CacheEntry cached = cacheByFingerprintAndKeyver.get(cacheKey(fingerprint, normalizedKeyver));
            if (isValid(cached)) {
                return cached.result().withSource("cache");
            }
        } else {
            CacheEntry current = currentByFingerprint.get(fingerprint);
            if (isValid(current)) {
                return current.result().withSource("cache");
            }
        }

        RegisterKeyUpstreamClient.FetchedRegisterKey fetched = upstreamClient.fetch(profile);
        if (normalizedKeyver != null && fetched.keyver() != normalizedKeyver) {
            throw new RegisterKeyVersionMismatchException("registerkey version mismatch");
        }

        long expiresAtMs = computeExpiresAtMs();
        RegisterKeyResolveResult result = new RegisterKeyResolveResult(
            fingerprint,
            fetched.keyver(),
            fetched.realKeyHex(),
            expiresAtMs,
            "refresh"
        );
        CacheEntry entry = new CacheEntry(result, expiresAtMs);
        cacheByFingerprintAndKeyver.put(cacheKey(fingerprint, fetched.keyver()), entry);
        currentByFingerprint.put(fingerprint, entry);
        trimIfNeeded();
        return result;
    }

    public synchronized boolean invalidate(String deviceFingerprint) {
        if (deviceFingerprint == null || deviceFingerprint.isBlank()) {
            throw new IllegalArgumentException("device_fingerprint 不能为空");
        }
        boolean removed = currentByFingerprint.remove(deviceFingerprint) != null;
        for (String key : new ArrayList<>(cacheByFingerprintAndKeyver.keySet())) {
            if (key.startsWith(deviceFingerprint + ":")) {
                cacheByFingerprintAndKeyver.remove(key);
                removed = true;
            }
        }
        return removed;
    }

    private void trimIfNeeded() {
        int maxEntries = Math.max(1, properties.getRegisterKeyCacheMaxEntries());
        if (cacheByFingerprintAndKeyver.size() <= maxEntries) {
            return;
        }
        int overflow = cacheByFingerprintAndKeyver.size() - maxEntries;
        int removed = 0;
        for (Map.Entry<String, CacheEntry> entry : cacheByFingerprintAndKeyver.entrySet()) {
            CacheEntry removedEntry = cacheByFingerprintAndKeyver.remove(entry.getKey());
            if (removedEntry != null) {
                currentByFingerprint.entrySet().removeIf(current -> current.getValue().equals(removedEntry));
            }
            removed++;
            if (removed >= overflow) {
                break;
            }
        }
    }

    private boolean isValid(CacheEntry entry) {
        return entry != null && entry.expiresAtMs() >= System.currentTimeMillis();
    }

    private long computeExpiresAtMs() {
        long ttlMs = Math.max(0L, properties.getRegisterKeyCacheTtlMs());
        return ttlMs == 0L ? Long.MAX_VALUE : System.currentTimeMillis() + ttlMs;
    }

    private Long normalizeKeyver(Long value) {
        if (value == null || value <= 0L) {
            return null;
        }
        return value;
    }

    private String cacheKey(String fingerprint, long keyver) {
        return fingerprint + ":" + keyver;
    }

    private String fingerprint(DeviceProfile profile) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        String raw = String.join("|",
            safe(profile.name()),
            safe(profile.userAgent()),
            safe(profile.cookie()),
            safe(profile.device().aid()),
            safe(profile.device().cdid()),
            safe(profile.device().deviceId()),
            safe(profile.device().deviceType()),
            safe(profile.device().deviceBrand()),
            safe(profile.device().installId()),
            safe(profile.device().versionCode()),
            safe(profile.device().versionName()),
            safe(profile.device().updateVersionCode()),
            safe(profile.device().resolution()),
            safe(profile.device().dpi()),
            safe(profile.device().romVersion()),
            safe(profile.device().hostAbi()),
            safe(profile.device().osVersion()),
            safe(profile.device().osApi())
        );
        byte[] hashed = digest.digest(raw.getBytes(StandardCharsets.UTF_8));
        StringBuilder builder = new StringBuilder();
        for (byte value : hashed) {
            builder.append(String.format("%02x", value));
        }
        return builder.toString();
    }

    private String safe(String value) {
        return value == null ? "" : value.trim();
    }

    private record CacheEntry(RegisterKeyResolveResult result, long expiresAtMs) {
    }
}
