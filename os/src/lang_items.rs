//! The panic handler

use crate::sbi::shutdown;
use core::panic::PanicInfo;
use log::*;

#[panic_handler] // 用于标记核心库core中的 panic! 宏要对接的函数
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "[kernel] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        error!("[kernel] Panicked: {}", info.message().unwrap());
    }
    shutdown(true)
}
