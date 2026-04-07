package com.mengying.fqnovel.unidbg;

import com.github.unidbg.AndroidEmulator;
import com.github.unidbg.Emulator;
import com.github.unidbg.Module;
import com.github.unidbg.arm.backend.Unicorn2Factory;
import com.github.unidbg.file.FileResult;
import com.github.unidbg.file.IOResolver;
import com.github.unidbg.file.linux.AndroidFileIO;
import com.github.unidbg.linux.android.AndroidEmulatorBuilder;
import com.github.unidbg.linux.android.AndroidResolver;
import com.github.unidbg.linux.android.dvm.AbstractJni;
import com.github.unidbg.linux.android.dvm.BaseVM;
import com.github.unidbg.linux.android.dvm.DalvikModule;
import com.github.unidbg.linux.android.dvm.DvmClass;
import com.github.unidbg.linux.android.dvm.DvmObject;
import com.github.unidbg.linux.android.dvm.StringObject;
import com.github.unidbg.linux.android.dvm.VM;
import com.github.unidbg.linux.android.dvm.VaList;
import com.github.unidbg.linux.android.dvm.VarArg;
import com.github.unidbg.linux.android.dvm.array.ArrayObject;
import com.github.unidbg.linux.android.dvm.array.ByteArray;
import com.github.unidbg.linux.android.dvm.wrapper.DvmBoolean;
import com.github.unidbg.linux.file.SimpleFileIO;
import com.github.unidbg.memory.Memory;
import com.github.unidbg.pointer.UnidbgPointer;
import com.github.unidbg.spi.SyscallHandler;
import com.github.unidbg.virtualmodule.android.AndroidModule;
import com.github.unidbg.virtualmodule.android.JniGraphics;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.locks.ReentrantLock;

@SuppressWarnings("unchecked")
public final class IdleFQ extends AbstractJni implements IOResolver<AndroidFileIO> {

    private static final Logger log = LoggerFactory.getLogger(IdleFQ.class);

    private static final String BASE_PATH = "com/dragon/read/oversea/gp";
    private static final String DEFAULT_APK_RESOURCE_PATH = BASE_PATH + "/apk/base.apk";
    private static final String SO_METASEC_ML_PATH = BASE_PATH + "/lib/libmetasec_ml.so";
    private static final String SO_C_SHARE_PATH = BASE_PATH + "/lib/libc++_shared.so";
    private static final String MS_CERT_FILE_PATH = BASE_PATH + "/other/ms_16777218.bin";
    private static final String SO_METASEC_ML_NAME = "libmetasec_ml.so";
    private static final String SO_C_SHARE_NAME = "libc++_shared.so";

    private static final String PACKAGE_NAME = "com.dragon.read.oversea.gp";
    private static final String DATA_USER_DIR = "/data/user/0/" + PACKAGE_NAME;
    private static final String DATA_FILES_DIR = DATA_USER_DIR + "/files";
    private static final String MSDATA_VFS_PATH = DATA_FILES_DIR + "/.msdata";
    private static final String APK_INSTALL_PATH = "/data/app/com.dragon.read.oversea.gp-q5NyjSN9BLSTVBJ54kg7YA==/base.apk";

    private static final int SDK_VERSION = 23;
    private static final int APP_UID = 10074;
    private static final int APP_VERSION_CODE = 68132;
    private static final long SIGN_FUNCTION_OFFSET = 0x168c80L;

    private static final int MS_METHOD_DATA_PATH = 65539;
    private static final int MS_METHOD_BOOL_1 = 33554433;
    private static final int MS_METHOD_BOOL_2 = 33554434;
    private static final int MS_METHOD_VERSION_CODE = 16777232;
    private static final int MS_METHOD_VERSION_NAME = 16777233;
    private static final int MS_METHOD_CERT = 16777218;
    private static final int MS_METHOD_NOW_MS = 268435470;

