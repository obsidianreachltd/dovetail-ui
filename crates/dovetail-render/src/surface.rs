use crate::{Color, DisplayAndWindowHandle, Gpu, GpuError};

/// A size in physical pixels: the pixels the display actually has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalSize {
  /// Width in physical pixels.
  pub width: u32,
  /// Height in physical pixels.
  pub height: u32,
}

impl PhysicalSize {
  /// True when either side is zero, as when a window is minimised.
  pub fn is_empty(self) -> bool {
    self.width == 0 || self.height == 0
  }
}

/// What happened when a frame was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOutcome {
  /// The frame was drawn and shown.
  Presented,
  /// Nothing was drawn because the window has no area (it is minimised).
  Skipped,
  /// The surface was out of date and has been set up again. Draw again.
  Retry,
  /// The GPU device is lost. Recover the [`Gpu`], reconfigure, and draw again.
  DeviceLost,
}

/// The part of one window that the GPU draws into.
pub struct WindowSurface {
  surface: wgpu::Surface<'static>,
  config: Option<wgpu::SurfaceConfiguration>,
  size: PhysicalSize,
}

impl WindowSurface {
  /// Creates the surface for a window.
  ///
  /// The window must stay alive as long as the surface, so it is taken by
  /// shared ownership, for example `Arc<winit::window::Window>`.
  /// For the first window, use [`Gpu::with_window`] instead.
  pub fn new(
    gpu: &Gpu,
    window: impl DisplayAndWindowHandle + 'static,
    size: PhysicalSize,
  ) -> Result<Self, GpuError> {
    let surface = gpu
      .instance()
      .create_surface(window)
      .map_err(GpuError::Surface)?;
    Ok(Self::from_raw(gpu, surface, size))
  }

  pub(crate) fn from_raw(
    gpu: &Gpu,
    surface: wgpu::Surface<'static>,
    size: PhysicalSize,
  ) -> Self {
    let mut this = Self {
      surface,
      config: None,
      size: PhysicalSize::default(),
    };
    this.resize(gpu, size);
    this
  }

  /// The wgpu surface, so the adapter chosen after a device loss can draw
  /// into it.
  pub fn raw(&self) -> &wgpu::Surface<'static> {
    &self.surface
  }

  /// The current size in physical pixels.
  pub fn size(&self) -> PhysicalSize {
    self.size
  }

  /// The pixel format of the surface, once it has a size.
  pub fn format(&self) -> Option<wgpu::TextureFormat> {
    self.config.as_ref().map(|c| c.format)
  }

  /// Changes the size. A zero size (a minimised window) is remembered but
  /// not applied, because wgpu does not allow it. Drawing is skipped until
  /// the window has an area again.
  pub fn resize(
    &mut self,
    gpu: &Gpu,
    size: PhysicalSize,
  ) {
    self.size = size;
    if size.is_empty() {
      return;
    }
    let config = match self.config.take() {
      Some(mut config) => {
        config.width = size.width;
        config.height = size.height;
        Some(config)
      },
      None => self.default_config(gpu, size),
    };
    if let Some(config) = &config {
      self.surface.configure(gpu.device(), config);
    }
    self.config = config;
  }

  /// Sets the surface up again from scratch with the device it already
  /// uses, for example after the surface reports that it is out of date.
  /// After a device loss, make a new surface instead (see [`Gpu::replacing`]).
  pub fn reconfigure(
    &mut self,
    gpu: &Gpu,
  ) {
    self.config = None;
    self.resize(gpu, self.size);
  }

  fn default_config(
    &self,
    gpu: &Gpu,
    size: PhysicalSize,
  ) -> Option<wgpu::SurfaceConfiguration> {
    let mut config = self.surface.get_default_config(
      &gpu.current().adapter,
      size.width,
      size.height,
    )?;
    // Prefer an sRGB format, so colours match what designers expect.
    let caps = self
      .surface
      .get_capabilities(&gpu.current().adapter);
    if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
      config.format = srgb;
    }
    // Fifo (wait for the display) is supported everywhere and saves power.
    config.present_mode = wgpu::PresentMode::AutoVsync;
    Some(config)
  }

  /// Draws one frame that clears the whole window to `color`, and shows it.
  pub fn draw_clear(
    &mut self,
    gpu: &Gpu,
    color: Color,
  ) -> FrameOutcome {
    let Some(format) = self.format() else {
      return FrameOutcome::Skipped;
    };
    let clear = crate::color::to_wgpu(color, format);
    self.draw_with(gpu, |encoder, view| {
      let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("dovetail clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          depth_slice: None,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear),
            store: wgpu::StoreOp::Store,
          },
        })],
        ..Default::default()
      });
    })
  }

  /// Draws one frame: gets the window's next image, lets `record` add GPU
  /// work that draws into it, then submits and shows it.
  pub fn draw_with(
    &mut self,
    gpu: &Gpu,
    record: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView),
  ) -> FrameOutcome {
    if gpu.is_lost() {
      return FrameOutcome::DeviceLost;
    }
    if self.config.is_none() || self.size.is_empty() {
      return FrameOutcome::Skipped;
    }
    let (frame, suboptimal) = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
      // Suboptimal still works; set it up again after this frame.
      wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
      wgpu::CurrentSurfaceTexture::Timeout
      | wgpu::CurrentSurfaceTexture::Occluded => {
        return FrameOutcome::Skipped;
      },
      wgpu::CurrentSurfaceTexture::Outdated
      | wgpu::CurrentSurfaceTexture::Lost => {
        self.reconfigure(gpu);
        return FrameOutcome::Retry;
      },
      wgpu::CurrentSurfaceTexture::Validation => {
        return if gpu.is_lost() {
          FrameOutcome::DeviceLost
        } else {
          FrameOutcome::Skipped
        };
      },
    };
    let view = frame
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
      gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
          label: Some("dovetail frame"),
        });
    record(&mut encoder, &view);
    gpu.queue().submit([encoder.finish()]);
    gpu.queue().present(frame);
    if suboptimal {
      self.reconfigure(gpu);
    }
    if gpu.is_lost() {
      FrameOutcome::DeviceLost
    } else {
      FrameOutcome::Presented
    }
  }
}

#[cfg(test)]
mod tests {
  use super::PhysicalSize;

  #[test]
  fn zero_sized_window_is_empty() {
    assert!(PhysicalSize { width: 0, height: 600 }.is_empty());
    assert!(PhysicalSize { width: 800, height: 0 }.is_empty());
    assert!(!PhysicalSize { width: 1, height: 1 }.is_empty());
  }
}
