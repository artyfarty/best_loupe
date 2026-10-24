//! Update check against the latest GitHub release of `config.update_repo` (WinHTTP, no deps).
//!
//! Checks ~30 s after start and then daily while `check_updates` is on; "Check now" forces one.
//! Status changes are announced to registered windows with [`WM_UPDATE_STATUS`].

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering::*};
use std::time::Duration;

use windows::Win32::Foundation::*;
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, PCWSTR, w};

use crate::{config, log};

pub const WM_UPDATE_STATUS: u32 = WM_APP + 20;
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available { version: String, url: String },
    Failed(String),
}

static STATUS: Mutex<Status> = Mutex::new(Status::Idle);
static CHECK_NOW: AtomicUsize = AtomicUsize::new(0);
static LISTENERS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

pub fn status() -> Status {
    STATUS.lock().unwrap().clone()
}

/// Windows that get `WM_UPDATE_STATUS` posted on every status change.
pub fn add_listener(hwnd: HWND) {
    LISTENERS.lock().unwrap().push(hwnd.0 as usize);
}

pub fn remove_listener(hwnd: HWND) {
    LISTENERS.lock().unwrap().retain(|&h| h != hwnd.0 as usize);
}

fn set_status(s: Status) {
    *STATUS.lock().unwrap() = s;
    for &h in LISTENERS.lock().unwrap().iter() {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_UPDATE_STATUS, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn check_now() {
    let h = CHECK_NOW.load(Acquire);
    if h != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(h as *mut _));
        }
    }
}

pub fn spawn() {
    let event = unsafe { CreateEventW(None, false, false, None) }.expect("CreateEvent");
    CHECK_NOW.store(event.0 as usize, Release);
    let raw = event.0 as usize;
    std::thread::Builder::new()
        .name("update".into())
        .spawn(move || run(HANDLE(raw as *mut _)))
        .expect("spawn update thread");
}

fn run(event: HANDLE) {
    let mut wait = FIRST_CHECK_DELAY;
    loop {
        let forced = unsafe { WaitForSingleObject(event, wait.as_millis() as u32) } == WAIT_OBJECT_0;
        wait = CHECK_INTERVAL;
        let cfg = config::current();
        if !forced && !cfg.check_updates {
            continue;
        }
        set_status(Status::Checking);
        let s = match fetch_latest(&cfg.update_repo) {
            Ok(Some((tag, url))) if is_newer(&tag, crate::VERSION) => {
                log!("update: {tag} available ({url})");
                Status::Available { version: tag.trim_start_matches('v').to_string(), url }
            }
            Ok(latest) => {
                log!("update: up to date (latest release: {:?})", latest.map(|l| l.0));
                Status::UpToDate
            }
            Err(e) => {
                log!("update: check failed: {e}");
                Status::Failed(e)
            }
        };
        set_status(s);
    }
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next().unwrap_or(Some(0))?))
}

fn is_newer(tag: &str, current: &str) -> bool {
    matches!((parse_version(tag), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// Returns `(tag_name, html_url)` of the latest release, `None` if the repo has no releases.
fn fetch_latest(repo: &str) -> Result<Option<(String, String)>, String> {
    let path = format!("/repos/{repo}/releases/latest");
    let (status, body) = https_get("api.github.com", &path).map_err(|e| e.to_string())?;
    match status {
        200 => {}
        404 => return Ok(None),
        s => return Err(format!("HTTP {s}")),
    }
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("no tag_name")?.to_string();
    let url = v["html_url"].as_str().unwrap_or_default().to_string();
    Ok(Some((tag, url)))
}

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn https_get(host: &str, path: &str) -> windows::core::Result<(u32, Vec<u8>)> {
    unsafe {
        let ua = HSTRING::from(format!("best_loupe/{}", crate::VERSION));
        let session = Handle(WinHttpOpen(&ua, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0));
        if session.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let _ = WinHttpSetTimeouts(session.0, 10_000, 10_000, 15_000, 15_000);
        let conn = Handle(WinHttpConnect(session.0, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if conn.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let req = Handle(WinHttpOpenRequest(
            conn.0,
            w!("GET"),
            &HSTRING::from(path),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if req.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let headers: Vec<u16> = "Accept: application/vnd.github+json\r\n".encode_utf16().collect();
        WinHttpAddRequestHeaders(req.0, &headers, WINHTTP_ADDREQ_FLAG_ADD)?;
        WinHttpSendRequest(req.0, None, None, 0, 0, 0)?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())?;

        let mut status = 0u32;
        let mut len = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut _),
            &mut len,
            std::ptr::null_mut(),
        )?;

        let mut body = Vec::new();
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(req.0, &mut avail)?;
            if avail == 0 {
                break;
            }
            let start = body.len();
            body.resize(start + avail as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(req.0, body[start..].as_mut_ptr() as *mut _, avail, &mut read)?;
            body.truncate(start + read as usize);
            if body.len() > 4 << 20 {
                break; // a release JSON is a few KB; don't let a broken proxy eat memory
            }
        }
        Ok((status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(is_newer("v0.1.1-rc1", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }
}
