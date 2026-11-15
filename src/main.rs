#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("ZelenkaMusic currently supports Windows 10 and Windows 11 only.");

mod app;
use zelenka_music::{audio, database, library, playlists, settings};

slint::include_modules!();

fn main() {
    if let Err(error) = app::run() {
        // A startup failure is visible even for the release GUI subsystem.
        let message: Vec<u16> = format!("Не удалось запустить ZelenkaMusic.\n\n{error:#}\0")
            .encode_utf16()
            .collect();
        let title: Vec<u16> = "ZelenkaMusic\0".encode_utf16().collect();
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                flags: u32,
            ) -> i32;
        }
        unsafe {
            MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), 0x10);
        }
    }
}
