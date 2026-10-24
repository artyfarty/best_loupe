//! Global input: Ctrl+Alt chord tracking and wheel capture via low-level hooks.
//!
//! The keyboard hook is always installed. The mouse hook is installed only while the chord is held,
//! so normal mouse input never pays for a hook round-trip. Hook procs only touch atomics and signal
//! the render thread's wake event.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::time::Instant;

use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const LCTRL: u32 = 1;
const RCTRL: u32 = 2;
const LALT: u32 = 4;
const RALT: u32 = 8;
const CTRL_MASK: u32 = LCTRL | RCTRL;
const ALT_MASK: u32 = LALT | RALT;

const MSG_MOUSE_HOOK_ON: u32 = WM_APP + 1;
const MSG_MOUSE_HOOK_OFF: u32 = WM_APP + 2;

static MODS: AtomicU32 = AtomicU32::new(0);
/// Another key was pressed while the chord was held; cleared when Ctrl or Alt is fully released.
static TAINTED: AtomicBool = AtomicBool::new(false);
/// A wheel event arrived during the chord: skip the activation delay.
static FORCED: AtomicBool = AtomicBool::new(false);
static CHORD_SINCE_MS: AtomicU64 = AtomicU64::new(0);
static WHEEL_ACCUM: AtomicI32 = AtomicI32::new(0);
static WAKE_EVENT: AtomicUsize = AtomicUsize::new(0);
static INPUT_TID: AtomicU32 = AtomicU32::new(0);
static START: OnceLock<Instant> = OnceLock::new();

pub fn now_ms() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn mods_held(m: u32) -> bool {
    m & CTRL_MASK != 0 && m & ALT_MASK != 0
}

/// Ctrl+Alt are both down and no other key was pressed since.
pub fn chord_held() -> bool {
    mods_held(MODS.load(Acquire)) && !TAINTED.load(Acquire)
}

pub fn chord_since_ms() -> u64 {
    CHORD_SINCE_MS.load(Acquire)
}

pub fn forced() -> bool {
    FORCED.load(Acquire)
}

/// Accumulated wheel delta (120 per notch) since the last call.
pub fn take_wheel() -> i32 {
    WHEEL_ACCUM.swap(0, AcqRel)
}

fn wake() {
    let h = WAKE_EVENT.load(Relaxed);
    if h != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(h as *mut _));
        }
    }
}

fn post_to_input_thread(msg: u32) {
    let tid = INPUT_TID.load(Acquire);
    if tid != 0 {
        unsafe {
            let _ = PostThreadMessageW(tid, msg, WPARAM(0), LPARAM(0));
        }
    }
}

fn on_chord_change(before: bool) {
    let after = chord_held();
    if before == after {
        return;
    }
    if after {
        FORCED.store(false, Release);
        WHEEL_ACCUM.store(0, Release);
        CHORD_SINCE_MS.store(now_ms(), Release);
        post_to_input_thread(MSG_MOUSE_HOOK_ON);
    } else {
        post_to_input_thread(MSG_MOUSE_HOOK_OFF);
    }
    wake();
}

/// Safety net for key-ups the hook never saw (e.g. released over an elevated window).
/// Called by the render thread while the lens is active; returns whether the chord is still held.
pub fn verify_with_async_state() -> bool {
    let down = |vk: VIRTUAL_KEY| unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000 != 0;
    let before = chord_held();
    let mut m = MODS.load(Acquire);
    if !down(VK_CONTROL) {
        m &= !CTRL_MASK;
    }
    if !down(VK_MENU) {
        m &= !ALT_MASK;
    }
    if !mods_held(m) {
        MODS.store(m, Release);
        TAINTED.store(false, Release);
        on_chord_change(before);
    }
    chord_held()
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let msg = wparam.0 as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let bit = match VIRTUAL_KEY(kb.vkCode as u16) {
            VK_LCONTROL => LCTRL,
            VK_RCONTROL => RCTRL,
            VK_LMENU => LALT,
            VK_RMENU => RALT,
            _ => 0,
        };
        let before = chord_held();
        if bit != 0 {
            let m = if down { MODS.fetch_or(bit, AcqRel) | bit } else { MODS.fetch_and(!bit, AcqRel) & !bit };
            if !mods_held(m) {
                TAINTED.store(false, Release);
            }
        } else if down {
            let vk = VIRTUAL_KEY(kb.vkCode as u16);
            let is_shift = matches!(vk, VK_LSHIFT | VK_RSHIFT | VK_SHIFT);
            if !is_shift && mods_held(MODS.load(Acquire)) {
                TAINTED.store(true, Release);
            }
        }
        on_chord_change(before);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_MOUSEWHEEL && chord_held() {
        let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let delta = (ms.mouseData >> 16) as u16 as i16 as i32;
        WHEEL_ACCUM.fetch_add(delta, AcqRel);
        FORCED.store(true, Release);
        wake();
        return LRESULT(1); // swallow: the app under the lens must not scroll/zoom
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Starts the input thread. `wake_event` is signalled on every chord/wheel change.
pub fn spawn(wake_event: HANDLE) {
    WAKE_EVENT.store(wake_event.0 as usize, Release);
    now_ms();
    std::thread::Builder::new()
        .name("input".into())
        .spawn(|| unsafe { input_thread() })
        .expect("spawn input thread");
}

unsafe fn input_thread() {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        let mut msg = MSG::default();
        // Make sure the thread has a message queue before anyone posts to it.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        INPUT_TID.store(GetCurrentThreadId(), Release);

        let hmod: HINSTANCE = GetModuleHandleW(None).map(Into::into).unwrap_or_default();
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(hmod), 0) {
            Ok(_) => crate::log!("input: keyboard hook installed"),
            Err(e) => crate::log!("input: keyboard hook FAILED: {e}"),
        }

        let mut mouse_hook: Option<HHOOK> = None;
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                MSG_MOUSE_HOOK_ON if mouse_hook.is_none() => {
                    match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(hmod), 0) {
                        Ok(h) => mouse_hook = Some(h),
                        Err(e) => crate::log!("input: mouse hook FAILED: {e}"),
                    }
                }
                MSG_MOUSE_HOOK_OFF => {
                    if let Some(h) = mouse_hook.take() {
                        let _ = UnhookWindowsHookEx(h);
                    }
                }
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
