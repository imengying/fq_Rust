package com.mengying.fqnovel;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.invoke.MethodHandle;
import java.nio.charset.Charset;
import java.util.concurrent.atomic.AtomicBoolean;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_BYTE;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

public final class ConsoleNoiseFilter {

    private static final String PROP_FILTER_CONSOLE_NOISE = "fq.log.filterConsoleNoise";
    private static final String DEFAULT_FILTER_CONSOLE_NOISE = "true";
    private static final String EMPTY_NATIVE_ERROR_LINE = "[main]E/:";
    private static final AtomicBoolean NATIVE_STDERR_FILTER_INSTALLED = new AtomicBoolean(false);

    private ConsoleNoiseFilter() {
    }

    public static void install() {
        if (!isEnabled()) {
            return;
        }

        Charset charset = Charset.defaultCharset();
        System.setErr(createFilteringPrintStream(System.err, charset));
        System.setOut(createFilteringPrintStream(System.out, charset));
        installNativeStderrFilter(charset);
    }

    private static boolean isEnabled() {
        String enabled = System.getProperty(PROP_FILTER_CONSOLE_NOISE, DEFAULT_FILTER_CONSOLE_NOISE);
        return !"false".equalsIgnoreCase(enabled);
    }

    private static PrintStream createFilteringPrintStream(OutputStream delegate, Charset charset) {
        return new PrintStream(new LineFilteringOutputStream(delegate, charset), true);
    }

    private static void installNativeStderrFilter(Charset charset) {
        if (!NativeStderrFilter.isSupportedPlatform()) {
            return;
        }
        if (!NATIVE_STDERR_FILTER_INSTALLED.compareAndSet(false, true)) {
            return;
        }
        try {
            NativeStderrFilter.install(charset);
        } catch (Throwable ignored) {
            NATIVE_STDERR_FILTER_INSTALLED.set(false);
        }
    }