    private static final String MS_DISPATCH_SIGNATURE =
        "com/bytedance/mobsec/metasec/ml/MS->b(IIJLjava/lang/String;Ljava/lang/Object;)Ljava/lang/Object;";
    private static final String CURRENT_THREAD_SIGNATURE = "java/lang/Thread->currentThread()Ljava/lang/Thread;";
    private static final String CURRENT_APPLICATION_SIGNATURE =
        "android/app/ActivityThread->currentApplication()Landroid/app/Application;";
    private static final String THREAD_STACK_TRACE_SIGNATURE =
        "java/lang/Thread->getStackTrace()[Ljava/lang/StackTraceElement;";
    private static final String STACK_TRACE_CLASS_NAME_SIGNATURE =
        "java/lang/StackTraceElement->getClassName()Ljava/lang/String;";
    private static final String STACK_TRACE_METHOD_NAME_SIGNATURE =
        "java/lang/StackTraceElement->getMethodName()Ljava/lang/String;";
    private static final String THREAD_GET_BYTES_SIGNATURE = "java/lang/Thread->getBytes(Ljava/lang/String;)[B";
    private static final String CONTEXT_GET_PACKAGE_NAME_SIGNATURE =
        "android/content/Context->getPackageName()Ljava/lang/String;";
    private static final String APPLICATION_GET_PACKAGE_NAME_SIGNATURE =
        "android/app/Application->getPackageName()Ljava/lang/String;";
    private static final String LEGACY_APPLICATION_GET_PACKAGE_NAME_SIGNATURE =
        "Landroid/app/Application;->getPackageName()Ljava/lang/String;";
    private static final String CONTEXT_GET_APPLICATION_CONTEXT_SIGNATURE =
        "android/content/Context->getApplicationContext()Landroid/content/Context;";
    private static final String CONTEXT_GET_FILES_DIR_SIGNATURE =
        "android/content/Context->getFilesDir()Ljava/io/File;";
    private static final String CONTEXT_GET_PACKAGE_MANAGER_SIGNATURE =
        "android/content/Context->getPackageManager()Landroid/content/pm/PackageManager;";
    private static final String APPLICATION_GET_PACKAGE_MANAGER_SIGNATURE =
        "android/app/Application->getPackageManager()Landroid/content/pm/PackageManager;";
    private static final String LEGACY_APPLICATION_GET_PACKAGE_MANAGER_SIGNATURE =
        "Landroid/app/Application;->getPackageManager()Landroid/content/pm/PackageManager;";
    private static final String CONTEXT_GET_APPLICATION_INFO_SIGNATURE =
        "android/content/Context->getApplicationInfo()Landroid/content/pm/ApplicationInfo;";
    private static final String APPLICATION_GET_APPLICATION_INFO_SIGNATURE =
        "android/app/Application->getApplicationInfo()Landroid/content/pm/ApplicationInfo;";
    private static final String LEGACY_APPLICATION_GET_APPLICATION_INFO_SIGNATURE =
        "Landroid/app/Application;->getApplicationInfo()Landroid/content/pm/ApplicationInfo;";
    private static final String CONTEXT_GET_PACKAGE_CODE_PATH_SIGNATURE =
        "android/content/Context->getPackageCodePath()Ljava/lang/String;";
    private static final String CONTEXT_GET_PACKAGE_RESOURCE_PATH_SIGNATURE =
        "android/content/Context->getPackageResourcePath()Ljava/lang/String;";
    private static final String APPLICATION_GET_PACKAGE_CODE_PATH_SIGNATURE =
        "android/app/Application->getPackageCodePath()Ljava/lang/String;";
    private static final String APPLICATION_GET_PACKAGE_RESOURCE_PATH_SIGNATURE =
        "android/app/Application->getPackageResourcePath()Ljava/lang/String;";
    private static final String FILE_GET_ABSOLUTE_PATH_SIGNATURE = "java/io/File->getAbsolutePath()Ljava/lang/String;";
    private static final String FILE_GET_PATH_SIGNATURE = "java/io/File->getPath()Ljava/lang/String;";
    private static final String LONG_VALUE_SIGNATURE = "java/lang/Long->longValue()J";
    private static final String INTEGER_VALUE_SIGNATURE = "java/lang/Integer->intValue()I";
    private static final String BOOLEAN_VALUE_SIGNATURE = "java/lang/Boolean->booleanValue()Z";
    private static final String PATCHED_MS_VOID_SIGNATURE = "com/bytedance/mobsec/metasec/ml/MS->a()V";
    private static final String PROCESS_MY_UID_SIGNATURE = "android/os/Process->myUid()I";
    private static final String DEBUG_IS_DEBUGGER_CONNECTED_SIGNATURE = "android/os/Debug->isDebuggerConnected()Z";
    private static final String CHECK_SELF_PERMISSION_SIGNATURE =
        "android/content/Context->checkSelfPermission(Ljava/lang/String;)I";
    private static final String BUILD_VERSION_RELEASE_SIGNATURE = "android/os/Build$VERSION->RELEASE:Ljava/lang/String;";
    private static final String BUILD_VERSION_SDK_SIGNATURE = "android/os/Build$VERSION->SDK:Ljava/lang/String;";
    private static final String BUILD_VERSION_SDK_INT_SIGNATURE = "android/os/Build$VERSION->SDK_INT:I";
    private static final String BUILD_BRAND_SIGNATURE = "android/os/Build->BRAND:Ljava/lang/String;";
    private static final String BUILD_MANUFACTURER_SIGNATURE = "android/os/Build->MANUFACTURER:Ljava/lang/String;";
    private static final String BUILD_MODEL_SIGNATURE = "android/os/Build->MODEL:Ljava/lang/String;";
    private static final String BUILD_DEVICE_SIGNATURE = "android/os/Build->DEVICE:Ljava/lang/String;";
    private static final String BUILD_PRODUCT_SIGNATURE = "android/os/Build->PRODUCT:Ljava/lang/String;";
    private static final String BUILD_HARDWARE_SIGNATURE = "android/os/Build->HARDWARE:Ljava/lang/String;";
    private static final String BUILD_CPU_ABI_SIGNATURE = "android/os/Build->CPU_ABI:Ljava/lang/String;";
    private static final String BUILD_CPU_ABI2_SIGNATURE = "android/os/Build->CPU_ABI2:Ljava/lang/String;";
    private static final String BUILD_SUPPORTED_ABIS_SIGNATURE = "android/os/Build->SUPPORTED_ABIS:[Ljava/lang/String;";
    private static final String BUILD_SUPPORTED_64_BIT_ABIS_SIGNATURE =
        "android/os/Build->SUPPORTED_64_BIT_ABIS:[Ljava/lang/String;";
    private static final String BUILD_SUPPORTED_32_BIT_ABIS_SIGNATURE =
        "android/os/Build->SUPPORTED_32_BIT_ABIS:[Ljava/lang/String;";
    private static final String APPLICATION_INFO_SOURCE_DIR_SIGNATURE =
        "android/content/pm/ApplicationInfo->sourceDir:Ljava/lang/String;";
    private static final String LEGACY_APPLICATION_INFO_SOURCE_DIR_SIGNATURE =
        "Landroid/content/pm/ApplicationInfo;->sourceDir:Ljava/lang/String;";
    private static final String ANDROID_RELEASE = "6.0";
    private static final String ANDROID_SDK = Integer.toString(SDK_VERSION);
    private static final String DEVICE_BRAND = "Xiaomi";
    private static final String DEVICE_MANUFACTURER = "Xiaomi";
    private static final String DEVICE_MODEL = "Sirius";
    private static final String DEVICE_NAME = "Sirius";
    private static final String DEVICE_PRODUCT = "Sirius";
    private static final String DEVICE_HARDWARE = "qcom";
    private static final String DEVICE_CPU_ABI = "arm64-v8a";

