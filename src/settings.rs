//! Settings / About window: one top-level window with a tab control (Lens, Zoom, General, About).
//! Every change is applied live through `config::update` (which also saves the file).
//! Runs on the main (tray) thread; `tray.rs` routes its messages through `is_dialog_message`.

use std::cell::RefCell;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{SS_ICON, SS_LEFT, SS_NOPREFIX, SS_REALSIZECONTROL, SS_SUNKEN};
use windows::Win32::UI::Controls::Dialogs::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use crate::config::{self, Config};
use crate::i18n::{self, Strings, t};
use crate::update::{self, Status};

pub const TAB_LENS: usize = 0;
pub const TAB_ABOUT: usize = 3;
const TAB_COUNT: usize = 4;
/// Missing from the `windows` crate: TBM_GETPOS = WM_USER.
const TBM_GETPOS: u32 = WM_USER;

const CLASS: PCWSTR = w!("best_loupe_settings");
const PAGE_CLASS: PCWSTR = w!("best_loupe_settings_page");

const ID_TAB: i32 = 100;
const ID_SLIDER: i32 = 200; // + slider index
const ID_CHK_SMOOTH: i32 = 400;
const ID_CHK_CTRL: i32 = 401;
const ID_CHK_ALT: i32 = 402;
const ID_CHK_SHIFT: i32 = 403;
const ID_CHK_AUTOSTART: i32 = 404;
const ID_CHK_UPDATES: i32 = 405;
const ID_BTN_COLOR: i32 = 500;
const ID_BTN_OPEN_CONFIG: i32 = 502;
const ID_BTN_RESET: i32 = 503;
const ID_BTN_CHECK: i32 = 504;
const ID_BTN_DOWNLOAD: i32 = 505;
const ID_BTN_PROJECT: i32 = 506;
const ID_BTN_LOG: i32 = 507;

/// The settings UI is drawn this much larger than standard Windows dialogs: best_loupe is for
/// people who need things bigger. Applied on top of the monitor DPI.
const UI_SCALE: f32 = 1.5;

/// Client size of the window, in DIPs (before `UI_SCALE`).
const CLIENT_W: i32 = 560;
const CLIENT_H: i32 = 400;

struct Slider {
    tab: usize,
    label: fn(&Strings) -> &'static str,
    min: i32,
    max: i32,
    get: fn(&Config) -> i32,
    set: fn(&mut Config, i32),
    fmt: fn(i32) -> String,
}

fn half_side(c: &Config) -> f32 {
    c.lens_width.min(c.lens_height) as f32 / 2.0
}

fn ms_or_off(ms: i32) -> String {
    if ms == 0 { t().off.to_string() } else { format!("{ms} ms") }
}

