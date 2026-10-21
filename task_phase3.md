# Milestone: Phase 3 — Canonical Sprite Unwarping & 2D+t CFT Vitality Kernels (`extract_canonical_sprite_cft`)

**Goal:** Implement Stage 3 spatiotemporal sprite decomposition—resampling tracked actor crops into canonical rest frames ($32 \times 32$), computing the 2D+t Clifford Fourier Transform $\hat{K}_i(\mathbf{k}, \omega)$, and retaining the dominant temporal oscillation harmonics ($\omega_0, \omega_1$) for continuous spectral animation playback.

## Plan & Implementation Tasks

- [ ] **1. Canonical Crop Resampling & Stabilization (`cc_field::enfold2d`)**
  - [ ] Extract tracked bounding-box sequences for each actor track.
  - [ ] Center and resample actor crops into canonical $32 \times 32$ rest grid via bilinear interpolation.
  - [ ] Alpha-matte isolation against the registered world atlas.

- [ ] **2. 2D+t Clifford Fourier Transform Kernel (`crates/cc_field/src/wave.rs` / `cft.rs`)**
  - [ ] 3D FFT ($N_x \times N_y \times N_t$) over stabilized temporal crop tubes.
  - [ ] Separate DC mode $\omega = 0$ (canonical spatial silhouette/albedo) from fundamental walk/chomp harmonic $\omega_1$.
  - [ ] Compress into compact spectral vitality record: $\{\omega_0, \omega_1, \mathbf{A}_1, \phi_1\}$.

- [ ] **3. Container Payload & Decoding (`crates/cc_field/src/model2d.rs`)**
  - [ ] Add spectral vitality modes to `TrackRecord` in `ModelManifest`.
  - [ ] Maintain backward compatibility with static PNG sprite fallbacks.

- [ ] **4. Spectral Synthesis at Runtime (`crates/cc_cli/src/main.rs`)**
  - [ ] Synthesize active walk/motion animation during unfold via inverse harmonic evaluation:
    $$I(x, y, t) = \hat{K}_0(x, y) + \text{Re}\{\hat{K}_1(x, y) e^{i \omega_1 t}\}$$
  - [ ] Replace boolean frame-counter flips with continuous CFT spectral phase evaluation.

- [ ] **5. Exit Gate Verification**
  - [ ] `cargo test -p cc_field` passes integration tests on SMB3, Pac-Man, and Raptor vitality kernels.
  - [ ] Rebuilt `.cc` models store spectral harmonics.
  - [ ] Verified demo recordings exported in `./recordings/`.
