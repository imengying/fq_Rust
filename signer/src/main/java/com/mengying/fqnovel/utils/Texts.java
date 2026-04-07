package com.mengying.fqnovel.utils;

/**
 * 轻量字符串工具，统一空白判断和回退取值。
 */
public final class Texts {

    private Texts() {
    }

    public static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }
}
