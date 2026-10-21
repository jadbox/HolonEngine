# Milestone: Phase 1 — Algebraic Kernel, PGA Incidence & State Idempotents (`cc_math`)

- [X] Keep existing `cc_math` tests passing (Q_Cl scale invariance, PGA fold/pose)
- [X] Add `crates/cc_math/src/pga.rs`
  - [X] `ray_hit_plane` exact ray/plane intersection (+ analytic `t`)
  - [X] `ray_hit_plane_pga` regressive-product cross-check
  - [X] `segment_hit_oriented_line` with one-way normal check
  - [X] `snap_manhattan_normal` / `snap_manhattan_plane`
  - [X] 2D translational camera motor + `Camera2D`
  - [X] 3D yaw+forward ego motor integrator
- [X] Add `crates/cc_math/src/idempotent.rs`
  - [X] `StateIdempotent` with ε² = ε and ε_s ε_t = 0
  - [X] `StateBasis` primitive/orthogonal/complete checks
  - [X] transition operator `T = ε_{s'} M ε_s`
- [X] `cargo test -p cc_math` passes: 18 tests (unit + property style)

## Findings
- `clifford 0.3` Plane conventions are split: `from_normal_and_distance` /
  `signed_distance` use `n·x + dist = 0`, but `Line ⊻ Plane` reads `dist` as
  `n·x = dist`. `ray_hit_plane_pga` flips the offset to bridge them.
- A 2D spinor space admits at most two mutually orthogonal rank-1 projectors, so
  `StateBasis` lifts machines with `n > 2` states to diagonal units `E_kk` in
  dimension `n` (complete: `Σ ε_s = I`).
