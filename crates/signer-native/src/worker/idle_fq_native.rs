use anyhow::{anyhow, Result};
use emulator::android::dvm::class::DvmClass;
use emulator::android::dvm::class_resolver::ClassResolver;
use emulator::android::dvm::member::{DvmField, DvmMethod};
use emulator::android::dvm::object::DvmObject;
use emulator::android::dvm::DalvikVM64;
use emulator::android::jni::{Jni, JniValue, MethodAcc, VaList};
use emulator::android::virtual_library::libc::Libc;
use emulator::AndroidEmulator;
use emulator::linux::file_system::{FileIO, StMode};
use emulator::linux::fs::direction::Direction;
use emulator::linux::fs::linux_file::LinuxFileIO;
use emulator::memory::svc_memory::{SimpleArm64Svc, SvcCallResult};
use emulator::UnicornArg;
use log::info;
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

const BASE_PATH: &str = "com/dragon/read/oversea/gp";
const DEFAULT_APK_RESOURCE_PATH: &str = "com/dragon/read/oversea/gp/apk/base.apk";
const SO_METASEC_ML_PATH: &str = "com/dragon/read/oversea/gp/lib/libmetasec_ml.so";
const SO_C_SHARE_PATH: &str = "com/dragon/read/oversea/gp/lib/libc++_shared.so";
const MS_CERT_FILE_PATH: &str = "com/dragon/read/oversea/gp/other/ms_16777218.bin";

const PACKAGE_NAME: &str = "com.dragon.read.oversea.gp";
const DATA_USER_DIR: &str = "/data/user/0/com.dragon.read.oversea.gp";
const DATA_FILES_DIR: &str = "/data/user/0/com.dragon.read.oversea.gp/files";
const MSDATA_VFS_PATH: &str = "/data/user/0/com.dragon.read.oversea.gp/files/.msdata";
const APK_INSTALL_PATH: &str =
    "/data/app/com.dragon.read.oversea.gp-q5NyjSN9BLSTVBJ54kg7YA==/base.apk";

const APP_UID: i32 = 10074;
const APP_VERSION_CODE: i32 = 68132;
const SIGN_FUNCTION_OFFSET: u64 = 0x168c80;

const MS_METHOD_DATA_PATH: i32 = 65539;
const MS_METHOD_BOOL_1: i32 = 33554433;
const MS_METHOD_BOOL_2: i32 = 33554434;
const MS_METHOD_VERSION_CODE: i32 = 16777232;
const MS_METHOD_VERSION_NAME: i32 = 16777233;
const MS_METHOD_CERT: i32 = 16777218;
const MS_METHOD_NOW_MS: i32 = 268435470;

const PID: u32 = 2667;
const PPID: u32 = 2427;

#[derive(Clone)]
struct CachedClasses {
    thread: Rc<DvmClass>,
    stack_trace_element: Rc<DvmClass>,
    integer: Rc<DvmClass>,
    long: Rc<DvmClass>,
    boolean: Rc<DvmClass>,
}

#[derive(Clone)]
struct StackTraceElementData {
    class_name: String,
    method_name: String,
}

pub struct IdleFqNative {
    loggable: bool,
    emulator: AndroidEmulator<'static, ()>,
    module_base: u64,
    resources: Resources,
    classes: CachedClasses,
}

struct Resources {
    apk_file: PathBuf,
    so_metasec_file: PathBuf,
    so_c_share_file: PathBuf,
    ms_cert_data: Vec<u8>,
    msdata_file: PathBuf,
}