    private final boolean loggable;
    private final AndroidEmulator emulator;
    private final Memory memory;
    private final Module module;
    private final DvmClass threadClass;
    private final DvmClass applicationClass;
    private final DvmClass contextClass;
    private final DvmClass stackTraceElementClass;
    private final DvmClass integerClass;
    private final DvmClass longClass;
    private final DvmClass fileClass;
    private final DvmClass packageManagerClass;
    private final DvmClass applicationInfoClass;
    private final File apkFile;
    private final File soMetasecMlFile;
    private final File soCShareFile;
    private final File rootfsDir;
    private final byte[] msCertData;
    private final ReentrantLock lifecycleLock = new ReentrantLock();

    private volatile boolean destroyed = false;

    public IdleFQ(boolean loggable, String apkPath, String resourceRoot) {
        this.loggable = loggable;

        AndroidEmulator emulatorCandidate = null;
        ResolvedResources resources = null;
        try {
            resources = resolveResources(apkPath, resourceRoot);
            this.apkFile = resources.apkFile();
            this.soMetasecMlFile = resources.soMetasecMlFile();
            this.soCShareFile = resources.soCShareFile();
            this.rootfsDir = resources.rootfsDir();
            this.msCertData = resources.msCertData();

            emulatorCandidate = createEmulator(this.rootfsDir);
            VM vmCandidate = createVm(emulatorCandidate);
            CachedClasses cachedClasses = cacheVmClasses(vmCandidate);
            Module moduleCandidate = loadMainModule(emulatorCandidate, vmCandidate);

            this.emulator = emulatorCandidate;
            this.memory = emulatorCandidate.getMemory();
            this.threadClass = cachedClasses.threadClass();
            this.applicationClass = cachedClasses.applicationClass();
            this.contextClass = cachedClasses.contextClass();
            this.stackTraceElementClass = cachedClasses.stackTraceElementClass();
            this.integerClass = cachedClasses.integerClass();
            this.longClass = cachedClasses.longClass();
            this.fileClass = cachedClasses.fileClass();
            this.packageManagerClass = cachedClasses.packageManagerClass();
            this.applicationInfoClass = cachedClasses.applicationInfoClass();
            this.module = moduleCandidate;

            logResolvedResources();
            log.info("初始化完成");
        } catch (Exception e) {
            cleanupAfterInitFailure(emulatorCandidate, resources);
            log.error("初始化失败", e);
            throw new RuntimeException("初始化失败", e);
        }
    }

