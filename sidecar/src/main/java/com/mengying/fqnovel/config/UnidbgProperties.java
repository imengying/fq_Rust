package com.mengying.fqnovel.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

/**
 * unidbg配置类
 *
 * @author AnJia
 * @since 2021-07-26 19:13
 */
@ConfigurationProperties(prefix = "application.unidbg")
public class UnidbgProperties {
    /**
     * 是否打印调用信息
     */
    private boolean verbose;

    /**
     * signer（unidbg）全局重置最小间隔（ms）。
     * <p>
     * 用于抑制上游抖动导致的“频繁 reset -> 更慢 -> 更容易空响应”的风暴。
     * 设为 0 可禁用节流。
     */
    private long resetCooldownMs = 2000L;

    /**
     * 空响应（UPSTREAM_EMPTY）触发 signer 重置时的专属最小间隔（ms）。
     * <p>
     * 该值通常应大于等于 resetCooldownMs，用于进一步抑制空响应抖动导致的短时间连续 reset。
     * 设为 0 可禁用该专属节流。
     */
    private long upstreamEmptyResetCooldownMs = 8000L;

    /**
     * 番茄小说 APK 文件路径（建议使用 base.apk 的绝对路径）
     * 优先级高于 apkClasspath；适合本地或容器运行时挂载文件。
     */
    private String apkPath;

    /**
     * 番茄小说 APK 的 classpath 资源路径（例如：com/dragon/read/oversea/gp/apk/base.apk）
     * 当 apkPath 未配置时使用；未配置时默认读取 classpath: com/dragon/read/oversea/gp/apk/base.apk
     */
    private String apkClasspath;

    public boolean isVerbose() {
        return verbose;
    }

    public void setVerbose(boolean verbose) {
        this.verbose = verbose;
    }

    public long getResetCooldownMs() {
        return resetCooldownMs;
    }

    public void setResetCooldownMs(long resetCooldownMs) {
        this.resetCooldownMs = resetCooldownMs;
    }

    public long getUpstreamEmptyResetCooldownMs() {
        return upstreamEmptyResetCooldownMs;
    }

    public void setUpstreamEmptyResetCooldownMs(long upstreamEmptyResetCooldownMs) {
        this.upstreamEmptyResetCooldownMs = upstreamEmptyResetCooldownMs;
    }

    public String getApkPath() {
        return apkPath;
    }

    public void setApkPath(String apkPath) {
        this.apkPath = apkPath;
    }

    public String getApkClasspath() {
        return apkClasspath;
    }

    public void setApkClasspath(String apkClasspath) {
        this.apkClasspath = apkClasspath;
    }
}
