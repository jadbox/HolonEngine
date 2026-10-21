//! Release-only throughput check for the Phase 2 lattice gate.
//!
//! Run with:
//!   cargo test --release -p cc_field --test throughput -- --ignored --nocapture
//!
//! The plan asks for a 256×256 multivector lattice at ≥ 120 Hz. This solver
//! stores the even subalgebra as one complex number per cell, so the measured
//! rate is an upper bound for that reduced representation, not the full
//! four-component `G(2,0)` multivector.

use std::time::Instant;

use cc_field::{Complex, WaveSolver, mass};

#[test]
#[ignore = "release-only 256² throughput check"]
fn cft_256_throughput() {
    let n = 256;
    let dx = 1.0 / n as f64;
    let mut solver = WaveSolver::new(n, dx, 0.5, 2.0, 0.001);
    let mut psi = vec![Complex::new(0.0, 0.0); n * n];
    for j in 0..n {
        for i in 0..n {
            let x = (i as f64 + 0.5) * dx;
            let env = 1.0 / (7.0 * (x - 0.5)).cosh();
            psi[j * n + i] = Complex::new(env, 0.0);
        }
    }

    let m0 = mass(&psi, dx);
    let steps = 200;
    let start = Instant::now();
    for _ in 0..steps {
        solver.step(&mut psi, None, None, None);
    }
    let secs = start.elapsed().as_secs_f64();
    let hz = steps as f64 / secs;
    let m1 = mass(&psi, dx);
    let drift = (m1 - m0).abs() / m0;

    println!(
        "{n}x{n}: {steps} steps in {secs:.3}s = {hz:.1} steps/s; mass drift {drift:.2e}"
    );
    assert!(drift < 1e-10, "mass drift {drift:.2e}");
    assert!(hz >= 120.0, "throughput below 120 Hz: {hz:.1} Hz");
}
