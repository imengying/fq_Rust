package com.mengying.fqnovel.service;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.mengying.fqnovel.config.SidecarUpstreamProperties;
import com.mengying.fqnovel.dto.DeviceProfile;
import com.mengying.fqnovel.dto.FqRegisterKeyPayload;
import com.mengying.fqnovel.dto.FqRegisterKeyResponse;
import com.mengying.fqnovel.dto.SignResult;
import com.mengying.fqnovel.utils.CookieUtils;
import com.mengying.fqnovel.utils.GzipUtils;
import com.mengying.fqnovel.utils.Texts;

import java.net.URI;
import java.net.URLDecoder;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

public class RegisterKeyUpstreamClient {

    private static final String REGISTER_KEY_PATH = "/reading/crypt/registerkey";
    private static final Set<String> ENCODE_KEYS = Set.of("device_type", "resolution", "rom_version");

    private final SidecarUpstreamProperties properties;
    private final SignerService signerService;
    private final ObjectMapper objectMapper;
    private final HttpClient httpClient;

    public RegisterKeyUpstreamClient(
        SidecarUpstreamProperties properties,
        SignerService signerService,
        ObjectMapper objectMapper
    ) {
        this.properties = properties;
        this.signerService = signerService;
        this.objectMapper = objectMapper;
        this.httpClient = HttpClient.newBuilder()
            .connectTimeout(Duration.ofMillis(Math.max(1000L, properties.getConnectTimeoutMs())))
            .build();
    }

    public FetchedRegisterKey fetch(DeviceProfile profile) throws Exception {
        long currentTime = System.currentTimeMillis();
        String baseUrl = Texts.trimToEmpty(properties.getBaseUrl());
        String fullUrl = buildUrlWithParams(baseUrl + REGISTER_KEY_PATH, buildCommonParams(profile, currentTime));
        Map<String, String> headers = buildRegisterKeyHeaders(profile, currentTime);
        SignResult signed = signerService.sign(fullUrl, headers);

        Map<String, String> httpHeaders = new LinkedHashMap<>();
        headers.forEach(httpHeaders::put);
        signed.headers().forEach(httpHeaders::put);
        httpHeaders.put("content-type", "application/json");

        String serverDeviceId = profile.device() == null ? null : profile.device().deviceId();
        FqCrypto crypto = new FqCrypto(FqCrypto.REG_KEY);
        FqRegisterKeyPayload payload = new FqRegisterKeyPayload(
            crypto.newRegisterKeyContent(serverDeviceId, "0"),
            1L
        );

        String payloadJson = objectMapper.writeValueAsString(payload);
        HttpRequest.Builder requestBuilder = HttpRequest.newBuilder(URI.create(fullUrl))
            .timeout(Duration.ofMillis(Math.max(1000L, properties.getReadTimeoutMs())))
            .POST(HttpRequest.BodyPublishers.ofString(payloadJson));
        httpHeaders.forEach(requestBuilder::header);

        HttpResponse<byte[]> response = httpClient.send(requestBuilder.build(), HttpResponse.BodyHandlers.ofByteArray());
        String responseBody = GzipUtils.decompressGzipResponse(response.body());

        if (!Texts.hasText(responseBody)) {
            throw new IllegalStateException("registerkey upstream 返回空响应");
        }
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw new IllegalStateException("registerkey upstream HTTP状态异常: " + response.statusCode());
        }

        FqRegisterKeyResponse parsed = objectMapper.readValue(responseBody, FqRegisterKeyResponse.class);
        if (parsed == null || parsed.data() == null || !Texts.hasText(parsed.data().key())) {
            throw new IllegalStateException("registerkey upstream 返回无效数据");
        }
        if (parsed.code() != 0L) {
            throw new IllegalStateException("registerkey upstream 失败: " + parsed.message());
        }

