# Execution Plan: The Clifford Implicate World Compiler (`cliffordcore`)

> **Revision 2026-10-21 (v3) — 2D-First (`example_input2d`) $\to$ 3D (`example_input3d`) Progression & Carmack Implementation Spec.**
>
> - **Two Real Gameplay Benchmarks (2D First, 3D Second):**
>   1. **`example_input2d/` (*Super Mario Bros. 3* World 1-1):** `smb3_w1_1_320x240.mp4` ($320 \times 240$ @ $30\text{ fps}$, 5.6 MB) + `smb3_w1_1_256x240.mp4` (native NES $256 \times 240$) + `smb3_w1_1.mp4` (720p) + `rules.md`. Proves 2D CFT scroll odometry, temporal-median ghost-free level stitching, tile-grid collision inference, canonical sprite CFT walk cycles, and playable 2D platforming.
>   2. **`example_input3d/` (*Wolfenstein 3D* Episode 1 Floor 1):** `wolf3d_e1m1_320x240.mp4` ($320 \times 240$ @ $30\text{ fps}$, 25 MB) + `wolf3d_e1m1.mp4` (720p) + `rules.md`. Extends the exact same $\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$ pipeline to 3D projective raycasting, horizon-column depth $1/z(x)$, Manhattan occupancy grids, and $\mathcal{Q}_{\text{Cl}}$ corridor navigation.
> - **Unified Camera & Geometry Model in $\mathbb{G}_{3,0,1}$:** A 2D side-scroller and a 3D corridor game share the same algebra: 2D is an orthographic camera with pure translational `Motor` $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_x e_{01} + v_y e_{02})\Delta t)$, while 3D is a perspective camera with yaw + forward `Motor` $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_{\text{fwd}} e_{03} + \Omega e_{12})\Delta t)$. CFT optical flow divergence ($\nabla \cdot \mathbf{v}_{\text{flow}}$) automatically selects the regime.

---

## 1. Architectural Thesis

Grounded in `general_philosophy.md` with the core concept in `storybook_philosophy.md`, `cliffordcore` compiles gameplay video(s) and text rules into a deterministic, playable Clifford world model:

1. **The Implicate Holon ($\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$):** Every world component is compiled into:
   $$\mathcal{H}_i = \left(B_i \in \mathbb{G}_{3,0,1},\; \psi_i \in (\mathbb{G}_{2,0}^+)^K,\; \{\epsilon_{i,s}, \hat{K}_{i,s}(\mathbf{k}, \omega)\}\right)$$
   * $B_i$: 2D/3D PGA blade (tile boundary `Line`s, 3D wall/door `Plane`s, actor `Point`s, and `Motor` poses).
   * $\psi_i$: $K$-channel semantic spinor projected via SVD from `llama-embedding` wording spaces.
   * $\{\epsilon_{i,s}, \hat{K}_{i,s}(\mathbf{k}, \omega)\}$: Primitive state idempotents ($\epsilon_s^2 = \epsilon_s$) paired with unwarped 2D+t CFT sprite/texture kernels.
2. **Strict Division of Labor:**
   * **CFT Spectral Extractor (`cc_field`):** Isolates zero-parallax HUD bars, estimates camera `Motor` odometry, separates static world geometry from moving actors, and extracts canonical 2D+t CFT sprite loops.
   * **Semantic Transducer (`llama.cpp` VLM + Embeddings):** Runs only during `cc enfold` (with deterministic `.cc_cache/` JSON fixtures) to label entity types/states and embed `rules.md`.
   * **Active-GATr (`cc_gatr`):** Snaps geometry to tile/Manhattan lattices and compiles PGA incidence predicates (`StompTop`, `HitBlockBottom`, `UseDoor`, `LineOfSight`, `RayHit`) into state transition operators $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$.
3. **Playable Unfoldment (`cc unfold`):** Zero AI at runtime. Steps at $> 120\text{ Hz}$ headless and $60\text{ Hz}$ interactive (`--play`), rendering either the 2D side-scrolling tile+sprite world or the 3D PGA column-raycast world alongside live $\mathcal{Q}_{\text{Cl}}$ potential diagnostics.

---

## 2. Workspace Structure & Tech Stack

* **Language:** Rust (Edition 2024).
* **Algebra Core:** `clifford = "0.3"` (`projective::dim3`, `euclidean::{dim2, dim3}`, `dual`, `ops`).
* **Spectral Solver:** `rustfft` (1D column CFT, 2D spatial phase-correlation CFT, and 2D+t canonical sprite CFT).
* **Media IO:** `ffmpeg` CLI pipe (frame ingest & MP4 output) + `image`.
* **Local AI Transducer:** `llama.cpp` (`llama-server` / `llama-mtmd-cli` / `llama-embedding`) with SHA-256 content-addressed caching under `<input_dir>/.cc_cache/`.

