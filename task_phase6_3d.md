# Milestone: Branch 2B — 3D Perspective Pipeline & Raycast Compiler (`cc_field` 3D)

**Goal:** Compile perspective 3D corridor gameplay (`example_input3d/wolf3d_e1m1_320x240.mp4` + `rules.md`) into a deterministic $\mathbb{G}_{3,0,1}$ world model (`wolf3d.cc`) using column projective depth, Manhattan occupancy accumulation, and canonical sprite/texture rectification.

## Plan & Implementation Tasks

- [ ] **1. Camera Regime & Viewport Bezel Partition (`cc_field::enfold3d`)**
  - [ ] Strip outer teal bezel/pillarboxes (`#004040`) and partition bottom status HUD ($y \ge 192$ in $320 \times 240$).
  - [ ] Detect centered foreground `WeaponOverlay` (zero translational parallax during camera yaw).
  - [ ] Compute optical flow divergence $\nabla \cdot \mathbf{v}_{\text{flow}}$ to verify `CameraMode::Perspective3D`.

- [ ] **2. Horizon Column Depth & Projective Plane Extraction**
  - [ ] Locate viewport horizon $y_h = H_{\text{view}}/2$ separating ceiling (`#383838`) and floor (`#707070`).
  - [ ] Measure per-column symmetric vertical wall span $h(x, t)$ around $y_h$ to obtain inverse projective depth $d(x, t) = f / h(x, t)$.
  - [ ] Mask foreground billboard actors (guards, lamps, pickups) by detecting discontinuities against the piecewise-linear depth trend.

- [ ] **3. Manhattan Occupancy Grid & Wall/Door Blades in $\mathbb{G}_{3,0,1}$**
  - [ ] Recover ego yaw $\Delta\theta_t$ and translation $\Delta s_t$ via 3D projective ego motor $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_{\text{fwd}} e_{03} + \Omega e_{12})\Delta t)$.
  - [ ] Cast column rays $(P_{\text{cam}}(t), \theta_t + \alpha_x, z(x, t))$ into 2D world grid quantized by wall CFT period $\lambda_{\text{tile}} = 64\text{ px}$.
  - [ ] Reconstruct orthogonal 3D PGA `Plane`s ($\Pi_k$) and sliding door axes with $< 2\%$ depth error.

- [ ] **4. Inverse-Perspective Texture & Sprite Rectification**
  - [ ] Rectify blue stone walls and cyan sliding doors to canonical $64 \times 64$ textures.
  - [ ] Extract canonical $64 \times 64$ guard billboard sprites and vitality walk/shoot states.

- [ ] **5. Exit Gate Verification (`crates/cc_field/tests/wolf3d_enfold.rs`)**
  - [ ] HUD + Weapon mask IoU $> 0.98$.
  - [ ] E1M1 starting room, door, and corridor geometry reconstructed within $< 2\%$ depth error.
  - [ ] Model roundtrip in `example_input3d/wolf3d.cc`.
  - [ ] Playable 3D raycast unfold demonstration in `recordings/task_demo_wolf3d_playable.mp4`.
