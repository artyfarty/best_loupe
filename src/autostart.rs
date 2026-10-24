//! "Start with Windows": `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\best_loupe`.
//! Explorer starts Run entries via ShellExecute, which uiAccess builds require.

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::*;
use windows::core::{HSTRING, w};

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: windows::core::PCWSTR = w!("best_loupe");

pub fn is_enabled() -> bool {
    let mut size = 0u32;
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    r == ERROR_SUCCESS
}

pub fn set_enabled(enabled: bool) {
    unsafe {
        if enabled {
            let Ok(exe) = std::env::current_exe() else { return };
            let cmd: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain(Some(0)).collect();
            let r = RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                VALUE,
                REG_SZ.0,
                Some(cmd.as_ptr() as *const _),
                (cmd.len() * 2) as u32,
            );
            crate::log!("autostart: enable -> {:?} ({})", r, HSTRING::from(exe.as_os_str()));
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE);
            crate::log!("autostart: disable -> {r:?}");
        }
    }
}
