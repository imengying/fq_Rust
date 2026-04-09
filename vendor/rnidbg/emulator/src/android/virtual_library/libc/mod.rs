//__system_property_get
//__system_property_find
//__system_property_read

use crate::backend::RegisterARM64;
use crate::emulator::{AndroidEmulator, VMPointer, CUSTOM_SVC_SYSCALL_NUMBER};
use crate::keystone;
use crate::memory::svc_memory::{Arm64Svc, HookListener, SimpleArm64Svc, SvcCallResult, SvcMemory};
use log::info;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

mod memory;
mod string;
pub(super) mod system_properties;

pub struct Libc<'a, T> {
    system_property_service: Option<Rc<Box<dyn Fn(&str) -> Option<String>>>>,
    pthread_tls_state: Rc<UnsafeCell<PthreadTlsState>>,
    pthread_once_state: Rc<UnsafeCell<HashMap<u64, bool>>>,
    alloc_state: Rc<UnsafeCell<AllocState>>,
    pd: PhantomData<&'a T>,
}

struct PthreadOnceSvc {
    state: Rc<UnsafeCell<HashMap<u64, bool>>>,
}
struct PthreadKeyCreateSvc {
    state: Rc<UnsafeCell<PthreadTlsState>>,
}
struct PthreadKeyDeleteSvc {
    state: Rc<UnsafeCell<PthreadTlsState>>,
}
struct PthreadSetSpecificSvc {
    state: Rc<UnsafeCell<PthreadTlsState>>,
}
struct PthreadGetSpecificSvc {
    state: Rc<UnsafeCell<PthreadTlsState>>,
}
struct AllocSvc<T: Clone> {
    name: &'static str,
    state: Rc<UnsafeCell<AllocState>>,
    handler: fn(&str, &AndroidEmulator<T>, &Rc<UnsafeCell<AllocState>>) -> SvcCallResult,
}

#[derive(Default)]
struct PthreadTlsState {
    next_key: u32,
    values: HashMap<(u32, u32), u64>,
}

#[derive(Default)]
struct AllocState {
    arena_base: u64,
    arena_size: usize,
    arena_offset: usize,
    allocations: HashMap<u64, (usize, bool)>,
}

static PTHREAD_ONCE_LOG_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static PTHREAD_TLS_LOG_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn debug_pthread_tls() -> bool {
    std::env::var("FQ_DEBUG_SIGNER_PTHREAD").ok().as_deref() == Some("1")
}

const PTHREAD_ONCE_INIT: i32 = 0;
const PTHREAD_ONCE_DONE: i32 = 2;
const PTHREAD_MUTEX_TYPE_OFFSET: u64 = 4;
const PTHREAD_MUTEX_PSHARED_OFFSET: u64 = 8;
const PTHREAD_MUTEX_SIZE: usize = 0x28;

fn return_zero<T: Clone>(_: &str, _: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(0)
}

fn current_thread_addr<T: Clone>(emu: &AndroidEmulator<T>) -> u64 {
    let tls_addr = emu.backend.reg_read(RegisterARM64::TPIDR_EL0).unwrap_or(0);
    if tls_addr == 0 {
        return emu.get_current_pid() as u64;
    }
    let tls = VMPointer::new(tls_addr, 0, emu.backend.clone());
    tls.read_u64_with_offset(8).unwrap_or(tls_addr)
}

fn pthread_self_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(current_thread_addr(emu) as i64)
}

fn pthread_gettid_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    SvcCallResult::RET(emu.get_current_pid() as i64)
}

fn pthread_equal_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let lhs = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let rhs = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
    SvcCallResult::RET((lhs == rhs) as i64)
}

fn alloc_malloc<T: Clone>(
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
    size: usize,
) -> SvcCallResult {
    let size = size.max(1);
    let aligned = (size + 15) & !15;
    let state = unsafe { &mut *state.get() };

    if state.arena_base == 0 {
        if let Ok(ptr) = emu.falloc(16 * 1024 * 1024, false) {
            state.arena_base = ptr.addr;
            state.arena_size = ptr.size;
            state.arena_offset = 0;
        }
    }

    if state.arena_base != 0 && state.arena_offset + aligned <= state.arena_size {
        let addr = state.arena_base + state.arena_offset as u64;
        state.arena_offset += aligned;
        state.allocations.insert(addr, (aligned, true));
        return SvcCallResult::RET(addr as i64);
    }

    match emu.falloc(aligned, false) {
        Ok(ptr) => {
            state.allocations.insert(ptr.addr, (aligned, false));
            SvcCallResult::RET(ptr.addr as i64)
        }
        Err(_) => SvcCallResult::RET(0),
    }
}

