//! Non-panicking detection of a usable CUDA GPU.
//!
//! burn loads the CUDA driver lazily and panics if it is missing. Since
//! release builds abort on panic, we check for it up front so the training can
//! fall back to the CPU instead of crashing.
//!
//! Only the driver is required: cubecl compiles kernels in-process (through
//! LLVM to PTX), so neither nvcc nor NVRTC has to be installed.

// The only unsafe code of this crate: probing for a shared library. See the
// SAFETY comment below.
#![allow(unsafe_code)]

use cudarc::driver::CudaContext;

use super::GpuInfo;

/// Returns the first CUDA device, or why none can be used.
pub(super) fn probe() -> Result<GpuInfo, String> {
    // SAFETY: this only attempts to `dlopen` the CUDA driver library and drops
    // the handle. It is `unsafe` because loading a shared library runs its
    // initialisers; this is the NVIDIA driver that burn would load anyway, so no
    // code runs that the GPU path wouldn't run.
    let driver_present = unsafe { cudarc::driver::sys::is_culib_present() };
    if !driver_present {
        return Err(
            "CUDA driver not found (no NVIDIA GPU, or container without GPU access)".into(),
        );
    }

    let count = CudaContext::device_count().map_err(|err| format!("CUDA init failed: {err}"))?;
    if count < 1 {
        return Err("CUDA driver loaded but no device visible".into());
    }

    let ctx = CudaContext::new(0).map_err(|err| format!("cannot open CUDA device 0: {err}"))?;
    let describe = |err| format!("cannot query CUDA device 0: {err}");
    let (free_vram, total_vram) = ctx.mem_get_info().map_err(describe)?;
    Ok(GpuInfo {
        name: ctx.name().map_err(describe)?,
        compute_capability: ctx.compute_capability().map_err(describe)?,
        total_vram,
        free_vram,
    })
}
