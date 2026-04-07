package com.mengying.fqnovel;

import org.springframework.boot.Banner;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.context.properties.ConfigurationPropertiesScan;

@SpringBootApplication
@ConfigurationPropertiesScan
public class SidecarApplication {

    public static void main(String[] args) {
        SpringApplication app = new SpringApplication(SidecarApplication.class);
        app.setBannerMode(Banner.Mode.OFF);
        app.run(args);
    }
}

