package com.mengying.fqnovel.dto;

import com.fasterxml.jackson.annotation.JsonProperty;

public record UpstreamDevice(
    String aid,
    String cdid,
    @JsonProperty("device_id") String deviceId,
    @JsonProperty("device_type") String deviceType,
    @JsonProperty("device_brand") String deviceBrand,
    @JsonProperty("install_id") String installId,
    String resolution,
    String dpi,
    @JsonProperty("rom_version") String romVersion,
    @JsonProperty("host_abi") String hostAbi,
    @JsonProperty("update_version_code") String updateVersionCode,
    @JsonProperty("version_code") String versionCode,
    @JsonProperty("version_name") String versionName,
    @JsonProperty("os_version") String osVersion,
    @JsonProperty("os_api") String osApi
) {
}

