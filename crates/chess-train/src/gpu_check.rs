//! `gpu-check`: proves the selected device computes correctly and fast enough.
//!
//! Runs the same matmul on the selected device and on the reference CPU
//! backend, compares the results, then checks a backward pass, which is what
//! training relies on.

use std::time::Instant;

use anyhow::{Context, Result, ensure};
use burn::prelude::*;

use crate::device::SelectedDevice;

/// Maximum accepted relative difference between device and CPU results
/// (GPU matmuls may use TF32 or a different summation order).
const TOLERANCE: f32 = 1e-2;

pub(crate) fn run(selected: &SelectedDevice, size: usize) -> Result<()> {
    println!("device: {}", selected.info);
    ensure!(size > 0, "--size must be positive");

    let lhs = pseudo_random(size * size, 1);
    let rhs = pseudo_random(size * size, 2);
    let reference = Device::flex();
    let tensor = |data: &[f32], device: &Device| {
        Tensor::<2>::from_data(TensorData::new(data.to_vec(), [size, size]), device)
    };

    // Warm-up compiles the kernels (JIT on CUDA) so the timing below is meaningful.
    let warmup = tensor(&lhs, &selected.device).matmul(tensor(&rhs, &selected.device));
    to_vec(warmup)?;

    let start = Instant::now();
    let product = tensor(&lhs, &selected.device).matmul(tensor(&rhs, &selected.device));
    let product = to_vec(product)?;
    let elapsed = start.elapsed();
    let gflops = 2.0 * (size as f64).powi(3) / elapsed.as_secs_f64() / 1e9;
    println!(
        "matmul {size}x{size}: {:.1} ms ({gflops:.1} GFLOP/s)",
        elapsed.as_secs_f64() * 1e3
    );

    if selected.device != reference {
        let expected = to_vec(tensor(&lhs, &reference).matmul(tensor(&rhs, &reference)))?;
        let error = max_relative_error(&product, &expected);
        println!("max relative error vs CPU: {error:.2e}");
        ensure!(
            error < TOLERANCE,
            "device result differs from CPU ({error:.2e} >= {TOLERANCE})"
        );
    }

    check_backward(&selected.device)?;
    println!("backward pass: ok");
    println!("gpu-check passed");
    Ok(())
}

/// d/dx sum(x²) = 2x.
fn check_backward(device: &Device) -> Result<()> {
    let device = device.clone().autodiff();
    let values = [1.0_f32, -2.0, 3.5];
    let x = Tensor::<1>::from_data(TensorData::new(values.to_vec(), [values.len()]), &device)
        .require_grad();
    let grads = (x.clone() * x.clone()).sum().backward();
    let grad = x.grad(&grads).context("no gradient for x")?;
    let grad = to_vec(grad)?;
    for (g, v) in grad.iter().zip(values) {
        ensure!((g - 2.0 * v).abs() < 1e-4, "wrong gradient: {grad:?}");
    }

    // d/dx sum(σ(x)) = σ(x)(1 - σ(x)). The training loss uses the sigmoid,
    // whose `exp` needs CUDA's libdevice on NVIDIA GPUs: check it here.
    let x = Tensor::<1>::from_data(TensorData::new(values.to_vec(), [values.len()]), &device)
        .require_grad();
    let grads = burn::tensor::activation::sigmoid(x.clone())
        .sum()
        .backward();
    let grad = to_vec(x.grad(&grads).context("no gradient for σ")?)
        .context("sigmoid failed on this device (on CUDA: is libdevice installed?)")?;
    for (g, v) in grad.iter().zip(values) {
        let s = 1.0 / (1.0 + (-v).exp());
        ensure!(
            (g - s * (1.0 - s)).abs() < 1e-4,
            "wrong sigmoid gradient: {grad:?}"
        );
    }
    Ok(())
}

fn to_vec<const D: usize>(tensor: Tensor<D>) -> Result<Vec<f32>> {
    tensor
        .try_into_data()
        .map_err(|err| anyhow::anyhow!("device error: {err:?}"))?
        .try_to_vec::<f32>()
        .map_err(|err| anyhow::anyhow!("cannot read tensor data: {err:?}"))
}

fn max_relative_error(actual: &[f32], expected: &[f32]) -> f32 {
    let scale = expected
        .iter()
        .fold(0.0_f32, |m, v| m.max(v.abs()))
        .max(f32::EPSILON);
    actual
        .iter()
        .zip(expected)
        .fold(0.0_f32, |m, (a, e)| m.max((a - e).abs()))
        / scale
}

/// Deterministic values in `[-1, 1)` (xorshift), so runs are reproducible.
fn pseudo_random(len: usize, seed: u64) -> Vec<f32> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DevicePreference, select};

    #[test]
    fn passes_on_cpu() {
        let selected = select(DevicePreference::Cpu).unwrap();
        run(&selected, 64).unwrap();
    }

    #[test]
    fn pseudo_random_is_bounded_and_deterministic() {
        let a = pseudo_random(1000, 7);
        assert_eq!(a, pseudo_random(1000, 7));
        assert!(a.iter().all(|v| (-1.0..1.0).contains(v)));
    }
}
