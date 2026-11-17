#![no_std]
#![no_main]

unsafe extern "Rust" {
    fn putch(c: u16) -> u16;
}

#[inline(never)]
fn offsets(words: &[u16; 4], rows: &[[u8; 3]; 3], n: isize) -> bool {
    // Called with n=2. All addresses remain inside their original arrays.
    let unit = &() as *const ();
    unsafe {
        let third = rows.as_ptr().offset(n);
        let back = third.offset(-1);
        *words.as_ptr().offset(n) == 0x6789
            && (*third)[1] == 8
            && (*back)[1] == 5
            && *rows.as_ptr().cast::<u8>().offset(n) == 3
            && unit.offset(n) == unit
    }
}

#[unsafe(no_mangle)]
pub fn main16() -> u16 {
    let words = [0x1020, 0x2345, 0x6789, 0xffff];
    let rows = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let pass = offsets(&words, &rows, 2);
    let message: &[u8] = if pass {
        b"OFFSET PASS\r\n"
    } else {
        b"OFFSET FAIL\r\n"
    };
    let mut i = 0;
    while i < message.len() {
        unsafe {
            putch(u16::from(message[i]));
        }
        i += 1;
    }
    if pass { 0 } else { 1 }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
