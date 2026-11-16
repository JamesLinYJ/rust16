//! Official rustc entry for cross-compiling the DOS-hosted tools.
#![no_std]
#![allow(
    dead_code,
    unused_mut,
    unused_parens,
    unused_assignments,
    unreachable_code
)]

mod rt {
    unsafe extern "Rust" {
        pub fn fetch(bank: u16, index: u16) -> u16;
        pub fn save(bank: u16, index: u16, value: u16) -> u16;
        pub fn input(arg: u16, bank: u16) -> u16;
        pub fn append(arg:u16,bank:u16,len:u16)->u16;
        pub fn output(arg: u16, bank: u16, len: u16) -> u16;
        pub fn arg(number: u16, index: u16) -> u16;
        pub fn putch(byte: u16) -> u16;
        pub fn fail(code: u16) -> !;
    }
}

// These declarations match the far-call ABI implemented in src/runtime.rs.
// The runtime owns every memory bank, file handle and argument buffer.
macro_rules! intrinsic {
    ($name:ident($($arg:ident),*) -> $ret:ty) => {
        #[inline(always)]
        fn $name($($arg:u16),*) -> $ret { unsafe { rt::$name($($arg),*) } }
    }
}
intrinsic!(fetch(bank,index)->u16);
intrinsic!(save(bank,index,value)->u16);
intrinsic!(input(arg,bank)->u16);
intrinsic!(append(arg,bank,len)->u16);
intrinsic!(output(arg,bank,len)->u16);
intrinsic!(arg(number,index)->u16);
intrinsic!(putch(byte)->u16);
intrinsic!(fail(code)->!);

mod program {
    use super::*;
    include!("../dos/common.rs");
    #[cfg(dos_cc)]
    include!("../dos/compile.rs");
    #[cfg(dos_ld)]
    include!("../dos/link.rs");
}
#[unsafe(no_mangle)]
pub fn main16() -> u16 {
    program::main16()
}
