package com.mengying.fqnovel.utils;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.zip.GZIPInputStream;

/**
 * GZIP 压缩/解压缩工具类
 * 统一处理上游响应的 GZIP 解压逻辑
 */
public final class GzipUtils {

    private GzipUtils() {}

    private static boolean hasGzipMagic(byte[] data) {
        return data != null
            && data.length >= 2
            && data[0] == (byte) 0x1f
            && data[1] == (byte) 0x8b;
    }

    private static String utf8(byte[] data) {
        return data == null ? "" : new String(data, StandardCharsets.UTF_8);
    }

    private static String decodeRawBody(byte[] data) {
        return utf8(data);
    }

    private static String ungzip(byte[] gzipData) throws Exception {
        try (GZIPInputStream gzipInputStream = new GZIPInputStream(new ByteArrayInputStream(gzipData))) {
            ByteArrayOutputStream byteArrayOutputStream = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int length;
            while ((length = gzipInputStream.read(buffer)) != -1) {
                byteArrayOutputStream.write(buffer, 0, length);
            }
            return utf8(byteArrayOutputStream.toByteArray());
        }
    }

    /**
     * 解压缩 GZIP 响应体（基础方法）
     *
     * @param gzipData 可能是 GZIP 压缩的字节数组
     * @return 解压后的字符串
     * @throws Exception 解压失败时抛出异常
     */
    public static String decompressGzipResponse(byte[] gzipData) throws Exception {
        if (gzipData == null || gzipData.length == 0) {
            return "";
        }

        // Some upstream responses are not gzip (or already decompressed).
        boolean looksLikeGzip = hasGzipMagic(gzipData);
        if (!looksLikeGzip) {
            return decodeRawBody(gzipData);
        }

        try {
            return ungzip(gzipData);
        } catch (java.util.zip.ZipException e) {
            // Fallback: not gzip (or already decompressed).
            return decodeRawBody(gzipData);
        }
    }

}
