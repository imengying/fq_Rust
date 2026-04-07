package com.mengying.fqnovel.utils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class TempFileUtils {

    private static final Logger log = LoggerFactory.getLogger(TempFileUtils.class);

    private static final Map<String, File> TEMP_FILES = new ConcurrentHashMap<>();

    private TempFileUtils() {
    }

    /**
     * 获取临时文件。如果临时文件不存在，从classpath复制。
     *
     * @param classpathFile classpath下的资源路径
     * @return 临时文件对象
     */
    public static synchronized File getTempFile(String classpathFile) {
        try {
            String normalizedPath = trimToNull(classpathFile);
            if (normalizedPath == null) {
                return null;
            }

            String cacheKey = md5Key(normalizedPath);
            File cached = TEMP_FILES.get(cacheKey);
            if (cached != null && cached.exists()) {
                return cached;
            }

            InputStream inputStream = Thread.currentThread()
                .getContextClassLoader()
                .getResourceAsStream(normalizedPath);
            if (inputStream == null) {
                log.error("资源文件不存在: {}", normalizedPath);
                return null;
            }

            // 获取文件扩展名
            String extension = fileExtensionOf(normalizedPath);

            // 创建临时文件
            File tempFile = File.createTempFile("unidbg_", extension);
            tempFile.deleteOnExit();

            // 复制资源到临时文件
            try (InputStream is = inputStream;
                 FileOutputStream fos = new FileOutputStream(tempFile)) {
                is.transferTo(fos);
            }

            TEMP_FILES.put(cacheKey, tempFile);
            log.debug("临时文件创建成功: {} -> {}", normalizedPath, tempFile.getAbsolutePath());
            return tempFile;
        } catch (IOException e) {
            log.error("创建临时文件失败: {}", classpathFile, e);
            return null;
        }
    }

    private static String md5Key(String path) {
        try {
            MessageDigest digest = MessageDigest.getInstance("MD5");
            byte[] hashed = digest.digest(path.getBytes(StandardCharsets.UTF_8));
            StringBuilder builder = new StringBuilder();
            for (byte value : hashed) {
                builder.append(String.format("%02x", value));
            }
            return builder.toString();
        } catch (Exception e) {
            throw new IllegalStateException("计算缓存键失败", e);
        }
    }

    private static String fileExtensionOf(String path) {
        int dotIndex = path.lastIndexOf(".");
        if (dotIndex <= 0) {
            return "";
        }
        return path.substring(dotIndex);
    }

    private static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }
}
