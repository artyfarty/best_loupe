//! Render thread: overlay window + DirectComposition swapchain on the output under the cursor,
//! fed by Desktop Duplication, drawn with the lens shader. See docs/design.md.

use std::time::Instant;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D::Fxc::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, PCSTR, Result, s, w};

use crate::capture::{Acquire, Duplication};
use crate::config::Config;
use crate::{input, log};

const SHADER_SRC: &str = include_str!("lens.hlsl");
const WINDOW_CLASS: windows::core::PCWSTR = w!("best_loupe_overlay");

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Constants {
    lens: [f32; 4],
    cursor_zoom: [f32; 4],
    style: [f32; 4],
    border_rgb: [f32; 4],
    viewport: [f32; 4],
}

struct OutputInfo {
    adapter: IDXGIAdapter1,
    output: IDXGIOutput,
    rect: RECT,
    name: String,
}

fn find_output_at(pt: POINT) -> Result<Option<OutputInfo>> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut ai = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(ai) } {
        ai += 1;
        let mut oi = 0;
        while let Ok(output) = unsafe { adapter.EnumOutputs(oi) } {
            oi += 1;
            let desc = unsafe { output.GetDesc()? };
            let r = desc.DesktopCoordinates;
            if desc.AttachedToDesktop.as_bool() && pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom {
                let name = String::from_utf16_lossy(&desc.DeviceName).trim_end_matches('\0').to_string();
                return Ok(Some(OutputInfo { adapter, output, rect: r, name }));
            }
        }
    }
    Ok(None)
}

fn rect_contains(r: &RECT, pt: POINT) -> bool {
    pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
}

fn compile(entry: PCSTR, target: PCSTR) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let res = unsafe {
        D3DCompile(
            SHADER_SRC.as_ptr() as _,
            SHADER_SRC.len(),
            s!("lens.hlsl"),
            None,
            None,
            entry,
            target,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(e) = res {
        if let Some(err) = errors {
            let msg = unsafe { std::slice::from_raw_parts(err.GetBufferPointer() as *const u8, err.GetBufferSize()) };
            log!("shader compile error: {}", String::from_utf8_lossy(msg));
        }
        return Err(e);
    }
    Ok(code.unwrap())
}

fn blob_bytes(b: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()) }
}

unsafe extern "system" fn overlay_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn register_class() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        RegisterClassExW(&wc);
    }
}

/// Everything bound to one output. Rebuilt when the cursor moves to another output or the device is lost.
struct Lens {
    info: OutputInfo,
    width: u32,
    height: u32,
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    swapchain: IDXGISwapChain2,
    waitable: HANDLE,
    rtv: ID3D11RenderTargetView,
    _dcomp: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
    hwnd: HWND,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cb: ID3D11Buffer,
    sampler: ID3D11SamplerState,
    desktop: Option<(ID3D11Texture2D, ID3D11ShaderResourceView)>,
    dup: Option<Duplication>,
    has_frame: bool,
    shown: bool,
}

impl Lens {
    fn new(info: OutputInfo, cfg: &Config) -> Result<Self> {
        let width = (info.rect.right - info.rect.left) as u32;
        let height = (info.rect.bottom - info.rect.top) as u32;
        log!("lens: building for {} {}x{} at ({},{})", info.name, width, height, info.rect.left, info.rect.top);

        let mut device = None;
        let mut ctx = None;
        unsafe {
            D3D11CreateDevice(
                &info.adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )?;
        }
        let device = device.unwrap();
        let ctx = ctx.unwrap();
        let dxgi_device: IDXGIDevice = device.cast()?;
        let factory: IDXGIFactory2 = unsafe { info.adapter.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
            ..Default::default()
        };
        let swapchain: IDXGISwapChain2 = unsafe { factory.CreateSwapChainForComposition(&device, &desc, None)? }.cast()?;
        unsafe { swapchain.SetMaximumFrameLatency(1)? };
        let waitable = unsafe { swapchain.GetFrameLatencyWaitableObject() };
        let backbuffer: ID3D11Texture2D = unsafe { swapchain.GetBuffer(0)? };
        let mut rtv = None;
        unsafe { device.CreateRenderTargetView(&backbuffer, None, Some(&mut rtv))? };
        let rtv = rtv.unwrap();

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP,
                WINDOW_CLASS,
                w!("best_loupe"),
                WS_POPUP,
                info.rect.left,
                info.rect.top,
                width as i32,
                height as i32,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        unsafe {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
            // Must be set before the first show, or the lens would capture (and magnify) itself.
            if let Err(e) = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) {
                log!("lens: SetWindowDisplayAffinity failed: {e}");
            }
        }

