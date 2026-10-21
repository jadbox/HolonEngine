//! Split-step Clifford Fourier lattice.
//!
//! The even part of `G(2,0)` is stored as `re + im e12`, i.e. one complex
//! number per cell. The nonlinear Schrödinger step is
//!
//! ```text
//! i ∂t Ψ = -α ∇²Ψ + V Ψ - λ |Ψ|² Ψ
//! ```
//!
//! integrated with Strang splitting:
//! `linear(dt/2) → nonlinear+potential(dt) → linear(dt/2)`.
//! Both factors have unit modulus, so mass is conserved to floating-point
//! precision when no damping or obstacle mask is applied.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

pub struct WaveSolver {
    pub n: usize,
    pub dx: f64,
    pub alpha: f64,
    pub lambda: f64,
    pub dt: f64,
    k2: Vec<f64>,
    fft: Arc<dyn Fft<f64>>,
    ifft: Arc<dyn Fft<f64>>,
    scratch: Vec<Complex<f64>>,
    col: Vec<Complex<f64>>,
}

impl WaveSolver {
    pub fn new(n: usize, dx: f64, alpha: f64, lambda: f64, dt: f64) -> Self {
        let mut planner = FftPlanner::<f64>::new();
        let fft = planner.plan_fft_forward(n);
        let ifft = planner.plan_fft_inverse(n);
        let scratch_len = fft
            .get_inplace_scratch_len()
            .max(ifft.get_inplace_scratch_len());
        let mut k2 = vec![0.0; n * n];
        let two_pi_over_l = 2.0 * std::f64::consts::PI / (n as f64 * dx);
        for j in 0..n {
            let ky = if j <= n / 2 {
                j as f64 * two_pi_over_l
            } else {
                (j as f64 - n as f64) * two_pi_over_l
            };
            for i in 0..n {
                let kx = if i <= n / 2 {
                    i as f64 * two_pi_over_l
                } else {
                    (i as f64 - n as f64) * two_pi_over_l
                };
                k2[j * n + i] = kx * kx + ky * ky;
            }
        }
        Self {
            n,
            dx,
            alpha,
            lambda,
            dt,
            k2,
            fft,
            ifft,
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            col: vec![Complex::new(0.0, 0.0); n],
        }
    }

    /// Advance one split-step.
    ///
    /// `potential` is a real `V` map (phase only); `damping` is a per-step
    /// multiplier; `obstacle` zeros cells after the step (hard impedance).
    pub fn step(
        &mut self,
        psi: &mut [Complex<f64>],
        potential: Option<&[f64]>,
        damping: Option<&[f64]>,
        obstacle: Option<&[bool]>,
    ) {
        assert_eq!(psi.len(), self.n * self.n);
        self.linear_half(psi);

        if let Some(v) = potential {
            for (p, &vv) in psi.iter_mut().zip(v) {
                let phase = (self.lambda * p.norm_sqr() - vv) * self.dt;
                *p *= Complex::from_polar(1.0, phase);
            }
        } else {
            for p in psi.iter_mut() {
                let phase = self.lambda * p.norm_sqr() * self.dt;
                *p *= Complex::from_polar(1.0, phase);
            }
        }

        self.linear_half(psi);

        if let Some(d) = damping {
            for (p, &dd) in psi.iter_mut().zip(d) {
                *p *= dd;
            }
        }
        if let Some(o) = obstacle {
            for (p, &oo) in psi.iter_mut().zip(o) {
                if oo {
                    *p = Complex::new(0.0, 0.0);
                }
            }
        }
    }

    fn linear_half(&mut self, psi: &mut [Complex<f64>]) {
        self.fft2(psi);
        let half = -0.5 * self.alpha * self.dt;
        for (p, &k) in psi.iter_mut().zip(&self.k2) {
            *p *= Complex::from_polar(1.0, half * k);
        }
        self.ifft2(psi);
    }

    fn fft2(&mut self, data: &mut [Complex<f64>]) {
        let n = self.n;
        let fft = Arc::clone(&self.fft);

        for j in 0..n {
            let row = &mut data[j * n..(j + 1) * n];
            fft.process_with_scratch(row, &mut self.scratch);
        }

        for i in 0..n {
            for j in 0..n {
                self.col[j] = data[j * n + i];
            }
            fft.process_with_scratch(&mut self.col, &mut self.scratch);
            for j in 0..n {
                data[j * n + i] = self.col[j];
            }
        }
    }

    fn ifft2(&mut self, data: &mut [Complex<f64>]) {
        let n = self.n;
        let ifft = Arc::clone(&self.ifft);

        for j in 0..n {
            let row = &mut data[j * n..(j + 1) * n];
            ifft.process_with_scratch(row, &mut self.scratch);
        }

        for i in 0..n {
            for j in 0..n {
                self.col[j] = data[j * n + i];
            }
            ifft.process_with_scratch(&mut self.col, &mut self.scratch);
            for j in 0..n {
                data[j * n + i] = self.col[j];
            }
        }

        let inv = 1.0 / (n * n) as f64;
        for v in data {
            *v *= inv;
        }
    }
}

