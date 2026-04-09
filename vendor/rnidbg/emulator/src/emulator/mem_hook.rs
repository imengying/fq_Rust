use crate::backend::Backend;
use crate::emulator::AndroidEmulator;
use crate::pointer::VMPointer;
use log::error;
use std::process::exit;
#[cfg(feature = "unicorn_backend")]
use unicorn_engine::unicorn_const::{HookType, MemType, Permission};
#[cfg(feature = "unicorn_backend")]
use unicorn_engine::RegisterARM64::*;
#[cfg(feature = "unicorn_backend")]
use unicorn_engine::{RegisterARM64, Unicorn};

#[cfg(feature = "unicorn_backend")]
const PAGE_SIZE: u64 = 0x1000;
#[cfg(feature = "unicorn_backend")]
const TBI_MASK: u64 = 0x00ff_ffff_ffff_ffff;

#[cfg(feature = "unicorn_backend")]
fn mirror_tagged_page<T: Clone>(backend: &mut Unicorn<T>, addr: u64) -> bool {
    let untagged = addr & TBI_MASK;
    if untagged == addr {
        return false;
    }

    let tagged_page = addr & !(PAGE_SIZE - 1);
    let untagged_page = untagged & !(PAGE_SIZE - 1);
    let mut page = vec![0u8; PAGE_SIZE as usize];
    if backend.mem_read(untagged_page, &mut page).is_err() {
        return false;
    }
    if backend
        .mem_map(tagged_page, PAGE_SIZE as usize, Permission::ALL)
        .is_err()
    {
        return false;
    }
    if backend.mem_write(tagged_page, &page).is_err() {
        return false;
    }
    true
}

#[cfg(feature = "unicorn_backend")]
fn mem_hook_unmapped_unicorn<T: Clone>(
    hook_type: HookType,
    backend: &mut Unicorn<T>,
    mem_type: MemType,
    addr: u64,
    size: usize,
    value: i64,
) -> bool {
    // For reads from very low addresses (e.g. NULL pointer dereference from locale code),
    // map a zero-filled page so the read returns 0 and execution continues.
    // This is necessary because bionic libc's locale functions may dereference a NULL/small
    // locale_t pointer during init when TLS_TPREL relocations are approximated.
    if hook_type.contains(HookType::MEM_READ_UNMAPPED) && addr < 0x1000 {
        let _ = backend.mem_map(0, PAGE_SIZE as usize, Permission::READ);
        return true; // retry the read - it will now read 0
    }
    if mirror_tagged_page(backend, addr) {
        return true;
    }
    error!(
        "{:?}::{:?}  memory failed: address=0x{:X}, size={}, value=0x{:X}, LR=0x{:X}",
        hook_type,
        mem_type,
        addr,
        size,
        value,
        backend.reg_read(RegisterARM64::LR).unwrap()
    );
    backend.dump_context(addr, size);
    false
}

//noinspection DuplicatedCode
pub fn register_mem_err_handler<T: Clone>(backend: Backend<T>) {
    #[cfg(feature = "unicorn_backend")]
    if let Backend::Unicorn(unicorn) = backend {
        unicorn
            .add_mem_hook(
                HookType::MEM_READ_UNMAPPED,
                1,
                0,
                |backend: &mut Unicorn<'_, T>,
                 mem_type: MemType,
                 addr: u64,
                 size: usize,
                 value: i64| {
                    mem_hook_unmapped_unicorn(
                        HookType::MEM_READ_UNMAPPED,
                        backend,
                        mem_type,
                        addr,
                        size,
                        value,
                    )
                },
            )
            .expect("failed to add MEM_READ_UNMAPPED hook");
        unicorn
            .add_mem_hook(
                HookType::MEM_WRITE_UNMAPPED,
                1,
                0,
                |backend: &mut Unicorn<'_, T>,
                 mem_type: MemType,
                 addr: u64,
                 size: usize,
                 value: i64| {
                    mem_hook_unmapped_unicorn(
                        HookType::MEM_WRITE_UNMAPPED,
                        backend,
                        mem_type,
                        addr,
                        size,
                        value,
                    )
                },
            )
            .expect("failed to add MEM_WRITE_UNMAPPED hook");
        unicorn
            .add_mem_hook(
                HookType::MEM_FETCH_UNMAPPED,
                1,
                0,
                |backend: &mut Unicorn<'_, T>,
                 mem_type: MemType,
                 addr: u64,
                 size: usize,
                 value: i64| {
                    mem_hook_unmapped_unicorn(
                        HookType::MEM_FETCH_UNMAPPED,
                        backend,
                        mem_type,
                        addr,
                        size,
                        value,
                    )
                },
            )
            .expect("failed to add MEM_FETCH_UNMAPPED hook");
        unicorn.add_mem_hook(HookType::MEM_INVALID, 1, 0, |backend: &mut Unicorn<'_, T>, mem_type: MemType, addr: u64, size: usize, value: i64| {
            error!("MEM_INVALID::{:?}  memory failed: address=0x{:X}, size={}, value=0x{:X}, LR=0x{:X}", mem_type, addr, size, value, backend.reg_read(RegisterARM64::LR).unwrap());
            backend.emu_stop().unwrap();
            return false;
        }).expect("failed to add MEM_INVALID hook");
        return;
    }

    #[cfg(feature = "dynarmic_backend")]
    if let Backend::Dynarmic(dynarmic) = backend {
        return;
    }

    unreachable!("Not supported backend: register_mem_err_handler")
}