impl IdleFqNative {
    pub fn new(
        loggable: bool,
        apk_path: Option<String>,
        resource_root: String,
        rnidbg_base_path: Option<String>,
        android_sdk_api: u32,
    ) -> Result<Self> {
        let resources = resolve_resources(apk_path, &resource_root)?;
        let rnidbg_base_path = rnidbg_base_path.unwrap_or_else(|| {
            Path::new("vendor/rnidbg/android/sdk31")
                .to_string_lossy()
                .to_string()
        });
        std::env::set_var("BASE_PATH", rnidbg_base_path);

        let mut emulator = AndroidEmulator::create_arm64(PID, PPID, PACKAGE_NAME, ());
        register_libc_hooks(&mut emulator, android_sdk_api);
        register_virtual_modules(&mut emulator);
        install_file_resolver(&mut emulator, &resources);

        let vm = emulator.get_dalvik_vm();
        let class_resolver = ClassResolver::new(vec![
            "java/lang/Thread",
            "java/lang/StackTraceElement",
            "java/lang/Integer",
            "java/lang/Long",
            "java/lang/Boolean",
            "com/bytedance/mobsec/metasec/ml/MS",
            "ms/bd/c/m",
            "ms/bd/c/a4$a",
        ]);
        vm.set_class_resolver(class_resolver);

        let classes = CachedClasses {
            thread: vm.resolve_class_unchecked("java/lang/Thread").1,
            stack_trace_element: vm.resolve_class_unchecked("java/lang/StackTraceElement").1,
            integer: vm.resolve_class_unchecked("java/lang/Integer").1,
            long: vm.resolve_class_unchecked("java/lang/Long").1,
            boolean: vm.resolve_class_unchecked("java/lang/Boolean").1,
        };

        vm.set_jni(Box::new(FqJni {
            loggable,
            classes: classes.clone(),
            ms_cert_data: resources.ms_cert_data.clone(),
        }));

        let _ = vm.load_library(emulator.clone(), resources.so_c_share_file.to_string_lossy().as_ref(), false)?;
        let module = vm.load_library(
            emulator.clone(),
            resources.so_metasec_file.to_string_lossy().as_ref(),
            true,
        )?;
        vm.call_jni_onload(emulator.clone(), unsafe { &*module.get() })?;

        info!("rust native signer initialized");
        Ok(Self {
            loggable,
            emulator,
            module_base: unsafe { &*module.get() }.base,
            resources,
            classes,
        })
    }

    pub fn generate_signature(&mut self, url: &str, headers: &str) -> Result<Option<String>> {
        let result = self.emulator.e_func(
            self.module_base + SIGN_FUNCTION_OFFSET,
            vec![
                UnicornArg::Str(url.to_string()),
                UnicornArg::Str(headers.to_string()),
            ],
        );
        let Some(ptr) = result else {
            return Ok(None);
        };
        if ptr == 0 {
            return Ok(None);
        }
        let text = self.emulator.backend.mem_read_c_string(ptr)?;
        if self.loggable {
            info!("rust native signer result: {}", text);
        }
        Ok(Some(text))
    }

    pub fn destroy(&mut self) {
        self.emulator.destroy();
    }
}

struct FqJni {
    loggable: bool,
    classes: CachedClasses,
    ms_cert_data: Vec<u8>,
}

impl Jni<()> for FqJni {
    fn resolve_method(
        &mut self,
        _vm: &mut DalvikVM64<()>,
        _class: &Rc<DvmClass>,
        _name: &str,
        _signature: &str,
        _is_static: bool,
    ) -> bool {
        true
    }

    fn resolve_filed(
        &mut self,
        _vm: &mut DalvikVM64<()>,
        _class: &Rc<DvmClass>,
        _name: &str,
        _signature: &str,
        _is_static: bool,
    ) -> bool {
        true
    }

