# Execution Plan: The HolonEngine Implicate World Compiler (`HolonEngine`)

> **Revision 2026-10-21 (v3) — 2D-First (`example_input2d`) $\to$ 3D (`example_input3d`) Progression & Carmack Implementation Spec.**
>
> - **Two Real Gameplay Benchmarks (2D First, 3D Second):**
>   1. **`example_input2d/` (*Super Mario Bros. 3* World 1-1):** `smb3_w1_1_320x240.mp4` ($320 \times 240$ @ $30\text{ fps}$, 5.6 MB) + `smb3_w1_1_256x240.mp4` (native NES $256 \times 240$) + `smb3_w1_1.mp4` (720p) + `rules.md`. Proves 2D CFT scroll odometry, temporal-median ghost-free level stitching, tile-grid collision inference, canonical sprite CFT walk cycles, and playable 2D platforming.
>   2. **`example_input3d/` (*Wolfenstein 3D* Episode 1 Floor 1):** `wolf3d_e1m1_320x240.mp4` ($320 \times 240$ @ $30\text{ fps}$, 25 MB) + `wolf3d_e1m1.mp4` (720p) + `rules.md`. Extends the exact same $\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$ pipeline to 3D projective raycasting, horizon-column depth $1/z(x)$, Manhattan occupancy grids, and $\mathcal{Q}_{\text{Cl}}$ corridor navigation.
> - **Unified Camera & Geometry Model in $\mathbb{G}_{3,0,1}$:** A 2D side-scroller and a 3D corridor game share the same algebra: 2D is an orthographic camera with pure translational `Motor` $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_x e_{01} + v_y e_{02})\Delta t)$, while 3D is a perspective camera with yaw + forward `Motor` $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_{\text{fwd}} e_{03} + \Omega e_{12})\Delta t)$. CFT optical flow divergence ($\nabla \cdot \mathbf{v}_{\text{flow}}$) automatically selects the regime.

---

## 1. Architectural Thesis

