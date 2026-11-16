#![no_std]

unsafe extern "Rust" {
    fn putch(value: u16) -> u16;
}

#[inline(never)]
fn triangle(limit: u16) -> u16 {
    let mut n = limit;
    let mut sum = 0;
    while n != 0 {
        sum += n;
        n -= 1;
    }
    sum
}

#[unsafe(no_mangle)]
pub fn main16() -> u16 {
    let result = triangle(12);
    if result != 78 {
        return 1;
    }
    // DOS runtime accepts the character in the low byte.
    unsafe {
        putch(48 + result / 10);
        putch(48 + result % 10);
        putch(13);
        putch(10);
    }
    0
}