    fn call_method_v(
        &mut self,
        vm: &mut DalvikVM64<()>,
        acc: MethodAcc,
        class: &Rc<DvmClass>,
        method: &DvmMethod,
        instance: Option<&mut DvmObject>,
        args: &mut VaList<()>,
    ) -> JniValue {
        let class_name = class.name.as_str();
        if acc.contains(MethodAcc::STATIC)
            && class_name == "com/bytedance/mobsec/metasec/ml/MS"
            && method.name == "b"
            && method.signature == "(IIJLjava/lang/String;Ljava/lang/Object;)Ljava/lang/Object;"
        {
            let _arg0 = args.get::<i32>(vm);
            let method_id = args.get::<i32>(vm);
            let _arg2 = args.get::<i64>(vm);
            let _arg3 = args.get::<String>(vm);
            let _arg4 = args.get::<DvmObject>(vm);
            return self.handle_ms_method(method_id);
        }

        if acc.contains(MethodAcc::STATIC)
            && class_name == "java/lang/Thread"
            && method.name == "currentThread"
            && method.signature == "()Ljava/lang/Thread;"
        {
            return self.classes.thread.new_simple_instance(vm).into();
        }

        if acc.contains(MethodAcc::VOID)
            && class_name == "com/bytedance/mobsec/metasec/ml/MS"
            && method.name == "a"
            && method.signature == "()V"
        {
            return JniValue::Void;
        }

        if class_name == "java/lang/Thread"
            && method.name == "getStackTrace"
            && method.signature == "()[Ljava/lang/StackTraceElement;"
        {
            return build_stack_trace_array(&self.classes).into();
        }

        if class_name == "java/lang/Thread"
            && method.name == "getBytes"
            && method.signature == "(Ljava/lang/String;)[B"
        {
            let value = args.get::<String>(vm);
            return value.into_bytes().into();
        }

        if class_name == "java/lang/StackTraceElement" && method.name == "getClassName" {
            if let Some(ref instance) = instance {
                if let Some(element) = stack_trace_element(instance) {
                    return element.class_name.clone().into();
                }
            }
        }

        if class_name == "java/lang/StackTraceElement" && method.name == "getMethodName" {
            if let Some(ref instance) = instance {
                if let Some(element) = stack_trace_element(instance) {
                    return element.method_name.clone().into();
                }
            }
        }

        if class_name == "java/lang/Integer" && method.name == "intValue" {
            if let Some(ref instance) = instance {
                if let Some(value) = data_value::<i32>(instance) {
                    return (*value).into();
                }
                if let Some(value) = data_value::<String>(instance) {
                    return value.parse::<i32>().unwrap_or_default().into();
                }
            }
        }

        if class_name == "java/lang/Long" && method.name == "longValue" {
            if let Some(ref instance) = instance {
                if let Some(value) = data_value::<i64>(instance) {
                    return (*value).into();
                }
            }
        }

        if class_name == "java/lang/Boolean" && method.name == "booleanValue" {
            if let Some(ref instance) = instance {
                if let Some(value) = data_value::<bool>(instance) {
                    return (*value).into();
                }
                if let Some(value) = data_value::<String>(instance) {
                    return value.parse::<bool>().unwrap_or(false).into();
                }
            }
        }

        if acc.contains(MethodAcc::CONSTRUCTOR) && class_name == "java/lang/String" {
            if method.signature == "([BLjava/lang/String;)V" {
                let bytes = args.get::<Vec<u8>>(vm);
                let _charset = args.get::<String>(vm);
                return String::from_utf8(bytes).unwrap_or_default().into();
            }
        }

        default_method_value(acc)
    }

    fn get_field_value(
        &mut self,
        _vm: &mut DalvikVM64<()>,
        class: &Rc<DvmClass>,
        field: &DvmField,
        _instance: Option<&mut DvmObject>,
    ) -> JniValue {
        if class.name == "com/bytedance/mobsec/metasec/ml/MS"
            && field.name == "a"
            && field.signature == "V"
        {
            return 0x40i32.into();
        }
        default_field_value(&field.signature)
    }

    fn set_field_value(
        &mut self,
        _vm: &mut DalvikVM64<()>,
        _class: &Rc<DvmClass>,
        _field: &DvmField,
        _instance: Option<&mut DvmObject>,
        _value: JniValue,
    ) {
    }
}

impl FqJni {
    fn handle_ms_method(&self, method_id: i32) -> JniValue {
        match method_id {
            MS_METHOD_DATA_PATH => MSDATA_VFS_PATH.to_string().into(),
            MS_METHOD_BOOL_1 | MS_METHOD_BOOL_2 => object_data(self.classes.boolean.clone(), true).into(),
            MS_METHOD_VERSION_CODE => object_data(self.classes.integer.clone(), APP_VERSION_CODE).into(),
            MS_METHOD_VERSION_NAME => "6.8.1.32".to_string().into(),
            MS_METHOD_CERT => self.ms_cert_data.clone().into(),
            MS_METHOD_NOW_MS => object_data(self.classes.long.clone(), current_time_millis()).into(),
            _ => JniValue::Null,
        }
    }
}

fn default_method_value(acc: MethodAcc) -> JniValue {
    if acc.contains(MethodAcc::VOID) {
        JniValue::Void
    } else if acc.contains(MethodAcc::BOOLEAN) {
        false.into()
    } else if acc.contains(MethodAcc::INT) {
        0i32.into()
    } else if acc.contains(MethodAcc::LONG) {
        0i64.into()
    } else if acc.contains(MethodAcc::FLOAT) {
        0f32.into()
    } else if acc.contains(MethodAcc::DOUBLE) {
        0f64.into()
    } else if acc.contains(MethodAcc::OBJECT) {
        JniValue::Null
    } else {
        JniValue::Null
    }
}

