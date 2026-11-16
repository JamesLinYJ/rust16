#![no_std]
#![no_main]
#[allow(unused_attributes)]
#[path = "../../mir.rs"]
mod demo;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
