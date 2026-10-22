# HolonEngine
<small>by [Jonathan Dunlap](https://www.linkedin.com/in/jonathandunlap/)</small>

**Note: This is a WIP (pre-alpha) experimental proof-of-concept world compiler and runtime grounded in Projective Geometric Algebra, Spinor Algebra, and Basil Hiley's Clifford Implicate Order.**

**HolonEngine** compiles raw gameplay video streams into deterministic, self-contained Implicate World Models that executes with zero neural inference at runtime.

Conventional game engines hardcode per-game state machines (`if is_pacman { eat_dot() } else if is_mario { stomp_goomba() }`), while autoregressive video world models hallucinate pixels frame-by-frame without persistent geometry or conservation laws. **HolonEngine** replaces both with an algebraic process ontology: **`holon enfold`** extracts camera odometry, actor-free panoramas, oriented geometric boundaries, and canonical spinor processes from raw video; **`holon unfold`** evolves the compiled `.cc` world forward via Bohmian momentum, state idempotents, and the Clifford Quantum Potential.

### WIP videos (BUGS! But it almost works.)

https://github.com/user-attachments/assets/6b4ef19c-9c5c-4da6-aa5d-4bac3b9edd62

https://github.com/user-attachments/assets/a32b8c12-7d2b-42a1-9578-b5d97f4688ef

https://github.com/user-attachments/assets/1bc33cda-196f-4127-8e3a-01ce6d0b7019

---

## Core Concepts: The Implicate Order as Computation

Conventional game engines treat worlds atomistically: independent objects manipulate loose bags of mutable variables, coordinated by imperative state machines and ad-hoc collision loops. Drawing on David Bohm and Basil Hiley's process physics, **HolonEngine** replaces object atomism with an unbroken implicate order. Every entity is an **Algebraic Holon**—a localized process enfolded in an active spatial medium:

1. **The Clifford Density Element (`CliffordDensityElement`)**: Rather than juggling fragmented variables (`position`, `velocity`, `facing_left`, `is_grounded`, `is_alive`), an entity's complete physical and operational posture is unified within an algebraic density element. Heading, momentum, and vitality are intrinsic geometric invariants rather than synchronized flags. When process density vanishes, the entity naturally ceases interaction, satisfying conservation laws without ad-hoc cleanup logic.
2. **Primitive State Idempotents & Incidence Transitions**: Discrete behavioral branches (patrolling, rebounding, absorbing impulses, collapsing) are mutually exclusive operational modes of a single entity. Physical boundary incidence acts as a geometric projection that rotates orientation and transitions behavioral regimes directly, preserving causal continuity across genres without nested conditional branches.
3. **Active Information & The Clifford Potential**: Space is an active information-bearing medium rather than an empty backdrop. Boundaries, corridors, and targets shape a continuous potential landscape whose curvature exerts steering torque on autonomous entities. Navigation along open corridors and around obstacles emerges directly from field interaction, eliminating graph-based pathfinding (A* or navmeshes).
4. **Spectral Camera & Scene Discovery**: Frequency-domain analysis separates visual streams into camera motion, stationary interface elements, and background geography without supervision. Phase correlation tracks camera translation across frames, isolates motionless HUD overlays through temporal persistence, and synthesizes ghost-free world panoramas across time without human labeling or inpainting artifacts.
5. **Continuous Geometry (No Tile Assumption Required)**: Space in game worlds is fundamentally continuous. Surfaces, slopes, barriers, and collectible tokens are represented as oriented geometric boundaries and field impedances. When periodic tile structures are present, the compiler discovers them as spectral resonances to optionally snap contours; when absent, the engine simulates continuous vector geometry without quantization bias.

*(For formal mathematical specifications, spinor derivations, and bilinear invariants, see the [Architecture Whitepaper](docs/holon_whitepaper.md).)*

---

## Quick Start & CLI Usage

```bash
# Run the workspace verification suite (Clifford algebra, PGA incidence, CFT odometry, and model roundtrips)
cargo test --release

# Build the `holon` compiler and runtime binary
cargo build --release

# 1. Enfold raw gameplay videos into self-contained .cc world models (no rules file required)
./target/release/holon enfold example_input2d/smb3_w1_1_256x240.mp4 -o example_input2d/smb3.cc
./target/release/holon enfold example_input2d_pacman/pacman_240x308.mp4 -o example_input2d_pacman/pacman.cc
./target/release/holon enfold example_input2d_raptor/raptor_320x240.mp4 --start 0 --duration 25 -o example_input2d_raptor/raptor.cc

# 2. Inspect a compiled .cc model (manifest metadata, boundary layout, and diagnostic PNG dumps)
./target/release/holon inspect example_input2d/smb3.cc --atlas /tmp/smb3_atlas.png --tiles /tmp/smb3_tiles.png
./target/release/holon inspect example_input2d_pacman/pacman.cc --atlas /tmp/pacman_atlas.png
./target/release/holon inspect example_input2d_raptor/raptor.cc --atlas /tmp/raptor_atlas.png

# 3. Unfold a compiled .cc model forward at >2,000 Hz and record MP4 verification demos
./target/release/holon unfold example_input2d/smb3.cc -o recordings/task_demo_smb3_playable.mp4
./target/release/holon unfold example_input2d_pacman/pacman.cc -o recordings/task_demo_pacman_replay.mp4
./target/release/holon unfold example_input2d_raptor/raptor.cc -o recordings/task_demo_raptor_playable.mp4
```

---

## Documentation

- [**Engine Architecture Whitepaper (`docs/holon_whitepaper.md`)**](./docs/holon_whitepaper.md): Mathematical and systems specification of `CliffordDensityElement`, bilinear invariants, quantum torque steering, and tile-free continuous implicate world compilation.
- [**Holon Philosophy (`docs/holon_philosophy.md`)**](./docs/holon_philosophy.md): Process ontology of enfoldment/unfoldment, algebraic holons, pregeometry, and Active Information.
- [**General Ontology (`philosophy.md`)**](https://github.com/jadbox/dzog/blob/main/philosophy.md): Philosophical and physical foundations of relation-first computing over continuous fields.
- [**Execution Plan & Roadmap (`docs/plan.md`)**](./docs/plan.md): Phased milestones, verification gates, and audit history across 2D and 3D benchmarks.

---

## Workspace Crates

| Crate | Purpose |
| :--- | :--- |
| [`crates/cc_math`](./crates/cc_math) | $\mathbb{G}_{2,0}$ multivectors (`Mv2`), Hiley's `CliffordDensityElement`, `CliffordQuantumPotential`, primitive `StateIdempotent` & `TransitionOperator`, and $\mathbb{G}_{3,0,1}$ PGA blades/motors. |
| [`crates/cc_field`](./crates/cc_field) | Split-step Fourier wave solver, sampled `QField` gradients, `ffmpeg` video IO, 2D CFT scroll odometry, temporal-mode atlas stitcher, spinor-coherent actor tracker, and `.cc` (`CC2D`) container codec. |
| [`crates/cc_cli`](./crates/cc_cli) | `holon` command-line compiler and runtime (`enfold`, `inspect`, `unfold`). |
