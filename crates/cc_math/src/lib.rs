//! Core Clifford kernel for the Living Page prototype.
//!
//! Two pieces are implemented:
//!
//! 1. The even subalgebra of `G(2,0)` as a four-component multivector
//!    `a + b e1 + c e2 + d e12`. The even part (`a + d e12`) is isomorphic to
//!    the complex numbers and is what the page lattice stores.
//! 2. Thin helpers around the `clifford` crate's 3D PGA types for page folds
//!    and rigid cutout poses.
//!
//! The Clifford quantum potential is
//!
//! ```text
//! Q_Cl = -(∇²Ψ) Ψ^{-1}
//! ```
//!
//! For an even field this is a scalar plus an `e12` bivector. The scalar part
//! is the scale-invariant valley field; the bivector part is spin torque.

use std::ops::{Add, Mul, Neg, Sub};

use clifford::specialized::projective::dim3;

pub use dim3::{Line, Motor, Plane, Point};

pub mod holon;
pub mod idempotent;
pub mod pga;

pub use holon::CliffordDensityElement;
pub use idempotent::{StateBasis, StateIdempotent, TransitionOperator};
pub use pga::{
    Camera2D, OrientedLine2, RayHit, SegmentHit, integrate_2d_camera, integrate_3d_ego,
    plane_through_point, ray_hit_plane, ray_hit_plane_pga, segment_hit_oriented_line,
    snap_manhattan_normal, snap_manhattan_plane,
};

/// Multivector of `G(2,0)`.
///
/// Basis: `1, e1, e2, e12`, with `e1² = e2² = 1` and `e12² = -1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mv2 {
    pub s: f64,
    pub e1: f64,
    pub e2: f64,
    pub e12: f64,
}

impl Mv2 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 0.0, 0.0, 0.0);
    pub const E1: Self = Self::new(0.0, 1.0, 0.0, 0.0);
    pub const E2: Self = Self::new(0.0, 0.0, 1.0, 0.0);
    pub const E12: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    pub const fn new(s: f64, e1: f64, e2: f64, e12: f64) -> Self {
        Self { s, e1, e2, e12 }
    }

    pub const fn scalar(s: f64) -> Self {
        Self::new(s, 0.0, 0.0, 0.0)
    }

    /// Map a complex number to the even subalgebra: `re + im e12`.
    pub const fn from_complex(re: f64, im: f64) -> Self {
        Self::new(re, 0.0, 0.0, im)
    }

    /// Even multivector `s + b e12`.
    pub const fn even(s: f64, b: f64) -> Self {
        Self::new(s, 0.0, 0.0, b)
    }

    pub const fn complex(self) -> (f64, f64) {
        (self.s, self.e12)
    }

    pub fn is_even(self) -> bool {
        self.e1 == 0.0 && self.e2 == 0.0
    }

    pub fn plus(self, rhs: Self) -> Self {
        Self::new(
            self.s + rhs.s,
            self.e1 + rhs.e1,
            self.e2 + rhs.e2,
            self.e12 + rhs.e12,
        )
    }

    pub fn minus(self, rhs: Self) -> Self {
        Self::new(
            self.s - rhs.s,
            self.e1 - rhs.e1,
            self.e2 - rhs.e2,
            self.e12 - rhs.e12,
        )
    }

    pub fn scale(self, k: f64) -> Self {
        Self::new(self.s * k, self.e1 * k, self.e2 * k, self.e12 * k)
    }

    /// Geometric product. Signs follow `e1 e2 = e12` and `e2 e1 = -e12`.
    pub fn gp(self, rhs: Self) -> Self {
        let s = self.s * rhs.s + self.e1 * rhs.e1 + self.e2 * rhs.e2 - self.e12 * rhs.e12;
        let e1 = self.s * rhs.e1 + self.e1 * rhs.s - self.e2 * rhs.e12 + self.e12 * rhs.e2;
        let e2 = self.s * rhs.e2 + self.e1 * rhs.e12 + self.e2 * rhs.s - self.e12 * rhs.e1;
        let e12 = self.s * rhs.e12 + self.e1 * rhs.e2 - self.e2 * rhs.e1 + self.e12 * rhs.s;
        Self::new(s, e1, e2, e12)
    }

    /// Grade reversal: `s + e1 + e2 - e12`.
    pub fn reverse(self) -> Self {
        Self::new(self.s, self.e1, self.e2, -self.e12)
    }

    /// Clifford conjugate: reverse plus grade involution.
    pub fn conjugate(self) -> Self {
        Self::new(self.s, -self.e1, -self.e2, -self.e12)
    }

    /// Sum of squared coefficients.
    pub fn norm_sqr(self) -> f64 {
        self.s * self.s + self.e1 * self.e1 + self.e2 * self.e2 + self.e12 * self.e12
    }

    /// Squared modulus of the even part.
    pub fn even_norm_sqr(self) -> f64 {
        self.s * self.s + self.e12 * self.e12
    }

    /// Amplitude `R` of an even multivector `Ψ = R U`.
    pub fn amplitude(self) -> f64 {
        self.even_norm_sqr().sqrt()
    }

    /// Inverse of an even multivector `s + b e12`.
    ///
    /// Returns `None` for the zero spinor. Odd components are ignored; use
    /// [`Mv2::is_even`] first if the distinction matters.
    pub fn inverse_even(self) -> Option<Self> {
        let n = self.even_norm_sqr();
        if n <= f64::MIN_POSITIVE {
            None
        } else {
            Some(Self::even(self.s / n, -self.e12 / n))
        }
    }

    /// Exponential of an even multivector: `exp(s + b e12)`.
    pub fn exp_even(self) -> Self {
        let r = self.s.exp();
        Self::even(r * self.e12.cos(), r * self.e12.sin())
    }

    pub fn scalar_part(self) -> f64 {
        self.s
    }

    pub fn e12_part(self) -> f64 {
        self.e12
    }
}