fn alloc_free<T: Clone>(
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
    addr: u64,
) -> SvcCallResult {
    if addr == 0 {
        return SvcCallResult::RET(0);
    }
    if let Some((size, from_arena)) = unsafe { &mut *state.get() }.allocations.remove(&addr) {
        if !from_arena {
            let _ = emu.ffree(addr, size);
        }
    }
    SvcCallResult::RET(0)
}

fn malloc_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let size = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0) as usize;
    alloc_malloc(emu, state, size)
}

fn calloc_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let nmemb = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0) as usize;
    let size = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0) as usize;
    alloc_malloc(emu, state, nmemb.saturating_mul(size))
}

fn free_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    alloc_free(emu, state, addr)
}

fn realloc_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let old_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let new_size = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0) as usize;
    if old_addr == 0 {
        return alloc_malloc(emu, state, new_size);
    }
    if new_size == 0 {
        return alloc_free(emu, state, old_addr);
    }

    let old_size = unsafe { &*state.get() }
        .allocations
        .get(&old_addr)
        .map(|(size, _)| *size)
        .unwrap_or(0);
    let new_ptr = match emu.falloc(new_size.max(1), false) {
        Ok(ptr) => ptr,
        Err(_) => return SvcCallResult::RET(0),
    };
    if old_size > 0 {
        let copy_size = old_size.min(new_size);
        let mut buf = vec![0u8; copy_size];
        if emu.backend.mem_read(old_addr, &mut buf).is_ok() {
            let _ = emu.backend.mem_write(new_ptr.addr, &buf);
        }
    }
    unsafe { &mut *state.get() }
        .allocations
        .insert(new_ptr.addr, (new_size.max(1), false));
    let _ = alloc_free(emu, state, old_addr);
    SvcCallResult::RET(new_ptr.addr as i64)
}

fn malloc_usable_size_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let size = unsafe { &*state.get() }
        .allocations
        .get(&addr)
        .map(|(size, _)| *size)
        .unwrap_or(0);
    SvcCallResult::RET(size as i64)
}

fn posix_memalign_stub<T: Clone>(
    _: &str,
    emu: &AndroidEmulator<T>,
    state: &Rc<UnsafeCell<AllocState>>,
) -> SvcCallResult {
    let out_ptr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let size = emu.backend.reg_read(RegisterARM64::X2).unwrap_or(0) as usize;
    let ret = alloc_malloc(emu, state, size);
    if let SvcCallResult::RET(addr) = ret {
        if out_ptr_addr != 0 {
            let out_ptr = VMPointer::new(out_ptr_addr, 0, emu.backend.clone());
            let _ = out_ptr.write_u64(addr as u64);
        }
        return SvcCallResult::RET(0);
    }
    SvcCallResult::RET(12)
}

fn pthread_mutexattr_init_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if attr_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let _ = attr.write_i32_with_offset(0, 0);
        let _ = attr.write_i32_with_offset(4, 0);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutexattr_destroy_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if attr_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let _ = attr.write_i32_with_offset(0, 0);
        let _ = attr.write_i32_with_offset(4, 0);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutexattr_settype_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let mutex_type = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0) as i32;
    if attr_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let _ = attr.write_i32_with_offset(0, mutex_type);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutexattr_gettype_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let out_addr = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
    if attr_addr != 0 && out_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let out = VMPointer::new(out_addr, 0, emu.backend.clone());
        let value = attr.read_i32_with_offset(0).unwrap_or(0);
        let _ = out.write_i32_with_offset(0, value);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutexattr_setpshared_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let shared = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0) as i32;
    if attr_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let _ = attr.write_i32_with_offset(4, shared);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutexattr_getpshared_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let attr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let out_addr = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
    if attr_addr != 0 && out_addr != 0 {
        let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
        let out = VMPointer::new(out_addr, 0, emu.backend.clone());
        let value = attr.read_i32_with_offset(4).unwrap_or(0);
        let _ = out.write_i32_with_offset(0, value);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutex_init_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let mutex_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    let attr_addr = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
    if mutex_addr != 0 {
        let mutex = VMPointer::new(mutex_addr, 0, emu.backend.clone());
        let _ = mutex.write_buf(vec![0u8; PTHREAD_MUTEX_SIZE]);
        if attr_addr != 0 {
            let attr = VMPointer::new(attr_addr, 0, emu.backend.clone());
            let mutex_type = attr.read_i32_with_offset(0).unwrap_or(0);
            let pshared = attr.read_i32_with_offset(4).unwrap_or(0);
            let _ = mutex.write_i32_with_offset(PTHREAD_MUTEX_TYPE_OFFSET, mutex_type);
            let _ = mutex.write_i32_with_offset(PTHREAD_MUTEX_PSHARED_OFFSET, pshared);
        }
    }
    SvcCallResult::RET(0)
}

