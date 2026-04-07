package com.mengying.fqnovel.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "internal.api")
public class InternalApiProperties {

    private String token = "change-me-in-production";

    public String getToken() {
        return token;
    }

    public void setToken(String token) {
        this.token = token;
    }
}

