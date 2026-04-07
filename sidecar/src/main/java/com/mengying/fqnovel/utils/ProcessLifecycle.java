package com.mengying.fqnovel.utils;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.concurrent.atomic.AtomicBoolean;

public final class ProcessLifecycle {

    private static final Logger log = LoggerFactory.getLogger(ProcessLifecycle.class);

    private static final AtomicBoolean SHUTTING_DOWN = new AtomicBoolean(false);
    private static volatile String shutdownReason = "";

    private ProcessLifecycle() {
    }

    public static boolean isShuttingDown() {
        return SHUTTING_DOWN.get();
    }

    public static String getShutdownReason() {
        return shutdownReason;
    }

    public static void markShuttingDown(String reason) {
        String r = Texts.nullToEmpty(reason);
        if (SHUTTING_DOWN.compareAndSet(false, true)) {
            shutdownReason = r;
            log.warn("进程进入退出中状态: reason={}", r);
        }
    }
}
