//! Hiley's Clifford Density Element (CDE) and Implicate Holon dynamics.
//!
//! Grounded in B. J. Hiley & R. E. Callaghan (2010), *The Clifford Algebra
//! approach to Quantum Mechanics A: The Schrödinger and Pauli Particles*:
//!
//! 1. **Minimal Left Ideal Element ($\Phi_L$):**
//!    $$\Phi_L(r, t) = \phi_L(r, t)\,\epsilon = R(r, t)\,U(r, t)\,\epsilon$$
//!    where $R \ge 0$ is the real amplitude, $U = \exp(-\frac{1}{2}\theta e_{12}) \in \text{Spin}(2,0)$
//!    is the rotor encoding planar orientation/phase, and $\epsilon^2 = \epsilon$ is a
//!    primitive state idempotent selecting the active process branch.
//!
//! 2. **Clifford Density Element ($\rho_c$):**
//!    $$\rho_c = \Phi_L \widetilde{\Phi}_L = \rho\, U \epsilon \widetilde{U}, \quad \rho = R^2$$
//!    All observable properties of the holon are extracted as bilinear invariants
//!    purely within the algebra without appealing to an external Hilbert space:
//!    * **First-Kind Invariant (Directed Spin/Heading Vector):**
//!      $$\mathbf{s} = U e_1 \widetilde{U} = (\cos\theta, \sin\theta)$$
//!    * **Second-Kind Invariant (Bohm Momentum & Quantum Torque):**
//!      $$\mathbf{P}_B = \rho\, v_0\, \mathbf{s}, \quad \frac{d\mathbf{P}_B}{dt} = -\nabla \mathcal{Q}_{\text{Cl}} + \boldsymbol{\tau}_Q$$
//!
//! 3. **Relational Wholeness & Incidence Transitions:**
//!    Interactions between holons or with oriented PGA boundary blades induce
//!    idempotent state transitions $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$ and
//!    rotor reflections $U \mapsto N U \widetilde{N}$.

use crate::Mv2;
use crate::idempotent::{StateBasis, StateIdempotent, TransitionOperator};

/// A Hiley Clifford Density Element $\rho_c = \Phi_L \widetilde{\Phi}_L = \rho U \epsilon \widetilde{U}$
/// representing an autonomous process holon in $\mathbb{G}_{2,0}$.
#[derive(Clone, Debug, PartialEq)]
pub struct CliffordDensityElement {
    /// Amplitude $R = \sqrt{\rho}$ of the left-ideal process.
    pub amplitude: f64,
    /// Spinor rotor $U = \cos(\theta/2) - \sin(\theta/2) e_{12} \in \text{Spin}(2,0)$.
    pub rotor: Mv2,
    /// Active primitive state idempotent $\epsilon_s$ ($\epsilon_s^2 = \epsilon_s$).
    pub idempotent: StateIdempotent,
    /// Intrinsic 3-channel chromatic/semantic spinor signature $\psi_{\text{id}} \in (\mathbb{G}_{2,0}^+)^3$.
    pub signature: [Mv2; 3],
}

impl CliffordDensityElement {
    /// Construct a CDE from amplitude $R$, heading angle $\theta$ (radians),
    /// initial state index `state`, total states `n_states`, and RGB/semantic signature.
    pub fn new(
        amplitude: f64,
        theta: f64,
        state: u32,
        n_states: usize,
        rgb_signature: [u8; 3],
    ) -> Self {
        let basis = StateBasis::new(n_states.max(2));
        let idx = state.min((n_states.max(2) - 1) as u32);
        let idempotent = basis.get(idx).expect("valid state index").clone();
        let half = 0.5 * theta;
        let rotor = Mv2::even(half.cos(), -half.sin());
        let signature = [
            Mv2::even(rgb_signature[0] as f64 / 255.0, 0.0),
            Mv2::even(rgb_signature[1] as f64 / 255.0, 0.0),
            Mv2::even(rgb_signature[2] as f64 / 255.0, 0.0),
        ];
        Self {
            amplitude,
            rotor,
            idempotent,
            signature,
        }
    }