    public String generateSignature(String url, String header) {
        lifecycleLock.lock();
        try {
            if (destroyed) {
                if (loggable) {
                    log.debug("已销毁，跳过签名生成");
                }
                return null;
            }
            if (loggable) {
                log.debug("准备生成签名 - URL: {}", url);
                log.debug("准备生成签名 - Header: {}", header);
            }

            UnidbgPointer result = invokeSignFunction(url, header);
            if (result == null) {
                return null;
            }

            String signature = result.getString(0);
            if (loggable) {
                log.debug("签名生成成功: {}", signature);
            }
            return signature;
        } catch (Exception e) {
            log.error("生成签名过程出错: {}", e.getMessage(), e);
            return null;
        } finally {
            lifecycleLock.unlock();
        }
    }

    @Override
    public DvmObject<?> callStaticObjectMethodV(BaseVM vm, DvmClass dvmClass, String signature, VaList vaList) {
        return switch (signature) {
            case MS_DISPATCH_SIGNATURE -> handleMSMethod(vm, vaList.getIntArg(0));
            case CURRENT_THREAD_SIGNATURE -> threadClass.newObject(Thread.currentThread());
            case CURRENT_APPLICATION_SIGNATURE -> applicationClass.newObject(null);
            default -> super.callStaticObjectMethodV(vm, dvmClass, signature, vaList);
        };
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    @Override
    public DvmObject<?> callObjectMethodV(BaseVM vm, DvmObject<?> dvmObject, String signature, VaList vaList) {
        return switch (signature) {
            case THREAD_STACK_TRACE_SIGNATURE -> buildStackTraceArray(vm);
            case STACK_TRACE_CLASS_NAME_SIGNATURE -> {
                StackTraceElement element = (StackTraceElement) dvmObject.getValue();
                yield new StringObject(vm, element.getClassName());
            }
            case STACK_TRACE_METHOD_NAME_SIGNATURE -> {
                StackTraceElement element = (StackTraceElement) dvmObject.getValue();
                yield new StringObject(vm, element.getMethodName());
            }
            case CONTEXT_GET_PACKAGE_NAME_SIGNATURE, APPLICATION_GET_PACKAGE_NAME_SIGNATURE,
                LEGACY_APPLICATION_GET_PACKAGE_NAME_SIGNATURE ->
                new StringObject(vm, PACKAGE_NAME);
            case CONTEXT_GET_APPLICATION_CONTEXT_SIGNATURE -> contextClass.newObject(null);
            case CONTEXT_GET_FILES_DIR_SIGNATURE -> fileClass.newObject(new File(DATA_FILES_DIR));
            case CONTEXT_GET_PACKAGE_MANAGER_SIGNATURE, APPLICATION_GET_PACKAGE_MANAGER_SIGNATURE,
                LEGACY_APPLICATION_GET_PACKAGE_MANAGER_SIGNATURE ->
                packageManagerClass.newObject(null);
            case CONTEXT_GET_APPLICATION_INFO_SIGNATURE, APPLICATION_GET_APPLICATION_INFO_SIGNATURE,
                LEGACY_APPLICATION_GET_APPLICATION_INFO_SIGNATURE ->
                applicationInfoClass.newObject(null);
            case CONTEXT_GET_PACKAGE_CODE_PATH_SIGNATURE, CONTEXT_GET_PACKAGE_RESOURCE_PATH_SIGNATURE,
                APPLICATION_GET_PACKAGE_CODE_PATH_SIGNATURE, APPLICATION_GET_PACKAGE_RESOURCE_PATH_SIGNATURE ->
                new StringObject(vm, apkFile.getAbsolutePath());
            case FILE_GET_ABSOLUTE_PATH_SIGNATURE, FILE_GET_PATH_SIGNATURE -> {
                File file = (File) dvmObject.getValue();
                yield new StringObject(vm, file.getAbsolutePath());
            }
            case THREAD_GET_BYTES_SIGNATURE -> {
                String arg0 = (String) vaList.getObjectArg(0).getValue();
                if (loggable) {
                    log.debug("java/lang/Thread->getBytes arg0: {}", arg0);
                }
                yield new ByteArray(vm, arg0.getBytes(StandardCharsets.UTF_8));
            }
            default -> super.callObjectMethodV(vm, dvmObject, signature, vaList);
        };
    }

    @Override
    public long callLongMethodV(BaseVM vm, DvmObject<?> dvmObject, String signature, VaList vaList) {
        if (LONG_VALUE_SIGNATURE.equals(signature)) {
            Object value = dvmObject.getValue();
            if (value instanceof Long l) {
                return l;
            }
        }
        return super.callLongMethodV(vm, dvmObject, signature, vaList);
    }

    @Override
    public void callVoidMethod(BaseVM vm, DvmObject<?> dvmObject, String signature, VarArg varArg) {
        if (loggable) {
            log.debug("callVoidMethod: {}", signature);
        }
        switch (signature) {
            case PATCHED_MS_VOID_SIGNATURE -> {
                if (loggable) {
                    log.debug("Patched: {}", PATCHED_MS_VOID_SIGNATURE);
                }
            }
            default -> super.callVoidMethod(vm, dvmObject, signature, varArg);
        }
    }

    @Override
    public int callIntMethodV(BaseVM vm, DvmObject<?> dvmObject, String signature, VaList vaList) {
        if (INTEGER_VALUE_SIGNATURE.equals(signature)) {
            Object value = dvmObject.getValue();
            if (value instanceof Integer i) {
                return i;
            }
            if (value instanceof String s) {
                return Integer.parseInt(s);
            }
        }
        if (CHECK_SELF_PERMISSION_SIGNATURE.equals(signature)) {
            return 0;
        }
        return super.callIntMethodV(vm, dvmObject, signature, vaList);
    }

    @Override
    public boolean callBooleanMethodV(BaseVM vm, DvmObject<?> dvmObject, String signature, VaList vaList) {
        if (BOOLEAN_VALUE_SIGNATURE.equals(signature)) {
            Object value = dvmObject.getValue();
            if (value instanceof Boolean b) {
                return b;
            }
            if (value instanceof String s) {
                return Boolean.parseBoolean(s);
            }
        }
        return super.callBooleanMethodV(vm, dvmObject, signature, vaList);
    }

    @Override
    public int callStaticIntMethodV(BaseVM vm, DvmClass dvmClass, String signature, VaList vaList) {
        if (PROCESS_MY_UID_SIGNATURE.equals(signature)) {
            return APP_UID;
        }
        return super.callStaticIntMethodV(vm, dvmClass, signature, vaList);
    }

    @Override
    public boolean callStaticBooleanMethodV(BaseVM vm, DvmClass dvmClass, String signature, VaList vaList) {
        if (DEBUG_IS_DEBUGGER_CONNECTED_SIGNATURE.equals(signature)) {
            return false;
        }
        return super.callStaticBooleanMethodV(vm, dvmClass, signature, vaList);
    }

    @Override
    public FileResult resolve(Emulator<AndroidFileIO> emulator, String pathname, int oflags) {
        if (loggable) {
            log.debug("resolve ==> {}", pathname);
        }

        if (pathname.contains(SO_METASEC_ML_NAME)) {
            return openVirtualFile(oflags, soMetasecMlFile, pathname);
        }
        if (pathname.contains(SO_C_SHARE_NAME)) {
            return openVirtualFile(oflags, soCShareFile, pathname);
        }
        if (APK_INSTALL_PATH.equals(pathname)) {
            return openVirtualFile(oflags, apkFile, pathname);
        }
        return null;
    }

    public void destroy() {
        lifecycleLock.lock();
        try {
            if (destroyed) {
                return;
            }
            destroyed = true;
            closeEmulator(emulator, "关闭模拟器失败");
            log.info("资源已释放");
        } finally {
            lifecycleLock.unlock();
        }

        cleanupRootfs(rootfsDir, "清理临时 rootfs 目录失败");
    }

    private ResolvedResources resolveResources(String apkPath, String resourceRoot) throws IOException {
        Path resourceBasePath = resolveResourceRoot(resourceRoot);
        File resolvedApkFile = resolveApkFile(apkPath, resourceBasePath);
        File resolvedSoMetasecFile = resolveBundledResourceFile(resourceBasePath, SO_METASEC_ML_PATH);
        File resolvedSoCShareFile = resolveBundledResourceFile(resourceBasePath, SO_C_SHARE_PATH);
        File resolvedMsCertFile = resolveBundledResourceFile(resourceBasePath, MS_CERT_FILE_PATH);
        byte[] resolvedMsCertData = Files.readAllBytes(resolvedMsCertFile.toPath());
        File resolvedRootfsDir = createPreparedRootfs();

        return new ResolvedResources(
            resolvedApkFile,
            resolvedSoMetasecFile,
            resolvedSoCShareFile,
            resolvedRootfsDir,
            resolvedMsCertData
        );
    }

    private AndroidEmulator createEmulator(File rootfsDir) {
        AndroidEmulator emulator = AndroidEmulatorBuilder
            .for64Bit()
            .setRootDir(rootfsDir)
            .setProcessName(PACKAGE_NAME)
            .addBackendFactory(new Unicorn2Factory(true))
            .build();

        Map<String, Integer> inode = new LinkedHashMap<>();
        inode.put("/data/system", 671745);
        inode.put("/data/app", 327681);
        inode.put("/sdcard/android", 294915);
        inode.put(DATA_USER_DIR, 655781);
        inode.put(DATA_FILES_DIR, 655864);
        emulator.set("inode", inode);
        emulator.set("uid", APP_UID);

        SyscallHandler<AndroidFileIO> handler = emulator.getSyscallHandler();
        handler.setVerbose(false);
        handler.addIOResolver(this);

        return emulator;
    }

    private VM createVm(AndroidEmulator emulator) {
        Memory emulatorMemory = emulator.getMemory();
        emulatorMemory.setLibraryResolver(new AndroidResolver(SDK_VERSION));

        VM vm = emulator.createDalvikVM(apkFile);
        vm.setJni(this);
        vm.setVerbose(loggable);

        new AndroidModule(emulator, vm).register(emulatorMemory);
        new JniGraphics(emulator, vm).register(emulatorMemory);
        return vm;
    }

    private CachedClasses cacheVmClasses(VM vm) {
        return new CachedClasses(
            vm.resolveClass("java/lang/Thread"),
            vm.resolveClass("android/app/Application"),
            vm.resolveClass("android/content/Context"),
            vm.resolveClass("java/lang/StackTraceElement"),
            vm.resolveClass("java.lang.Integer"),
            vm.resolveClass("java/lang/Long"),
            vm.resolveClass("java/io/File"),
            vm.resolveClass("android/content/pm/PackageManager"),
            vm.resolveClass("android/content/pm/ApplicationInfo")
        );
    }

    private Module loadMainModule(AndroidEmulator emulator, VM vm) {
        vm.loadLibrary(soCShareFile, false);

        DvmClass bridgeClass = vm.resolveClass("ms/bd/c/m");
        DvmClass a4a = vm.resolveClass("ms/bd/c/a4$a", bridgeClass);
        vm.resolveClass("com/bytedance/mobsec/metasec/ml/MS", a4a);

        DalvikModule dalvikModule = vm.loadLibrary(soMetasecMlFile, true);
        dalvikModule.callJNI_OnLoad(emulator);
        return dalvikModule.getModule();
    }

    private UnidbgPointer invokeSignFunction(String url, String header) {
        Number number = module.callFunction(emulator, SIGN_FUNCTION_OFFSET, url, header);
        if (number == null) {
            log.error("调用 native 签名函数失败，返回结果为 null");
            return null;
        }

        UnidbgPointer result = memory.pointer(number.longValue());
        if (result == null) {
            log.error("获取签名结果指针失败");
            return null;
        }
        return result;
    }

    private DvmObject<?> handleMSMethod(BaseVM vm, int methodId) {
        return switch (methodId) {
            case MS_METHOD_DATA_PATH -> new StringObject(vm, MSDATA_VFS_PATH);
            case MS_METHOD_BOOL_1, MS_METHOD_BOOL_2 -> DvmBoolean.valueOf(vm, true);
            case MS_METHOD_VERSION_CODE -> integerClass.newObject(APP_VERSION_CODE);
            case MS_METHOD_VERSION_NAME -> new StringObject(vm, "6.8.1.32");
            case MS_METHOD_CERT -> certificateBytes(vm);
            case MS_METHOD_NOW_MS -> longClass.newObject(System.currentTimeMillis());
            default -> {
                if (loggable) {
                    log.debug("未处理的 MS 方法 ID: {}", methodId);
                }
                yield null;
            }
        };
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private DvmObject<?> buildStackTraceArray(BaseVM vm) {
        StackTraceElement[] elements = Thread.currentThread().getStackTrace();
        DvmObject<?>[] objects = (DvmObject<?>[]) new DvmObject[elements.length];
        for (int i = 0; i < elements.length; i++) {
            objects[i] = stackTraceElementClass.newObject(elements[i]);
        }
        return new ArrayObject(objects);
    }

    private DvmObject<?> certificateBytes(BaseVM vm) {
        if (loggable) {
            log.debug("成功读取证书文件: {} bytes", msCertData.length);
        }
        return new ByteArray(vm, msCertData);
    }

    private DvmObject<?> buildStringArray(VM vm, String... values) {
        StringObject[] objects = new StringObject[values.length];
        for (int i = 0; i < values.length; i++) {
            objects[i] = new StringObject(vm, values[i]);
        }
        return new ArrayObject(objects);
    }

    @Override
    public int getStaticIntField(BaseVM vm, DvmClass dvmClass, String signature) {
        if (loggable) {
            log.debug("getStaticIntField: {}", signature);
        }
        if (PATCHED_MS_VOID_SIGNATURE.equals(signature)) {
            return 0x40;
        }
        if (BUILD_VERSION_SDK_INT_SIGNATURE.equals(signature)) {
            return SDK_VERSION;
        }
        if (loggable) {
            log.debug("未处理的静态整数字段，降级返回0: {}", signature);
        }
        return 0;
    }

    @Override
    public DvmObject<?> getStaticObjectField(BaseVM vm, DvmClass dvmClass, String signature) {
        return switch (signature) {
            case BUILD_VERSION_RELEASE_SIGNATURE -> new StringObject(vm, ANDROID_RELEASE);
            case BUILD_VERSION_SDK_SIGNATURE -> new StringObject(vm, ANDROID_SDK);
            case BUILD_BRAND_SIGNATURE -> new StringObject(vm, DEVICE_BRAND);
            case BUILD_MANUFACTURER_SIGNATURE -> new StringObject(vm, DEVICE_MANUFACTURER);
            case BUILD_MODEL_SIGNATURE -> new StringObject(vm, DEVICE_MODEL);
            case BUILD_DEVICE_SIGNATURE -> new StringObject(vm, DEVICE_NAME);
            case BUILD_PRODUCT_SIGNATURE -> new StringObject(vm, DEVICE_PRODUCT);
            case BUILD_HARDWARE_SIGNATURE -> new StringObject(vm, DEVICE_HARDWARE);
            case BUILD_CPU_ABI_SIGNATURE -> new StringObject(vm, DEVICE_CPU_ABI);
            case BUILD_CPU_ABI2_SIGNATURE -> new StringObject(vm, "");
            case BUILD_SUPPORTED_ABIS_SIGNATURE, BUILD_SUPPORTED_64_BIT_ABIS_SIGNATURE ->
                buildStringArray(vm, DEVICE_CPU_ABI);
            case BUILD_SUPPORTED_32_BIT_ABIS_SIGNATURE -> buildStringArray(vm);
            default -> super.getStaticObjectField(vm, dvmClass, signature);
        };
    }

    @Override
    public DvmObject<?> getObjectField(BaseVM vm, DvmObject<?> dvmObject, String signature) {
        return switch (signature) {
            case APPLICATION_INFO_SOURCE_DIR_SIGNATURE, LEGACY_APPLICATION_INFO_SOURCE_DIR_SIGNATURE ->
                new StringObject(vm, apkFile.getAbsolutePath());
            default -> super.getObjectField(vm, dvmObject, signature);
        };
    }

    private static FileResult openVirtualFile(int oflags, File file, String pathname) {
        return FileResult.success(new SimpleFileIO(oflags, file, pathname));
    }

    private static Path resolveResourceRoot(String resourceRoot) throws IOException {
        String normalizedRoot = trimToNull(resourceRoot);
        if (normalizedRoot == null) {
            throw new IOException("未配置 UNIDBG_RESOURCE_ROOT");
        }

        Path basePath = Path.of(normalizedRoot).toAbsolutePath().normalize();
        if (!Files.exists(basePath) || !Files.isDirectory(basePath)) {
            throw new IOException("资源目录不存在: " + basePath);
        }
        return basePath;
    }

    private static File resolveApkFile(String apkPath, Path resourceBasePath) throws IOException {
        String configuredApkPath = trimToNull(apkPath);
        if (configuredApkPath != null) {
            File file = new File(configuredApkPath);
            if (!file.exists() || !file.isFile()) {
                throw new IOException("APK 文件不存在: " + file.getAbsolutePath());
            }
            return file;
        }

        return resolveBundledResourceFile(resourceBasePath, DEFAULT_APK_RESOURCE_PATH);
    }

    private static File resolveBundledResourceFile(Path resourceBasePath, String relativePath) throws IOException {
        Path filePath = resourceBasePath.resolve(relativePath).normalize();
        if (!filePath.startsWith(resourceBasePath)) {
            throw new IOException("资源路径非法: " + relativePath);
        }
        File file = filePath.toFile();
        if (!file.exists() || !file.isFile()) {
            throw new IOException("资源文件不存在: " + file.getAbsolutePath());
        }
        return file;
    }

    private static File createPreparedRootfs() throws IOException {
        File rootfsDir = Files.createTempDirectory("fq_rootfs").toFile();
        try {
            prepareRootfs(rootfsDir.toPath());
            return rootfsDir;
        } catch (IOException e) {
            deleteRecursively(rootfsDir);
            throw e;
        }
    }

    private static void prepareRootfs(Path rootfs) throws IOException {
        Path msDataDir = rootfs.resolve("data/user/0/" + PACKAGE_NAME + "/files");
        Files.createDirectories(msDataDir);

        Path msDataFile = msDataDir.resolve(".msdata");
        if (!Files.exists(msDataFile)) {
            Files.createFile(msDataFile);
        }

        Files.createDirectories(rootfs.resolve("data/system"));
        Files.createDirectories(rootfs.resolve("data/app"));
        Files.createDirectories(rootfs.resolve("sdcard/android"));
    }

    private void logResolvedResources() {
        if (!loggable) {
            return;
        }

        log.debug("APK 文件: {}", apkFile.getAbsolutePath());
        log.debug("SO 主文件: {}", soMetasecMlFile.getAbsolutePath());
        log.debug("SO 共享库文件: {}", soCShareFile.getAbsolutePath());
        log.debug("证书缓存字节数: {}", msCertData.length);
        log.debug("rootfs 目录: {}", rootfsDir.getAbsolutePath());
    }

    private void cleanupAfterInitFailure(AndroidEmulator emulatorCandidate, ResolvedResources resources) {
        closeEmulator(emulatorCandidate, "初始化失败后关闭模拟器失败");
        if (resources != null) {
            cleanupRootfs(resources.rootfsDir(), "初始化失败后清理临时 rootfs 目录失败");
        }
    }

    private void closeEmulator(AndroidEmulator emulatorCandidate, String message) {
        if (emulatorCandidate == null) {
            return;
        }
        try {
            emulatorCandidate.close();
        } catch (Exception error) {
            if (loggable) {
                log.debug(message, error);
            } else {
                log.error(message, error);
            }
        }
    }

    private void cleanupRootfs(File targetRootfsDir, String debugMessage) {
        try {
            deleteRecursively(targetRootfsDir);
        } catch (Exception error) {
            if (loggable) {
                log.debug(debugMessage + ": {}", targetRootfsDir != null ? targetRootfsDir.getAbsolutePath() : null, error);
            }
        }
    }

    private static String trimToNull(String value) {
        if (value == null) {
            return null;
        }
        String trimmed = value.trim();
        return trimmed.isEmpty() ? null : trimmed;
    }

    private static void deleteRecursively(File file) {
        if (file == null || !file.exists()) {
            return;
        }
        if (file.isDirectory()) {
            File[] children = file.listFiles();
            if (children != null) {
                for (File child : children) {
                    deleteRecursively(child);
                }
            }
        }
        try {
            Files.deleteIfExists(file.toPath());
        } catch (Exception ignored) {
            // ignore
        }
    }

    private record ResolvedResources(
        File apkFile,
        File soMetasecMlFile,
        File soCShareFile,
        File rootfsDir,
        byte[] msCertData
    ) {
    }

    private record CachedClasses(
        DvmClass threadClass,
        DvmClass applicationClass,
        DvmClass contextClass,
        DvmClass stackTraceElementClass,
        DvmClass integerClass,
        DvmClass longClass,
        DvmClass fileClass,
        DvmClass packageManagerClass,
        DvmClass applicationInfoClass
    ) {
    }
}
