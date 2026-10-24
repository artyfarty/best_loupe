//! Tray icon with an Exit menu. Runs the main thread's message loop until Exit is chosen.

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Result, w};

const WM_TRAY: u32 = WM_APP + 10;
const CMD_EXIT: usize = 1;
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() },
        ..Default::default()
    };
    let tip: Vec<u16> = format!("best_loupe {} — hold Ctrl+Alt, scroll to zoom", crate::VERSION).encode_utf16().collect();
    let n = tip.len().min(nid.szTip.len() - 1);
    nid.szTip[..n].copy_from_slice(&tip[..n]);
    nid
}

fn add_icon(hwnd: HWND) {
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &icon_data(hwnd)).as_bool() {
            crate::log!("tray: Shell_NotifyIcon(NIM_ADD) failed");
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_TRAY {
            let ev = (lparam.0 & 0xffff) as u32;
            if ev == WM_RBUTTONUP || ev == WM_LBUTTONUP {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                if let Ok(menu) = CreatePopupMenu() {
                    let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, w!("Exit best_loupe"));
                    let _ = SetForegroundWindow(hwnd);
                    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, Some(0), hwnd, None);
                    let _ = DestroyMenu(menu);
                    if cmd.0 as usize == CMD_EXIT {
                        let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd));
                        PostQuitMessage(0);
                    }
                }
            }
            return LRESULT(0);
        }
        if msg != 0 && msg == TASKBAR_CREATED.load(Relaxed) {
            add_icon(hwnd); // Explorer restarted
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

pub fn run() -> Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = w!("best_loupe_tray");
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
        add_icon(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
