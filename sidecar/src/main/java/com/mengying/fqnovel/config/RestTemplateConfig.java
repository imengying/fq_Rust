package com.mengying.fqnovel.config;

import org.springframework.boot.restclient.RestTemplateBuilder;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.client.RestTemplate;

import java.time.Duration;

@Configuration
public class RestTemplateConfig {

    @Bean
    public RestTemplate restTemplate(RestTemplateBuilder builder, SidecarUpstreamProperties properties) {
        return builder
            .setConnectTimeout(Duration.ofMillis(Math.max(1000L, properties.getConnectTimeoutMs())))
            .setReadTimeout(Duration.ofMillis(Math.max(1000L, properties.getReadTimeoutMs())))
            .build();
    }
}
