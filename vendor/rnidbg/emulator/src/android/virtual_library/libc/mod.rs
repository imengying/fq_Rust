//__system_property_get
//__system_property_find
//__system_property_read

use std::marker::PhantomData;
use std::rc::Rc;
use log::info;
use crate::backend::RegisterARM64;
use crate::emulator::{AndroidEmulator, VMPointer};
use crate::memory::svc_memory::{HookListener, SimpleArm64Svc, SvcCallResult, SvcMemory};

pub(super) mod system_properties;
mod memory;
mod string;

pub struct Libc<'a, T> {
    system_property_service: Option<Rc<Box<dyn Fn(&str) -> Option<String>>>>,



    pd: PhantomData<&'a T>,
}

fn return_zero<T: Clone>(_: &str, _: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(0)
}

fn pthread_self_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(emu.get_current_pid() as i64)
}

fn pthread_equal_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let lhs = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let rhs = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
    SvcCallResult::RET((lhs == rhs) as i64)
}

fn pthread_key_create_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let key_ptr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if key_ptr != 0 {
        let key_ptr = VMPointer::new(key_ptr, 0, emu.backend.clone());
        let _ = key_ptr.write_i32_with_offset(0, 1);
    }
    SvcCallResult::RET(0)
}

fn pthread_once_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let once_ptr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if once_ptr != 0 {
        let once_ptr = VMPointer::new(once_ptr, 0, emu.backend.clone());
        let _ = once_ptr.write_i32_with_offset(0, 1);
    }
    SvcCallResult::RET(0)
}

impl<T: Clone> Libc<'_, T> {
    pub fn new<'a>() -> Libc<'a, T> {
        Libc {
            system_property_service: None,
            pd: PhantomData
        }
    }

    pub fn set_system_property_service(&mut self, service: Rc<Box<dyn Fn(&str) -> Option<String>>>) {
        self.system_property_service = Some(service);
    }
}

impl<'a, T: Clone> HookListener<'a, T> for Libc<'a, T> {
    fn hook(&self, emu: &AndroidEmulator<'a, T>, lib_name: String, symbol_name: String, old: u64) -> u64 {
        if lib_name != "libc.so" {
            return 0
        }
        if option_env!("SHOW_LIBC_TRY_LINK") == Some("1") {
            info!("[libc.so] link {}, old=0x{:X}", symbol_name, old)
        }
        let svc = &mut emu.inner_mut().svc_memory;
        let entry = match symbol_name.as_str() {
            "__system_property_get" => svc.register_svc(Box::new(system_properties::SystemPropertyGet::new(self.system_property_service.clone()))),
            "__system_property_find" => svc.register_svc(Box::new(system_properties::SystemPropertyFind::new(self.system_property_service.clone()))),
            "__system_property_read" => svc.register_svc(Box::new(system_properties::SystemPropertyRead::new(self.system_property_service.clone()))),
            "__cxa_finalize" => svc.register_svc(SimpleArm64Svc::new("__cxa_finalize", return_zero)),
            "__register_atfork" => svc.register_svc(SimpleArm64Svc::new("__register_atfork", return_zero)),
            "__cxa_atexit" => svc.register_svc(SimpleArm64Svc::new("__cxa_atexit", return_zero)),
            "__cxa_thread_atexit_impl" => svc.register_svc(SimpleArm64Svc::new("__cxa_thread_atexit_impl", return_zero)),
            "pthread_atfork" => svc.register_svc(SimpleArm64Svc::new("pthread_atfork", return_zero)),
            "pthread_self" | "pthread_gettid_np" => {
                svc.register_svc(SimpleArm64Svc::new(symbol_name.as_str(), pthread_self_stub))
            }
            "pthread_equal" => svc.register_svc(SimpleArm64Svc::new("pthread_equal", pthread_equal_stub)),
            "pthread_key_create" => {
                svc.register_svc(SimpleArm64Svc::new("pthread_key_create", pthread_key_create_stub))
            }
            "pthread_key_delete"
            | "pthread_setspecific"
            | "pthread_getspecific"
            | "pthread_detach"
            | "pthread_join"
            | "pthread_sigmask"
            | "pthread_mutex_init"
            | "pthread_mutex_destroy"
            | "pthread_mutex_trylock"
            | "pthread_mutex_lock"
            | "pthread_mutex_unlock"
            | "pthread_mutexattr_init"
            | "pthread_mutexattr_destroy"
            | "pthread_mutexattr_settype"
            | "pthread_mutexattr_gettype"
            | "pthread_mutexattr_setpshared"
            | "pthread_mutexattr_getpshared"
            | "pthread_cond_init"
            | "pthread_cond_destroy"
            | "pthread_cond_wait"
            | "pthread_cond_timedwait"
            | "pthread_cond_signal"
            | "pthread_cond_broadcast"
            | "pthread_condattr_init"
            | "pthread_condattr_destroy"
            | "pthread_condattr_setpshared"
            | "pthread_condattr_getpshared"
            | "pthread_condattr_setclock"
            | "pthread_condattr_getclock" => {
                svc.register_svc(SimpleArm64Svc::new(symbol_name.as_str(), return_zero))
            }
            "pthread_once" => svc.register_svc(SimpleArm64Svc::new("pthread_once", pthread_once_stub)),
            "strcmp" => svc.register_svc(Box::new(string::StrCmp)),
            "strncmp" => svc.register_svc(Box::new(string::StrNCmp)),
            "strcasecmp" => svc.register_svc(Box::new(string::StrCaseCmp)),
            "strncasecmp" => svc.register_svc(Box::new(string::StrNCasCmp)),
            _ => 0
        };


        entry
    }
}
