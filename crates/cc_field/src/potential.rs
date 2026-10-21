//! Sampled `Q_Cl` and its gradient.
//!
//! The grid stores cell-centred samples on `[0, 1]²`:
//! `x_i = (i + 0.5) dx`. Gradients are central differences; boundaries use
//! edge clamping (Neumann). Bilinear sampling keeps the guidance smooth enough
//! for inertial boats.

use cc_math::CliffordQuantumPotential;
use rustfft::num_complex::Complex;

/// A scalar quantum-potential field plus its sampled gradient.
#[derive(Clone, Debug)]
pub struct QField {
    pub n: usize,
    pub dx: f64,
    /// `⟨Q_Cl⟩_0`
    pub q: Vec<f64>,
    /// `∂x ⟨Q_Cl⟩_0`
    pub gx: Vec<f64>,
    /// `∂y ⟨Q_Cl⟩_0`
    pub gy: Vec<f64>,
    /// `⟨Q_Cl⟩_2` (the `e12` coefficient). Zero for a real amplitude field.
    pub q12: Vec<f64>,
}

impl QField {
    /// Build `Q = -∇²R / R` from a real amplitude field.
    pub fn from_amplitude(n: usize, dx: f64, amplitude: &[f64], floor: f64) -> Self {
        assert_eq!(amplitude.len(), n * n);
        let lap = laplacian(amplitude, n, dx);
        let mut q = vec![0.0; n * n];
        for i in 0..n * n {
            q[i] = -lap[i] / amplitude[i].max(floor);
        }
        let (gx, gy) = gradient(&q, n, dx);
        Self {
            n,
            dx,
            q,
            gx,
            gy,
            q12: vec![0.0; n * n],
        }
    }

    /// Build the full `Q_Cl = -(∇²Ψ) Ψ^{-1}` from an even spinor field.
    ///
    /// `psi` stores `re + im e12`.
    pub fn from_complex_field(n: usize, dx: f64, psi: &[Complex<f64>]) -> Self {
        assert_eq!(psi.len(), n * n);
        let lap = laplacian_complex(psi, n, dx);
        let mut q = vec![0.0; n * n];
        let mut q12 = vec![0.0; n * n];
        for i in 0..n * n {
            let (re, im) = (psi[i].re, psi[i].im);
            let den = re * re + im * im;
            if den > 1e-24 {
                let (lr, li) = (lap[i].re, lap[i].im);
                let pot = CliffordQuantumPotential {
                    scalar: -(lr * re + li * im) / den,
                    bivector: -(li * re - lr * im) / den,
                };
                q[i] = pot.scalar;
                q12[i] = pot.bivector;
            }
        }
        let (gx, gy) = gradient(&q, n, dx);
        Self {
            n,
            dx,
            q,
            gx,
            gy,
            q12,
        }
    }

    #[inline]
    pub fn idx(&self, i: usize, j: usize) -> usize {
        j * self.n + i
    }

    /// Bilinear sample of `⟨Q_Cl⟩_0`.
    pub fn value_at(&self, x: f64, y: f64) -> f64 {
        self.sample(&self.q, x, y)
    }

    /// Bilinear sample of `-∇⟨Q_Cl⟩_0`, the guidance acceleration direction.
    pub fn gradient_at(&self, x: f64, y: f64) -> [f64; 2] {
        [self.sample(&self.gx, x, y), self.sample(&self.gy, x, y)]
    }

    /// Bilinear sample of the bivector spin torque.
    pub fn spin_at(&self, x: f64, y: f64) -> f64 {
        self.sample(&self.q12, x, y)
    }

