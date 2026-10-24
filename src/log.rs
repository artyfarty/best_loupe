//! Minimal file + stderr logger. Log file: `%LOCALAPPDATA%\best_loupe\best_loupe.log`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static SINK: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

pub fn init() {
    START.get_or_init(Instant::now);
    let file = log_path().and_then(|p| {
        std::fs::create_dir_all(p.parent()?).ok()?;
        OpenOptions::new().create(true).write(true).truncate(true).open(p).ok()
    });
    let _ = SINK.set(Mutex::new(file));
}

pub fn log_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("best_loupe").join("best_loupe.log"))
}

pub fn write(args: std::fmt::Arguments) {
    let t = START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    let line = format!("[{t:10.4}] {args}\n");
    eprint!("{line}");
    if let Some(sink) = SINK.get() {
        if let Ok(mut guard) = sink.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(format_args!($($arg)*)) };
}