/// Mass `Σ |Ψ|² dx²`.
pub fn mass(psi: &[Complex<f64>], dx: f64) -> f64 {
    psi.iter().map(|c| c.norm_sqr()).sum::<f64>() * dx * dx
}

/// Maximum amplitude.
pub fn peak(psi: &[Complex<f64>]) -> f64 {
    psi.iter().map(|c| c.norm()).fold(0.0, f64::max)
}

/// Per-step absorbing sponge factor near the domain edges.
///
/// `width` is in cells; `strength` is the edge attenuation exponent per step.
pub fn sponge(n: usize, width: usize, strength: f64) -> Vec<f64> {
    let mut out = vec![1.0; n * n];
    if width == 0 {
        return out;
    }
    for j in 0..n {
        for i in 0..n {
            let edge = i.min(n - 1 - i).min(j).min(n - 1 - j);
            if edge < width {
                let t = 1.0 - edge as f64 / width as f64;
                out[j * n + i] = (-strength * t * t).exp();
            }
        }
    }
    out
}

/// Boolean disk mask.
pub fn disk_mask(n: usize, dx: f64, cx: f64, cy: f64, radius: f64) -> Vec<bool> {
    let mut out = vec![false; n * n];
    let ccx = cx / dx - 0.5;
    let ccy = cy / dx - 0.5;
    let r2 = (radius / dx) * (radius / dx);
    for j in 0..n {
        for i in 0..n {
            let d2 = (i as f64 - ccx).powi(2) + (j as f64 - ccy).powi(2);
            out[j * n + i] = d2 <= r2;
        }
    }
    out
}

/// Disk potential `V = height` inside `radius`.
pub fn disk_potential(n: usize, dx: f64, cx: f64, cy: f64, radius: f64, height: f64) -> Vec<f64> {
    let mask = disk_mask(n, dx, cx, cy, radius);
    mask.iter().map(|&m| if m { height } else { 0.0 }).collect()
}

/// Add a soft disk source with phase `-ωt`.
#[allow(clippy::too_many_arguments)]
pub fn add_drive(
    psi: &mut [Complex<f64>],
    n: usize,
    dx: f64,
    dt: f64,
    cx: f64,
    cy: f64,
    radius: f64,
    amplitude: f64,
    omega: f64,
    t: f64,
) {
    let ccx = cx / dx - 0.5;
    let ccy = cy / dx - 0.5;
    let r2 = (radius / dx) * (radius / dx);
    let sigma2 = (0.5 * radius / dx).powi(2).max(1e-9);
    let phase = -omega * t;
    for j in 0..n {
        for i in 0..n {
            let d2 = (i as f64 - ccx).powi(2) + (j as f64 - ccy).powi(2);
            if d2 <= r2 {
                let falloff = (-0.5 * d2 / sigma2).exp();
                psi[j * n + i] += Complex::from_polar(amplitude * falloff * dt, phase);
            }
        }
    }
}

/// 3×3 binomial blur with edge clamping.
pub fn blur3(src: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * n];
    const K: [[f64; 3]; 3] = [[1.0, 2.0, 1.0], [2.0, 4.0, 2.0], [1.0, 2.0, 1.0]];
    for j in 0..n {
        for i in 0..n {
            let mut acc = 0.0;
            for (dj, row) in K.iter().enumerate() {
                let jj = (j as isize + dj as isize - 1).clamp(0, n as isize - 1) as usize;
                for (di, &w) in row.iter().enumerate() {
                    let ii = (i as isize + di as isize - 1).clamp(0, n as isize - 1) as usize;
                    acc += w * src[jj * n + ii];
                }
            }
            out[j * n + i] = acc / 16.0;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smooth_field(n: usize, dx: f64) -> Vec<Complex<f64>> {
        let mut psi = vec![Complex::new(0.0, 0.0); n * n];
        for j in 0..n {
            for i in 0..n {
                let x = (i as f64 + 0.5) * dx;
                let y = (j as f64 + 0.5) * dx;
                let env = (-((x - 0.5).powi(2) + (y - 0.5).powi(2)) / 0.04).exp();
                psi[j * n + i] = Complex::new(env * (12.0 * x).cos(), env * (9.0 * y).sin());
            }
        }
        psi
    }

    #[test]
    fn mass_is_conserved_without_damping() {
        let (n, dx) = (32, 1.0 / 32.0);
        let mut solver = WaveSolver::new(n, dx, 0.5, 2.0, 0.001);
        let mut psi = smooth_field(n, dx);
        let m0 = mass(&psi, dx);
        for _ in 0..100 {
            solver.step(&mut psi, None, None, None);
        }
        let m1 = mass(&psi, dx);
        assert!((m1 - m0).abs() / m0 < 1e-12);
    }

    #[test]
    fn zero_field_stays_zero() {
        let (n, dx) = (16, 1.0 / 16.0);
        let mut solver = WaveSolver::new(n, dx, 0.5, 2.0, 0.001);
        let mut psi = vec![Complex::new(0.0, 0.0); n * n];
        solver.step(&mut psi, None, None, None);
        assert!(peak(&psi) == 0.0);
    }
}