fn pthread_mutex_destroy_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let mutex_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if mutex_addr != 0 {
        let mutex = VMPointer::new(mutex_addr, 0, emu.backend.clone());
        let _ = mutex.write_i32_with_offset(0, 0);
    }
    SvcCallResult::RET(0)
}

fn pthread_mutex_lock_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let mutex_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if mutex_addr != 0 {
        let mutex = VMPointer::new(mutex_addr, 0, emu.backend.clone());
        let _ = mutex.write_u64(current_thread_addr(emu));
    }
    SvcCallResult::RET(0)
}

fn pthread_mutex_trylock_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    pthread_mutex_lock_stub("pthread_mutex_trylock", emu)
}

fn pthread_mutex_unlock_stub<T: Clone>(_: &str, emu: &AndroidEmulator<T>) -> SvcCallResult {
    let mutex_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
    if mutex_addr != 0 {
        let mutex = VMPointer::new(mutex_addr, 0, emu.backend.clone());
        let _ = mutex.write_u64(0);
    }
    SvcCallResult::RET(0)
}

impl<T: Clone> Arm64Svc<T> for PthreadOnceSvc {
    fn name(&self) -> &str {
        "pthread_once"
    }

    fn on_register(&self, svc: &mut SvcMemory<T>, number: u32) -> u64 {
        let code = [
            "sub sp, sp, #0x10",
            "stp x29, x30, [sp]",
            &format!("mov x12, #0x{:x}", number),
            &format!("mov x16, #0x{:x}", CUSTOM_SVC_SYSCALL_NUMBER),
            "svc #0",
            "cmp x0, #0",
            "b.eq done",
            "blr x0",
            "mov x0, #0",
            "done:",
            "ldp x29, x30, [sp]",
            "add sp, sp, #0x10",
            "ret",
        ]
        .join("\n");
        let code = keystone::assemble_no_check(&code);
        let pointer = svc.allocate(code.len(), "pthread_once");
        pointer
            .write_buf(code)
            .expect("try register pthread_once svc failed");
        pointer.addr
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        let once_ptr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
        let init_routine = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);

        let callback = if once_ptr_addr == 0 || init_routine == 0 {
            0
        } else {
            let once_state = unsafe { &mut *self.state.get() };
            let once_ptr = VMPointer::new(once_ptr_addr, 0, emu.backend.clone());
            let state = once_ptr.read_i32_with_offset(0).unwrap_or(0);
            if debug_pthread_tls()
                && PTHREAD_ONCE_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 32
            {
                let caller = emu
                    .find_caller()
                    .map(|module_cell| {
                        let module = unsafe { &*module_cell.get() };
                        let lr = emu.get_lr().unwrap_or(0);
                        format!("{}@0x{:X}", module.name, lr - module.base)
                    })
                    .unwrap_or_else(|| "<unknown>".to_string());
                eprintln!(
                    "pthread_once once=0x{:X} state={} callback=0x{:X} caller={}",
                    once_ptr_addr, state, init_routine, caller
                );
            }
            if once_state.get(&once_ptr_addr).copied().unwrap_or(false) {
                return SvcCallResult::RET(0);
            }
            if state == PTHREAD_ONCE_INIT {
                once_state.insert(once_ptr_addr, true);
                if let Some(module_cell) = emu.memory().find_module_by_address(init_routine) {
                    let module = unsafe { &*module_cell.get() };
                    let _ = once_ptr.write_i32_with_offset(0, PTHREAD_ONCE_DONE);
                    info!(
                        "pthread_once init callback: once=0x{:X}, callback=0x{:X}, module={}, base=0x{:X}, size=0x{:X}",
                        once_ptr_addr,
                        init_routine,
                        module.name,
                        module.base,
                        module.size
                    );
                    init_routine
                } else {
                    let _ = once_ptr.write_i32_with_offset(0, PTHREAD_ONCE_DONE);
                    info!(
                        "pthread_once init callback outside loaded modules: once=0x{:X}, callback=0x{:X}",
                        once_ptr_addr,
                        init_routine
                    );
                    0
                }
            } else {
                0
            }
        };
        SvcCallResult::RET(callback as i64)
    }
}

