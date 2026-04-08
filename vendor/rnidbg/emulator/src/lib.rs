#![allow(
    dead_code,
    irrefutable_let_patterns,
    private_interfaces,
    unreachable_patterns,
    unnecessary_transmutes,
    unused_assignments,
    unused_comparisons,
    unused_mut,
    unused_unsafe,
    unused_variables
)]

pub mod android;
pub mod emulator;
pub mod keystone;
pub mod linux;
pub mod memory;
pub mod pointer;
pub(crate) mod tool;
pub(crate) mod elf;
mod backend;

pub use emulator::AndroidEmulator;
pub use tool::UnicornArg;
