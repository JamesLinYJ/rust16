#![no_std]
#![no_main]

unsafe extern "Rust" {
    fn putch(c: u16) -> u16;
}

#[inline(never)]
fn change(bytes: &mut [u8], start: usize) -> bool {
    let Some(part) = bytes.get_mut(start..start + 2) else {
        return false;
    };
    part[0] = 41;
    part[1] = 42;
    true
}
#[inline(never)]
fn verify(bytes: &[u8], start: usize) -> bool {
    let Some(part) = bytes.get(start..start + 2) else {
        return false;
    };
    part.len() == 2
        && part[0] == 41
        && part[1] == 42
        && bytes.get(bytes.len()..).is_some()
        && bytes.get(bytes.len() + 1..).is_none()
}
#[inline(never)]
fn words(a: &mut [u32; 3], b: &mut [u32; 3]) {
    *a = [0u32; 3];
    *b = [0x1234_abcd; 3];
}
#[unsafe(no_mangle)]
pub fn main16() -> u16 {
    let mut data = [7u8; 5];
    let mut zero = [7, 8, 9];
    let mut pat = [5, 6, 7];
    words(&mut zero, &mut pat);
    let mut ok = change(&mut data, 2)
        && verify(&data, 2)
        && !change(&mut data, 4)
        && data[0] == 7
        && data[1] == 7
        && data[4] == 7;
    let mut i = 0;
    while i < 3 {
        ok = ok && zero[i] == 0 && pat[i] == 0x1234_abcd;
        i += 1;
    }
    let msg: &[u8] = if ok {
        b"SLICES PASS\r\n"
    } else {
        b"SLICES FAIL\r\n"
    };
    i = 0;
    while i < msg.len() {
        unsafe {
            putch(u16::from(msg[i]));
        }
        i += 1;
    }
    u16::from(!ok)
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
