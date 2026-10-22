//! [Primitive State Idempotents & Incidence Transitions]
//! Primitive state idempotents and transition operators.
//!
//! Discrete actor states (small/super/raccoon, walk/flat/shell, patrol/alert/
//! attack/dead, door closed/sliding/open) are primitive idempotents
//! `ε_s² = ε_s` with `ε_s ε_{s'} = 0` for `s ≠ s'`.
//!
//! Two representations are supported, both as row-major matrices acting on the
//! even-spinor coefficient space:
//!
//! * **Dimension 2** — the even subalgebra of `G(2,0)`, `ψ = s + e e12`. A
//!   direction `u = (u₀, u₁)` gives the rank-1 projector `ε_u = u uᵀ`. Only two
//!   such projectors are mutually orthogonal (`u ⊥ v`), matching `dim = 2`.
//! * **Dimension n** — the `n × n` coefficient representation, where the
//!   primitive idempotents are the diagonal units `E_kk`. This is the lift used
//!   when a machine has more than two states; the set is complete,
//!   `Σ ε_s = I`.
//!
//! A motor `M` acts on the spinor space as a real matrix (for an even motor
//! `a + b e12` that matrix is `[[a, -b], [b, a]]`), giving the transition
//! operator
//!
//! ```text
//! T_{s → s'} = ε_{s'} M ε_s
//! ```

use crate::Mv2;

/// A primitive idempotent on the spinor coefficient space.
#[derive(Clone, Debug, PartialEq)]
pub struct StateIdempotent {
    /// State label, opaque to the algebra.
    pub state: u32,
    /// Side length of the (square) coefficient matrix.
    pub dim: usize,
    /// Row-major `dim × dim` matrix.
    pub m: Vec<f64>,
}

impl StateIdempotent {
    /// Rank-1 projector `u uᵀ` in `dim = 2` from a spinor direction.
    ///
    /// The spinor is normalized to unit length first.
    pub fn from_spinor(state: u32, psi: Mv2) -> Self {
        let (s, e) = (psi.s, psi.e12);
        let n = (s * s + e * e).sqrt().max(1e-12);
        let u = [s / n, e / n];
        Self {
            state,
            dim: 2,
            m: vec![u[0] * u[0], u[0] * u[1], u[1] * u[0], u[1] * u[1]],
        }
    }

    /// Rank-1 projector in `dim = 2` from an angle: `u = (cos θ, sin θ)`.
    pub fn from_angle(state: u32, theta: f64) -> Self {
        Self::from_spinor(state, Mv2::even(theta.cos(), theta.sin()))
    }

    /// Rank-1 projector in `dim = 2` from a coefficient pair.
    pub fn from_coeffs(state: u32, coeffs: [f64; 2]) -> Self {
        Self::from_spinor(state, Mv2::even(coeffs[0], coeffs[1]))
    }

    /// Diagonal unit `E_kk` in dimension `dim`.
    ///
    /// Panics when `index >= dim`.
    pub fn diagonal(state: u32, dim: usize, index: usize) -> Self {
        assert!(index < dim, "state index {index} out of range for dim {dim}");
        let mut m = vec![0.0; dim * dim];
        m[index * dim + index] = 1.0;
        Self { state, dim, m }
    }

    /// Coefficient at `(row, col)`.
    #[inline]
    pub fn at(&self, row: usize, col: usize) -> f64 {
        self.m[row * self.dim + col]
    }

    /// Matrix product `self · other`. Panics on dimension mismatch.
    pub fn mul(&self, other: &Self) -> Self {
        assert_eq!(self.dim, other.dim, "dimension mismatch");
        let d = self.dim;
        let mut m = vec![0.0; d * d];
        for i in 0..d {
            for k in 0..d {
                let a = self.m[i * d + k];
                if a == 0.0 {
                    continue;
                }
                for j in 0..d {
                    m[i * d + j] += a * other.m[k * d + j];
                }
            }
        }
        Self {
            state: self.state,
            dim: d,
            m,
        }
    }