fn default_field_value(signature: &str) -> JniValue {
    match signature {
        "Z" => false.into(),
        "I" => 0i32.into(),
        "J" => 0i64.into(),
        "F" => 0f32.into(),
        "D" => 0f64.into(),
        _ => JniValue::Null,
    }
}

fn build_stack_trace_array(classes: &CachedClasses) -> DvmObject {
    let frames = vec![
        StackTraceElementData {
            class_name: "java.lang.Thread".to_string(),
            method_name: "getStackTrace".to_string(),
        },
        StackTraceElementData {
            class_name: "com.bytedance.mobsec.metasec.ml.MS".to_string(),
            method_name: "b".to_string(),
        },
        StackTraceElementData {
            class_name: "com.dragon.read.oversea.gp".to_string(),
            method_name: "sign".to_string(),
        },
    ];

    DvmObject::ObjectArray(
        classes.stack_trace_element.clone(),
        frames
            .into_iter()
            .map(|frame| Some(object_data(classes.stack_trace_element.clone(), frame)))
            .collect(),
    )
}

fn object_data<T: Any>(class: Rc<DvmClass>, value: T) -> DvmObject {
    DvmObject::DataInstance(class, Rc::new(Box::new(value)))
}

fn data_value<'a, T: Any>(object: &'a DvmObject) -> Option<&'a T> {
    match object {
        DvmObject::DataInstance(_, value) => value.as_ref().downcast_ref::<T>(),
        DvmObject::DataMutInstance(_, value) => unsafe { &*value.get() }.downcast_ref::<T>(),
        _ => None,
    }
}

fn stack_trace_element(object: &DvmObject) -> Option<&StackTraceElementData> {
    data_value::<StackTraceElementData>(object)
}

fn current_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or_default()
}

fn resolve_resources(apk_path: Option<String>, resource_root: &str) -> Result<Resources> {
    let base = PathBuf::from(resource_root);
    if !base.is_dir() {
        return Err(anyhow!("resource root not found: {}", base.display()));
    }

    let apk_file = apk_path
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join(DEFAULT_APK_RESOURCE_PATH));
    let so_metasec_file = base.join(SO_METASEC_ML_PATH);
    let so_c_share_file = base.join(SO_C_SHARE_PATH);
    let ms_cert_file = base.join(MS_CERT_FILE_PATH);
    let ms_cert_data = std::fs::read(&ms_cert_file)?;

    let root = std::env::temp_dir().join(format!(
        "fq_rnidbg_{}",
        current_time_millis()
    ));
    let msdata_dir = root.join("data/user/0").join(PACKAGE_NAME).join("files");
    std::fs::create_dir_all(&msdata_dir)?;
    std::fs::create_dir_all(root.join("data/system"))?;
    std::fs::create_dir_all(root.join("data/app"))?;
    std::fs::create_dir_all(root.join("sdcard/android"))?;
    let msdata_file = msdata_dir.join(".msdata");
    if !msdata_file.exists() {
        std::fs::File::create(&msdata_file)?;
    }

    Ok(Resources {
        apk_file,
        so_metasec_file,
        so_c_share_file,
        ms_cert_data,
        msdata_file,
    })
}

fn register_libc_hooks(emulator: &mut AndroidEmulator<'static, ()>, android_sdk_api: u32) {
    let mut libc = Libc::new();
    libc.set_system_property_service(Rc::new(Box::new(move |name| match name {
        "ro.build.version.sdk" => Some(android_sdk_api.to_string()),
        "persist.sys.timezone" => Some("Asia/Shanghai".to_string()),
        "ro.product.name" | "ro.product.device" => Some("Sirius".to_string()),
        "ro.product.manufacturer" | "ro.product.brand" => Some("Xiaomi".to_string()),
        "ro.product.model" => Some("Sirius".to_string()),
        "ro.hardware" => Some("qcom".to_string()),
        "ro.product.cpu.abi" | "ro.product.cpu.abilist" => Some("arm64-v8a".to_string()),
        "ro.boot.hardware" => Some("qcom".to_string()),
        "ro.recovery_id" => Some("0x11451419".to_string()),
        _ => None,
    })));
    emulator.memory().add_hook_listeners(Box::new(libc));
}

fn register_virtual_modules(emulator: &mut AndroidEmulator<'static, ()>) {
    register_libandroid(emulator);
    register_libjnigraphics(emulator);
}

