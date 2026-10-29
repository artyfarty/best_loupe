//! User configuration, loaded from `%APPDATA%\best_loupe\config.toml`.
//!
//! The live config is process-global: the settings window updates it (and saves the file), the
//! render and input threads pick changes up via [`generation`].

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Extra modifier that, held together with the hotkey, freezes the picture in the lens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FreezeKey {
    None,
    Shift,
    Ctrl,
    Alt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Lens size in physical pixels.
    pub lens_width: u32,
    pub lens_height: u32,
    /// Corner radius in pixels; >= half of the smaller side gives a circle/capsule.
    pub corner_radius: f32,
    pub border_width: f32,
    /// Border colour, sRGB 0xRRGGBB.
    pub border_color: u32,
    /// Soft drop shadow size in pixels (0 disables).
    pub shadow_size: f32,
    /// Zoom on first activation.
    pub initial_zoom: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    /// Zoom multiplier per wheel notch (120 delta units).
    pub zoom_step: f32,
    /// Time constant of the zoom animation, ms (0 = instant).
    pub zoom_smoothing_ms: f32,
    /// Fade-in duration, ms (0 = instant).
    pub fade_in_ms: f32,
    /// Hold the hotkey this long before the lens shows (a wheel event shows it immediately).
    pub activation_delay_ms: u32,
    /// Bilinear filtering of the magnified image (false = crisp nearest-neighbour pixels).
    pub smoothing: bool,
    /// Hotkey: modifiers that must all be held. At least one must be set.
    pub hotkey_ctrl: bool,
    pub hotkey_alt: bool,
    pub hotkey_shift: bool,
    /// Hold this too (with the hotkey) to freeze the lens picture, e.g. to read tooltips that
    /// vanish when the mouse moves. Must not be part of the hotkey.
    pub freeze_key: FreezeKey,
    /// Border colour while the picture is frozen, sRGB 0xRRGGBB.
    pub freeze_border_color: u32,
    /// Periodically check GitHub releases of `update_repo` for a newer version.
    pub check_updates: bool,
    /// GitHub `owner/name` whose releases are checked.
    pub update_repo: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lens_width: 640,
            lens_height: 400,
            corner_radius: 18.0,
            border_width: 2.0,
            border_color: 0x9aa0a6,
            shadow_size: 14.0,
            initial_zoom: 2.0,
            min_zoom: 1.25,
            max_zoom: 16.0,
            zoom_step: 1.25,
            zoom_smoothing_ms: 45.0,
            fade_in_ms: 40.0,
            activation_delay_ms: 90,
            smoothing: true,
            hotkey_ctrl: true,
            hotkey_alt: true,
            hotkey_shift: false,
            freeze_key: FreezeKey::Shift,
            freeze_border_color: 0x3d8bfd,
            check_updates: true,
            update_repo: "artyfarty/best_loupe".into(),
        }
    }
}

impl Config {
    pub fn sanitize(mut self) -> Self {
        self.lens_width = self.lens_width.clamp(64, 4096);
        self.lens_height = self.lens_height.clamp(64, 4096);
        let half = self.lens_width.min(self.lens_height) as f32 / 2.0;
        self.corner_radius = self.corner_radius.clamp(0.0, half);
        self.border_width = self.border_width.clamp(0.0, 32.0);
        self.border_color &= 0xff_ffff;
        self.shadow_size = self.shadow_size.clamp(0.0, 128.0);
        self.min_zoom = self.min_zoom.clamp(1.0, 64.0);
        self.max_zoom = self.max_zoom.clamp(self.min_zoom, 64.0);
        self.initial_zoom = self.initial_zoom.clamp(self.min_zoom, self.max_zoom);
        self.zoom_step = self.zoom_step.clamp(1.01, 4.0);
        self.zoom_smoothing_ms = self.zoom_smoothing_ms.clamp(0.0, 1000.0);
        self.fade_in_ms = self.fade_in_ms.clamp(0.0, 1000.0);
        self.activation_delay_ms = self.activation_delay_ms.min(2000);
        if !(self.hotkey_ctrl || self.hotkey_alt || self.hotkey_shift) {
            self.hotkey_ctrl = true;
            self.hotkey_alt = true;
        }
        self.freeze_border_color &= 0xff_ffff;
        let clash = match self.freeze_key {
            FreezeKey::Shift => self.hotkey_shift,
            FreezeKey::Ctrl => self.hotkey_ctrl,
            FreezeKey::Alt => self.hotkey_alt,
            FreezeKey::None => false,
        };
        if clash {
            self.freeze_key = FreezeKey::None;
        }
        self
    }
}

static CURRENT: RwLock<Option<Config>> = RwLock::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Snapshot of the live config.
pub fn current() -> Config {
    CURRENT.read().unwrap().clone().unwrap_or_default()
}

/// Bumped on every change; compare against a cached value to detect updates.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Applies `f` to the live config, sanitizes, saves to disk and notifies listeners.
pub fn update(f: impl FnOnce(&mut Config)) {
    let cfg = {
        let mut guard = CURRENT.write().unwrap();
        let mut cfg = guard.clone().unwrap_or_default();
        f(&mut cfg);
        let cfg = cfg.sanitize();
        if guard.as_ref() == Some(&cfg) {
            return;
        }
        *guard = Some(cfg.clone());
        cfg
    };
    GENERATION.fetch_add(1, Ordering::AcqRel);
    save(&cfg);
    crate::input::apply_config(&cfg);
}

pub fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("best_loupe").join("config.toml"))
}

fn save(cfg: &Config) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match toml::to_string_pretty(cfg) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                crate::log!("config: write {} failed: {e}", path.display());
            }
        }
        Err(e) => crate::log!("config: serialize failed: {e}"),
    }
}

/// Loads the config from disk into the live slot; writes a default file if none exists.
/// Never fails: falls back to defaults.
pub fn load() -> Config {
    let cfg = match config_path().map(|p| (std::fs::read_to_string(&p), p)) {
        None => {
            crate::log!("config: APPDATA not set, using defaults");
            Config::default()
        }
        Some((Ok(text), path)) => match toml::from_str::<Config>(&text) {
            Ok(cfg) => {
                crate::log!("config: loaded {}", path.display());
                cfg.sanitize()
            }
            Err(e) => {
                crate::log!("config: parse error in {}: {e}; using defaults", path.display());
                Config::default()
            }
        },
        Some((Err(_), path)) => {
            let cfg = Config::default();
            save(&cfg);
            crate::log!("config: wrote defaults to {}", path.display());
            cfg
        }
    };
    *CURRENT.write().unwrap() = Some(cfg.clone());
    GENERATION.fetch_add(1, Ordering::AcqRel);
    crate::input::apply_config(&cfg);
    cfg
}