impl<T: Clone> Arm64Svc<T> for PthreadKeyCreateSvc {
    fn name(&self) -> &str {
        "pthread_key_create"
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        let key_ptr_addr = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0);
        if key_ptr_addr == 0 {
            return SvcCallResult::RET(0);
        }

        let state = unsafe { &mut *self.state.get() };
        state.next_key = state.next_key.saturating_add(1).max(1);
        let key_ptr = VMPointer::new(key_ptr_addr, 0, emu.backend.clone());
        let _ = key_ptr.write_i32_with_offset(0, state.next_key as i32);
        if debug_pthread_tls()
            && PTHREAD_TLS_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 32
        {
            eprintln!(
                "pthread_key_create key_ptr=0x{:X} key={}",
                key_ptr_addr, state.next_key
            );
        }
        SvcCallResult::RET(0)
    }
}

impl<T: Clone> Arm64Svc<T> for PthreadKeyDeleteSvc {
    fn name(&self) -> &str {
        "pthread_key_delete"
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        let key = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0) as u32;
        let state = unsafe { &mut *self.state.get() };
        state
            .values
            .retain(|(_, current_key), _| *current_key != key);
        SvcCallResult::RET(0)
    }
}

impl<T: Clone> Arm64Svc<T> for PthreadSetSpecificSvc {
    fn name(&self) -> &str {
        "pthread_setspecific"
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        let key = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0) as u32;
        let value = emu.backend.reg_read(RegisterARM64::X1).unwrap_or(0);
        let tid = emu.get_current_pid();
        let state = unsafe { &mut *self.state.get() };
        state.values.insert((tid, key), value);
        if debug_pthread_tls()
            && PTHREAD_TLS_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 32
        {
            eprintln!(
                "pthread_setspecific tid={} key={} value=0x{:X}",
                tid, key, value
            );
        }
        SvcCallResult::RET(0)
    }
}

impl<T: Clone> Arm64Svc<T> for PthreadGetSpecificSvc {
    fn name(&self) -> &str {
        "pthread_getspecific"
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        let key = emu.backend.reg_read(RegisterARM64::X0).unwrap_or(0) as u32;
        let tid = emu.get_current_pid();
        let state = unsafe { &*self.state.get() };
        let value = state.values.get(&(tid, key)).copied().unwrap_or(0);
        if debug_pthread_tls()
            && PTHREAD_TLS_LOG_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 32
        {
            eprintln!(
                "pthread_getspecific tid={} key={} -> 0x{:X}",
                tid, key, value
            );
        }
        SvcCallResult::RET(value as i64)
    }
}

impl<T: Clone> Arm64Svc<T> for AllocSvc<T> {
    fn name(&self) -> &str {
        self.name
    }

    fn handle(&self, emu: &AndroidEmulator<T>) -> SvcCallResult {
        (self.handler)(self.name, emu, &self.state)
    }
}

impl<T: Clone> Libc<'_, T> {
    pub fn new<'a>() -> Libc<'a, T> {
        Libc {
            system_property_service: None,
            pthread_tls_state: Rc::new(UnsafeCell::new(PthreadTlsState::default())),
            pthread_once_state: Rc::new(UnsafeCell::new(HashMap::new())),
            alloc_state: Rc::new(UnsafeCell::new(AllocState::default())),
            pd: PhantomData,
        }
    }

    pub fn set_system_property_service(
        &mut self,
        service: Rc<Box<dyn Fn(&str) -> Option<String>>>,
    ) {
        self.system_property_service = Some(service);
    }
}

