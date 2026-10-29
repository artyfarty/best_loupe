//! Tray icon + menu (Settings, About, update, Exit). Runs the main thread's message loop.

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{LIM_SMALL, LoadIconMetric};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, Result, w};

use crate::i18n::{self, t};
use crate::update::{self, Status};
use crate::{config, input, settings};

const WM_TRAY: u32 = WM_APP + 10;
/// Posted by a second instance: open settings on tab `wparam`.
const WM_OPEN_SETTINGS: u32 = WM_APP + 11;
/// Posted by `best_loupe.exe --quit`: exit (a uiAccess process cannot be killed from a normal one).
const WM_QUIT_REQUEST: u32 = WM_APP + 12;
const CLASS: PCWSTR = w!("best_loupe_tray");
const CMD_SETTINGS: usize = 1;
const CMD_ABOUT: usize = 2;
const CMD_UPDATE: usize = 3;
const CMD_EXIT: usize = 4;
const CMD_PAUSE: usize = 5;
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Version we already showed a balloon for (once per version per run).
    static NOTIFIED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn copy_wstr(dst: &mut [u16], s: &str) {
    let src: Vec<u16> = s.encode_utf16().collect();
    let n = src.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&src[..n]);
    dst[n] = 0;
}

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None).map(Into::into).unwrap_or_default() };
    // Resource 1 = normal icon, 2 = greyed-out "paused" icon.
    let id = if input::enabled() { 1 } else { 2 };
    let icon = unsafe { LoadIconMetric(Some(hinstance), PCWSTR(id as *const u16), LIM_SMALL) }
        .or_else(|_| unsafe { LoadIconW(None, IDI_APPLICATION) })
        .unwrap_or_default();
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    let tip = t().tip.replace("{hotkey}", &i18n::hotkey_label(&config::current()));
    let tip = if input::enabled() { tip } else { format!("{} — {tip}", t().tip_paused) };
    copy_wstr(&mut nid.szTip, &format!("{tip} ({})", crate::VERSION));
    nid
}

fn add_icon(hwnd: HWND) {
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &icon_data(hwnd)).as_bool() {
            crate::log!("tray: Shell_NotifyIcon(NIM_ADD) failed");
        }
    }
}

fn show_balloon(hwnd: HWND, version: &str) {
    let mut nid = icon_data(hwnd);
    nid.uFlags |= NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    copy_wstr(&mut nid.szInfoTitle, &t().upd_balloon_title.replace("{version}", version));
    copy_wstr(&mut nid.szInfo, t().upd_balloon_text);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

fn open_release_page() {
    if let Status::Available { url, .. } = update::status() {
        unsafe {
            ShellExecuteW(None, w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        // Refresh the tooltip (hotkey may have changed in settings).
        let _ = Shell_NotifyIconW(NIM_MODIFY, &icon_data(hwnd));
        let Ok(menu) = CreatePopupMenu() else { return };
        let s = t();
        let pause_flags = if input::enabled() { MF_STRING } else { MF_STRING | MF_CHECKED };
        let _ = AppendMenuW(menu, pause_flags, CMD_PAUSE, &HSTRING::from(s.menu_pause));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS, &HSTRING::from(s.menu_settings));
        let _ = AppendMenuW(menu, MF_STRING, CMD_ABOUT, &HSTRING::from(s.menu_about));
        if let Status::Available { version, .. } = update::status() {
            let _ = AppendMenuW(menu, MF_STRING, CMD_UPDATE, &HSTRING::from(s.menu_update.replace("{version}", &version)));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, &HSTRING::from(s.menu_exit));
        let _ = SetMenuDefaultItem(menu, CMD_SETTINGS as u32, 0);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, Some(0), hwnd, None);
        let _ = DestroyMenu(menu);
        match cmd.0 as usize {
            CMD_SETTINGS => settings::open(settings::TAB_LENS),
            CMD_ABOUT => settings::open(settings::TAB_ABOUT),
            CMD_UPDATE => open_release_page(),
            CMD_PAUSE => {
                input::set_enabled(!input::enabled());
                crate::log!("tray: lens {}", if input::enabled() { "resumed" } else { "paused" });
                let _ = Shell_NotifyIconW(NIM_MODIFY, &icon_data(hwnd));
            }
            CMD_EXIT => {
                let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
                PostQuitMessage(0);
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAY => {
                match (lparam.0 & 0xffff) as u32 {
                    WM_LBUTTONUP => settings::open(settings::TAB_LENS),
                    WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                    NIN_BALLOONUSERCLICK => open_release_page(),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_QUIT_REQUEST => {
                crate::log!("tray: quit requested by another instance");
                let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_OPEN_SETTINGS => {
                settings::open(wparam.0);
                LRESULT(0)
            }
            update::WM_UPDATE_STATUS => {
                if let Status::Available { version, .. } = update::status() {
                    let first = NOTIFIED.with(|n| {
                        let mut n = n.borrow_mut();
                        let first = *n != version;
                        *n = version.clone();
                        first
                    });
                    if first {
                        show_balloon(hwnd, &version);
                    }
                }
                LRESULT(0)
            }
            _ if msg != 0 && msg == TASKBAR_CREATED.load(Relaxed) => {
                add_icon(hwnd); // Explorer restarted
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Asks a running instance to exit; returns whether one was found.
pub fn quit_running_instance() -> bool {
    unsafe {
        match FindWindowW(CLASS, PCWSTR::null()) {
            Ok(hwnd) => PostMessageW(Some(hwnd), WM_QUIT_REQUEST, WPARAM(0), LPARAM(0)).is_ok(),
            Err(_) => false,
        }
    }
}

pub fn signal_running_instance(tab: usize) {
    unsafe {
        if let Ok(hwnd) = FindWindowW(CLASS, PCWSTR::null()) {
            // The running instance may be uiAccess / another integrity level: allow it to activate.
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let _ = AllowSetForegroundWindow(pid);
            let _ = PostMessageW(Some(hwnd), WM_OPEN_SETTINGS, WPARAM(tab), LPARAM(0));
        }
    }
}

pub fn run() -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = CLASS;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Relaxed);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("best_loupe"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        // A second instance may run at a different integrity level than this (uiAccess) one.
        let _ = ChangeWindowMessageFilterEx(hwnd, WM_OPEN_SETTINGS, MSGFLT_ALLOW, None);
        let _ = ChangeWindowMessageFilterEx(hwnd, WM_QUIT_REQUEST, MSGFLT_ALLOW, None);
        add_icon(hwnd);
        update::add_listener(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if settings::is_dialog_message(&msg) {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
