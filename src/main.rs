#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod capture;
mod config;
mod i18n;
mod input;
mod log;
mod render;
mod settings;
mod tray;
mod update;

use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::HiDpi::*;
use windows::core::w;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// `git describe` of the source the binary was built from, e.g. `v0.1.0` or `v0.1.0-3-gabc1234-dirty`.
pub const BUILD: &str = env!("BEST_LOUPE_BUILD");

fn main() {
    log::init();
    std::panic::set_hook(Box::new(|info| log!("PANIC: {info}")));

    unsafe {
        // The manifest already declares PerMonitorV2; this is a fallback if it was not embedded.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let _mutex = unsafe { CreateMutexW(None, true, w!("Local\\best_loupe_single_instance")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // Launching it again (e.g. from the Start menu) opens the running instance's settings.
        let tab = if std::env::args().any(|a| a == "--about") { settings::TAB_ABOUT } else { settings::TAB_LENS };
        tray::signal_running_instance(tab);
        return;
    }

    let cfg = config::load();
    log!("best_loupe {VERSION} ({BUILD}) starting: {cfg:?}");

    let wake = unsafe { CreateEventW(None, false, false, None) }.expect("CreateEvent");
    input::spawn(wake);
    update::spawn();
    let wake_raw = wake.0 as usize;
    std::thread::Builder::new()
        .name("render".into())
        .spawn(move || render::run(HANDLE(wake_raw as *mut _)))
        .expect("spawn render thread");

    if let Err(e) = tray::run() {
        log!("tray failed: {e}");
    }
    log!("exiting");
    std::process::exit(0);
}