fn sliders() -> [Slider; 11] {
    [
        Slider {
            tab: 0,
            label: |s| s.lens_width,
            min: 16,
            max: 160,
            get: |c| (c.lens_width / 10) as i32,
            set: |c, v| c.lens_width = v as u32 * 10,
            fmt: |v| format!("{} px", v * 10),
        },
        Slider {
            tab: 0,
            label: |s| s.lens_height,
            min: 10,
            max: 100,
            get: |c| (c.lens_height / 10) as i32,
            set: |c, v| c.lens_height = v as u32 * 10,
            fmt: |v| format!("{} px", v * 10),
        },
        Slider {
            tab: 0,
            label: |s| s.corner_radius,
            min: 0,
            max: 100,
            get: |c| (c.corner_radius / half_side(c) * 100.0).round() as i32,
            set: |c, v| c.corner_radius = v as f32 / 100.0 * half_side(c),
            fmt: |v| format!("{v} %"),
        },
        Slider {
            tab: 0,
            label: |s| s.border_width,
            min: 0,
            max: 8,
            get: |c| c.border_width.round() as i32,
            set: |c, v| c.border_width = v as f32,
            fmt: |v| if v == 0 { t().off.to_string() } else { format!("{v} px") },
        },
        Slider {
            tab: 0,
            label: |s| s.shadow,
            min: 0,
            max: 60,
            get: |c| c.shadow_size.round() as i32,
            set: |c, v| c.shadow_size = v as f32,
            fmt: |v| if v == 0 { t().off.to_string() } else { format!("{v} px") },
        },
        Slider {
            tab: 1,
            label: |s| s.initial_zoom,
            min: 5,
            max: 32,
            get: |c| (c.initial_zoom * 4.0).round() as i32,
            set: |c, v| c.initial_zoom = v as f32 / 4.0,
            fmt: |v| format!("{:.2}×", v as f32 / 4.0),
        },
        Slider {
            tab: 1,
            label: |s| s.max_zoom,
            min: 2,
            max: 32,
            get: |c| c.max_zoom.round() as i32,
            set: |c, v| c.max_zoom = v as f32,
            fmt: |v| format!("{v}×"),
        },
        Slider {
            tab: 1,
            label: |s| s.zoom_step,
            min: 105,
            max: 200,
            get: |c| (c.zoom_step * 100.0).round() as i32,
            set: |c, v| c.zoom_step = v as f32 / 100.0,
            fmt: |v| format!("×{:.2}", v as f32 / 100.0),
        },
        Slider {
            tab: 1,
            label: |s| s.zoom_smoothing,
            min: 0,
            max: 40,
            get: |c| (c.zoom_smoothing_ms / 5.0).round() as i32,
            set: |c, v| c.zoom_smoothing_ms = v as f32 * 5.0,
            fmt: |v| ms_or_off(v * 5),
        },
        Slider {
            tab: 2,
            label: |s| s.activation_delay,
            min: 0,
            max: 50,
            get: |c| (c.activation_delay_ms / 10) as i32,
            set: |c, v| c.activation_delay_ms = v as u32 * 10,
            fmt: |v| ms_or_off(v * 10),
        },
        Slider {
            tab: 2,
            label: |s| s.fade_in,
            min: 0,
            max: 40,
            get: |c| (c.fade_in_ms / 5.0).round() as i32,
            set: |c, v| c.fade_in_ms = v as f32 * 5.0,
            fmt: |v| ms_or_off(v * 5),
        },
    ]
}

#[derive(Clone)]
struct Ui {
    hwnd: HWND,
    tab: HWND,
    pages: [HWND; TAB_COUNT],
    font: HFONT,
    title_font: HFONT,
    /// (trackbar, value label), indexed like `sliders()`.
    sliders: Vec<(HWND, HWND)>,
    checks: Vec<(i32, HWND)>,
    swatch: HWND,
    swatch_brush: HBRUSH,
    howto: HWND,
    upd_status: HWND,
    btn_download: HWND,
    icons: Vec<HICON>,
    current_tab: usize,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn ui() -> Option<Ui> {
    UI.with(|u| u.try_borrow().ok().and_then(|u| u.clone()))
}

fn hinstance() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).map(Into::into).unwrap_or_default() }
}

fn wide(s: &str) -> HSTRING {
    HSTRING::from(s)
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }
}

