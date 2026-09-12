use std::{process::Command as StdCommand, sync::OnceLock};
use tokio::process::Command;

/// GUI Sprite Studio has no console. Spawning `agent.cmd` or `bash.exe` then
/// opens a visible window for every Cursor tool. Allocate one hidden console so
/// those console processes inherit it instead of flashing.
#[cfg(windows)]
fn hidden_console_mode() -> HiddenConsoleMode {
    static MODE: OnceLock<HiddenConsoleMode> = OnceLock::new();
    *MODE.get_or_init(|| {
        if attach_or_create_hidden_console() {
            HiddenConsoleMode::Inherit
        } else {
            HiddenConsoleMode::CreateNoWindow
        }
    })
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum HiddenConsoleMode {
    Inherit,
    CreateNoWindow,
}

#[cfg(windows)]
fn attach_or_create_hidden_console() -> bool {
    const SW_HIDE: i32 = 0;
    #[link(name = "kernel32")]
    extern "system" {
        fn AllocConsole() -> i32;
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }
    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: *mut core::ffi::c_void, cmd_show: i32) -> i32;
    }
    // SAFETY: AllocConsole / GetConsoleWindow / ShowWindow are process-wide
    // and called once from hidden_console_mode.
    unsafe {
        if !GetConsoleWindow().is_null() {
            // cargo test and some `tauri dev` sessions already own a console;
            // inherit it so we never hide the test runner.
            return true;
        }
        if AllocConsole() == 0 {
            return false;
        }
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
        true
    }
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
pub(crate) fn apply_std_headless_flags(command: &mut StdCommand) {
    use std::os::windows::process::CommandExt;
    match hidden_console_mode() {
        HiddenConsoleMode::Inherit => {}
        HiddenConsoleMode::CreateNoWindow => {
            command.creation_flags(CREATE_NO_WINDOW);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn apply_std_headless_flags(_command: &mut StdCommand) {}

#[cfg(windows)]
pub(crate) fn apply_tokio_headless_flags(command: &mut Command) {
    match hidden_console_mode() {
        HiddenConsoleMode::Inherit => {}
        HiddenConsoleMode::CreateNoWindow => {
            command.creation_flags(CREATE_NO_WINDOW);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn apply_tokio_headless_flags(_command: &mut Command) {}
