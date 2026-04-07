package com.mengying.fqnovel.config;

public class SidecarUpstreamProperties {

    private String baseUrl = "https://api5-normal-sinfonlineb.fqnovel.com";
    private long registerKeyCacheTtlMs = 60 * 60 * 1000L;
    private int registerKeyCacheMaxEntries = 128;
    private long connectTimeoutMs = 15000L;
    private long readTimeoutMs = 30000L;

    public String getBaseUrl() {
        return baseUrl;
    }

    public void setBaseUrl(String baseUrl) {
        this.baseUrl = baseUrl;
    }

    public long getRegisterKeyCacheTtlMs() {
        return registerKeyCacheTtlMs;
    }

    public void setRegisterKeyCacheTtlMs(long registerKeyCacheTtlMs) {
        this.registerKeyCacheTtlMs = registerKeyCacheTtlMs;
    }

    public int getRegisterKeyCacheMaxEntries() {
        return registerKeyCacheMaxEntries;
    }

    public void setRegisterKeyCacheMaxEntries(int registerKeyCacheMaxEntries) {
        this.registerKeyCacheMaxEntries = registerKeyCacheMaxEntries;
    }

    public long getConnectTimeoutMs() {
        return connectTimeoutMs;
    }

    public void setConnectTimeoutMs(long connectTimeoutMs) {
        this.connectTimeoutMs = connectTimeoutMs;
    }

    public long getReadTimeoutMs() {
        return readTimeoutMs;
    }

    public void setReadTimeoutMs(long readTimeoutMs) {
        this.readTimeoutMs = readTimeoutMs;
    }

    public static SidecarUpstreamProperties fromEnv() {
        SidecarUpstreamProperties properties = new SidecarUpstreamProperties();
        properties.setBaseUrl(stringEnv("FQ_UPSTREAM_BASE_URL", properties.getBaseUrl()));
        properties.setRegisterKeyCacheTtlMs(longEnv("REGISTER_KEY_CACHE_TTL_MS", properties.getRegisterKeyCacheTtlMs()));
        properties.setRegisterKeyCacheMaxEntries((int) longEnv("REGISTER_KEY_CACHE_MAX_ENTRIES", properties.getRegisterKeyCacheMaxEntries()));
        properties.setConnectTimeoutMs(longEnv("FQ_UPSTREAM_CONNECT_TIMEOUT_MS", properties.getConnectTimeoutMs()));
        properties.setReadTimeoutMs(longEnv("FQ_UPSTREAM_READ_TIMEOUT_MS", properties.getReadTimeoutMs()));
        return properties;
    }

    private static String stringEnv(String key, String defaultValue) {
        String value = System.getenv(key);
        if (value == null || value.trim().isEmpty()) {
            return defaultValue;
        }
        return value.trim();
    }

    private static long longEnv(String key, long defaultValue) {
        try {
            return Long.parseLong(System.getenv().getOrDefault(key, String.valueOf(defaultValue)).trim());
        } catch (Exception ignored) {
            return defaultValue;
        }
    }
}