fn open_shell(target: &str) {
    unsafe {
        ShellExecuteW(None, w!("open"), &wide(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
    }
}

fn load_icon(size: i32) -> HICON {
    unsafe {
        LoadImageW(Some(hinstance()), PCWSTR(1 as *const u16), IMAGE_ICON, size, size, LR_DEFAULTCOLOR)
            .map(|h| HICON(h.0))
            .unwrap_or_default()
    }
}

/// Routes keyboard navigation (Tab, arrows, mnemonics) for the settings window.
pub fn is_dialog_message(msg: &MSG) -> bool {
    match ui() {
        Some(u) => unsafe { IsDialogMessageW(u.hwnd, msg).as_bool() },
        None => false,
    }
}

/// Opens the window (or brings it to the front) on the given tab.
pub fn open(tab: usize) {
    if let Some(u) = ui() {
        select_tab(&u, tab);
        unsafe {
            let _ = ShowWindow(u.hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(u.hwnd);
        }
        return;
    }
    unsafe {
        register_classes();
        let title = wide(t().window_title);
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            &title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            100,
            100,
            None,
            None,
            Some(hinstance()),
            None,
        ) else {
            crate::log!("settings: CreateWindowEx failed");
            return;
        };
        let dpi = GetDpiForWindow(hwnd).max(96);
        place_window(hwnd, dpi, None);
        let icons = vec![load_icon(GetSystemMetricsForDpi(SM_CXICON, dpi)), load_icon(GetSystemMetricsForDpi(SM_CXSMICON, dpi))];
        send(hwnd, WM_SETICON, ICON_BIG as usize, icons[0].0 as isize);
        send(hwnd, WM_SETICON, ICON_SMALL as usize, icons[1].0 as isize);
        let mut u = build(hwnd, dpi, icons);
        select_tab_inner(&mut u, tab);
        UI.with(|cell| *cell.borrow_mut() = Some(u));
        update::add_listener(hwnd);
        sync_controls();
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

unsafe fn register_classes() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_TAB_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        });
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let procs: [(PCWSTR, WNDPROC); 2] = [(CLASS, Some(wndproc)), (PAGE_CLASS, Some(page_proc))];
        for (name, proc_) in procs {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: proc_,
                hInstance: hinstance(),
                hCursor: cursor,
                hbrBackground: GetSysColorBrush(COLOR_WINDOW),
                lpszClassName: name,
                ..Default::default()
            };
            RegisterClassExW(&wc);
        }
    });
}

/// Sizes the window for `dpi` and centres it on the work area (or uses the suggested rect).
unsafe fn place_window(hwnd: HWND, dpi: u32, suggested: Option<RECT>) {
    unsafe {
        let scale = dpi as f32 / 96.0 * UI_SCALE;
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        let mut r = RECT { left: 0, top: 0, right: (CLIENT_W as f32 * scale) as i32, bottom: (CLIENT_H as f32 * scale) as i32 };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE(0), dpi);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let (x, y) = match suggested {
            Some(s) => (s.left, s.top),
            None => {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
                let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                let _ = GetMonitorInfoW(mon, &mut mi);
                let wa = mi.rcWork;
                (wa.left + (wa.right - wa.left - w) / 2, wa.top + (wa.bottom - wa.top - h) / 2)
            }
        };
        let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

unsafe fn make_fonts(dpi: u32) -> (HFONT, HFONT) {
    unsafe {
        let mut ncm = NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
        let _ = SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            0,
            dpi,
        );
        let mut base = ncm.lfMessageFont;
        base.lfHeight = (base.lfHeight as f32 * UI_SCALE).round() as i32;
        let font = CreateFontIndirectW(&base);
        let mut title = base;
        title.lfHeight = (title.lfHeight as f32 * 1.8) as i32;
        title.lfWeight = FW_SEMIBOLD.0 as i32;
        (font, CreateFontIndirectW(&title))
    }
}

struct Builder {
    scale: f32,
    font: HFONT,
}

impl Builder {
    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    #[allow(clippy::too_many_arguments)]
    fn child(&self, parent: HWND, class: PCWSTR, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: i32) -> HWND {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                &wide(text),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                self.px(x),
                self.px(y),
                self.px(w),
                self.px(h),
                Some(parent),
                Some(HMENU(id as isize as *mut _)),
                Some(hinstance()),
                None,
            )
            .unwrap_or_default();
            send(hwnd, WM_SETFONT, self.font.0 as usize, 1);
            hwnd
        }
    }

    fn label(&self, parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
        self.child(parent, w!("STATIC"), text, SS_LEFT.0 | SS_NOPREFIX.0, x, y, w, h, -1)
    }

    fn button(&self, parent: HWND, text: &str, x: i32, y: i32, w: i32, id: i32) -> HWND {
        self.child(parent, w!("BUTTON"), text, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, x, y, w, 28, id)
    }

    fn checkbox(&self, parent: HWND, text: &str, x: i32, y: i32, w: i32, id: i32) -> HWND {
        self.child(parent, w!("BUTTON"), text, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, x, y, w, 24, id)
    }
}

