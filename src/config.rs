//! User configuration, loaded from `%APPDATA%\best_loupe\config.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Hold Ctrl+Alt this long before the lens shows (a wheel event shows it immediately).
    pub activation_delay_ms: u32,
    /// Bilinear filtering of the magnified image (false = crisp nearest-neighbour pixels).
    pub smoothing: bool,
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
        }
    }
}

impl Config {
    fn sanitize(mut self) -> Self {
        self.lens_width = self.lens_width.clamp(64, 4096);
        self.lens_height = self.lens_height.clamp(64, 4096);
        let half = self.lens_width.min(self.lens_height) as f32 / 2.0;
        self.corner_radius = self.corner_radius.clamp(0.0, half);
        self.border_width = self.border_width.clamp(0.0, 32.0);
        self.shadow_size = self.shadow_size.clamp(0.0, 128.0);
        self.min_zoom = self.min_zoom.clamp(1.0, 64.0);
        self.max_zoom = self.max_zoom.clamp(self.min_zoom, 64.0);
        self.initial_zoom = self.initial_zoom.clamp(self.min_zoom, self.max_zoom);
        self.zoom_step = self.zoom_step.clamp(1.01, 4.0);
        self.zoom_smoothing_ms = self.zoom_smoothing_ms.clamp(0.0, 1000.0);
        self.fade_in_ms = self.fade_in_ms.clamp(0.0, 1000.0);
        self.activation_delay_ms = self.activation_delay_ms.min(2000);
        self
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("best_loupe").join("config.toml"))
}

/// Loads the config; writes a default file if none exists. Never fails: falls back to defaults.
pub fn load() -> Config {
    let Some(path) = config_path() else {
        crate::log!("config: APPDATA not set, using defaults");
        return Config::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<Config>(&text) {
            Ok(cfg) => {
                crate::log!("config: loaded {}", path.display());
                cfg.sanitize()
            }
            Err(e) => {
                crate::log!("config: parse error in {}: {e}; using defaults", path.display());
                Config::default()
            }
        },
        Err(_) => {
            let cfg = Config::default();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(text) = toml::to_string_pretty(&cfg) {
                let _ = std::fs::write(&path, text);
                crate::log!("config: wrote defaults to {}", path.display());
            }
            cfg
        }
    }
}