    private static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }

    private static boolean shouldDropLine(String line) {
        String trimmed = trimToNull(line);
        if (trimmed == null) {
            return false;
        }

        if (EMPTY_NATIVE_ERROR_LINE.equals(trimmed)) {
            return true;
        }

        String upper = trimmed.toUpperCase();
        return upper.contains("METASEC")
            || trimmed.contains("MSTaskManager::DoLazyInit()")
            || trimmed.contains("SDK not init, crashing");
    }

    static final class LineFilteringOutputStream extends OutputStream {
        private final OutputStream delegate;
        private final Charset charset;
        private final ByteArrayOutputStream buffer = new ByteArrayOutputStream(256);

        LineFilteringOutputStream(OutputStream delegate, Charset charset) {
            this.delegate = delegate;
            this.charset = charset;
        }

        @Override
        public synchronized void write(int b) throws IOException {
            buffer.write(b);
            if (b == '\n') {
                flushBufferAsLine();
            }
        }

        @Override
        public synchronized void write(byte[] b, int off, int len) throws IOException {
            for (int i = 0; i < len; i++) {
                write(b[off + i]);
            }
        }

        @Override
        public synchronized void flush() throws IOException {
            if (buffer.size() > 0) {
                flushBufferAsLine();
            }
            delegate.flush();
        }

        @Override
        public synchronized void close() throws IOException {
            flush();
            delegate.close();
        }

        private void flushBufferAsLine() throws IOException {
            byte[] lineBytes = buffer.toByteArray();
            buffer.reset();

            String line = new String(lineBytes, charset);
            if (shouldDropLine(line)) {
                return;
            }
            delegate.write(lineBytes);
        }
    }

    static final class NativeStderrFilter implements Runnable {
        private static final int STDERR_FD = 2;
        private static final int PIPE_READ_INDEX = 0;
        private static final int PIPE_WRITE_INDEX = 1;
        private static final int READ_BUFFER_SIZE = 1024;

        private static final Linker LINKER = Linker.nativeLinker();
        private static final SymbolLookup LOOKUP = LINKER.defaultLookup();
        private static final MethodHandle PIPE = downcall("pipe", FunctionDescriptor.of(JAVA_INT, ADDRESS));
        private static final MethodHandle DUP = downcall("dup", FunctionDescriptor.of(JAVA_INT, JAVA_INT));
        private static final MethodHandle DUP2 = downcall("dup2", FunctionDescriptor.of(JAVA_INT, JAVA_INT, JAVA_INT));
        private static final MethodHandle READ = downcall("read", FunctionDescriptor.of(JAVA_LONG, JAVA_INT, ADDRESS, JAVA_LONG));
        private static final MethodHandle WRITE = downcall("write", FunctionDescriptor.of(JAVA_LONG, JAVA_INT, ADDRESS, JAVA_LONG));
        private static final MethodHandle CLOSE = downcall("close", FunctionDescriptor.of(JAVA_INT, JAVA_INT));

        private final int readFd;
        private final int originalErrFd;
        private final Charset charset;

        private NativeStderrFilter(int readFd, int originalErrFd, Charset charset) {
            this.readFd = readFd;
            this.originalErrFd = originalErrFd;
            this.charset = charset;
        }

        static boolean isSupportedPlatform() {
            String osName = System.getProperty("os.name", "");
            return !osName.toLowerCase().contains("win");
        }

        static void install(Charset charset) throws IOException {
            int readFd;
            int writeFd;
            int originalErrFd = -1;

            try (Arena arena = Arena.ofConfined()) {
                MemorySegment pipeFds = arena.allocate(JAVA_INT.byteSize() * 2L, JAVA_INT.byteAlignment());
                if (invokeInt(PIPE, pipeFds) != 0) {
                    throw new IOException("创建 stderr 管道失败");
                }
                readFd = pipeFds.get(JAVA_INT, PIPE_READ_INDEX * JAVA_INT.byteSize());
                writeFd = pipeFds.get(JAVA_INT, PIPE_WRITE_INDEX * JAVA_INT.byteSize());
            }

            try {
                originalErrFd = invokeInt(DUP, STDERR_FD);
                if (originalErrFd < 0) {
                    throw new IOException("复制 stderr 失败");
                }
                if (invokeInt(DUP2, writeFd, STDERR_FD) < 0) {
                    throw new IOException("重定向 stderr 失败");
                }
            } catch (IOException e) {
                closeQuietly(readFd);
                closeQuietly(writeFd);
                closeQuietly(originalErrFd);
                throw e;
            }

            closeQuietly(writeFd);

            try {
                Thread thread = new Thread(
                    new NativeStderrFilter(readFd, originalErrFd, charset),
                    "fq-native-stderr-filter"
                );
                thread.setDaemon(true);
                thread.start();
            } catch (Throwable t) {
                try {
                    invokeInt(DUP2, originalErrFd, STDERR_FD);
                } catch (IOException ignored) {
                    // ignore
                }
                closeQuietly(readFd);
                closeQuietly(originalErrFd);
                throw new IOException("启动 native stderr 过滤线程失败", t);
            }
        }

        @Override
        public void run() {
            ByteArrayOutputStream lineBuffer = new ByteArrayOutputStream(256);
            try (Arena readArena = Arena.ofConfined()) {
                MemorySegment nativeBuffer = readArena.allocate(READ_BUFFER_SIZE);
                while (true) {
                    long bytesRead = invokeLong(READ, readFd, nativeBuffer, (long) READ_BUFFER_SIZE);
                    if (bytesRead <= 0) {
                        break;
                    }
                    for (int i = 0; i < bytesRead; i++) {
                        byte value = nativeBuffer.get(JAVA_BYTE, i);
                        lineBuffer.write(value);
                        if (value == '\n') {
                            flushLine(lineBuffer);
                        }
                    }
                }
                if (lineBuffer.size() > 0) {
                    flushLine(lineBuffer);
                }
            } catch (Throwable ignored) {
                // 原生 stderr 过滤失败时静默降级，避免再次递归污染控制台。
            } finally {
                closeQuietly(readFd);
                closeQuietly(originalErrFd);
            }
        }

        private void flushLine(ByteArrayOutputStream lineBuffer) throws IOException {
            byte[] lineBytes = lineBuffer.toByteArray();
            lineBuffer.reset();
            if (shouldDropNativeLine(lineBytes)) {
                return;
            }
            writeFully(originalErrFd, lineBytes);
        }

        private boolean shouldDropNativeLine(byte[] lineBytes) {
            if (lineBytes == null || lineBytes.length == 0) {
                return false;
            }
            String line = new String(lineBytes, charset);
            String trimmed = trimToNull(line);
            return EMPTY_NATIVE_ERROR_LINE.equals(trimmed);
        }

        private static void writeFully(int fd, byte[] data) throws IOException {
            if (data == null || data.length == 0) {
                return;
            }
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment segment = arena.allocate(data.length);
                for (int i = 0; i < data.length; i++) {
                    segment.set(JAVA_BYTE, i, data[i]);
                }

                long offset = 0L;
                while (offset < data.length) {
                    long written = invokeLong(WRITE, fd, segment.asSlice(offset), (long) data.length - offset);
                    if (written <= 0) {
                        break;
                    }
                    offset += written;
                }
            }
        }

        private static MethodHandle downcall(String symbol, FunctionDescriptor descriptor) {
            return LINKER.downcallHandle(LOOKUP.findOrThrow(symbol), descriptor);
        }

        private static int invokeInt(MethodHandle handle, Object... args) throws IOException {
            try {
                return ((Number) handle.invokeWithArguments(args)).intValue();
            } catch (Throwable t) {
                throw new IOException("调用 native int 函数失败", t);
            }
        }

        private static long invokeLong(MethodHandle handle, Object... args) throws IOException {
            try {
                return ((Number) handle.invokeWithArguments(args)).longValue();
            } catch (Throwable t) {
                throw new IOException("调用 native long 函数失败", t);
            }
        }

        private static void closeQuietly(int fd) {
            try {
                invokeInt(CLOSE, fd);
            } catch (IOException ignored) {
                // ignore
            }
        }
    }
}