/// Creates the tab control, the pages and all controls for `dpi`.
unsafe fn build(hwnd: HWND, dpi: u32, icons: Vec<HICON>) -> Ui {
    unsafe {
        let s = t();
        let (font, title_font) = make_fonts(dpi);
        let b = Builder { scale: dpi as f32 / 96.0 * UI_SCALE, font };

        let tab = b.child(hwnd, WC_TABCONTROLW, "", WS_CLIPSIBLINGS.0 | WS_TABSTOP.0, 8, 8, CLIENT_W - 16, CLIENT_H - 16, ID_TAB);
        for (i, name) in [s.tab_lens, s.tab_zoom, s.tab_general, s.tab_about].iter().enumerate() {
            let mut text: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let item = TCITEMW { mask: TCIF_TEXT, pszText: PWSTR(text.as_mut_ptr()), ..Default::default() };
            send(tab, TCM_INSERTITEMW, i, &item as *const _ as isize);
        }
        // Page area = tab display rect, in main-window coordinates.
        let mut r = RECT::default();
        let _ = GetClientRect(tab, &mut r);
        send(tab, TCM_ADJUSTRECT, 0, &mut r as *mut _ as isize);
        let off = b.px(8);
        let pages: [HWND; TAB_COUNT] = std::array::from_fn(|_| {
            CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PAGE_CLASS,
                PCWSTR::null(),
                WS_CHILD,
                r.left + off,
                r.top + off,
                r.right - r.left,
                r.bottom - r.top,
                Some(hwnd),
                None,
                Some(hinstance()),
                None,
            )
            .unwrap_or_default()
        });

        // Slider rows: label | trackbar | value. The General page starts below the hotkey block.
        let mut rows = [16, 16, 92, 16];
        let mut slider_handles = Vec::new();
        for (i, sl) in sliders().iter().enumerate() {
            let page = pages[sl.tab];
            let y = rows[sl.tab];
            b.label(page, (sl.label)(s), 16, y + 5, 180, 20);
            let tb = b.child(page, TRACKBAR_CLASSW, "", TBS_HORZ | TBS_NOTICKS | TBS_FIXEDLENGTH | WS_TABSTOP.0, 200, y, 230, 30, ID_SLIDER + i as i32);
            send(tb, TBM_SETTHUMBLENGTH, b.px(22) as usize, 0);
            send(tb, TBM_SETRANGEMIN, 0, sl.min as isize);
            send(tb, TBM_SETRANGEMAX, 1, sl.max as isize);
            send(tb, TBM_SETPAGESIZE, 0, ((sl.max - sl.min) / 10).max(1) as isize);
            let value = b.label(page, "", 440, y + 5, 90, 20);
            slider_handles.push((tb, value));
            rows[sl.tab] += 38;
        }

        // Lens: border colour.
        let lens = pages[0];
        let y = rows[0];
        b.label(lens, s.border_color, 16, y + 5, 180, 20);
        b.button(lens, s.choose_color, 200, y, 120, ID_BTN_COLOR);
        let swatch = b.child(lens, w!("STATIC"), "", SS_SUNKEN.0, 332, y + 2, 44, 24, -1);

        // Zoom: smoothing.
        let mut checks = Vec::new();
        checks.push((ID_CHK_SMOOTH, b.checkbox(pages[1], s.smoothing, 16, rows[1] + 4, 500, ID_CHK_SMOOTH)));

        // General: hotkey block on top (sliders were placed below it), then checkboxes.
        let general = pages[2];
        b.label(general, s.hotkey, 16, 21, 180, 20);
        checks.push((ID_CHK_CTRL, b.checkbox(general, "Ctrl", 200, 18, 70, ID_CHK_CTRL)));
        checks.push((ID_CHK_ALT, b.checkbox(general, "Alt", 276, 18, 70, ID_CHK_ALT)));
        checks.push((ID_CHK_SHIFT, b.checkbox(general, "Shift", 352, 18, 80, ID_CHK_SHIFT)));
        b.label(general, s.hotkey_hint, 16, 52, 510, 20);
        let mut y = rows[2];
        checks.push((ID_CHK_AUTOSTART, b.checkbox(general, s.autostart, 16, y + 4, 500, ID_CHK_AUTOSTART)));
        y += 30;
        checks.push((ID_CHK_UPDATES, b.checkbox(general, s.check_updates, 16, y + 4, 500, ID_CHK_UPDATES)));
        y += 44;
        b.button(general, s.open_config, 16, y, 220, ID_BTN_OPEN_CONFIG);
        b.button(general, s.reset_defaults, 248, y, 220, ID_BTN_RESET);

        // About.
        let about = pages[3];
        let big = load_icon(b.px(64));
        let icon = b.child(about, w!("STATIC"), "", SS_ICON.0 | SS_REALSIZECONTROL.0, 16, 16, 64, 64, -1);
        send(icon, STM_SETICON, big.0 as usize, 0);
        let title = b.label(about, "best_loupe", 96, 18, 400, 36);
        send(title, WM_SETFONT, title_font.0 as usize, 1);
        let version = s.version.replace("{version}", crate::VERSION).replace("{build}", crate::BUILD);
        b.label(about, &version, 96, 58, 420, 20);
        b.label(about, s.tagline, 16, 100, 510, 40);
        let howto = b.label(about, "", 16, 146, 510, 44);
        let upd_status = b.label(about, "", 16, 204, 510, 40);
        b.button(about, s.check_now, 16, 250, 170, ID_BTN_CHECK);
        let btn_download = b.button(about, "", 196, 250, 230, ID_BTN_DOWNLOAD);
        b.button(about, s.project_page, 16, 290, 170, ID_BTN_PROJECT);
        b.button(about, s.open_log, 196, 290, 170, ID_BTN_LOG);

        let mut icons = icons;
        icons.push(big);
        Ui {
            hwnd,
            tab,
            pages,
            font,
            title_font,
            sliders: slider_handles,
            checks,
            swatch,
            swatch_brush: HBRUSH::default(),
            howto,
            upd_status,
            btn_download,
            icons,
            current_tab: 0,
        }
    }
}

