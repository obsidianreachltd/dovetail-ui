use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::block_on::block_on;
use crate::{
  DisplayAndWindowHandle, PhysicalSize, WgpuHasDisplayHandle, WindowSurface,
};

/// Why the GPU could not be set up.
#[derive(Debug)]
pub enum GpuError {
  /// No GPU adapter fits (for example, no driver is installed).
  NoAdapter(wgpu::RequestAdapterError),
  /// The adapter was found, but it would not create a device.
  NoDevice(wgpu::RequestDeviceError),
  /// wgpu could not create a surface for a window.
  Surface(wgpu::CreateSurfaceError),
}

impl fmt::Display for GpuError {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    match self {
      GpuError::NoAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
      GpuError::NoDevice(e) => {
        write!(f, "the GPU adapter refused to create a device: {e}")
      },
      GpuError::Surface(e) => {
        write!(f, "could not create a window surface: {e}")
      },
    }
  }
}

impl std::error::Error for GpuError {}

/// One device and queue, made from one adapter.
pub(crate) struct DeviceSet {
  pub(crate) adapter: wgpu::Adapter,
  pub(crate) device: wgpu::Device,
  pub(crate) queue: wgpu::Queue,
  lost: Arc<AtomicBool>,
}

/// The GPU, shared by every window in the application (D0009).
///
/// `Gpu` owns the wgpu instance, the adapter and the current device. When
/// windows are open and the device is lost, the whole `Gpu` and every
/// surface are replaced; see [`Gpu::replacing`].
pub struct Gpu {
  instance: wgpu::Instance,
  current: DeviceSet,
  /// How many times the device has been replaced.
  generation: u64,
}

impl Gpu {
  /// Sets up the GPU together with the surface of the first window, so
  /// the adapter chosen is one that can draw into that window.
  ///
  /// `display` is the platform's display connection (from winit,
  /// `OwnedDisplayHandle`). Some backends need it, such as OpenGL on Wayland.
  pub fn with_window(
    display: Box<dyn WgpuHasDisplayHandle>,
    window: impl DisplayAndWindowHandle + 'static,
    size: PhysicalSize,
  ) -> Result<(Self, WindowSurface), GpuError> {
    let instance = wgpu::Instance::new(
      wgpu::InstanceDescriptor::new_with_display_handle_from_env(display),
    );
    let surface = instance
      .create_surface(window)
      .map_err(GpuError::Surface)?;
    let current = request_device(&instance, Some(&surface))?;
    let gpu = Self { instance, current, generation: 0 };
    let surface = WindowSurface::from_raw(&gpu, surface, size);
    Ok((gpu, surface))
  }

  /// Sets up the GPU without any window, for tests and off-screen work.
  pub fn headless() -> Result<Self, GpuError> {
    let instance = wgpu::Instance::new(
      wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
    );
    let current = request_device(&instance, None)?;
    Ok(Self { instance, current, generation: 0 })
  }

  /// The wgpu instance. Window surfaces are created from it.
  pub(crate) fn instance(&self) -> &wgpu::Instance {
    &self.instance
  }

  pub(crate) fn current(&self) -> &DeviceSet {
    &self.current
  }

  /// The wgpu device. Custom render passes use it (M09).
  pub fn device(&self) -> &wgpu::Device {
    &self.current.device
  }

  /// The wgpu queue that work is submitted to.
  pub fn queue(&self) -> &wgpu::Queue {
    &self.current.queue
  }

  /// Facts about the adapter in use, such as its name and backend.
  pub fn adapter_info(&self) -> wgpu::AdapterInfo {
    self.current.adapter.get_info()
  }

  /// How many times the device has been replaced since start-up.
  pub fn generation(&self) -> u64 {
    self.generation
  }

  /// True when the device has been lost and must be replaced with
  /// [`Gpu::recover`].
  pub fn is_lost(&self) -> bool {
    self.current.lost.load(Ordering::Acquire)
  }