Grounded in the [Dzog philosophy](https://github.com/jadbox/dzog/blob/main/philosophy.md), with the core concept in `holon_philosophy.md` and `holon_whitepaper.md` for engine technicals. `HolonEngine` compiles gameplay video(s) and text rules into a deterministic, playable Clifford world model:

1. **The Implicate Holon ($\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$):** Every world component is compiled into:
   $$\mathcal{H}_i = \left(B_i \in \mathbb{G}_{3,0,1},\; \psi_i \in (\mathbb{G}_{2,0}^+)^K,\; \{\epsilon_{i,s}, \hat{K}_{i,s}(\mathbf{k}, \omega)\}\right)$$
   * $B_i$: 2D/3D PGA blade (tile boundary `Line`s, 3D wall/door `Plane`s, actor `Point`s, and `Motor` poses).
   * $\psi_i$: $K$-channel semantic spinor projected via SVD from `llama-embedding` wording spaces.
   * $\{\epsilon_{i,s}, \hat{K}_{i,s}(\mathbf{k}, \omega)\}$: Primitive state idempotents ($\epsilon_s^2 = \epsilon_s$) paired with unwarped 2D+t CFT sprite/texture kernels.
2. **Strict Division of Labor:**
   * **CFT Spectral Extractor (`cc_field`):** Isolates zero-parallax HUD bars, estimates camera `Motor` odometry, separates static world geometry from moving actors, and extracts canonical 2D+t CFT sprite loops.
   * **Semantic Transducer (`llama.cpp` VLM + Embeddings):** Runs only during `cc enfold` (with deterministic `.cc_cache/` JSON fixtures) to label entity types/states and embed `rules.md`.
   * **Algebraic Incidence & Transition Compiler (`cc_math` + `cc_field`):** Snaps geometry to tile/Manhattan lattices via spectral Bragg peaks and `pga::snap_manhattan_plane`, and compiles empirical PGA incidence predicates (`StompTop`, `HitBlockBottom`, `UseDoor`, `LineOfSight`, `RayHit`) directly into state transition operators $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$.
3. **Playable Unfoldment (`cc unfold`):** Zero AI at runtime. Steps at $> 120\text{ Hz}$ headless and $60\text{ Hz}$ interactive (`--play`), rendering either the 2D side-scrolling tile+sprite world or the 3D PGA column-raycast world alongside live $\mathcal{Q}_{\text{Cl}}$ potential diagnostics.

---

## 2. Workspace Structure & Tech Stack

* **Language:** Rust (Edition 2024).
* **Algebra Core:** `clifford = "0.3"` (`projective::dim3`, `euclidean::{dim2, dim3}`, `dual`, `ops`).
* **Spectral Solver:** `rustfft` (1D column CFT, 2D spatial phase-correlation CFT, and 2D+t canonical sprite CFT).
* **Media IO:** `ffmpeg` CLI pipe (frame ingest & MP4 output) + `image`.
* **Local AI Transducer:** `llama.cpp` (`llama-server` / `llama-mtmd-cli` / `llama-embedding`) with SHA-256 content-addressed caching under `<input_dir>/.cc_cache/`.

```text
holonengine/
├── Cargo.toml
├── example_input2d/   # Super Mario Bros. 3 World 1-1 (256x240, 320x240, 720p + rules.md)
├── example_input3d/   # Wolfenstein 3D Episode 1 Floor 1 (320x240, 720p + rules.md)
├── crates/
│   ├── cc_math/       # G(2,0) spinors, idempotents, Q_Cl, G(3,0,1) PGA blades, Motors, meet/join
│   ├── cc_field/      # HUD separator, 2D CFT scroll + atlas stitcher, 3D column depth, 2D+t sprite CFT
│   └── cc_cli/        # `cc enfold | inspect | unfold` CLI, 2D & 3D playable runtimes
```

---

## 3. Concrete Algorithms: How `cc enfold` Compiles 2D and 3D Worlds

### Stage 1 — Bezel & HUD Partition (`partition_hud_and_viewport`)
1. Compute per-row and per-pixel temporal luminance/phase variance $\sigma_t^2(x, y)$ and cross-correlation with global camera optical flow across a sampled frame window.
2. Strip outer zero-variance borders/pillarboxes and static bezels (`#004040` in Wolf3D; black border in SMB3).
3. Partition contiguous horizontal bands with zero flow-correlation into the screen-locked `HudBar` (`y >= 192` in SMB3 `256x240`/`320x240`; `y >= 192` in Wolf3D `320x240`).
4. In 3D mode, isolate the bottom-center foreground sprite bank with zero translation parallax during camera yaw as `WeaponOverlay`.

### Stage 2 — Camera Regime Detection & World Geometry Reconstruction
Compute the 2D spatial CFT cross-power spectrum $\frac{\hat{I}_t \hat{I}_{t-1}^*}{|\hat{I}_t \hat{I}_{t-1}^*|}$ across viewport blocks:
* **Regime Check:** Measure radial optical-flow divergence $\nabla \cdot \mathbf{v}_{\text{flow}}$. Near-zero divergence selects the **2D Orthographic Pipeline** (`CameraMode::Ortho2D`); horizon-symmetric vertical scaling selects the **3D Perspective Pipeline** (`CameraMode::Perspective3D`).

### 2D Orthographic Pipeline (`example_input2d`)
1. **Sub-Pixel Scroll Odometry (`estimate_2d_scroll`):** Inverse FFT of the cross-power spectrum between consecutive viewports yields exact camera translation $(\Delta x_t, \Delta y_t)$ and cumulative world camera pose $(X_{\text{cam}}(t), Y_{\text{cam}}(t))$.
2. **Ghost-Free Temporal Median Atlas (`build_2d_world_atlas`):** Map every viewport frame into world coordinates $(X, Y) = (x + X_{\text{cam}}(t), y + Y_{\text{cam}}(t))$. Take the **per-pixel temporal median RGB** across all frames visiting $(X, Y)$. Transient moving actors (Mario, Goombas, Koopas, items) vanish automatically, producing a clean, static level panorama.
3. **Actor Residual Tubes (`extract_2d_actors`):** Threshold the difference $|I(x, y, t) - \text{Atlas}(x + X_{\text{cam}}(t), y + Y_{\text{cam}}(t))| > \tau_{\text{diff}}$, group connected components, and link actor trajectories $(X_i(t), Y_i(t), w_i, h_i)$ across frames via Hungarian bounding-box + CFT phase matching.
4. **Tile Lattice & Oriented Boundary Inference (`infer_2d_tile_physics`):**
   * Find the fundamental Bragg peak $(k_{\text{tile}, x}, k_{\text{tile}, y})$ of the atlas 2D CFT to lock the tile size ($16 \times 16$ in $256 \times 240$).
   * Classify each tile's oriented PGA boundary `Line`s ($L_{\text{top}}, L_{\text{bottom}}, L_{\text{left}}, L_{\text{right}}$) from observed actor trajectory discontinuities:
     * **Solid Tile (ground, pipe, brick, `?` block):** arrests downward velocity ($v_y > 0 \to 0$ on $L_{\text{top}}$), blocks upward head hits ($v_y < 0 \to 0$ on $L_{\text{bottom}}$), and reverses horizontal actor motion ($v_x \to -v_x$ on $L_{\text{left/right}}$).
     * **One-Way Semisolid Platform (pastel screw-boxes):** arrests downward landing on $L_{\text{top}}$ while allowing upward passage ($v_y < 0$) through its interior and bottom edge.

### Stage 3 — Canonical Sprite Unwarping & 2D+t CFT Vitality Kernels (`extract_canonical_sprite_cft`)
* For each tracked actor state segment (e.g., Mario run/jump, Goomba walk, Wolf3D guard patrol/shoot), resample the actor crop sequence into a canonical rest grid.
* Compute the 2D+t CFT $\hat{K}_{i,s}(k_x, k_y, \omega)$ and retain the top $M$ spectral modes (storing spatial silhouette/albedo in $\omega = 0$ and periodic limb/walk oscillation in $\omega_1 \dots \omega_M$).

### Stage 4 — Algebraic Incidence & State-Transition Binding (`compile_transitions`)
* Bind observed state transitions $\epsilon_s \to \epsilon_{s'}$ directly to empirical PGA geometric incidence predicates and velocity discontinuities ($\Delta \mathbf{P}_B$):
  * **2D Predicates:** `TopStomp` ($P_{\text{player\_foot}} \wedge L_{\text{enemy\_top}}$ with $v_y > 0$), `BottomBump` ($P_{\text{player\_head}} \wedge L_{\text{block\_bottom}}$ with $v_y < 0$), `SideHit` ($P_{\text{player}} \wedge L_{\text{enemy\_side}}$).


### Stage 5 — Optional `llama.cpp` VLM + Wording-Space Spinor Projection (FEATURE ON HOLD)
* Run `llama-mtmd-cli` / `llama-server` with `--temp 0 --seed 42` and a strict GBNF grammar (`holon_schema.gbnf`) on a contact sheet of extracted tiles and canonical actor sprites to assign canonical labels and state names.
* Run `llama-embedding` on VLM descriptions and `rules.md` clauses; compute the thin SVD of the normalized Gram matrix $S_{ij} = \langle \hat{e}_i, \hat{e}_j \rangle$ to map each Holon and rule into a $K$-channel spinor $\psi_i = R_i e^{\theta_i e_{12}} \in (\mathbb{G}_{2,0}^+)^K$, binding rules via the bilinear spinor inner product $\langle \psi_{\text{rule}}, \widetilde{\psi}_{\text{holon}} \rangle_0$.

### Stage 6: 3D Perspective Pipeline (`example_input3d`) (FEATURE ON HOLD)
1. **Horizon-Symmetric Column Depth (`extract_column_depth`):** Locate the viewport horizon $y_h = H_{\text{view}}/2$ separating flat ceiling (`#383838`) and floor (`#707070`). For each column $x$, measure the symmetric vertical wall span $h(x, t)$ around $y_h$ to obtain inverse projective depth $d(x, t) = f / h(x, t)$.
2. **Foreground Sprite Residual Masking:** Columns where color/span breaks the local piecewise-linear $d(x, t)$ trend and moves independently of camera yaw are masked as 3D billboard actors (guards, lamps, pickups), and wall depth $d(x, t)$ is linearly interpolated behind them.
3. **Log-Odds Manhattan Occupancy Map (`build_3d_raycast_map`):**
   * Estimate incremental camera yaw $\Delta \theta_t$ from horizontal shift of column spans and forward translation $\Delta s_t$ from radial expansion $\dot{d}(x, t) / d(x, t)$.
   * Cast each column ray $(P_{\text{cam}}(t), \theta_t + \alpha_x, z(x, t))$ into a 2D world grid quantized by the wall CFT period $\lambda_{\text{tile}}$. Accumulate log-odds free space along the ray and orthogonal wall/door `Plane` normals ($\pm e_1, \pm e_2$) at the termination cell.
4. **Inverse-Perspective Texture Rectification (`extract_wall_textures`):** Resample frontal wall and cyan door spans vertically by $h(x, t) \to 64$ and horizontally along the wall plane to $64$, recovering crisp $64 \times 64$ wall and door textures.
5.  **3D Predicates State-Transition:** `UseDoor` ($\langle P_{\text{player}} \vee \Pi_{\text{door}} \rangle < r_{\text{use}} \land \text{Use}$), `EnemyAlert` ($(P_{\text{player}} \vee P_{\text{enemy}}) \wedge \Pi_{\text{wall}} = \varnothing$), `WeaponRayHit` ($L_{\text{aim}} \wedge \Pi_{\text{enemy}} \neq \varnothing \land \text{Fire}$).

---

## 4. Phased Implementation Milestones & Verification Gates

### Phase 1: Algebraic Kernel, PGA Incidence & State Idempotents (`cc_math`)
* **Implementor Steps:**
  1. Retain all passing `cc_math` and `cc_field` tests ($\mathcal{Q}_{\text{Cl}}$ scale invariance, PGA spine fold, CFT soliton retention, distance-cone navigation).
  2. Add `crates/cc_math/src/pga.rs`:
     * 2D/3D oriented boundary `Line` and `Plane` intersection (`ray_hit_plane`, `segment_hit_oriented_line` with one-way normal check $\langle \mathbf{v}, \mathbf{n} \rangle < 0$).
     * Orthogonal Manhattan plane snapper (`snap_manhattan_normal`).
     * Translational 2D camera `Motor` and 3D yaw+forward ego `Motor` integrators.
  3. Add `crates/cc_math/src/idempotent.rs`:
     * `StateIdempotent` ($\epsilon_s^2 = \epsilon_s$, $\epsilon_s \epsilon_{s'} = 0$ for $s \neq s'$) and transition operator $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$.
* **Exit Gate:** `cargo test -p cc_math` passes unit + property tests for one-way platform line incidence, exact 3D ray-plane distances, Manhattan snapping, and idempotent state algebra.

### Phase 2: 2D Side-Scroller Compiler & Playable SMB3 Benchmark (`cc_field` + `cc_cli` 2D)
* **Implementor Steps:**
  1. Add `crates/cc_field/src/video_io.rs`: `ffmpeg` raw RGB24 frame reader and MP4/PPM writer.
  2. Add `crates/cc_field/src/enfold2d.rs`:
     * `partition_hud_and_viewport` (isolates bottom HUD bar $y \ge 192$ in `example_input2d/smb3_w1_1_320x240.mp4`).
     * `estimate_2d_scroll` (2D CFT phase-correlation odometry returning frame-by-frame $(X_{\text{cam}}(t), Y_{\text{cam}}(t))$).
     * `build_2d_world_atlas` (registers frames into world space and computes temporal median RGB to produce a clean, actor-free World 1-1 panorama).
     * `extract_2d_actors_and_tiles` (residual actor bounding-box tracker, canonical $32 \times 32$ 2D+t CFT sprite kernel extractor, and $16 \times 16$ tile solidity/one-way platform classifier).
  3. Add 2D playable runtime in `cc_cli`: scrolling atlas renderer, CFT sprite synthesizer, HUD overlay, gravity + one-way PGA line physics, and $\mathcal{Q}_{\text{Cl}}$-guided Goomba/Koopa patrol.
* **Exit Gate:**
  * `cargo test -p cc_field --test smb3_enfold` runs on `example_input2d/smb3_w1_1_320x240.mp4` (`00:00:05`–`00:00:40`) and verifies:
    1. HUD mask IoU $> 0.98$,
    2. Monotonic rightward scroll odometry matching ground-truth level displacement within $< 1.5\text{ px/frame}$,
    3. Ghost-free stitched World 1-1 background panorama (temporal median removes $> 99\%$ of Mario/Goomba pixels from the static atlas),
    4. Playable `cc unfold smb3.cc` run where Mario runs, jumps onto a one-way screw-box platform, hits a `?` block from below, and stomps a Goomba at $> 120\text{ FPS}$ headless / $60\text{ FPS}$ interactive.

#### Phase 2 audit — defect resolutions (2026-10-21)

1. **Odometry HUD exclusion:** `ScrollEstimator::gray()` now stores `band_h` and limits sampling to rows `0..band_h`, zeroing the static HUD buffer. Eliminates the $\pm 10\text{ px}$ registration smear; clouds and block borders are pixel-sharp.
2. **Actor mask coordinates:** Pass 3 passes residual boxes directly in frame coordinates (`*b`), ensuring moving actors are properly masked out during atlas synthesis.
3. **Grounded obstacle shelf extraction:** Grounded components ($h \ge 2\times\text{TILE}$, bottom touching ground line) are segmented into horizontal flat shelves of width $\ge 12\text{ px}$. Pastel block tops and the green pipe top are promoted to one-way collision surfaces while leaving curved green hills as scenery.
4. **Sprite rendering:** `cc unfold` composites the canonical extracted player sprite with alpha blending, centered and feet-aligned.
5. **Autonomous Actor Holons & Clifford Patrol / Stomp:** Modeled on Basil Hiley's process ontology (`crates/cc_math/src/holon.rs`, `CliffordDensityElement`). Entities inhabit minimal left ideals $\mathcal{I}_i = \mathcal{C}\ell \epsilon_i$ with density element $\rho_{c,i} = \Phi_{L,i}\widetilde{\Phi}_{L,i} = \rho_i U_i \epsilon_{i,s}\widetilde{U}_i$, bilinear heading $\mathbf{s}_i = U_i e_1 \widetilde{U}_i$, and Bohm momentum $\mathbf{P}_{B,i} = \rho_i v_{0,i} \mathbf{s}_i$. Principal mobility axis ($e_1$ vs $e_2$) and excursion bounds are inferred directly from the observed world-space excursion tensor `TrackRecord::span` without sprite-size heuristics. Lateral or vertical excursion boundary incidence triggers rotor reflection $U_i \mapsto N U_i \widetilde{N}$ and idempotent transition $T_{0 \leftrightarrow 1}$. Top-down relational incidence collapses the actor to $\epsilon_2$ ($\rho_i \to 0$) and imparts upward bounce restitution to the player. Demonstrated at $> 2500\text{ Hz}$ headless throughput (`recordings/task_demo_smb3_playable.mp4`).
6. **Planar Zero-Gravity Arena Generalization (`example_input2d_pacman`):** Added spectral static-camera detection (`detect_camera_is_static`), topological 8-connected-component separation of extended maze walls ($\max(w, h) \ge 2 \times \text{tile}$) vs isolated corridor collectible excitations ($\max(w, h) \le \text{tile} + 4$, stored in `ModelManifest::collectibles`), chromatic-spinor-coherent actor tracking (`track_actors_coherent`) with toroidal warp-tunnel distance and composite-overlap rejection, and $\mathcal{Q}_{\text{Cl}}$-guided planar unfoldment (`recordings/task_demo_pacman_replay.mp4`) with zero game-specific RGB constants.
7. **Tile-Free Continuous World Representation (`Model2D::solid_at_world`, `one_way_at_world`, `is_open_below_rect`, `build_continuous_qfield`):** Decoupled runtime collision and $\mathcal{Q}_{\text{Cl}}$ potential synthesis from discrete tile grids (`kinds: Vec<TileKind>`). Continuous world-space boundary rects/lines (`solids`, `platforms`, `collectibles`, `OrientedLine2`) are now the primary geometric representation so non-tile 2D games (vector arenas, freeform contours, curved slopes) execute with `tile = 0, kinds = []`, treating Bragg-lattice snapping strictly as an optional spectral prior when periodic CFT peaks exist.
8. **Visual-Physical Support Coincidence & Phantom-Overhang Elimination:** Eliminated 1-px border bleed into adjacent empty tiles (`overlap >= 4 px`) and replaced hardcoded player hitbox constants (`PW`, `PH`, `head_ext`) with learned `TrackRecord::size` dimensions so upward block impacts and top-down stomps coincide to the pixel (`impacts=4, coins=1` on `smb3.cc`).
9. **Top-Down & Vertical Flight Shmup Benchmark (`example_input2d_raptor`):** Generalized `Ortho2D-Scroll` to vertical translational camera ego motors ($M_{\text{ego}}(t) = \exp(-\frac{1}{2} v_y e_{02} \Delta t)$), decoupled zero-gravity scrolling worlds ($g = 0$) from 1D platformer jumping, added abstract attached process effects (`AttachedWake` trailing excitations along $-\mathbf{s}$, `SolitonProjectile` along $+\mathbf{s}$, `FieldExcitation` collectible absorption, and `RadialDispersion` burst on state idempotent collapse), enfolded *Raptor: Call of the Shadows* into `example_input2d_raptor/raptor.cc` (320x684 stitched level atlas, 12 actor tracks), and verified playable unfoldment at $> 2,600\text{ Hz}$ (`recordings/task_demo_raptor_playable.mp4`).

#### Phase 2.5 — Pure Implicate Dynamics & De-Clichéing (Completed)

1. **Inferred Background Drift & Impulse Generators (Replacing Hardcoded `gravity`, `RUN`, `JUMP`):**
   - [X] Replaced `manifest.gravity` (`0.48` if `has_ground_plane`) and runtime constants (`RUN = 1.6`, `JUMP = 6.2`) with `env_drift: [f32; 2]` and `control_motor: [f32; 2]` regressed via `infer_world_kinematics` over multi-frame finite differences $\ddot{\mathbf{x}}_i(t)$ and ballistic apex heights $\Delta h$ (`crates/cc_field/src/model2d.rs`).
2. **Empirical Boundary Admittance Blades & Pre-Incidence Causal Texture Restoration (Replacing `BlockProp` and `?` Glyph Forgery):**
   - [X] Added `BoundaryBlade` (`rect`, `permeability: [left, right, top, bottom]`, `excitable`, `frame_color`) inferred via `Model2D::infer_boundary_blades`.
   - [X] Added pre-incidence causal restoration in `enfold2d::enfold` (`init_acc` over pre-impact frames) so excitable blocks retain their genuine video State 0 appearance in `atlas` while `frame_color` captures their post-incidence State 2 perimeter color—deleting `BlockProp` and the hardcoded `Question mark glyph pattern` in `crates/cc_cli/src/main.rs`.
3. **2D+t CFT Vitality Kernels (Replacing Procedural Pixel Shaders):**
   - [X] Extracted DC ($\omega_0$) and first temporal harmonic ($\omega_1$, stored in `harmonic_sprite`) CFT kernels in `enfold2d` Pass 3 and added `Model2D::evaluate_sprite_cft(sprite_idx, harmonic_idx, phase, flip_x)`.
   - [X] Deleted all procedural pixel routines in `cc_cli` (`chomp_open` wedge cut, `skirt_shift`, pupil RGB shifts, and `?` glyph drawing).
4. **Unified Continuous `WorldState2D` Step & Spinor Occlusion Factorization:**
   - [X] Merged the `is_static`, `is_flight`, and terrestrial loops in `crates/cc_cli/src/main.rs` into a single `WorldState2D::step` and `WorldState2D::render`.
   - [X] Added spinor factorization of 2-body crossing occlusions in `track_actors_coherent` (verified by `enfold2d::tests::spinor_factorization_splits_crossing_tracks`) and removed Mario-specific `ActorKind::Goomba` / `Koopa` enum variants.


### Phase 3: 3D Raycast Compiler & Wolfenstein 3D Extraction (`cc_field` 3D)
* **Implementor Steps:**
  1. Add `crates/cc_field/src/enfold3d.rs`:
     * `detect_camera_regime` (verifies `Ortho2D` on `example_input2d` and `Perspective3D` on `example_input3d`).
     * `extract_column_depth_and_planes` (measures symmetric vertical wall span $h(x, t)$ around horizon $y_h$ in `example_input3d/wolf3d_e1m1_320x240.mp4`, masks foreground guard/lamp residuals, and computes $d(x, t) = f / h(x, t)$).
     * `build_3d_raycast_map` (integrates yaw/forward ego-motion and accumulates the Manhattan log-odds occupancy grid of walls and cyan sliding doors).
     * `extract_wall_textures_and_sprites` (rectifies $64 \times 64$ stone/door textures and canonical $64 \times 64$ guard/pistol 2D+t CFT kernels).
* **Exit Gate:**
  * `cargo test -p cc_field --test wolf3d_enfold` runs on `example_input3d/wolf3d_e1m1_320x240.mp4` (`00:00:12`–`00:01:00`) and verifies:
    1. Bezel + bottom HUD bar + centered pistol separation with IoU $> 0.98$,
    2. Reconstructed orthogonal E1M1 starting room, cyan door, and corridor wall planes within $< 2\%$ depth error,
    3. Canonical $64 \times 64$ guard and wall texture synthesis with $> 25\text{ dB}$ PSNR.

### Phase 4: Wording-Space Spinor Projection & Counterfactual CLI (`cc_field` + `cc_cli`)
* **Implementor Steps:**
  1. Add optional `llama.cpp` subprocess/HTTP client (`llama-mtmd-cli` / `llama-server` + `llama-embedding`), GBNF schema, and `.cc_cache/` fixture persistence.
  2. Implement Gram-matrix SVD spinor projection and bilinear spinor coherence coupling $\langle \psi_i, \widetilde{\psi}_j \rangle_0$ across both `example_input2d` and `example_input3d`.
  3. Complete the unified `cc` CLI (`cc enfold <dir> -o <model.cc>`, `cc inspect <model.cc> [--query]`, `cc unfold <model.cc> [--play | --input | --perturb] [-o out.mp4]`) supporting both 2D side-scrolling and 3D PGA raycast worlds.
* **Exit Gate (Full Engine Complete):**
  1. **2D Gate:** `cc enfold example_input2d -o smb3.cc && cc unfold smb3.cc --input example_input2d/play_trace.json -o smb3_play.mp4` produces a verified 60 FPS playable side-scroller + $\mathcal{Q}_{\text{Cl}}$ diagnostic video.
  2. **3D Gate:** `cc enfold example_input3d -o wolf3d.cc && cc unfold wolf3d.cc --input example_input3d/play_trace.json -o wolf3d_play.mp4` produces a verified 60 FPS playable 3D raycast corridor + top-down $\mathcal{Q}_{\text{Cl}}$ diagnostic video.
  3. **Counterfactual Gate:** Passing `--perturb` (e.g., `"guards flee from player"` or `"goombas leap toward Mario"`) flips the wording-space spinor phase and deterministically alters actor trajectories in both 2D and 3D runtimes.

Development NOTES:
- Rate your task planning as 1-10 in confidence (10 being the most).
- Do NOT "overfit" the engine/model to one speicific game (like having is_pipe_green extractor that is only for mario games) as this model should adapt to any 2d or 3d game.
- be sure your updating task_phase[N].md files with progress and what task is actively being worked on and add any new next tasks discovered to do. See and update ./notes.md for developer self-notes for random insights and tips.
- If you get stuck on a hard problem with low confidence, you can call OpenCode CLI in non-interactive mode to use the great model Google Gemini 4 Argon to give you a solution plan to solve a hard problem. However, Argon is very expensive so only use it when you are stuck or really need another second oppinion. Example "opencode run --model google/argon-4 "Your name is Argon. Explain how closures work in JavaScript"" or adding file context "opencode run --model google/argon-4 -f src/main.ts "Your name is Argon. Review this file for edge cases"
- save generated models (like smb3_v2.cc) into the example folder it was used to be generated from (i.e. example_input2d) and override cc models as needed.
- If you pretty sure you fixed a visual bug, do another new video recording so I can keep up to date. Smartly determine if the bug fix requires folding a new model or just using the last built one (if the bug was just in the use of the model).
- Consider updating docs/holon_whitepaper.md with any key notable engine design systems concepts like using CliffordDensityElement.