fn select_tab(u: &Ui, tab: usize) {
    UI.with(|cell| {
        if let Ok(mut g) = cell.try_borrow_mut() {
            if let Some(g) = g.as_mut() {
                select_tab_inner(g, tab);
            }
        }
    });
    let _ = u;
}

fn select_tab_inner(u: &mut Ui, tab: usize) {
    let tab = tab.min(TAB_COUNT - 1);
    send(u.tab, TCM_SETCURSEL, tab, 0);
    for (i, &p) in u.pages.iter().enumerate() {
        unsafe {
            if i == tab {
                let _ = SetWindowPos(p, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            } else {
                let _ = ShowWindow(p, SW_HIDE);
            }
        }
    }
    u.current_tab = tab;
}

fn set_text(hwnd: HWND, text: &str) {
    unsafe {
        let _ = SetWindowTextW(hwnd, &wide(text));
    }
}

/// Pushes the live config (and update status) into every control.
fn sync_controls() {
    let Some(u) = ui() else { return };
    let cfg = config::current();
    for (sl, &(tb, value)) in sliders().iter().zip(&u.sliders) {
        let v = (sl.get)(&cfg).clamp(sl.min, sl.max);
        if send(tb, TBM_GETPOS, 0, 0).0 as i32 != v {
            send(tb, TBM_SETPOS, 1, v as isize);
        }
        set_text(value, &(sl.fmt)(v));
    }
    for &(id, h) in &u.checks {
        let on = match id {
            ID_CHK_SMOOTH => cfg.smoothing,
            ID_CHK_CTRL => cfg.hotkey_ctrl,
            ID_CHK_ALT => cfg.hotkey_alt,
            ID_CHK_SHIFT => cfg.hotkey_shift,
            ID_CHK_AUTOSTART => crate::autostart::is_enabled(),
            ID_CHK_UPDATES => cfg.check_updates,
            _ => false,
        };
        send(h, BM_SETCHECK, if on { BST_CHECKED.0 as usize } else { BST_UNCHECKED.0 as usize }, 0);
    }
    let hotkey = i18n::hotkey_label(&cfg);
    set_text(u.howto, &t().howto.replace("{hotkey}", &hotkey));
    refresh_swatch(&cfg);
    refresh_update_status();
}

fn refresh_swatch(cfg: &Config) {
    let Some(u) = ui() else { return };
    let c = cfg.border_color;
    let colorref = COLORREF(((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff));
    let brush = unsafe { CreateSolidBrush(colorref) };
    let old = UI.with(|cell| {
        cell.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(|g| std::mem::replace(&mut g.swatch_brush, brush)))
    });
    if let Some(old) = old {
        if !old.is_invalid() {
            unsafe {
                let _ = DeleteObject(old.into());
            }
        }
    }
    unsafe {
        let _ = InvalidateRect(Some(u.swatch), None, true);
    }
}

fn refresh_update_status() {
    let Some(u) = ui() else { return };
    let s = t();
    let cfg = config::current();
    let status = update::status();
    let text = match &status {
        Status::Idle if !cfg.check_updates => s.upd_disabled.to_string(),
        Status::Idle => s.upd_idle.to_string(),
        Status::Checking => s.upd_checking.to_string(),
        Status::UpToDate => s.upd_latest.to_string(),
        Status::Available { version, .. } => s.upd_available.replace("{version}", version),
        Status::Failed(e) => s.upd_failed.replace("{error}", e),
    };
    set_text(u.upd_status, &text);
    let (label, enabled) = match &status {
        Status::Available { version, .. } => (s.download.replace("{version}", version), true),
        _ => (s.download.replace(" {version}", "").replace("{version}", ""), false),
    };
    set_text(u.btn_download, &label);
    unsafe {
        let _ = EnableWindow(u.btn_download, enabled);
    }
}

fn on_slider(index: usize) {
    let Some(u) = ui() else { return };
    let Some(&(tb, _)) = u.sliders.get(index) else { return };
    let pos = send(tb, TBM_GETPOS, 0, 0).0 as i32;
    let set = sliders()[index].set;
    config::update(|c| set(c, pos));
    sync_controls();
}

fn on_command(id: i32, ctl: HWND) {
    let checked = || send(ctl, BM_GETCHECK, 0, 0).0 as u32 == BST_CHECKED.0;
    match id {
        ID_CHK_SMOOTH => config::update(|c| c.smoothing = checked()),
        ID_CHK_UPDATES => {
            let on = checked();
            config::update(|c| c.check_updates = on);
            if on {
                update::check_now();
            }
        }
        ID_CHK_AUTOSTART => crate::autostart::set_enabled(checked()),
        ID_CHK_CTRL | ID_CHK_ALT | ID_CHK_SHIFT => {
            let on = checked();
            let mut cfg = config::current();
            match id {
                ID_CHK_CTRL => cfg.hotkey_ctrl = on,
                ID_CHK_ALT => cfg.hotkey_alt = on,
                _ => cfg.hotkey_shift = on,
            }
            // The hotkey needs at least one modifier: refuse to clear the last one.
            if cfg.hotkey_ctrl || cfg.hotkey_alt || cfg.hotkey_shift {
                config::update(|c| {
                    c.hotkey_ctrl = cfg.hotkey_ctrl;
                    c.hotkey_alt = cfg.hotkey_alt;
                    c.hotkey_shift = cfg.hotkey_shift;
                });
            }
        }
        ID_BTN_COLOR => choose_color(),
        ID_BTN_OPEN_CONFIG => {
            if let Some(dir) = config::config_path().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
                open_shell(&dir.display().to_string());
            }
        }
        ID_BTN_RESET => config::update(|c| {
            let repo = std::mem::take(&mut c.update_repo);
            *c = Config { update_repo: repo, ..Config::default() };
        }),
        ID_BTN_CHECK => update::check_now(),
        ID_BTN_DOWNLOAD => {
            if let Status::Available { url, .. } = update::status() {
                open_shell(&url);
            }
        }
        ID_BTN_PROJECT => open_shell(&format!("https://github.com/{}", config::current().update_repo)),
        ID_BTN_LOG => {
            if let Some(p) = crate::log::log_path() {
                open_shell(&p.display().to_string());
            }
        }
        _ => return,
    }
    sync_controls();
}