        long keyver = parsed.data().keyver();
        String realKeyHex = FqCrypto.getRealKey(parsed.data().key());
        return new FetchedRegisterKey(keyver, realKeyHex);
    }

    private Map<String, String> buildCommonParams(DeviceProfile profile, long currentTime) {
        Map<String, String> params = new LinkedHashMap<>();
        String installId = profile.device().installId();
        params.put("iid", installId);
        params.put("device_id", profile.device().deviceId());
        params.put("ac", "wifi");
        params.put("channel", "googleplay");
        params.put("aid", profile.device().aid());
        params.put("app_name", "novelapp");
        params.put("version_code", profile.device().versionCode());
        params.put("version_name", profile.device().versionName());
        params.put("device_platform", "android");
        params.put("os", "android");
        params.put("ssmix", "a");
        params.put("device_type", profile.device().deviceType());
        params.put("device_brand", profile.device().deviceBrand());
        params.put("language", "zh");
        params.put("os_api", profile.device().osApi());
        params.put("os_version", profile.device().osVersion());
        params.put("manifest_version_code", profile.device().versionCode());
        params.put("resolution", profile.device().resolution());
        params.put("dpi", profile.device().dpi());
        params.put("update_version_code", profile.device().updateVersionCode());
        params.put("_rticket", String.valueOf(currentTime));
        params.put("host_abi", profile.device().hostAbi());
        params.put("dragon_device_type", "phone");
        params.put("pv_player", profile.device().versionCode());
        params.put("compliance_status", "0");
        params.put("need_personal_recommend", "1");
        params.put("player_so_load", "1");
        params.put("is_android_pad_screen", "0");
        params.put("rom_version", profile.device().romVersion());
        params.put("cdid", profile.device().cdid());
        return params;
    }

    private Map<String, String> buildRegisterKeyHeaders(DeviceProfile profile, long currentTime) {
        Map<String, String> headers = new LinkedHashMap<>();
        String installId = profile.device() == null ? null : profile.device().installId();
        headers.put("accept", "application/json; charset=utf-8,application/x-protobuf");
        headers.put("cookie", Objects.requireNonNullElse(CookieUtils.normalizeInstallId(profile.cookie(), installId), ""));
        headers.put("user-agent", Objects.requireNonNullElse(profile.userAgent(), ""));
        headers.put("accept-encoding", "gzip");
        headers.put("x-xs-from-web", "0");
        headers.put("x-vc-bdturing-sdk-version", "3.7.2.cn");
        headers.put("x-reading-request", currentTime + "-" + java.util.concurrent.ThreadLocalRandom.current().nextInt(2_000_000_000));
        headers.put("sdk-version", "2");
        headers.put("x-tt-store-region-src", "did");
        headers.put("x-tt-store-region", "cn-zj");
        headers.put("lc", "101");
        headers.put("x-ss-req-ticket", String.valueOf(currentTime));
        headers.put("passport-sdk-version", "50564");
        headers.put("x-ss-dp", Objects.requireNonNullElse(profile.device().aid(), ""));
        headers.put("content-type", "application/json");
        return headers;
    }

    private String buildUrlWithParams(String baseUrl, Map<String, String> params) {
        if (params == null || params.isEmpty()) {
            return baseUrl;
        }
        StringBuilder builder = new StringBuilder(baseUrl).append("?");
        boolean first = true;
        for (Map.Entry<String, String> entry : params.entrySet()) {
            if (!first) {
                builder.append("&");
            }
            builder.append(entry.getKey()).append("=");
            String value = Objects.requireNonNullElse(entry.getValue(), "");
            builder.append(ENCODE_KEYS.contains(entry.getKey()) ? encodeIfNeeded(value) : value);
            first = false;
        }
        return builder.toString();
    }

    private String encodeIfNeeded(String value) {
        try {
            String decoded = URLDecoder.decode(value, StandardCharsets.UTF_8);
            if (!decoded.equals(value)) {
                return value;
            }
        } catch (IllegalArgumentException ignored) {
            // ignore
        }
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }

    public record FetchedRegisterKey(long keyver, String realKeyHex) {
    }
}