    /// `ε²`.
    pub fn square(&self) -> Self {
        self.mul(self)
    }

    /// Apply the projector to a coefficient vector.
    pub fn apply(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(v.len(), self.dim, "vector length must equal dim");
        let d = self.dim;
        let mut out = vec![0.0; d];
        for i in 0..d {
            for j in 0..d {
                out[i] += self.m[i * d + j] * v[j];
            }
        }
        out
    }

    /// Trace of the matrix.
    pub fn trace(&self) -> f64 {
        (0..self.dim).map(|i| self.at(i, i)).sum()
    }

    /// Rank-1 check: `tr = 1` and every `2 × 2` minor vanishes.
    pub fn is_rank_one(&self, tol: f64) -> bool {
        if (self.trace() - 1.0).abs() > tol {
            return false;
        }
        let d = self.dim;
        for i in 0..d {
            for j in 0..d {
                if i == j {
                    continue;
                }
                for k in 0..d {
                    for l in 0..d {
                        if k == l {
                            continue;
                        }
                        let minor = self.at(i, k) * self.at(j, l) - self.at(i, l) * self.at(j, k);
                        if minor.abs() > tol {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Idempotence check `ε² = ε`.
    pub fn is_idempotent(&self, tol: f64) -> bool {
        let sq = self.square().m;
        sq.iter()
            .zip(&self.m)
            .all(|(a, b)| (a - b).abs() < tol)
    }

    /// Orthogonality check `ε_s ε_t = 0`.
    pub fn is_orthogonal_to(&self, other: &Self, tol: f64) -> bool {
        self.mul(other).m.iter().all(|v| v.abs() < tol)
    }

    /// Projector direction for the `dim = 2` representation.
    pub fn spinor(&self) -> Option<Mv2> {
        if self.dim != 2 {
            return None;
        }
        // Recover a unit direction from the rank-1 projector.
        let m = &self.m;
        let (a, b) = (m[0], m[1]);
        let n = (a * a + b * b).sqrt();
        if n < 1e-9 {
            None
        } else {
            Some(Mv2::even(a / n, b / n))
        }
    }
}

/// A state transition `T_{s → s'} = ε_{s'} M ε_s`.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionOperator {
    pub from: u32,
    pub to: u32,
    /// Side length of the (square) operator matrix.
    pub dim: usize,
    /// Row-major `dim × dim` matrix.
    pub m: Vec<f64>,
}

impl TransitionOperator {
    /// Bind an even motor to a source/target state pair (`dim = 2` only).
    ///
    /// Returns `None` when either idempotent is not 2-dimensional.
    pub fn from_motor(from: &StateIdempotent, to: &StateIdempotent, motor: Mv2) -> Option<Self> {
        if from.dim != 2 || to.dim != 2 {
            return None;
        }
        let a = motor_matrix(motor);
        Self::from_matrix(from, to, &a)
    }

    /// Bind an arbitrary coefficient matrix to a source/target state pair.
    ///
    /// Returns `None` when dimensions disagree.
    pub fn from_matrix(
        from: &StateIdempotent,
        to: &StateIdempotent,
        motor_matrix: &[f64],
    ) -> Option<Self> {
        let d = from.dim;
        if to.dim != d || motor_matrix.len() != d * d {
            return None;
        }
        // T = ε_to · M · ε_from
        let mut left = vec![0.0; d * d];
        for i in 0..d {
            for k in 0..d {
                let a = to.m[i * d + k];
                if a == 0.0 {
                    continue;
                }
                for j in 0..d {
                    left[i * d + j] += a * motor_matrix[k * d + j];
                }
            }
        }
        let mut t = vec![0.0; d * d];
        for i in 0..d {
            for k in 0..d {
                let a = left[i * d + k];
                if a == 0.0 {
                    continue;
                }
                for j in 0..d {
                    t[i * d + j] += a * from.m[k * d + j];
                }
            }
        }
        Some(Self {
            from: from.state,
            to: to.state,
            dim: d,
            m: t,
        })
    }

    /// Apply the transition to a coefficient vector: `ε_{s'} M ε_s ψ`.
    pub fn apply(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(v.len(), self.dim, "vector length must equal dim");
        let d = self.dim;
        let mut out = vec![0.0; d];
        for i in 0..d {
            for j in 0..d {
                out[i] += self.m[i * d + j] * v[j];
            }
        }
        out
    }

    /// `T²` (for diagnostics: rank-1 `T` satisfies `T² = tr(T)·T`).
    pub fn square(&self) -> Vec<f64> {
        let d = self.dim;
        let mut m = vec![0.0; d * d];
        for i in 0..d {
            for k in 0..d {
                let a = self.m[i * d + k];
                if a == 0.0 {
                    continue;
                }
                for j in 0..d {
                    m[i * d + j] += a * self.m[k * d + j];
                }
            }
        }
        m
    }

    /// Trace of the operator matrix.
    pub fn trace(&self) -> f64 {
        (0..self.dim).map(|i| self.m[i * self.dim + i]).sum()
    }
}

/// A set of mutually orthogonal primitive idempotents.
#[derive(Clone, Debug, Default)]
pub struct StateBasis {
    pub states: Vec<StateIdempotent>,
}

impl StateBasis {
    /// The `n`-state basis of diagonal units in dimension `n`.
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "a state basis needs at least one state");
        let states = (0..n)
            .map(|k| StateIdempotent::diagonal(k as u32, n, k))
            .collect();
        Self { states }
    }

    /// The two-state spinor basis `u₀ = (1, 0)`, `u₁ = (0, 1)`.
    pub fn spinor_pair() -> Self {
        Self {
            states: vec![
                StateIdempotent::from_coeffs(0, [1.0, 0.0]),
                StateIdempotent::from_coeffs(1, [0.0, 1.0]),
            ],
        }
    }

    pub fn get(&self, state: u32) -> Option<&StateIdempotent> {
        self.states.iter().find(|s| s.state == state)
    }

    /// All members are primitive (idempotent, rank 1) and mutually orthogonal.
    pub fn is_primitive_set(&self, tol: f64) -> bool {
        for (i, a) in self.states.iter().enumerate() {
            if !a.is_idempotent(tol) || !a.is_rank_one(tol) {
                return false;
            }
            for b in &self.states[i + 1..] {
                if !a.is_orthogonal_to(b, tol) || !b.is_orthogonal_to(a, tol) {
                    return false;
                }
            }
        }
        true
    }

    /// Completeness `Σ ε_s = I` (only for the diagonal representation).
    pub fn is_complete(&self, tol: f64) -> bool {
        let d = self.states.first().map(|s| s.dim).unwrap_or(0);
        if d == 0 || self.states.iter().any(|s| s.dim != d) {
            return false;
        }
        let mut sum = vec![0.0; d * d];
        for s in &self.states {
            for (a, b) in sum.iter_mut().zip(&s.m) {
                *a += b;
            }
        }
        for i in 0..d {
            for j in 0..d {
                let expect = if i == j { 1.0 } else { 0.0 };
                if (sum[i * d + j] - expect).abs() > tol {
                    return false;
                }
            }
        }
        true
    }
}

/// Matrix of the linear map `ψ ↦ M ψ` for an even motor `M = a + b e12`.
pub fn motor_matrix(m: Mv2) -> [f64; 4] {
    [m.s, -m.e12, m.e12, m.s]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinor_projector_is_idempotent_and_rank_one() {
        let e = StateIdempotent::from_coeffs(0, [0.6, 0.8]);
        assert!(e.is_idempotent(1e-12));
        assert!(e.is_rank_one(1e-12));
        assert_eq!(e.square().m, e.m);
        assert!((e.trace() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn orthogonal_spinor_states_annihilate() {
        let a = StateIdempotent::from_coeffs(0, [1.0, 0.0]);
        let b = StateIdempotent::from_coeffs(1, [0.0, 1.0]);
        assert!(a.is_orthogonal_to(&b, 1e-12));
        assert!(b.is_orthogonal_to(&a, 1e-12));
        assert!(a.mul(&b).m.iter().all(|v| v.abs() < 1e-12));
        // The two-dimensional spinor space admits only one orthogonal partner.
        let c = StateIdempotent::from_angle(2, 0.7);
        assert!(!a.is_orthogonal_to(&c, 1e-6));
    }

    #[test]
    fn diagonal_basis_is_primitive_orthogonal_complete() {
        for n in 1..=8 {
            let basis = StateBasis::new(n);
            assert!(basis.is_primitive_set(1e-12), "n = {n}");
            assert!(basis.is_complete(1e-12), "n = {n}");
        }
    }

    #[test]
    fn spinor_pair_basis_is_primitive() {
        let basis = StateBasis::spinor_pair();
        assert!(basis.is_primitive_set(1e-12));
        assert!(basis.is_complete(1e-12));
    }

    #[test]
    fn transition_from_motor_maps_source_to_target() {
        let basis = StateBasis::spinor_pair();
        let small = basis.get(0).unwrap();
        let grown = basis.get(1).unwrap();
        // A 90° even motor swaps the two orthogonal state directions.
        let motor = Mv2::even(
            std::f64::consts::FRAC_PI_2.cos(),
            std::f64::consts::FRAC_PI_2.sin(),
        );
        let t = TransitionOperator::from_motor(small, grown, motor).unwrap();
        let out = t.apply(&[1.0, 0.0]);
        assert!((out[1] - 1.0).abs() < 1e-12);
        assert!(out[0].abs() < 1e-12);
        // The same transition annihilates the target state.
        let dead = t.apply(&[0.0, 1.0]);
        assert!(dead[0].abs() < 1e-12 && dead[1].abs() < 1e-12);
        // Rank 1: T² = tr(T)·T.
        let sq = t.square();
        let tr = t.trace();
        for (a, b) in sq.iter().zip(&t.m) {
            assert!((a - tr * b).abs() < 1e-12);
        }
    }

    #[test]
    fn transition_from_matrix_moves_states_in_higher_dimensions() {
        // dim = 3 basis; the permutation matrix E_20 maps state 0 → state 2.
        let basis = StateBasis::new(3);
        let s0 = basis.get(0).unwrap();
        let s2 = basis.get(2).unwrap();
        let mut motor = vec![0.0; 9];
        motor[2 * 3 + 0] = 1.0; // row 2, col 0
        let t = TransitionOperator::from_matrix(s0, s2, &motor).unwrap();
        let out = t.apply(&[1.0, 0.0, 0.0]);
        assert_eq!(out, vec![0.0, 0.0, 1.0]);
        // Idempotent partners are annihilated.
        let dead = t.apply(&[0.0, 1.0, 0.0]);
        assert!(dead.iter().all(|v| v.abs() < 1e-12));
        assert!(TransitionOperator::from_motor(s0, s2, Mv2::ONE).is_none());
    }

    #[test]
    fn motor_matrix_matches_geometric_product() {
        let m = Mv2::even(0.4, -0.9);
        let psi = Mv2::even(-0.3, 0.7);
        let direct = m.gp(psi);
        let mat = motor_matrix(m);
        let via = [mat[0] * psi.s + mat[1] * psi.e12, mat[2] * psi.s + mat[3] * psi.e12];
        assert!((direct.s - via[0]).abs() < 1e-12);
        assert!((direct.e12 - via[1]).abs() < 1e-12);
    }
}