fn choose_color() {
    let Some(u) = ui() else { return };
    let c = config::current().border_color;
    let mut custom = [COLORREF(0x00ff_ffff); 16];
    let mut cc = CHOOSECOLORW {
        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: u.hwnd,
        rgbResult: COLORREF(((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff)),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_RGBINIT | CC_FULLOPEN,
        ..Default::default()
    };
    // Modal: runs its own message loop and re-enters our wndproc, so no UI borrow is held here.
    if unsafe { ChooseColorW(&mut cc) }.as_bool() {
        let r = cc.rgbResult.0;
        let rgb = ((r & 0xff) << 16) | (r & 0xff00) | ((r >> 16) & 0xff);
        config::update(|c| c.border_color = rgb);
    }
}

fn destroy_children(u: &Ui) {
    unsafe {
        for &p in &u.pages {
            let _ = DestroyWindow(p);
        }
        let _ = DestroyWindow(u.tab);
        let _ = DeleteObject(u.font.into());
        let _ = DeleteObject(u.title_font.into());
    }
}

unsafe extern "system" fn page_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_COMMAND | WM_HSCROLL | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_NOTIFY => {
                let parent = GetParent(hwnd).unwrap_or_default();
                SendMessageW(parent, msg, Some(wparam), Some(lparam))
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_NOTIFY => {
                let hdr = &*(lparam.0 as *const NMHDR);
                if hdr.idFrom == ID_TAB as usize && hdr.code == TCN_SELCHANGE {
                    if let Some(u) = ui() {
                        let sel = send(u.tab, TCM_GETCURSEL, 0, 0).0 as usize;
                        select_tab(&u, sel);
                    }
                }
                LRESULT(0)
            }
            WM_HSCROLL => {
                let id = GetDlgCtrlID(HWND(lparam.0 as *mut _));
                if id >= ID_SLIDER && id < ID_SLIDER + 100 {
                    on_slider((id - ID_SLIDER) as usize);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                let code = ((wparam.0 >> 16) & 0xffff) as u32;
                if code == BN_CLICKED {
                    on_command(id, HWND(lparam.0 as *mut _));
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let hdc = HDC(wparam.0 as *mut _);
                let ctl = HWND(lparam.0 as *mut _);
                if let Some(u) = ui() {
                    if ctl == u.swatch && !u.swatch_brush.is_invalid() {
                        return LRESULT(u.swatch_brush.0 as isize);
                    }
                }
                SetBkColor(hdc, COLORREF(GetSysColor(COLOR_WINDOW)));
                LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize)
            }
            update::WM_UPDATE_STATUS => {
                refresh_update_status();
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as u32;
                let suggested = *(lparam.0 as *const RECT);
                if let Some(old) = ui() {
                    destroy_children(&old);
                    let tab = old.current_tab;
                    place_window(hwnd, dpi, Some(suggested));
                    let mut u = build(hwnd, dpi, old.icons.clone());
                    u.swatch_brush = old.swatch_brush;
                    select_tab_inner(&mut u, tab);
                    UI.with(|cell| *cell.borrow_mut() = Some(u));
                    sync_controls();
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                update::remove_listener(hwnd);
                if let Some(u) = UI.with(|cell| cell.borrow_mut().take()) {
                    let _ = DeleteObject(u.font.into());
                    let _ = DeleteObject(u.title_font.into());
                    if !u.swatch_brush.is_invalid() {
                        let _ = DeleteObject(u.swatch_brush.into());
                    }
                    for i in u.icons {
                        let _ = DestroyIcon(i);
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