```text
cliffordcore/
├── Cargo.toml
├── example_input2d/   # Super Mario Bros. 3 World 1-1 (256x240, 320x240, 720p + rules.md)
├── example_input3d/   # Wolfenstein 3D Episode 1 Floor 1 (320x240, 720p + rules.md)
├── crates/
│   ├── cc_math/       # G(2,0) spinors, idempotents, Q_Cl, G(3,0,1) PGA blades, Motors, meet/join
│   ├── cc_field/      # HUD separator, 2D CFT scroll + atlas stitcher, 3D column depth, 2D+t sprite CFT
│   ├── cc_gatr/       # Tile/Manhattan regularizer, wording-space spinor SVD, Active-GATr rule compiler
│   └── cc_cli/        # `cc enfold | inspect | unfold` CLI, llama.cpp client, 2D & 3D playable runtimes
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

#### Branch 2A: 2D Orthographic Pipeline (`example_input2d`)
1. **Sub-Pixel Scroll Odometry (`estimate_2d_scroll`):** Inverse FFT of the cross-power spectrum between consecutive viewports yields exact camera translation $(\Delta x_t, \Delta y_t)$ and cumulative world camera pose $(X_{\text{cam}}(t), Y_{\text{cam}}(t))$.
2. **Ghost-Free Temporal Median Atlas (`build_2d_world_atlas`):** Map every viewport frame into world coordinates $(X, Y) = (x + X_{\text{cam}}(t), y + Y_{\text{cam}}(t))$. Take the **per-pixel temporal median RGB** across all frames visiting $(X, Y)$. Transient moving actors (Mario, Goombas, Koopas, items) vanish automatically, producing a clean, static level panorama.
3. **Actor Residual Tubes (`extract_2d_actors`):** Threshold the difference $|I(x, y, t) - \text{Atlas}(x + X_{\text{cam}}(t), y + Y_{\text{cam}}(t))| > \tau_{\text{diff}}$, group connected components, and link actor trajectories $(X_i(t), Y_i(t), w_i, h_i)$ across frames via Hungarian bounding-box + CFT phase matching.
4. **Tile Lattice & Oriented Boundary Inference (`infer_2d_tile_physics`):**
   * Find the fundamental Bragg peak $(k_{\text{tile}, x}, k_{\text{tile}, y})$ of the atlas 2D CFT to lock the tile size ($16 \times 16$ in $256 \times 240$).
   * Classify each tile's oriented PGA boundary `Line`s ($L_{\text{top}}, L_{\text{bottom}}, L_{\text{left}}, L_{\text{right}}$) from observed actor trajectory discontinuities:
     * **Solid Tile (ground, pipe, brick, `?` block):** arrests downward velocity ($v_y > 0 \to 0$ on $L_{\text{top}}$), blocks upward head hits ($v_y < 0 \to 0$ on $L_{\text{bottom}}$), and reverses horizontal actor motion ($v_x \to -v_x$ on $L_{\text{left/right}}$).
     * **One-Way Semisolid Platform (pastel screw-boxes):** arrests downward landing on $L_{\text{top}}$ while allowing upward passage ($v_y < 0$) through its interior and bottom edge.

#### Branch 2B: 3D Perspective Pipeline (`example_input3d`)
1. **Horizon-Symmetric Column Depth (`extract_column_depth`):** Locate the viewport horizon $y_h = H_{\text{view}}/2$ separating flat ceiling (`#383838`) and floor (`#707070`). For each column $x$, measure the symmetric vertical wall span $h(x, t)$ around $y_h$ to obtain inverse projective depth $d(x, t) = f / h(x, t)$.
2. **Foreground Sprite Residual Masking:** Columns where color/span breaks the local piecewise-linear $d(x, t)$ trend and moves independently of camera yaw are masked as 3D billboard actors (guards, lamps, pickups), and wall depth $d(x, t)$ is linearly interpolated behind them.
3. **Log-Odds Manhattan Occupancy Map (`build_3d_raycast_map`):**
   * Estimate incremental camera yaw $\Delta \theta_t$ from horizontal shift of column spans and forward translation $\Delta s_t$ from radial expansion $\dot{d}(x, t) / d(x, t)$.
   * Cast each column ray $(P_{\text{cam}}(t), \theta_t + \alpha_x, z(x, t))$ into a 2D world grid quantized by the wall CFT period $\lambda_{\text{tile}}$. Accumulate log-odds free space along the ray and orthogonal wall/door `Plane` normals ($\pm e_1, \pm e_2$) at the termination cell.