        let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi_device)? };
        let target = unsafe { dcomp.CreateTargetForHwnd(hwnd, true)? };
        let visual = unsafe { dcomp.CreateVisual()? };
        unsafe {
            visual.SetContent(&swapchain)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
        }

        let vs_blob = compile(s!("vs_main"), s!("vs_5_0"))?;
        let ps_blob = compile(s!("ps_main"), s!("ps_5_0"))?;
        let mut vs = None;
        let mut ps = None;
        unsafe {
            device.CreateVertexShader(blob_bytes(&vs_blob), None, Some(&mut vs))?;
            device.CreatePixelShader(blob_bytes(&ps_blob), None, Some(&mut ps))?;
        }

        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of::<Constants>() as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };
        let mut cb = None;
        unsafe { device.CreateBuffer(&cb_desc, None, Some(&mut cb))? };

        let filter = if cfg.smoothing { D3D11_FILTER_MIN_MAG_MIP_LINEAR } else { D3D11_FILTER_MIN_MAG_MIP_POINT };
        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: filter,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let mut sampler = None;
        unsafe { device.CreateSamplerState(&sampler_desc, Some(&mut sampler))? };

        Ok(Self {
            info,
            width,
            height,
            device,
            ctx,
            swapchain,
            waitable,
            rtv,
            _dcomp: dcomp,
            _target: target,
            _visual: visual,
            hwnd,
            vs: vs.unwrap(),
            ps: ps.unwrap(),
            cb: cb.unwrap(),
            sampler: sampler.unwrap(),
            desktop: None,
            dup: None,
            has_frame: false,
            shown: false,
        })
    }

    fn ensure_duplication(&mut self) -> Result<()> {
        if self.dup.is_some() {
            return Ok(());
        }
        let dup = Duplication::new(&self.info.output, &self.device)?;
        if dup.width != self.width || dup.height != self.height {
            log!("lens: duplication {}x{} != output {}x{}", dup.width, dup.height, self.width, self.height);
            return Err(DXGI_ERROR_ACCESS_LOST.into());
        }
        if self.desktop.is_none() {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.width,
                Height: self.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..Default::default()
            };
            let mut tex = None;
            let mut srv = None;
            unsafe {
                self.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
                self.device.CreateShaderResourceView(tex.as_ref().unwrap(), None, Some(&mut srv))?;
            }
            self.desktop = Some((tex.unwrap(), srv.unwrap()));
        }
        self.dup = Some(dup);
        Ok(())
    }

    /// Pulls the latest desktop image, if any. Recreates the duplication on loss.
    fn capture(&mut self, timeout_ms: u32) -> Result<()> {
        if self.dup.is_none() {
            if let Err(e) = self.ensure_duplication() {
                // Typical while a secure desktop / mode switch is in progress; keep the last frame.
                if e.code() == DXGI_ERROR_DEVICE_REMOVED || e.code() == DXGI_ERROR_DEVICE_RESET {
                    return Err(e);
                }
                return Ok(());
            }
        }
        let (dup, (tex, _)) = (self.dup.as_ref().unwrap(), self.desktop.as_ref().unwrap());
        match dup.acquire_into(&self.ctx, tex, timeout_ms)? {
            Acquire::NewFrame => self.has_frame = true,
            Acquire::NoChange => {}
            Acquire::Lost => {
                log!("lens: duplication lost, recreating");
                self.dup = None;
            }
        }
        Ok(())
    }

    fn wait_for_frame_slot(&self, wake: HANDLE, timeout_ms: u32) -> bool {
        let handles = [self.waitable, wake];
        let r = unsafe { MsgWaitForMultipleObjects(Some(&handles), false, timeout_ms, QS_ALLINPUT) };
        r == WAIT_OBJECT_0
    }

    fn render(&self, c: &Constants) -> Result<()> {
        unsafe {
            let ctx = &self.ctx;
            ctx.ClearRenderTargetView(&self.rtv, &[0.0; 4]);
            if self.has_frame {
                ctx.UpdateSubresource(&self.cb, 0, None, c as *const Constants as *const _, 0, 0);
                ctx.OMSetRenderTargets(Some(&[Some(self.rtv.clone())]), None);
                ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: self.width as f32,
                    Height: self.height as f32,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
                ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
                ctx.IASetInputLayout(None);
                ctx.VSSetShader(&self.vs, None);
                ctx.VSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
                ctx.PSSetShader(&self.ps, None);
                ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
                ctx.PSSetShaderResources(0, Some(&[Some(self.desktop.as_ref().unwrap().1.clone())]));
                ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                ctx.Draw(4, 0);
            }
            self.swapchain.Present(1, DXGI_PRESENT(0)).ok()
        }
    }

    fn show(&mut self) {
        if !self.shown {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            self.shown = true;
        }
    }

    fn hide(&mut self) {
        if self.shown {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            self.shown = false;
        }
        // Capture only lives while the lens is visible so games keep independent flip otherwise.
        self.dup = None;
        self.has_frame = false;
    }
}

