//! Global input: hotkey (modifier chord, Ctrl+Alt by default) tracking and wheel capture via low-level hooks.
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
const LSHIFT: u32 = 16;
const RSHIFT: u32 = 32;
const CTRL_MASK: u32 = LCTRL | RCTRL;
const ALT_MASK: u32 = LALT | RALT;
const SHIFT_MASK: u32 = LSHIFT | RSHIFT;
const GROUPS: [u32; 3] = [CTRL_MASK, ALT_MASK, SHIFT_MASK];

const MSG_MOUSE_HOOK_ON: u32 = WM_APP + 1;
const MSG_MOUSE_HOOK_OFF: u32 = WM_APP + 2;

static MODS: AtomicU32 = AtomicU32::new(0);
/// Modifier groups (CTRL_MASK | ALT_MASK | SHIFT_MASK subsets) that make up the hotkey.
static REQUIRED: AtomicU32 = AtomicU32::new(CTRL_MASK | ALT_MASK);
/// Extra modifier group that freezes the lens picture while held on top of the hotkey (0 = off).
static FREEZE: AtomicU32 = AtomicU32::new(SHIFT_MASK);
/// Tray "pause": while false the hotkey does nothing.
static ENABLED: AtomicBool = AtomicBool::new(true);
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

/// Every required modifier group has at least one key down.
fn mods_held(m: u32) -> bool {
    let req = REQUIRED.load(Acquire);
    req != 0 && GROUPS.iter().all(|&g| req & g == 0 || m & g != 0)
}

pub fn apply_config(cfg: &crate::config::Config) {
    let mut req = 0;
    if cfg.hotkey_ctrl {
        req |= CTRL_MASK;
    }
    if cfg.hotkey_alt {
        req |= ALT_MASK;
    }
    if cfg.hotkey_shift {
        req |= SHIFT_MASK;
    }
    let freeze = match cfg.freeze_key {
        crate::config::FreezeKey::Shift => SHIFT_MASK,
        crate::config::FreezeKey::Ctrl => CTRL_MASK,
        crate::config::FreezeKey::Alt => ALT_MASK,
        crate::config::FreezeKey::None => 0,
    };
    let before = chord_held();
    FREEZE.store(freeze & !req, Release);
    REQUIRED.store(req, Release);
    TAINTED.store(false, Release);
    on_chord_change(before);
}

/// The hotkey modifiers are all down, no other key was pressed since, and the lens is not paused.
pub fn chord_held() -> bool {
    ENABLED.load(Acquire) && mods_held(MODS.load(Acquire)) && !TAINTED.load(Acquire)
}

/// The hotkey is held together with the freeze modifier: show a still picture.
pub fn frozen() -> bool {
    let freeze = FREEZE.load(Acquire);
    freeze != 0 && MODS.load(Acquire) & freeze != 0 && chord_held()
}

pub fn enabled() -> bool {
    ENABLED.load(Acquire)
}

/// Tray "pause": a paused lens ignores the hotkey (and installs no mouse hook). Not persisted.
pub fn set_enabled(on: bool) {
    let before = chord_held();
    ENABLED.store(on, Release);
    on_chord_change(before);
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
    for (vk, mask) in [(VK_CONTROL, CTRL_MASK), (VK_MENU, ALT_MASK), (VK_SHIFT, SHIFT_MASK)] {
        if !down(vk) {
            m &= !mask;
        }
    }
    // Clear every modifier that is physically up (also a stale freeze modifier).
    MODS.fetch_and(m | !(CTRL_MASK | ALT_MASK | SHIFT_MASK), AcqRel);
    if !mods_held(m) {
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
            VK_LSHIFT => LSHIFT,
            VK_RSHIFT => RSHIFT,
            _ => 0,
        };
        let before = chord_held();
        let required = REQUIRED.load(Acquire);
        let freeze = FREEZE.load(Acquire);
        if bit != 0 {
            let held_before = mods_held(MODS.load(Acquire));
            let m = if down { MODS.fetch_or(bit, AcqRel) | bit } else { MODS.fetch_and(!bit, AcqRel) & !bit };
            if !mods_held(m) {
                TAINTED.store(false, Release);
            } else if down && held_before && GROUPS.iter().any(|&g| g & bit != 0 && required & g == 0 && freeze & g == 0) {
                // A modifier that is neither the hotkey nor the freeze key (e.g. Shift with Ctrl+Alt
                // when freeze is off): a shortcut.
                TAINTED.store(true, Release);
            }
        } else if down && mods_held(MODS.load(Acquire)) {
            TAINTED.store(true, Release);
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
