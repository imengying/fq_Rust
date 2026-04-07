package com.mengying.fqnovel.utils;

import org.springframework.http.ResponseEntity;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Locale;
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

    private static boolean hasGzipContentEncoding(List<String> encodings) {
        if (encodings == null) {
            return false;
        }
        for (String encoding : encodings) {
            if (encoding != null && encoding.toLowerCase(Locale.ROOT).contains("gzip")) {
                return true;
            }
        }
        return false;
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

    /**
     * 统一解码上游响应（自动处理 GZIP 压缩）
     * 根据 Content-Encoding 头部和魔数自动判断是否需要解压
     *
     * @param response HTTP 响应
     * @return 解码后的字符串
     */
    public static String decodeUpstreamResponse(ResponseEntity<byte[]> response) {
        if (response == null) {
            return "";
        }
        byte[] body = response.getBody();
        if (body == null || body.length == 0) {
            return "";
        }

        List<String> enc = response.getHeaders().get("Content-Encoding");
        boolean isGzip = hasGzipContentEncoding(enc) || hasGzipMagic(body);

        if (!isGzip) {
            return decodeRawBody(body);
        }

        try {
            return ungzip(body);
        } catch (java.util.zip.ZipException e) {
            // 上游偶尔会返回非 gzip 内容但误标为 gzip，兜底为原始文本
            return decodeRawBody(body);
        } catch (Exception e) {
            return decodeRawBody(body);
        }
    }
}
