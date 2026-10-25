//! Compute device selection: CUDA when available, CPU otherwise.

#[cfg(feature = "cuda")]
mod cuda_probe;

use std::fmt;

use anyhow::{Result, bail};
use burn::tensor::Device;

/// Which device the user asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum DevicePreference {
    /// GPU if one is detected, CPU otherwise.
    #[default]
    Auto,
    /// GPU, failing if none is usable.
    Cuda,
    /// CPU, even if a GPU is present.
    Cpu,
}

/// Description of a detected NVIDIA GPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GpuInfo {
    pub(crate) name: String,
    pub(crate) compute_capability: (i32, i32),
    pub(crate) total_vram: usize,
    pub(crate) free_vram: usize,
}

/// What the training actually runs on, for logs and metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeviceInfo {
    /// Only constructed when built with the `cuda` feature.
    #[cfg_attr(not(feature = "cuda"), allow(dead_code))]
    Cuda(GpuInfo),
    Cpu {
        /// Why the CPU was chosen.
        reason: String,
    },
}

impl fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MIB: usize = 1024 * 1024;
        match self {
            Self::Cuda(gpu) => write!(
                f,
                "GPU {} (compute {}.{}, {} MiB VRAM, {} MiB free)",
                gpu.name,
                gpu.compute_capability.0,
                gpu.compute_capability.1,
                gpu.total_vram / MIB,
                gpu.free_vram / MIB,
            ),
            Self::Cpu { reason } => write!(f, "CPU fallback ({reason})"),
        }
    }
}

/// A burn device together with its description.
#[derive(Debug, Clone)]
pub(crate) struct SelectedDevice {
    pub(crate) device: Device,
    pub(crate) info: DeviceInfo,
}

/// Picks the training device according to `preference` and logs the choice.
pub(crate) fn select(preference: DevicePreference) -> Result<SelectedDevice> {
    let selected = match preference {
        DevicePreference::Cpu => cpu("CPU requested".into()),
        DevicePreference::Cuda => match probe_cuda() {
            Ok(gpu) => cuda(gpu),
            Err(reason) => bail!("CUDA requested but unavailable: {reason}"),
        },
        DevicePreference::Auto => match probe_cuda() {
            Ok(gpu) => cuda(gpu),
            Err(reason) => cpu(reason),
        },
    };
    tracing::info!(device = %selected.info, "training device selected");
    Ok(selected)
}

fn cpu(reason: String) -> SelectedDevice {
    SelectedDevice {
        device: Device::flex(),
        info: DeviceInfo::Cpu { reason },
    }
}

#[cfg(feature = "cuda")]
fn cuda(gpu: GpuInfo) -> SelectedDevice {
    SelectedDevice {
        device: Device::cuda(0),
        info: DeviceInfo::Cuda(gpu),
    }
}

#[cfg(not(feature = "cuda"))]
fn cuda(gpu: GpuInfo) -> SelectedDevice {
    // Unreachable: without the feature `probe_cuda` never returns a GPU.
    cpu(format!(
        "GPU {} found but binary built without CUDA",
        gpu.name
    ))
}

#[cfg(feature = "cuda")]
fn probe_cuda() -> Result<GpuInfo, String> {
    cuda_probe::probe()
}

#[cfg(not(feature = "cuda"))]
fn probe_cuda() -> Result<GpuInfo, String> {
    Err("binary built without the `cuda` feature".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_preference_always_succeeds() {
        let selected = select(DevicePreference::Cpu).unwrap();
        assert!(matches!(selected.info, DeviceInfo::Cpu { .. }));
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn auto_falls_back_to_cpu_without_cuda_feature() {
        let selected = select(DevicePreference::Auto).unwrap();
        assert!(selected.info.to_string().starts_with("CPU fallback"));
        assert!(select(DevicePreference::Cuda).is_err());
    }
}
