//! Desktop Duplication (DXGI) capture of one output into our own shader-readable texture.

use std::cell::Cell;

use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{Interface, Result};

pub enum Acquire {
    /// A new desktop image was copied into the destination texture.
    NewFrame,
    /// Nothing changed (or only the pointer moved).
    NoChange,
    /// Duplication must be recreated (mode change, secure desktop, fullscreen switch...).
    Lost,
}

pub struct Duplication {
    dup: IDXGIOutputDuplication,
    /// No image copied yet: the first acquired frame holds the full desktop even if
    /// `LastPresentTime` is 0, and a static desktop would never produce another one.
    fresh: Cell<bool>,
    pub width: u32,
    pub height: u32,
}

impl Duplication {
    pub fn new(output: &IDXGIOutput, device: &ID3D11Device) -> Result<Self> {
        let dup = unsafe {
            match output.cast::<IDXGIOutput5>() {
                // DuplicateOutput1 is required for per-monitor-DPI-aware (v2) processes.
                Ok(o5) => o5.DuplicateOutput1(device, 0, &[DXGI_FORMAT_B8G8R8A8_UNORM])?,
                Err(_) => output.cast::<IDXGIOutput1>()?.DuplicateOutput(device)?,
            }
        };
        let desc = unsafe { dup.GetDesc() };
        if desc.Rotation != DXGI_MODE_ROTATION_IDENTITY && desc.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED {
            crate::log!("capture: rotated output ({:?}) is not supported yet", desc.Rotation);
        }
        Ok(Self { dup, fresh: Cell::new(true), width: desc.ModeDesc.Width, height: desc.ModeDesc.Height })
    }

    /// Waits up to `timeout_ms` for a new frame and copies it into `dst` (same size, BGRA).
    pub fn acquire_into(&self, ctx: &ID3D11DeviceContext, dst: &ID3D11Texture2D, timeout_ms: u32) -> Result<Acquire> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        match unsafe { self.dup.AcquireNextFrame(timeout_ms, &mut info, &mut resource) } {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(Acquire::NoChange),
            Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => return Ok(Acquire::Lost),
            Err(e) => return Err(e),
        }
        let mut result = Ok(Acquire::NoChange);
        if info.LastPresentTime != 0 || self.fresh.get() {
            if let Some(res) = resource {
                result = res.cast::<ID3D11Texture2D>().map(|tex| {
                    unsafe { ctx.CopyResource(dst, &tex) };
                    self.fresh.set(false);
                    Acquire::NewFrame
                });
            }
        }
        unsafe {
            let _ = self.dup.ReleaseFrame();
        }
        result
    }
}