impl Add for Mv2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.plus(rhs)
    }
}

impl Sub for Mv2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.minus(rhs)
    }
}

impl Neg for Mv2 {
    type Output = Self;
    fn neg(self) -> Self {
        self.scale(-1.0)
    }
}

impl Mul for Mv2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        self.gp(rhs)
    }
}

impl Mul<f64> for Mv2 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        self.scale(rhs)
    }
}

impl Mul<Mv2> for f64 {
    type Output = Mv2;
    fn mul(self, rhs: Mv2) -> Mv2 {
        rhs.scale(self)
    }
}

/// `Q_Cl = -(∇²Ψ) Ψ^{-1}`, decomposed into scalar and `e12` parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CliffordQuantumPotential {
    /// `⟨Q_Cl⟩_0`: scale-invariant valley field used for guidance.
    pub scalar: f64,
    /// `⟨Q_Cl⟩_2`: bivector spin torque (`e12` coefficient).
    pub bivector: f64,
}

impl CliffordQuantumPotential {
    /// Evaluate `Q_Cl` from an even field and its geometric Laplacian.
    pub fn from_field(psi: Mv2, laplacian: Mv2) -> Option<Self> {
        let inv = psi.inverse_even()?;
        let q = -laplacian.gp(inv);
        Some(Self {
            scalar: q.s,
            bivector: q.e12,
        })
    }

    /// Gradient-flow acceleration `-∇⟨Q_Cl⟩_0`.
    pub fn guidance(self, grad_scalar: [f64; 2]) -> [f64; 2] {
        [-grad_scalar[0], -grad_scalar[1]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometric_product_identities() {
        let a = Mv2::new(1.2, -0.4, 0.7, 0.3);
        let b = Mv2::new(-0.2, 0.9, -0.5, 1.1);
        let c = Mv2::new(0.6, -0.1, 0.2, -0.8);

        assert!((a.gp(Mv2::ONE) - a).norm_sqr() < 1e-24);
        assert!((Mv2::ONE.gp(a) - a).norm_sqr() < 1e-24);
        assert!((a.gp(b).gp(c) - a.gp(b.gp(c))).norm_sqr() < 1e-20);

        // e1 e2 = e12, e2 e1 = -e12
        assert_eq!(Mv2::E1.gp(Mv2::E2), Mv2::E12);
        assert_eq!(Mv2::E2.gp(Mv2::E1), -Mv2::E12);
        // e12² = -1
        assert_eq!(Mv2::E12.gp(Mv2::E12), -Mv2::ONE);
    }

    #[test]
    fn even_inverse_round_trip() {
        let a = Mv2::from_complex(0.37, -0.82);
        let inv = a.inverse_even().unwrap();
        assert!((a.gp(inv) - Mv2::ONE).norm_sqr() < 1e-28);
    }

    #[test]
    fn quantum_potential_scale_invariance() {
        let psi = Mv2::from_complex(0.35, -0.62);
        let lap = Mv2::from_complex(1.7, 0.9);
        let q1 = CliffordQuantumPotential::from_field(psi, lap).unwrap();
        let q2 = CliffordQuantumPotential::from_field(psi * 13.7, lap * 13.7).unwrap();
        assert!((q1.scalar - q2.scalar).abs() < 1e-12);
        assert!((q1.bivector - q2.bivector).abs() < 1e-12);
    }

    #[test]
    fn pga_fold_preserves_distances() {
        let m = pga::fold_motor(0.63);
        let a = Point::from_cartesian(0.8, 0.2, 0.4);
        let b = Point::from_cartesian(-0.3, 0.5, 0.9);
        let a2 = pga::transform_point(&m, &a);
        let b2 = pga::transform_point(&m, &b);
        assert!((a.distance(&b) - a2.distance(&b2)).abs() < 1e-10);
    }

    #[test]
    fn pose_motor_maps_origin_and_axis() {
        let m = pga::pose_motor(0.3, 0.4, std::f64::consts::FRAC_PI_2);
        let o = pga::transform_point(&m, &Point::from_cartesian(0.0, 0.0, 0.0));
        let x = pga::transform_point(&m, &Point::from_cartesian(1.0, 0.0, 0.0));
        assert!((o.cartesian_x() - 0.3).abs() < 1e-12);
        assert!((o.cartesian_y() - 0.4).abs() < 1e-12);
        assert!((x.cartesian_x() - 0.3).abs() < 1e-12);
        assert!((x.cartesian_y() - 1.4).abs() < 1e-12);
    }
}