impl<'a, T: Clone> HookListener<'a, T> for Libc<'a, T> {
    fn hook(
        &self,
        emu: &AndroidEmulator<'a, T>,
        lib_name: String,
        symbol_name: String,
        old: u64,
    ) -> u64 {
        if lib_name != "libc.so" {
            return 0;
        }
        if option_env!("SHOW_LIBC_TRY_LINK") == Some("1") {
            info!("[libc.so] link {}, old=0x{:X}", symbol_name, old)
        }
        let svc = &mut emu.inner_mut().svc_memory;
        let pthread_tls_state = self.pthread_tls_state.clone();
        let pthread_once_state = self.pthread_once_state.clone();
        let alloc_state = self.alloc_state.clone();
        let entry = match symbol_name.as_str() {
            "__system_property_get" => svc.register_svc(Box::new(
                system_properties::SystemPropertyGet::new(self.system_property_service.clone()),
            )),
            "__system_property_find" => svc.register_svc(Box::new(
                system_properties::SystemPropertyFind::new(self.system_property_service.clone()),
            )),
            "__system_property_read" => svc.register_svc(Box::new(
                system_properties::SystemPropertyRead::new(self.system_property_service.clone()),
            )),
            "__cxa_finalize" => {
                svc.register_svc(SimpleArm64Svc::new("__cxa_finalize", return_zero))
            }
            "__register_atfork" => {
                svc.register_svc(SimpleArm64Svc::new("__register_atfork", return_zero))
            }
            "__cxa_atexit" => svc.register_svc(SimpleArm64Svc::new("__cxa_atexit", return_zero)),
            "__cxa_thread_atexit_impl" => {
                svc.register_svc(SimpleArm64Svc::new("__cxa_thread_atexit_impl", return_zero))
            }
            "pthread_atfork" => {
                svc.register_svc(SimpleArm64Svc::new("pthread_atfork", return_zero))
            }
            "malloc" => svc.register_svc(Box::new(AllocSvc {
                name: "malloc",
                state: alloc_state.clone(),
                handler: malloc_stub,
            })),
            "calloc" => svc.register_svc(Box::new(AllocSvc {
                name: "calloc",
                state: alloc_state.clone(),
                handler: calloc_stub,
            })),
            "free" => svc.register_svc(Box::new(AllocSvc {
                name: "free",
                state: alloc_state.clone(),
                handler: free_stub,
            })),
            "realloc" => svc.register_svc(Box::new(AllocSvc {
                name: "realloc",
                state: alloc_state.clone(),
                handler: realloc_stub,
            })),
            "malloc_usable_size" => svc.register_svc(Box::new(AllocSvc {
                name: "malloc_usable_size",
                state: alloc_state.clone(),
                handler: malloc_usable_size_stub,
            })),
            "posix_memalign" => svc.register_svc(Box::new(AllocSvc {
                name: "posix_memalign",
                state: alloc_state.clone(),
                handler: posix_memalign_stub,
            })),
            "pthread_key_create" => svc.register_svc(Box::new(PthreadKeyCreateSvc {
                state: pthread_tls_state.clone(),
            })),
            "pthread_key_delete" => svc.register_svc(Box::new(PthreadKeyDeleteSvc {
                state: pthread_tls_state.clone(),
            })),
            "pthread_setspecific" => svc.register_svc(Box::new(PthreadSetSpecificSvc {
                state: pthread_tls_state.clone(),
            })),
            "pthread_getspecific" => svc.register_svc(Box::new(PthreadGetSpecificSvc {
                state: pthread_tls_state.clone(),
            })),
            "pthread_detach" | "pthread_join" => {
                svc.register_svc(SimpleArm64Svc::new(symbol_name.as_str(), return_zero))
            }
            "pthread_once" => svc.register_svc(Box::new(PthreadOnceSvc {
                state: pthread_once_state,
            })),
            "strcmp" => svc.register_svc(Box::new(string::StrCmp)),
            "strncmp" => svc.register_svc(Box::new(string::StrNCmp)),
            "strcasecmp" => svc.register_svc(Box::new(string::StrCaseCmp)),
            "strncasecmp" => svc.register_svc(Box::new(string::StrNCasCmp)),
            _ => 0,
        };

        entry
    }
}
