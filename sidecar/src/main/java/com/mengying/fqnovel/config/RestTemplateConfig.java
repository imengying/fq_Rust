package com.mengying.fqnovel.config;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.client.SimpleClientHttpRequestFactory;
import org.springframework.web.client.RestTemplate;

@Configuration
public class RestTemplateConfig {

    @Bean
    public RestTemplate restTemplate(SidecarUpstreamProperties properties) {
        SimpleClientHttpRequestFactory requestFactory = new SimpleClientHttpRequestFactory();
        requestFactory.setConnectTimeout((int) Math.max(1000L, properties.getConnectTimeoutMs()));
        requestFactory.setReadTimeout((int) Math.max(1000L, properties.getReadTimeoutMs()));
        return new RestTemplate(requestFactory);
    }
}
