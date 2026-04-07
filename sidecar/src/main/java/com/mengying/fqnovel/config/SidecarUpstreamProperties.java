package com.mengying.fqnovel.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "fq.upstream")
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
}