fn register_libandroid(emulator: &mut AndroidEmulator<'static, ()>) {
    let svc = emulator.svc_memory_mut();
    let mut symbol = HashMap::new();
    symbol.insert(
        "ASensorManager_getDefaultSensor".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorManager_getDefaultSensor", ret_one)),
    );
    symbol.insert(
        "ASensorManager_getInstance".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorManager_getInstance", ret_one)),
    );
    symbol.insert(
        "ALooper_pollOnce".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ALooper_pollOnce", ret_zero)),
    );
    symbol.insert(
        "ASensorManager_createEventQueue".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorManager_createEventQueue", ret_one)),
    );
    symbol.insert(
        "ASensorManager_destroyEventQueue".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorManager_destroyEventQueue", ret_zero)),
    );
    symbol.insert(
        "ASensorEventQueue_getEvents".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorEventQueue_getEvents", ret_zero)),
    );
    symbol.insert(
        "ALooper_prepare".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ALooper_prepare", ret_one)),
    );
    symbol.insert(
        "ALooper_forThread".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ALooper_forThread", ret_one)),
    );
    symbol.insert(
        "ASensorEventQueue_enableSensor".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorEventQueue_enableSensor", ret_zero)),
    );
    symbol.insert(
        "ASensorEventQueue_disableSensor".to_string(),
        svc.register_svc(SimpleArm64Svc::new("ASensorEventQueue_disableSensor", ret_zero)),
    );
    let _ = emulator
        .memory()
        .load_virtual_module("libandroid.so".to_string(), symbol);
}

fn register_libjnigraphics(emulator: &mut AndroidEmulator<'static, ()>) {
    let svc = emulator.svc_memory_mut();
    let mut symbol = HashMap::new();
    symbol.insert(
        "AndroidBitmap_getInfo".to_string(),
        svc.register_svc(SimpleArm64Svc::new("AndroidBitmap_getInfo", ret_zero)),
    );
    symbol.insert(
        "AndroidBitmap_lockPixels".to_string(),
        svc.register_svc(SimpleArm64Svc::new("AndroidBitmap_lockPixels", ret_zero)),
    );
    symbol.insert(
        "AndroidBitmap_unlockPixels".to_string(),
        svc.register_svc(SimpleArm64Svc::new("AndroidBitmap_unlockPixels", ret_zero)),
    );
    let _ = emulator
        .memory()
        .load_virtual_module("libjnigraphics.so".to_string(), symbol);
}

fn ret_zero<T: Clone>(_name: &str, _emu: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(0)
}

fn ret_one<T: Clone>(_name: &str, _emu: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(1)
}

fn install_file_resolver(emulator: &mut AndroidEmulator<'static, ()>, resources: &Resources) {
    let apk = resources.apk_file.clone();
    let so_metasec = resources.so_metasec_file.clone();
    let so_c_share = resources.so_c_share_file.clone();
    let msdata = resources.msdata_file.clone();

    emulator.get_file_system().set_file_resolver(Box::new(move |_fs, path, flags, _mode| {
        if path.contains("libmetasec_ml.so") {
            return Some(FileIO::File(LinuxFileIO::new(
                so_metasec.to_string_lossy().as_ref(),
                path,
                flags.bits(),
                APP_UID,
                StMode::APP_FILE,
            )));
        }

        if path.contains("libc++_shared.so") {
            return Some(FileIO::File(LinuxFileIO::new(
                so_c_share.to_string_lossy().as_ref(),
                path,
                flags.bits(),
                APP_UID,
                StMode::APP_FILE,
            )));
        }

        if path == APK_INSTALL_PATH {
            return Some(FileIO::File(LinuxFileIO::new(
                apk.to_string_lossy().as_ref(),
                path,
                flags.bits(),
                APP_UID,
                StMode::APP_FILE,
            )));
        }

        if path == MSDATA_VFS_PATH {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&msdata)
                .ok()?;
            return Some(FileIO::File(LinuxFileIO::new_with_file(
                file,
                path,
                flags.bits(),
                APP_UID,
                StMode::APP_FILE,
            )));
        }

        if path == "/data/system" || path == "/data/app" || path == "/sdcard/android" || path == DATA_USER_DIR || path == DATA_FILES_DIR {
            return Some(FileIO::Direction(Direction::new(VecDeque::new(), path)));
        }

        None
    }));
}