  /// Marks this GPU as the replacement for a lost one whose
  /// [`Gpu::generation`] was `lost_generation`.
  ///
  /// Recovery with windows works like this (B0001, B0002):
  ///
  /// 1. Drop every [`WindowSurface`] while the lost GPU still exists. Each
  ///    one unconfigures itself on the device it was set up with.
  /// 2. Drop the lost GPU. On DX12 its command queue keeps the old
  ///    swapchains alive, and a window can have only one.
  /// 3. Make a new GPU with [`Gpu::with_window`] for one window, and new
  ///    surfaces for the others with [`WindowSurface::new`].
  /// 4. Call `new_gpu.replacing(lost_generation)`.
  ///
  /// Reconfiguring an old surface with a new device does not work: wgpu
  /// keeps the old swapchain, which then waits on the destroyed device.
  pub fn replacing(
    mut self,
    lost_generation: u64,
  ) -> Self {
    self.generation = lost_generation + 1;
    self
  }

  /// Replaces a lost device with a new one, for a GPU with no windows.
  /// With windows, follow the steps in [`Gpu::replacing`]. Afterwards, call
  /// [`WindowSurface::reconfigure`](crate::WindowSurface::reconfigure) on
  /// every surface.
  pub fn recover(
    &mut self,
    compatible_with: Option<&wgpu::Surface<'_>>,
  ) -> Result<(), GpuError> {
    self.current = request_device(&self.instance, compatible_with)?;
    self.generation += 1;
    Ok(())
  }

  /// Destroys the device on purpose, as if the driver had been reset.
  /// Used to test that recovery works.
  #[cfg(feature = "device-loss-testing")]
  pub fn simulate_device_loss(&self) {
    self.current.device.destroy();
    // Destroying marks the device lost through the callback. Some
    // backends report it late, so mark it here as well.
    self.current.lost.store(true, Ordering::Release);
  }
}

fn request_device(
  instance: &wgpu::Instance,
  compatible_with: Option<&wgpu::Surface<'_>>,
) -> Result<DeviceSet, GpuError> {
  let adapter =
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
      compatible_surface: compatible_with,
      ..Default::default()
    }))
    .map_err(GpuError::NoAdapter)?;

  let (device, queue) =
    block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      label: Some("dovetail device"),
      ..Default::default()
    }))
    .map_err(GpuError::NoDevice)?;

  let lost = Arc::new(AtomicBool::new(false));
  let flag = Arc::clone(&lost);
  device.set_device_lost_callback(move |reason, message| {
    eprintln!("dovetail: GPU device lost ({reason:?}): {message}");
    flag.store(true, Ordering::Release);
  });

  // A lost device reports errors for every call made before recovery.
  // Without a handler, wgpu would stop the program on the first one.
  let flag = Arc::clone(&lost);
  device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
    if !flag.load(Ordering::Acquire) {
      eprintln!("dovetail: GPU error: {error}");
    }
  }));

  Ok(DeviceSet { adapter, device, queue, lost })
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Needs a GPU or a software adapter, so it is not run by default.
  /// Run it with `cargo test -p dovetail-render --features device-loss-testing -- --ignored`.
  #[test]
  #[ignore = "needs a GPU adapter"]
  #[cfg(feature = "device-loss-testing")]
  fn lost_device_is_detected_and_replaced() {
    let mut gpu = Gpu::headless().expect("GPU");
    assert!(!gpu.is_lost());
    gpu.simulate_device_loss();
    assert!(gpu.is_lost());
    gpu.recover(None).expect("recover");
    assert!(!gpu.is_lost());
    assert_eq!(gpu.generation(), 1);

    // The new device works: submit an empty command buffer.
    let encoder = gpu
      .device()
      .create_command_encoder(&Default::default());
    gpu.queue().submit([encoder.finish()]);
  }
}