impl Drop for Lens {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn cursor_pos() -> POINT {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    pt
}

fn srgb_to_floats(rgb: u32) -> [f32; 4] {
    let ch = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
    [ch(16), ch(8), ch(0), 1.0]
}

struct Animator {
    zoom_target: f32,
    zoom: f32,
    alpha: f32,
    last: Instant,
}

pub fn run(cfg: Config, wake: HANDLE) {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
    register_class();

    let mut lens: Option<Lens> = None;
    let mut active = false;
    let mut anim = Animator { zoom_target: cfg.initial_zoom, zoom: cfg.initial_zoom, alpha: 0.0, last: Instant::now() };
    let mut frames: u64 = 0;
    let mut stats_since = Instant::now();

    loop {
        pump_messages();

        let mut chord = input::chord_held();
        if active && chord {
            chord = input::verify_with_async_state();
        }
        let held_ms = input::now_ms().saturating_sub(input::chord_since_ms());
        let want = chord && (input::forced() || held_ms >= cfg.activation_delay_ms as u64);

        if !want {
            if active {
                if let Some(l) = lens.as_mut() {
                    l.hide();
                }
                active = false;
                log!("lens: hidden ({frames} frames since last stats)");
            }
            let timeout = if chord { cfg.activation_delay_ms as u64 - held_ms.min(cfg.activation_delay_ms as u64) } else { INFINITE as u64 };
            unsafe {
                MsgWaitForMultipleObjects(Some(&[wake]), false, timeout as u32, QS_ALLINPUT);
            }
            continue;
        }

        let cursor = cursor_pos();

        if !active {
            if lens.as_ref().is_some_and(|l| !rect_contains(&l.info.rect, cursor)) {
                lens = None;
            }
            if lens.is_none() {
                match find_output_at(cursor).and_then(|o| o.map(|o| Lens::new(o, &cfg)).transpose()) {
                    Ok(Some(l)) => lens = Some(l),
                    Ok(None) => {
                        log!("lens: no output under cursor {:?}", (cursor.x, cursor.y));
                        wait_or_wake(wake, 200);
                        continue;
                    }
                    Err(e) => {
                        log!("lens: build failed: {e}");
                        wait_or_wake(wake, 500);
                        continue;
                    }
                }
            }
            let l = lens.as_mut().unwrap();
            let t0 = Instant::now();
            // Block briefly for the first full desktop image so we never show an empty/stale lens.
            if let Err(e) = l.capture(100) {
                log!("lens: capture failed: {e}");
                lens = None;
                wait_or_wake(wake, 500);
                continue;
            }
            log!("lens: activated in {:.1} ms (frame: {})", t0.elapsed().as_secs_f64() * 1000.0, l.has_frame);
            active = true;
            anim.alpha = if cfg.fade_in_ms > 0.0 { 0.0 } else { 1.0 };
            anim.last = Instant::now();
            frames = 0;
            stats_since = Instant::now();
        }

        let l = lens.as_mut().unwrap();
        if !l.wait_for_frame_slot(wake, 100) {
            continue; // woken by input or messages: re-evaluate state first
        }

        // Cursor moved to another output: rebuild there on the next iteration.
        let cursor = cursor_pos();
        if !rect_contains(&l.info.rect, cursor) {
            l.hide();
            active = false;
            continue;
        }

        if let Err(e) = l.capture(0) {
            log!("lens: capture error {e}, rebuilding");
            lens = None;
            active = false;
            continue;
        }

        let now = Instant::now();
        let dt = now.duration_since(anim.last).as_secs_f32() * 1000.0;
        anim.last = now;

        let wheel = input::take_wheel();
        if wheel != 0 {
            anim.zoom_target = (anim.zoom_target * cfg.zoom_step.powf(wheel as f32 / 120.0)).clamp(cfg.min_zoom, cfg.max_zoom);
        }
        if cfg.zoom_smoothing_ms > 0.0 {
            let k = 1.0 - (-dt / cfg.zoom_smoothing_ms).exp();
            anim.zoom = (anim.zoom.ln() + (anim.zoom_target.ln() - anim.zoom.ln()) * k).exp();
            if (anim.zoom - anim.zoom_target).abs() < 1e-3 {
                anim.zoom = anim.zoom_target;
            }
        } else {
            anim.zoom = anim.zoom_target;
        }
        anim.alpha = if cfg.fade_in_ms > 0.0 { (anim.alpha + dt / cfg.fade_in_ms).min(1.0) } else { 1.0 };

        // Sample the cursor as late as possible.
        let cursor = cursor_pos();
        let (w, h) = (l.width as f32, l.height as f32);
        let cx = (cursor.x - l.info.rect.left) as f32 + 0.5;
        let cy = (cursor.y - l.info.rect.top) as f32 + 0.5;
        let lw = (cfg.lens_width as f32).min(w);
        let lh = (cfg.lens_height as f32).min(h);
        let x0 = (cx - lw / 2.0).round().clamp(0.0, w - lw);
        let y0 = (cy - lh / 2.0).round().clamp(0.0, h - lh);

        let c = Constants {
            lens: [x0, y0, x0 + lw, y0 + lh],
            cursor_zoom: [cx, cy, anim.zoom, anim.alpha],
            style: [cfg.corner_radius, cfg.border_width, cfg.shadow_size, 0.0],
            border_rgb: srgb_to_floats(cfg.border_color),
            viewport: [w, h, 1.0 / w, 1.0 / h],
        };
        if let Err(e) = l.render(&c) {
            log!("lens: present failed: {e}, rebuilding");
            lens = None;
            active = false;
            continue;
        }
        if l.has_frame && !l.shown {
            l.show();
            log!("lens: shown");
        }

        frames += 1;
        if stats_since.elapsed().as_secs() >= 5 {
            log!("lens: {:.1} fps, zoom {:.2}", frames as f64 / stats_since.elapsed().as_secs_f64(), anim.zoom);
            frames = 0;
            stats_since = Instant::now();
        }
    }
}

fn wait_or_wake(wake: HANDLE, ms: u32) {
    unsafe {
        WaitForSingleObject(wake, ms);
    }
}
