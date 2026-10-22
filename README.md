# HolonEngine
<small>by [Jonathan Dunlap](https://www.linkedin.com/in/jonathandunlap/)</small>

**Note: This is a WIP (pre-alpha) experimental proof-of-concept world compiler and runtime grounded in Projective Geometric Algebra, Spinor Algebra, and Basil Hiley's Clifford Implicate Order.**

**HolonEngine** compiles raw gameplay video streams into deterministic, self-contained Implicate World Models that executes with zero neural inference at runtime.

Conventional game engines hardcode per-game state machines (`if is_pacman { eat_dot() } else if is_mario { stomp_goomba() }`), while autoregressive video world models hallucinate pixels frame-by-frame without persistent geometry or conservation laws. **HolonEngine** replaces both with an algebraic process ontology: **`cc enfold`** extracts camera odometry, actor-free panoramas, oriented geometric boundaries, and canonical spinor processes from raw video; **`cc unfold`** evolves the compiled `.cc` world forward via Bohmian momentum, state idempotents, and the Clifford Quantum Potential ($\mathcal{Q}_{\text{Cl}}$).

### WIP videos (BUGS! But it almost works.)



https://github.com/user-attachments/assets/6ba0d2f6-e1cf-411a-923d-9891045ddc85

https://github.com/user-attachments/assets/f1584bc7-a3e8-4273-87ef-3d432059eddd

https://github.com/user-attachments/assets/21c82063-c287-4301-a63c-7987b43b3e23


---

## Core Concepts: The Implicate Order as Computation

Drawing on David Bohm and Basil Hiley's *The Undivided Universe* (1993), Hiley & Callaghan's *The Clifford Algebra Approach to Quantum Mechanics* (2010), and `docs/holon_philosophy.md`, every component of a compiled world is an **Algebraic Holon** coupled to an active spatial medium:

1. **The Clifford Density Element (`CliffordDensityElement`)**: Rather than storing ad-hoc boolean flags, each entity inhabits a minimal left ideal $\Phi_{L,i} = R_i\,U_i\,\epsilon_{i,s}$ with Clifford Density Element $\rho_{c,i} = \Phi_{L,i}\widetilde{\Phi}_{L,i} = \rho_i\,U_i\,\epsilon_{i,s}\,\widetilde{U}_i$. Heading $\mathbf{s}_i = U_i e_1 \widetilde{U}_i$ and Bohm momentum $\mathbf{P}_{B,i} = \rho_i v_{0,i}\mathbf{s}_i$ are extracted directly as algebraic bilinear invariants.
2. **Primitive State Idempotents & Incidence Transitions**: Discrete behavioral branches (patrol, reflected return, bumped impulse, collapsed/consumed) are mutually orthogonal idempotents $\epsilon_s^2 = \epsilon_s$, $\epsilon_s \epsilon_{s'} = 0$. Geometric incidence in $\mathbb{G}_{3,0,1}$ triggers algebraic transitions $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$ and rotor reflections $U_i \mapsto N U_i \widetilde{N}$.
3. **Active Information ($\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1}$)**: The spatial medium carries a continuous amplitude-and-spinor field $\Psi = R\,\mathcal{U}$. Its scale-invariant scalar part $\langle \mathcal{Q}_{\text{Cl}} \rangle_0 = -\nabla^2 R / R$ and bivector spin-torque $\Omega_Q = \mathbf{s} \wedge (-\nabla \mathcal{Q}_{\text{Cl}})$ steer autonomous holons through open corridors and around obstacles without game-specific pathfinding.
4. **Spectral Camera & Scene Discovery**: 2D FFT cross-power phase correlation automatically partitions static HUD overlays, discovers camera regimes (`Ortho2D-Static` arenas, `Ortho2D-Scroll` side-scrollers, `Perspective3D` raycast corridors), and synthesizes ghost-free world panoramas via quantized temporal mode accumulation.
5. **Continuous Geometry (No Tile Assumption Required)**: Obstacles, one-way surfaces, and collectible excitations are represented as continuous oriented PGA blades ($L_k \in \mathbb{G}_{3,0,1}$) and continuous field impedance $R(x, y)$. When a periodic Bragg peak is present in the 2D CFT, lattice snapping acts as an optional regularizer rather than a structural requirement.

---

## Quick Start & CLI Usage

```bash
# Run the workspace verification suite (Clifford algebra, PGA incidence, CFT odometry, and model roundtrips)
cargo test --release

# Build the `cc` compiler and runtime binary
cargo build --release

# 1. Enfold raw gameplay videos into self-contained .cc world models (no rules file required)
./target/release/cc enfold example_input2d/smb3_w1_1_256x240.mp4 -o example_input2d/smb3.cc
./target/release/cc enfold example_input2d_pacman/pacman_240x308.mp4 -o example_input2d_pacman/pacman.cc
./target/release/cc enfold example_input2d_raptor/raptor_320x240.mp4 --start 0 --duration 25 -o example_input2d_raptor/raptor.cc

# 2. Inspect a compiled .cc model (manifest metadata, boundary layout, and diagnostic PNG dumps)
./target/release/cc inspect example_input2d/smb3.cc --atlas /tmp/smb3_atlas.png --tiles /tmp/smb3_tiles.png
./target/release/cc inspect example_input2d_pacman/pacman.cc --atlas /tmp/pacman_atlas.png
./target/release/cc inspect example_input2d_raptor/raptor.cc --atlas /tmp/raptor_atlas.png

# 3. Unfold a compiled .cc model forward at >2,000 Hz and record MP4 verification demos
./target/release/cc unfold example_input2d/smb3.cc -o recordings/task_demo_smb3_playable.mp4
./target/release/cc unfold example_input2d_pacman/pacman.cc -o recordings/task_demo_pacman_replay.mp4
./target/release/cc unfold example_input2d_raptor/raptor.cc -o recordings/task_demo_raptor_playable.mp4

# 4. Step a live 2D Clifford wave + Q_Cl potential field with inertial boats
./target/release/cc sim --grid 128 --frames 240 --boats 6 -o /tmp/qcl_sim.mp4
```

---

## Documentation

- [**Engine Architecture Whitepaper (`docs/holon_whitepaper.md`)**](./docs/holon_whitepaper.md): Mathematical and systems specification of `CliffordDensityElement`, bilinear invariants, $\mathcal{Q}_{\text{Cl}}$ quantum torque, and tile-free continuous implicate world compilation.
- [**Holon Philosophy (`docs/holon_philosophy.md`)**](./docs/holon_philosophy.md): The Living Page thesis—Implicate Holons in $\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$, CFT spectral decomposition, and Active-GATr dynamics.
- [**General Ontology (`philosophy.md`)**](https://github.com/jadbox/dzog/blob/main/philosophy.md): Philosophical and physical foundations of relation-first computing over continuous fields.
- [**Execution Plan & Roadmap (`docs/plan.md`)**](./docs/plan.md): Phased milestones, verification gates, and audit history across 2D and 3D benchmarks.

---

## Workspace Crates

| Crate | Purpose |
| :--- | :--- |
| [`crates/cc_math`](./crates/cc_math) | $\mathbb{G}_{2,0}$ multivectors (`Mv2`), Hiley's `CliffordDensityElement`, `CliffordQuantumPotential` ($\mathcal{Q}_{\text{Cl}}$), primitive `StateIdempotent` & `TransitionOperator`, and $\mathbb{G}_{3,0,1}$ PGA blades/motors. |
| [`crates/cc_field`](./crates/cc_field) | Split-step Fourier wave solver, sampled `QField` gradients, `ffmpeg` video IO, 2D CFT scroll odometry, temporal-mode atlas stitcher, spinor-coherent actor tracker, and `.cc` (`CC2D`) container codec. |
| [`crates/cc_cli`](./crates/cc_cli) | `cc` command-line compiler and runtime (`enfold`, `inspect`, `unfold`, `sim`, `enfold-dump`). |