    /// [Bilinear Invariant of the First Kind: Process Density]
    /// Density $\rho = R^2 = \text{tr}(\rho_c)$.
    #[inline]
    pub fn density(&self) -> f64 {
        self.amplitude * self.amplitude * self.idempotent.trace()
    }

    /// Active discrete state index $s$.
    #[inline]
    pub fn state(&self) -> u32 {
        self.idempotent.state
    }

    /// Set the rotor directly from a planar heading angle $\theta$.
    #[inline]
    pub fn set_heading_angle(&mut self, theta: f64) {
        let half = 0.5 * theta;
        self.rotor = Mv2::even(half.cos(), -half.sin());
    }

    /// [Bilinear Invariant of the First Kind: Directed Spin / Heading Vector]
    /// $\mathbf{s} = U e_1 \widetilde{U} = (\cos\theta, \sin\theta)$ (Hiley eq. 4.5).
    #[inline]
    pub fn heading_vector(&self) -> [f64; 2] {
        let v = self.rotor.gp(Mv2::E1).gp(self.rotor.reverse());
        [v.e1, v.e2]
    }

    /// Recover the planar heading angle $\theta \in (-\pi, \pi]$ from $\mathbf{s} = U e_1 \widetilde{U}$.
    #[inline]
    pub fn heading_angle(&self) -> f64 {
        let [hx, hy] = self.heading_vector();
        hy.atan2(hx)
    }

    /// [Bilinear Invariant of the Second Kind: Bohmian Momentum]
    /// $\mathbf{P}_B = \rho\, v_0\, \mathbf{s}$ (Hiley eq. 3.3, 4.15). Vanishes when the holon collapses
    /// to a passive/spent ideal ($\rho = 0$).
    #[inline]
    pub fn bohm_momentum(&self, speed: f64) -> [f64; 2] {
        let rho = self.density();
        let [hx, hy] = self.heading_vector();
        [rho * speed * hx, rho * speed * hy]
    }

    /// [Active Information: Quantum Torque Steering]
    /// Steer the spinor rotor $U$ via the Quantum Torque induced by the Clifford
    /// Quantum Potential gradient $-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$
    /// (Hiley eq. 5.18, 5.27):
    /// $$\Omega_Q = \mathbf{s} \wedge (-\nabla \mathcal{Q}_{\text{Cl}}), \quad U \leftarrow U \exp\left(-\frac{1}{2}\Omega_Q\,\Delta t\right)$$
    pub fn apply_quantum_torque(&mut self, neg_grad_q: [f64; 2], gain: f64) {
        let [sx, sy] = self.heading_vector();
        let gn = (neg_grad_q[0] * neg_grad_q[0] + neg_grad_q[1] * neg_grad_q[1]).sqrt();
        if gn <= 1e-9 {
            return;
        }
        let gx = neg_grad_q[0] / gn;
        let gy = neg_grad_q[1] / gn;
        // Bivector spin-torque: \Omega_Q = s \wedge (-\nabla Q_Cl)
        let wedge_12 = sx * gy - sy * gx;
        let dtheta = gain * wedge_12;
        // Rotor evolution under quantum torque: U <- U * exp(-0.5 * \Omega_Q * \Delta t)
        let delta_rotor = Mv2::even((0.5 * dtheta).cos(), -(0.5 * dtheta).sin());
        self.rotor = self.rotor.gp(delta_rotor);
        // Renormalize rotor to stay on Spin(2,0)
        let norm = self.rotor.amplitude().max(1e-12);
        self.rotor = self.rotor.scale(1.0 / norm);
    }

    /// Snap the rotor $U$ to the nearest cardinal Manhattan lattice direction
    /// ($0, \frac{\pi}{2}, \pi, -\frac{\pi}{2}$).
    pub fn snap_cardinal_rotor(&mut self) {
        let [hx, hy] = self.heading_vector();
        let angle = if hx.abs() >= hy.abs() {
            if hx >= 0.0 { 0.0 } else { std::f64::consts::PI }
        } else if hy >= 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        self.set_heading_angle(angle);
    }

