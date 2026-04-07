package com.mengying.fqnovel.utils;

import java.util.Locale;

public final class CookieUtils {

    private CookieUtils() {
    }

    /**
     * 确保 cookie 中的 install_id 与 installId 一致（不存在则追加）。
     */
    public static String normalizeInstallId(String cookie, String installId) {
        if (cookie == null) {
            return null;
        }
        if (!Texts.hasText(installId)) {
            return cookie;
        }

        String iid = Texts.trimToEmpty(installId);
        String lower = cookie.toLowerCase(Locale.ROOT);
        String key = "install_id=";
        int idx = lower.indexOf(key);
        if (idx < 0) {
            String trimmed = Texts.trimToEmpty(cookie);
            if (trimmed.isEmpty()) {
                return key + iid;
            }
            if (trimmed.endsWith(";")) {
                return trimmed + " " + key + iid + ";";
            }
            return trimmed + "; " + key + iid + ";";
        }

        int valueStart = idx + key.length();
        int valueEnd = cookie.indexOf(';', valueStart);
        if (valueEnd < 0) {
            valueEnd = cookie.length();
        }
        return cookie.substring(0, valueStart) + iid + cookie.substring(valueEnd);
    }
}