4. **Inverse-Perspective Texture Rectification (`extract_wall_textures`):** Resample frontal wall and cyan door spans vertically by $h(x, t) \to 64$ and horizontally along the wall plane to $64$, recovering crisp $64 \times 64$ wall and door textures.

### Stage 3 — Canonical Sprite Unwarping & 2D+t CFT Vitality Kernels (`extract_canonical_sprite_cft`)
* For each tracked actor state segment (e.g., Mario run/jump, Goomba walk, Wolf3D guard patrol/shoot), resample the actor crop sequence into a canonical rest grid ($32 \times 32$ for 2D sprites; $64 \times 64$ for 3D billboards).
* Compute the 2D+t CFT $\hat{K}_{i,s}(k_x, k_y, \omega)$ and retain the top $M$ spectral modes (storing spatial silhouette/albedo in $\omega = 0$ and periodic limb/walk oscillation in $\omega_1 \dots \omega_M$).

### Stage 4 — `llama.cpp` VLM + Wording-Space Spinor Projection (`cc_gatr`)
* Run `llama-mtmd-cli` / `llama-server` with `--temp 0 --seed 42` and a strict GBNF grammar (`holon_schema.gbnf`) on a contact sheet of extracted tiles and canonical actor sprites to assign canonical labels (`mario`, `goomba`, `koopa`, `question_block`, `guard`, `sliding_door`, `hud_bar`) and state names.
* Run `llama-embedding` on VLM descriptions and `rules.md` clauses; compute the thin SVD of the normalized Gram matrix $S_{ij} = \langle \hat{e}_i, \hat{e}_j \rangle$ to map each Holon and rule into a $K$-channel spinor $\psi_i = R_i e^{\theta_i e_{12}} \in (\mathbb{G}_{2,0}^+)^K$.

### Stage 5 — Active-GATr State-Transition Binding (`compile_transitions`)
* Bind observed state transitions $\epsilon_s \to \epsilon_{s'}$ to PGA geometric incidence predicates and semantic spinor alignment $\mathcal{A}_{ij}$:
  * **2D Predicates:** `TopStomp` ($P_{\text{player\_foot}} \wedge L_{\text{enemy\_top}}$ with $v_y > 0$), `BottomBump` ($P_{\text{player\_head}} \wedge L_{\text{block\_bottom}}$ with $v_y < 0$), `SideHit` ($P_{\text{player}} \wedge L_{\text{enemy\_side}}$).
  * **3D Predicates:** `UseDoor` ($\langle P_{\text{player}} \vee \Pi_{\text{door}} \rangle < r_{\text{use}} \land \text{Use}$), `EnemyAlert` ($(P_{\text{player}} \vee P_{\text{enemy}}) \wedge \Pi_{\text{wall}} = \varnothing$), `WeaponRayHit` ($L_{\text{aim}} \wedge \Pi_{\text{enemy}} \neq \varnothing \land \text{Fire}$).

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
5. **Autonomous Actor Holons & Clifford Patrol / Stomp:** Modeled on Basil Hiley's process ontology. Entities inhabit minimal left ideals $\mathcal{I}_i = \mathcal{C}\ell \epsilon_i$ with state idempotents ($\epsilon_0$ left patrol, $\epsilon_1$ right patrol, $\epsilon_2$ squashed). Lateral wall incidence triggers idempotent reflection $T_{0 \leftrightarrow 1}$. Top-down relational incidence (downward player velocity onto actor top) collapses the actor to $\epsilon_2$ and imparts upward bounce restitution to the player. Demonstrated at $> 2500\text{ Hz}$ headless throughput. Recorded to `recordings/task_demo_smb3_playable.mp4`.


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

### Phase 4: `llama.cpp` Wording-Space & Active-GATr Compiler (`cc_gatr` + `cc_cli`)
* **Implementor Steps:**
  1. Create `crates/cc_gatr` with `llama.cpp` subprocess/HTTP client (`llama-mtmd-cli` / `llama-server` + `llama-embedding`), GBNF schema, and `.cc_cache/` fixture persistence.
  2. Implement Gram-matrix SVD spinor projection and Active-GATr coupling matrix $\mathcal{A}_{ij}$ across both `example_input2d` (`smb3_w1_1_320x240.mp4` + `rules.md`) and `example_input3d` (`wolf3d_e1m1_320x240.mp4` + `rules.md`).
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