    /// Bilinear sample of a cell-centred scalar array.
    pub fn sample(&self, field: &[f64], x: f64, y: f64) -> f64 {
        let n = self.n;
        let fx = x / self.dx - 0.5;
        let fy = y / self.dx - 0.5;
        let i = fx.floor().clamp(0.0, (n - 2) as f64) as usize;
        let j = fy.floor().clamp(0.0, (n - 2) as f64) as usize;
        let ax = (fx - i as f64).clamp(0.0, 1.0);
        let ay = (fy - j as f64).clamp(0.0, 1.0);
        let i1 = (i + 1).min(n - 1);
        let j1 = (j + 1).min(n - 1);
        let v00 = field[j * n + i];
        let v10 = field[j * n + i1];
        let v01 = field[j1 * n + i];
        let v11 = field[j1 * n + i1];
        (1.0 - ax) * (1.0 - ay) * v00
            + ax * (1.0 - ay) * v10
            + (1.0 - ax) * ay * v01
            + ax * ay * v11
    }

    /// Minimum and maximum of the scalar potential.
    pub fn range(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in &self.q {
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
        (lo, hi)
    }
}

/// Five-point Laplacian with edge-clamped (Neumann) boundaries.
pub fn laplacian(src: &[f64], n: usize, dx: f64) -> Vec<f64> {
    let mut out = vec![0.0; n * n];
    let inv = 1.0 / (dx * dx);
    for j in 0..n {
        let jm = j.saturating_sub(1);
        let jp = (j + 1).min(n - 1);
        for i in 0..n {
            let im = i.saturating_sub(1);
            let ip = (i + 1).min(n - 1);
            let lap = src[j * n + im] + src[j * n + ip] + src[jm * n + i] + src[jp * n + i]
                - 4.0 * src[j * n + i];
            out[j * n + i] = lap * inv;
        }
    }
    out
}

fn laplacian_complex(src: &[Complex<f64>], n: usize, dx: f64) -> Vec<Complex<f64>> {
    let mut out = vec![Complex::new(0.0, 0.0); n * n];
    let inv = 1.0 / (dx * dx);
    for j in 0..n {
        let jm = j.saturating_sub(1);
        let jp = (j + 1).min(n - 1);
        for i in 0..n {
            let im = i.saturating_sub(1);
            let ip = (i + 1).min(n - 1);
            let lap = src[j * n + im] + src[j * n + ip] + src[jm * n + i] + src[jp * n + i]
                - src[j * n + i] * 4.0;
            out[j * n + i] = lap * inv;
        }
    }
    out
}

/// Central-difference gradient with edge clamping.
pub fn gradient(src: &[f64], n: usize, dx: f64) -> (Vec<f64>, Vec<f64>) {
    let mut gx = vec![0.0; n * n];
    let mut gy = vec![0.0; n * n];
    let inv = 1.0 / (2.0 * dx);
    for j in 0..n {
        let jm = j.saturating_sub(1);
        let jp = (j + 1).min(n - 1);
        for i in 0..n {
            let im = i.saturating_sub(1);
            let ip = (i + 1).min(n - 1);
            gx[j * n + i] = (src[j * n + ip] - src[j * n + im]) * inv;
            gy[j * n + i] = (src[jp * n + i] - src[jm * n + i]) * inv;
        }
    }
    (gx, gy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_field_has_zero_potential() {
        let n = 32;
        let dx = 1.0 / n as f64;
        let r = vec![1.0; n * n];
        let q = QField::from_amplitude(n, dx, &r, 1e-3);
        assert!(q.q.iter().all(|v| v.abs() < 1e-12));
        assert!(q.gx.iter().all(|v| v.abs() < 1e-12));
    }

    #[test]
    fn bilinear_sampling_hits_cell_centres() {
        let n = 8;
        let dx = 1.0 / n as f64;
        let mut r = vec![1.0; n * n];
        for j in 0..n {
            for i in 0..n {
                r[j * n + i] = i as f64;
            }
        }
        let q = QField::from_amplitude(n, dx, &r, 1e-3);
        // The q field itself varies; sample a known linear component instead.
        let x = 2.5 * dx;
        let y = 3.5 * dx;
        let v = q.sample(&r, x, y);
        assert!((v - 2.0).abs() < 1e-12);
    }
}
