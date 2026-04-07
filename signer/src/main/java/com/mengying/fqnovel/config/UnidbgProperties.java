package com.mengying.fqnovel.config;

/**
 * unidbg配置类
 *
 * @author AnJia
 * @since 2021-07-26 19:13
 */
public class UnidbgProperties {
    /**
     * 是否打印调用信息
     */
    private boolean verbose;

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

    public static UnidbgProperties fromEnv() {
        UnidbgProperties properties = new UnidbgProperties();
        properties.setVerbose(Boolean.parseBoolean(System.getenv().getOrDefault("UNIDBG_VERBOSE", "false")));
        properties.setApkPath(trimToNull(System.getenv("UNIDBG_APK_PATH")));
        properties.setApkClasspath(trimToNull(System.getenv("UNIDBG_APK_CLASSPATH")));
        if (properties.getApkClasspath() == null) {
            properties.setApkClasspath("com/dragon/read/oversea/gp/apk/base.apk");
        }
        return properties;
    }

    private static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }
}
