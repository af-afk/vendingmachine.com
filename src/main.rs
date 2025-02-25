#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

#[allow(unused)]
fn main() {}