    /// [Primitive State Transitions: Incidence Sandwich Operator]
    /// Apply a state transition $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$ within `basis`.
    pub fn transition_to(&mut self, target_state: u32, basis: &StateBasis) -> Option<TransitionOperator> {
        let to = basis.get(target_state)?;
        let d = self.idempotent.dim;
        let mut ones = vec![0.0; d * d];
        for i in 0..d {
            for j in 0..d {
                ones[i * d + j] = 1.0;
            }
        }
        let op = TransitionOperator::from_matrix(&self.idempotent, to, &ones)?;
        self.idempotent = to.clone();
        Some(op)
    }

    /// [Primitive State Idempotents & Incidence Transitions: Boundary Reflection]
    /// Reflect the holon's heading vector across an oriented boundary normal $\hat{\mathbf{n}}$
    /// ($\mathbf{s} \mapsto -\hat{\mathbf{n}}\mathbf{s}\hat{\mathbf{n}}$) and transition between
    /// patrol states $\epsilon_0 \leftrightarrow \epsilon_1$.
    pub fn reflect_at_boundary(&mut self, normal: [f64; 2], basis: &StateBasis) {
        let [hx, hy] = self.heading_vector();
        let dot = hx * normal[0] + hy * normal[1];
        if dot < 0.0 {
            let rx = hx - 2.0 * dot * normal[0];
            let ry = hy - 2.0 * dot * normal[1];
            self.set_heading_angle(ry.atan2(rx));
            let next_state = if self.state() == 0 { 1 } else { 0 };
            let _ = self.transition_to(next_state, basis);
        }
    }

    /// [Bilinear Invariant of the First Kind: Intrinsic Spinor Coherence]
    /// Algebraic spinor coherence $\langle \psi_i, \widetilde{\psi}_j \rangle_0 \in [0, 1]$ between
    /// two holons' intrinsic signatures.
    pub fn signature_coherence(&self, other: &Self) -> f64 {
        let mut dist_sq = 0.0;
        for c in 0..3 {
            let d = (self.signature[c] - other.signature[c]).norm_sqr();
            dist_sq += d;
        }
        (1.0 - (dist_sq / 3.0).sqrt()).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cde_heading_and_bohm_momentum_follow_rotor_sandwich() {
        let cde = CliffordDensityElement::new(1.0, std::f64::consts::FRAC_PI_2, 0, 3, [255, 200, 0]);
        let [hx, hy] = cde.heading_vector();
        assert!(hx.abs() < 1e-12, "hx = {hx}");
        assert!((hy - 1.0).abs() < 1e-12, "hy = {hy}");
        let [px, py] = cde.bohm_momentum(2.5);
        assert!(px.abs() < 1e-12);
        assert!((py - 2.5).abs() < 1e-12);
    }

    #[test]
    fn quantum_torque_steers_rotor_toward_potential_valley() {
        let mut cde = CliffordDensityElement::new(1.0, 0.0, 0, 3, [255, 0, 0]);
        // Valley is in the +y direction: -∇Q = (0, 1)
        for _ in 0..20 {
            cde.apply_quantum_torque([0.0, 1.0], 0.15);
        }
        let [hx, hy] = cde.heading_vector();
        assert!(hy > 0.85, "expected rotor to turn toward +y, got ({hx}, {hy})");
    }

    #[test]
    fn boundary_reflection_flips_rotor_and_switches_idempotent() {
        let basis = StateBasis::new(3);
        let mut cde = CliffordDensityElement::new(1.0, std::f64::consts::PI, 0, 3, [180, 80, 40]);
        assert_eq!(cde.state(), 0);
        // Moving left (-1, 0) into a right-facing wall normal (+1, 0)
        cde.reflect_at_boundary([1.0, 0.0], &basis);
        assert_eq!(cde.state(), 1);
        let [hx, hy] = cde.heading_vector();
        assert!((hx - 1.0).abs() < 1e-12);
        assert!(hy.abs() < 1e-12);
    }
}
