# HolonEngine: The Implicate World Compiler

### Architecture & Systems Whitepaper: Process Ontology, Clifford Density Elements, and Active Information

<small>by [Jonathan Dunlap](https://www.linkedin.com/in/jonathandunlap/)</small>

---

## 1. Foundational Thesis: From Atomism to the Implicate Order

Interactive world engines have historically split into two flawed paradigms:
1. **Atomistic State Machines (Traditional Engines):** Sealed structs sit inside an inert coordinate void, governed by hand-coded boolean flags and game-specific branching (`if is_pacman { eat_dot() } else if is_mario { stomp_goomba() }`).
2. **Frame-by-Frame Neural Hallucination (Autoregressive Video Models):** Pixel buffers are predicted statistically per frame without persistent geometry, conservation laws, or object identity.

A single static image cannot define space or causality because it is a frozen *explicate* slice ($t = t_0$) with zero temporal derivative ($\partial_t \Psi = 0$). Gameplay video, by contrast, encodes **non-commutative process transitions** $M_{A \to B} = \Psi_B \Psi_A^{-1}$ across time: camera parallax, occlusion, invariant HUD overlays, boundary incidence, and actor harmonics.

`HolonEngine` implements a third architecture grounded in **David Bohm and Basil Hiley's Implicate Order** (*The Undivided Universe*, 1993; Hiley & Callaghan, "The Clifford Algebra Approach to Quantum Mechanics", 2010) and an **Algebraic Tower of Clifford Algebras** ($\mathcal{C}\ell_{0,1} \to \mathcal{C}\ell_{2,0} \to \mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$), where higher-order algebras naturally enfold lower ones without destroying them:
* **Enfoldment (`cc enfold`):** Compiles raw gameplay video into an invariant **Implicate World Model (`.cc`)**—extracting camera `Motor` odometry, ghost-free world panoramas, oriented PGA boundary blades, chromatic/semantic spinor signatures, and canonical actor processes without game-specific rules.
* **Unfoldment (`cc unfold`):** Runs the compiled `.cc` model forward at thousands of hertz as a unified, zero-AI Clifford dynamical system where every entity is an algebraic process coupled to the surrounding spatial medium.

---

## 2. Core Algebraic Abstractions (`cc_math`)

### 2.1 Minimal Left Ideals, Pregeometry, & The Clifford Density Element (`CliffordDensityElement`)
In classical geometry, space is a pre-existing container populated by zero-dimensional points. In Bohm and Hiley's algebraic *pregeometry* (Hiley & Callaghan, 2010, §2.1–2.5), there are no fundamental points in an external void; instead, **primitive idempotents** ($\epsilon^2 = \epsilon$) act as "generalized points"—self-sustaining processes that reproduce themselves under multiplication while generating spatial incidence through non-commutative relations.

`HolonEngine` dispenses with external state vectors and ad-hoc entity flags. Every compiled world component (player, autonomous actor, interactive boundary, collectible token) is an **Algebraic Holon** represented as the unified tuple:

$$\mathcal{H}_i = \left(\underbrace{B_i \in \mathbb{G}_{3,0,1}}_{\text{PGA Boundary Blade + Motor}},\; \underbrace{\Phi_{L,i} \in \mathcal{C}\ell\,\epsilon_{i,s}}_{\text{Minimal Left Ideal State}},\; \underbrace{\psi_i = R_i e^{\theta_i e_{12}} \in (\mathbb{G}_{2,0}^+)^K}_{\text{Intrinsic Spinor Signature}},\; \underbrace{\hat{K}_i(\mathbf{k}, \omega)}_{\text{2D+t CFT Vitality Kernel}}\right)$$

Each holon's active process state inhabits a minimal left ideal $\mathcal{I}_i = \mathcal{C}\ell\,\epsilon_i$:

$$\Phi_{L,i}(r, t) = \phi_{L,i}(r, t)\,\epsilon_{i,s} = R_i(r, t)\,U_i(r, t)\,\epsilon_{i,s}$$

where:
* $R_i(r, t) \ge 0$ is the real amplitude envelope,
* $U_i(r, t) = \exp\left(-\frac{1}{2}\theta_i e_{12}\right) \in \text{Spin}(2,0)$ is the even spinor rotor encoding planar heading and phase,
* $\epsilon_{i,s}$ is a primitive state idempotent ($\epsilon_{i,s}^2 = \epsilon_{i,s}$) selecting the active behavioral branch.

The complete state of the holon is encoded within the algebra by its **Clifford Density Element (CDE)**:

$$\rho_{c,i}(r, t) = \Phi_{L,i}(r, t)\,\widetilde{\Phi}_{L,i}(r, t) = \rho_i\, U_i\,\epsilon_{i,s}\,\widetilde{U}_i, \qquad \rho_i = R_i^2$$

where $\widetilde{\Phi}_L$ denotes the Clifford conjugate.

### 2.2 Bilinear Invariants of the First and Second Kind
All observable kinematics and interactions of a `CliffordDensityElement` are computed as algebraic bilinear invariants:

1. **Bilinear Invariants of the First Kind (Kinematic Posture & Identity):**
   * **Process Density:** $\rho_i = \text{tr}(\rho_{c,i}) = R_i^2\,\text{tr}(\epsilon_{i,s})$. Collapsing a holon to a null/consumed state sets $\rho_i \to 0$.
   * **Directed Spin / Heading Vector (`heading_vector`):**
     $$\mathbf{s}_i = U_i\,e_1\,\widetilde{U}_i = (\cos\theta_i,\;\sin\theta_i)$$
   * **Intrinsic Spinor Coherence (`signature_coherence`):**
     Each holon carries a multi-channel chromatic/semantic spinor $\psi_i \in (\mathbb{G}_{2,0}^+)^K$. The inner product $\langle \psi_i, \widetilde{\psi}_j \rangle_0 \in [0, 1]$ measures identity coherence across frames, occlusions, and warp boundaries.

2. **Bilinear Invariants of the Second Kind (Bohm Momentum & Quantum Torque):**
   * **Bohm Momentum (`bohm_momentum`):**
     Derived from the antisymmetric Dirac derivative $\Phi_L \overleftrightarrow{\partial} \widetilde{\Phi}_L = (\partial \Phi_L)\widetilde{\Phi}_L - \Phi_L(\partial \widetilde{\Phi}_L)$ (Hiley eq. 2.7, 3.3, 4.15):
     $$\mathbf{P}_{B,i} = -\Omega_i \cdot S_i = \rho_i\,v_{0,i}\,\mathbf{s}_i$$
   * **Quantum Torque Steering (`apply_quantum_torque`):**
     The spatial gradient of the Clifford Quantum Potential $-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$ exerts a bivector torque on the holon's spinor rotor (Hiley eq. 5.18, 5.27):
     $$\Omega_{Q,i} = \mathbf{s}_i \wedge \left(-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0\right), \qquad U_i(t + \Delta t) = U_i(t)\,\exp\left(-\frac{1}{2}\gamma\,\Omega_{Q,i}\,\Delta t\right)$$
     This single equation steers autonomous holons smoothly along open corridor valleys and around obstacles without game-specific pathfinding.

### 2.3 Primitive State Idempotents & Transition Operators (`StateBasis`, `TransitionOperator`)
Discrete behavioral regimes are represented by mutually orthogonal primitive idempotents (`crates/cc_math/src/idempotent.rs`):

$$\epsilon_s^2 = \epsilon_s, \qquad \epsilon_s \epsilon_{s'} = 0 \;\;(s \neq s'), \qquad \sum_{s=0}^{N-1} \epsilon_s = I$$

When a geometric incidence predicate fires in $\mathbb{G}_{3,0,1}$, the holon transitions via the sandwich operator:

$$T_{s \to s'} = \epsilon_{s'}\,M\,\epsilon_s$$

Simultaneously, boundary incidence against normal $\hat{\mathbf{n}}$ reflects the heading vector $\mathbf{s}_i \mapsto -\hat{\mathbf{n}}\,\mathbf{s}_i\,\hat{\mathbf{n}} = \mathbf{s}_i - 2\langle \mathbf{s}_i, \hat{\mathbf{n}} \rangle \hat{\mathbf{n}}$ (`reflect_at_boundary`), updating the rotor $U_i \in \text{Spin}(2,0)$ to the reflected heading while preserving unit normalization:

| Incidence Event | Geometric Predicate ($\mathbb{G}_{3,0,1}$) | Algebraic Operator ($T_{s \to s'}$) | Emergent Behavior Across Genres |
| :--- | :--- | :--- | :--- |
| **Boundary Reflection** | $P_i \wedge L_{\text{wall}} = 0,\; \langle \mathbf{s}_i, \hat{\mathbf{n}} \rangle < 0$ | $\mathbf{s}_i \mapsto -\hat{\mathbf{n}}\mathbf{s}_i\hat{\mathbf{n}},\; T_{0 \leftrightarrow 1}$ | Platformer enemy reverses patrol at pipe/ledge; maze actor turns at corridor wall. |
| **Top-Down Incidence** | $P_{\text{actor}} \wedge L_{\text{target,top}} = 0,\; \mathbf{P}_B \cdot e_2 > 0$ | Target $T_{s \to 2}$ (collapse), Actor $P_{B,y} \mapsto -\eta P_{B,y}$ | Stomping an enemy collapses it to passive state $\epsilon_2$ and imparts upward restitution. |
| **Upward Boundary Impulse** | $P_{\text{actor}} \wedge L_{\text{prop,bottom}} = 0,\; \mathbf{P}_B \cdot e_2 < 0$ | Prop $T_{0 \to 1 \to 2}$, emits excitation token | Striking a floating block from below triggers harmonic displacement and token emission. |
| **Field Excitation Absorption** | $P_{\text{actor}} \cap B_{\text{excitation}} \neq \varnothing$ | Excitation $T_{0 \to 2}$ ($\rho \to 0$) | Passing through a corridor dot, pellet, or coin absorbs the excitation into actor energy. |

### 2.4 Abstract Attached Process Effects & Medium Excitations
Rather than hardcoding ad-hoc visual particle routines or game-specific cosmetic emitters (e.g., jet nozzles, foot dust, or muzzle flashes), `HolonEngine` models all secondary visual phenomena as **Abstract Attached Process Excitations** coupled directly to a holon's boundary blades $B_i$, heading rotor $U_i$, and Bohm momentum $\mathbf{P}_{B,i}$:

1. **Attached Trailing Wakes (`AttachedWake`):**
   Emanates from the holon's trailing boundary blade $\mathbf{p}_{\text{tail}} = \mathbf{p}_{\text{center}} - \frac{1}{2}h\,\mathbf{s}_i$ opposite to the heading vector $-\mathbf{s}_i$. Characterized by process lifetime $\tau$, velocity $\mathbf{v}_{\text{wake}} \propto -\mathbf{s}_i$, and dispersion radius $r(t)$. Generalizes across aero/fluid thrusters, vessel wakes, running dust, and particle trails.
2. **Soliton Directional Projectiles (`SolitonProjectile`):**
   Emitted along the leading boundary blade $\mathbf{p}_{\text{lead}} = \mathbf{p}_{\text{center}} + \frac{1}{2}h\,\mathbf{s}_i$, propagating as a coherent solitary excitation with velocity $\mathbf{v} = v_0\,\mathbf{s}_i$.
3. **Radial Field Dispersions (`RadialDispersion`):**
   Triggered on state idempotent collapse $T_{s \to \text{collapsed}}$, dissipating stored internal process energy radially into the medium with harmonic expansion $r(t) = r_0 + \alpha\,t$.
4. **Consumable Field Excitations (`FieldExcitation`):**
   Discrete localized minimal left ideals embedded in the spatial medium ($B_{\text{excitation}} \in \mathbb{G}_{3,0,1}$), which annihilate ($\rho \to 0$) upon projective incidence with a primary holon.

---

## 3. Active Information & The Clifford Quantum Potential (`cc_field::potential`)

During `cc unfold`, the spatial lattice carries a real-time amplitude field $\Psi(x, y, t) = R(x, y, t)\,\mathcal{U}(x, y, t)$ governed by the **Clifford Quantum Potential**:

$$\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1} = \underbrace{-\frac{\nabla^2 R}{R}}_{\text{Scalar Guidance } \langle \mathcal{Q}_{\text{Cl}} \rangle_0} - \underbrace{\left(2(\nabla \ln R)\mathcal{U}\nabla\widetilde{\mathcal{U}} + (\nabla^2 \mathcal{U})\widetilde{\mathcal{U}}\right)}_{\text{Bivector Spin-Torque } \langle \mathcal{Q}_{\text{Cl}} \rangle_2}$$

* **Scale-Invariant Valley Guidance:** Because $\langle \mathcal{Q}_{\text{Cl}} \rangle_0 = -\nabla^2 R / R$ normalizes spatial curvature by amplitude, guidance depends solely on the *geometric form* of the environment rather than raw field magnitude.
* **Bivector Spin-Torque Steering:** Autonomous holons sample the local potential gradient $-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$, experiencing an active steering torque:
  $$\Omega_{Q,i} = \mathbf{s}_i \wedge \left(-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0\right)$$
  which steers heading rotors $U_i$ through open corridors and around barriers without graph-based pathfinding (e.g. A* or navmeshes).
* **Continuous Boundary Impedance & Attractor Basins (`Model2D::build_continuous_qfield`):** Impermeable boundary contours clamp $R \to 0.08$, forming steep positive $\mathcal{Q}_{\text{Cl}}$ repeller ridges along walls and obstacles; semi-permeable contours set intermediate impedance ($R \to 0.60$); open corridors and active `FieldExcitation`s inject smooth amplitude domes ($R \to 1.25$), carving negative-$\mathcal{Q}_{\text{Cl}}$ guidance valleys without requiring a discrete tile grid.

---

## 4. Game-Agnostic Spatiotemporal Compiler & Unified Runtime (`cc_field` + `cc_cli`)

`cc enfold` compiles raw gameplay video into a self-contained `.cc` world model without per-game rules files, palette constants, or genre branches. Every architectural stage exists to replace a traditional hardcoded game assumption with an inferred algebraic or spectral invariant:

### 4.1 Scene Stability Segmentation & Spectral Camera Regime Discovery
* **Why It Matters:** Gameplay recordings contain title screens, menus, iris transitions, and static HUD bars that corrupt odometry if mixed with active playfield motion. Moreover, the compiler cannot pre-assume whether a game is a static arena, a horizontal side-scroller, or a vertical corridor shooter.
* **How It Is Used:**
  1. `detect_playfield_segment` locates the longest contiguous active scene sharing a stable border backdrop mode and applies a settle margin (`0.6s`) after scene cuts so transition frames never shift world origin coordinates.
  2. `detect_camera_is_static` and `ScrollEstimator` evaluate 2D FFT cross-correlation across the playfield band (with normalized phase correlation $C/|C|$ available in `push_both`), refined to single-pixel registration via local SSAD (`refine_offset`). Given consecutive frames $I_t$ and $I_{t+1}$, the frequency-domain cross-spectrum yields the displacement:
     $$C(u, v) = \mathcal{F}[I_{t+1}](u, v) \cdot \mathcal{F}^*[I_t](u, v), \qquad \Delta \mathbf{X}_{\text{cam}} = \operatorname{argmax}_{(x, y)} \mathcal{F}^{-1}\left\{ C(u, v) \right\}$$
     Near-zero drift ($\max |\Delta \mathbf{X}_{\text{cam}}| \le 4\text{ px}$) selects `Ortho2D-Static`; translational flow selects `Ortho2D-Scroll` governed by the PGA camera ego motor:
     $$M_{\text{ego}}(t) = \exp\left(-\frac{1}{2}(v_x e_{01} + v_y e_{02})\Delta t\right)$$
  3. Static HUD overlays are partitioned by detecting regions of zero camera-relative variance, and ghost-free world panoramas are synthesized via quantized temporal mode accumulation over the registered camera trajectory:
     $$A(\mathbf{x}) = \operatorname{mode}_{t} \left\{ I\left(\mathbf{x} - \mathbf{X}_{\text{cam}}(t),\, t\right) \right\}$$
     which filters out transient foreground actors without procedural painting or inpainting artifacts.

### 4.2 Topological Boundary & Collectible Separation (`extract_2d_tiles_with`)
* **Why It Matters:** Hardcoding RGB palettes for walls, pipes, or collectible dots overfits the compiler to a single game.
* **How It Is Used:**
  * The backdrop color is learned from the playfield border mode.
  * In worlds with a continuous floor (`has_ground`), per-column dense runs identify the primary ground surface, raised terrain steps (bridging across foreground occluders such as bushes and hills up to the atlas boundary), and flat-topped semi-permeable shelves while relegating curved contours (clouds, hills, bushes) to non-collidable background scenery.
  * In floorless planar worlds, 8-connected components of non-background pixels separate extended structural walls ($\max(w, h) \ge 2 \times \text{tile\_size}$) from isolated corridor **Collectible Field Excitations** (`ModelManifest::collectibles`).

### 4.3 Surface-Clipped Residuals & Spinor Occlusion Factorization (`track_actors_coherent`)
* **Why It Matters:** Two failure modes plague naive bounding-box tracking from video:
  1. *Subterranean Stripe Bleed:* High-contrast diagonal ground textures under $\pm 1\text{ px}$ scroll jitter generate spurious residual boxes inside the floor or pull the bottom edge of walking actors 3–6 px below their supporting surface.
  2. *Multi-Actor Crossing Occlusions:* When two actors cross paths, connected-component segmentation merges their silhouettes into a single compound box ($\text{area} > \frac{5}{4}\text{area}_0$), causing greedy assignment to drop tracks or inflate hitboxes.
* **How It Is Used:**
  * **Surface Clipping:** Prior to Pass 2 tracking, `enfold` extracts the preliminary solid surface profile `pre_surface[tx]` from Pass 1. Residual boxes extending into `pre_surface[tx]` are clipped at `surf_frame_y - 1`, and boxes originating inside the terrain (`b.y0 >= surf_frame_y - 6`) are rejected.
  * **Spinor Factorization:** Before greedy 1-to-1 frame assignment, any unmatched compound residual box spanning two established open tracks is factored into per-track sub-boxes anchored at each track's predicted center and canonical dimensions.
  * **Excursion-Based Role Discovery:** Tracks are classified without game-specific size buckets (`Goomba` and `Koopa` enum variants have been removed in favor of `ActorKind::Player` and `ActorKind::Autonomous`). Camera-coupled trajectories and 2D ballistic jumpers ($\Delta X \ge 60\text{ px},\; \Delta Y \ge 48\text{ px}$) are unified as state branches of the primary `Player` holon.

### 4.4 Trajectory-Regressed Field Drift & Control Motor Basis (`infer_world_kinematics`)
* **Why It Matters:** Hardcoding `gravity = 0.48`, `RUN = 1.6`, or `JUMP = 6.2` bakes platformer micro-mechanics into the runtime. Conversely, regressing acceleration from single-frame ($\Delta t = 1$) finite differences on integer-quantized bounding boxes suffers from severe integer truncation bias.
* **How It Is Used:**
  * `infer_world_kinematics` (`crates/cc_field/src/model2d.rs`) evaluates second-order finite differences over a multi-frame stride ($\Delta t = 4,\; \Delta t^2 = 16$):
    $$\bar{a}_y(t) = \frac{y(t + 4) - 2y(t) + y(t - 4)}{16}$$
    across uncollided free-flight trajectory segments to populate `ModelManifest::env_drift: [f32; 2]` ($\mathbf{a}_{\text{env}}$).
  * From the primary holon's trajectory, steady ground-contact horizontal speed gives `drive_speed` ($v_0$), while the 75th-percentile ballistic apex height $\Delta h$ above support yields the vertical launch impulse via energy conservation:
    $$J_\perp = \sqrt{2\,a_y\,\Delta h}$$
    stored together as `ModelManifest::control_motor: [f32; 2]`.

### 4.5 Stationary `BoundaryBlade` Admittance & Pre-Incidence Causal Restoration
* **Why It Matters:** Traditional engines hardcode entity classes like `BlockProp` or draw procedural pixel art at runtime (such as a hardcoded `?` block glyph). In video compilation, a second subtle trap arises: an interactive block struck early in a recording (e.g., at $t = 1.5\text{ s}$ of a $21\text{ s}$ clip) spends $> 90\%$ of the video in its post-impact *spent* state ($\epsilon_2$), causing a full-stream temporal mode to bake the spent appearance into the initial world atlas.
* **How It Is Used:**
  * `Model2D::infer_boundary_blades` compiles static world geometry into `BoundaryBlade`s (`ModelManifest::boundaries`), each carrying continuous world bounds `rect: [i32; 4]`, directional permeability `permeability: [f32; 4]` along `[left, right, top, bottom]`, `excitable: bool`, and `frame_color: [u8; 3]`.
  * **Pre-Incidence Causal Restoration (`init_acc`):** During `enfold`, an unmasked pre-incidence accumulator over the initial frames ($t < t_{\text{hit}}$) restores the genuine video State 0 ($\epsilon_0$) texture onto suspended boundaries in `atlas`, while `learn_rect_frame_color` extracts the post-incidence perimeter color for State 2 ($\epsilon_2$).
  * At runtime, `WorldState2D::render` renders State 0 directly from the restored `atlas` pixels, State 1 by displacing the `atlas` patch along a parabolic impulse arc, and State 2 using the learned `frame_color`—requiring zero procedural glyph code.

### 4.6 2D+t CFT Sprite Vitality Kernels ($\hat{K}_0, \hat{K}_1$) & Fourier Shift Synthesis (`evaluate_sprite_cft`)
* **Why It Matters:** Storing only a single static median PNG per actor leaves characters visually frozen during unfoldment unless the runtime resorts to game-specific procedural pixel hacks (mouth wedges, skirt shifts, or eye RGB checks).
* **How It Is Used:**
  * **Neighborhood-Filtered Extraction:** In Pass 3 of `enfold`, foreground actor pixels are tested against a $7 \times 5$ neighborhood of `plain_atlas` (`dx in -3..=3, dy in -2..=2`) so $\pm 1\text{--}3\text{ px}$ registration shifts over striped background hills or bushes never bleed background scenery into actor sprites.
  * **Quadrature Harmonic Storage:** Pass 3 accumulates both the DC canonical silhouette $\hat{K}_0(x, y)$ (`TrackRecord::sprite`) and the first temporal CFT harmonic $\hat{K}_1(x, y)$ (`TrackRecord::harmonic_sprite`, encoded losslessly as `128 + delta` RGBA PNG).
  * **Runtime Spectral + Fourier Shift Evaluation:** `Model2D::evaluate_sprite_cft` synthesizes each active holon's appearance from its arc-length phase $\phi(t) = \int \|\mathbf{P}_{B,i}\|\,dt$ and spinor heading $\mathbf{s}_i$:
    $$I(x, y, \phi) = \mathcal{T}_\phi\!\left[\hat{K}_0(x, y) + \operatorname{Re}\!\left\{\hat{K}_1(x, y)\,e^{i\phi}\right\}\right]$$
    By the Fourier Shift Theorem, the spatial phase-shift operator $\mathcal{T}_\phi$ decomposes the canonical rest frame vertically into three coupled bands:
    1. *Lower Band ($y \in [\frac{2}{3}H, H)$):* Antisymmetric bipedal left/right foot lift ($\Delta y_{\text{footL/R}} \propto \pm\sin\phi$) and horizontal leg stride shear ($\Delta x_{\text{stride}} \propto \sin\phi$).
    2. *Middle Band ($y \in [\frac{1}{3}H, \frac{2}{3}H)$):* Counter-phase arm/torso swing ($\Delta x_{\text{arm}} \propto -\sin\phi$) and lateral wing/limb vertical oscillation ($\Delta y_{\text{wing}} \propto \cos\phi$).
    3. *Upper Band ($y \in [0, \frac{1}{3}H)$):* Double-frequency stride bob ($\Delta y_{\text{bob}} \propto \sin 2\phi$), followed by $\text{Pin}(2,0)$ horizontal reflection $e_1 x e_1$ aligned with heading rotor $U_i$.

### 4.7 Unified Implicate Runtime (`WorldState2D::step` & `WorldState2D::render`)
* **Why It Matters:** Branching runtime execution into separate genre loops (`if is_static`, `if is_flight`, `else platformer`) reintroduces the atomistic state-machine paradigm at the engine level.
* **How It Is Used:**
  * `crates/cc_cli/src/main.rs` executes a single `WorldState2D` engine across all 2D models (`smb3.cc`, `pacman.cc`, `raptor.cc`):
    1. Steps `BoundaryHolon` impulse timers ($T_{1 \to 2}$) and secondary medium excitations (`AttachedWake`, `SolitonProjectile`, `FieldExcitation`, `RadialDispersion`).
    2. Steps all `MobileHolon`s under `env_drift`, `control_motor`, `BoundaryBlade` directional admittance (resolving vertical surface support prior to lateral wall reflection so actors traverse raised terrain steps without snagging), and $\mathcal{Q}_{\text{Cl}}$ quantum torque (`apply_quantum_torque`).
    3. Resolves relational incidences ($P_{\text{primary}} \wedge L_{\text{actor,top}}$ top-down restitution, soliton-holon collapse, and `FieldExcitation` absorption).
    4. Renders the camera-transformed atlas, active/collapsed `BoundaryHolon`s, and CFT-synthesized `MobileHolon` sprites in a single pass at $900\text{--}2,800\text{ Hz}$.

---

## 5. Architectural Assessment: Tile-Free Continuous Implicate World Representation

### 5.1 Why a 2D World Model Must Not Assume a Tile Grid
Many 2D games—vector space arenas (*Asteroids*, *Lunar Lander*, *Geometry Wars*), hand-painted continuous worlds (*Braid*, *Rayman*, *Limbo*), physics sandboxes (*Worms*), and freeform arcade tables (*Pinball*, *Pong*)—have **no discrete tile grid**. Even in games that use tiles (*Super Mario Bros. 3*, *Pac-Man*), treating `kinds: Vec<TileKind>` as the primary world ontology introduces artificial quantization, fixed cell-size assumptions, and axis-aligned bias.

In accordance with `docs/holon_philosophy.md` (§II–§III), the fundamental representation of a world in `HolonEngine` is **continuous**:

1. **Continuous PGA Boundary Blades ($B_k \in \mathbb{G}_{3,0,1}$):**
   Static and semi-permeable world structures are stored in continuous world coordinates (`ModelManifest::solids`, `ModelManifest::platforms`, `ModelManifest::collectibles`, and oriented PGA lines `OrientedLine2`):
   $$L_k = n_{x,k} e_1 + n_{y,k} e_2 + d_k e_0 \in \mathbb{G}_{3,0,1}, \qquad \mathbf{x} \in [\mathbf{p}_{0,k}, \mathbf{p}_{1,k}]$$
   * `Model2D::solid_at_world(wx, wy)` and `Model2D::one_way_at_world(wx, wy)` evaluate incidence directly in continuous world space $(X, Y) \in \mathbb{R}^2$.
   * `Model2D::is_open_below_rect(r)` inspects the continuous world atlas beneath a boundary segment to distinguish suspended interactive props from grounded terrain without referencing tile rows or columns.
   * `segment_hit_oriented_line` (`crates/cc_math/src/pga.rs`) resolves arbitrary-angle boundary incidence ($\langle \mathbf{P}_B, \hat{\mathbf{n}}_k \rangle < 0$) for non-axis-aligned slopes and polygonal contours.

2. **Continuous Field Impedance & $\mathcal{Q}_{\text{Cl}}$ Construction (`Model2D::build_continuous_qfield`):**
   The Clifford Quantum Potential $\mathcal{Q}_{\text{Cl}} = -(\nabla^2 R)/R$ is constructed directly from continuous world-space geometry (`solids`, `platforms`, `collectibles`) over $[0, W_{\text{world}}] \times [0, H_{\text{world}}]$—requiring **zero tile metadata** (`tile = 0, cols = 0, rows = 0, kinds = []` is a valid, fully functional `.cc` model, verified by `model2d::tests::tile_free_continuous_world_queries_and_qfield`).

3. **Bragg Lattice Snapping as an Optional Spectral Regularizer:**
   When the 2D CFT of the temporal-mode world atlas $\hat{A}(k_x, k_y)$ exhibits sharp Bragg peaks at $(k_{\text{tile},x}, k_{\text{tile},y}) = (2\pi/\lambda_x, 2\pi/\lambda_y)$ with peak-to-median spectral energy exceeding threshold $\tau_{\text{Bragg}}$, a Manhattan/lattice spectral regularizer snaps noisy pixel contours onto the discovered period $(\lambda_x, \lambda_y)$. When no Bragg peak is present (continuous/non-tile 2D games), $\tau_{\text{Bragg}}$ is not met and boundaries remain unquantized continuous PGA contours in $\mathbb{G}_{3,0,1}$.

### 5.2 Empirical Systems Laws Discovered from Benchmark Unfoldment

1. **The Quantization Phantom Trap (1-Pixel Outline Bleed):**
   * **Phenomenon:** Sprite and block artwork frequently includes a 1-pixel dark outline outside the nominal geometry (e.g., `x = 175..208` around a `176..207` block). Naive cell-overlap predicates (`tx1 >= c.x0 && tx0 <= c.x1`) promote the entire adjacent empty-space tile (`16 px` wide) into a solid/one-way surface, creating invisible phantom ledges in open air that premature jumps collide with.
   * **Law:** Geometric blades $B_k \in \mathbb{G}_{3,0,1}$ must never be dilated by grid quantization. Optional cell classification requires fractional overlap $\mu(C \cap \text{cell}) \ge 4\text{ px}$, while runtime incidence operates on continuous contour bounds.

2. **Visual-Physical Support Coincidence (No Detached Hitboxes):**
   * **Phenomenon:** Using a fixed physics hitbox (`12 x 16`) with a learned sprite canvas (`18 x 28`) violates projective incidence ($P_{\text{head}} \wedge L_{\text{bottom}} = 0$): the physics solver halts at one coordinate while the rendered silhouette either penetrates 12 px inside an overhead block or appears to trigger an impact in empty space.
   * **Law:** Every holon's PGA bounding blade $(w_i, h_i)$ must be derived directly from its learned canonical silhouette (`TrackRecord::size`), guaranteeing that physical boundary incidence and visual contact coincide to the pixel.

3. **Anisotropic Permeability of Suspended Boundaries:**
   * **Phenomenon:** A tall holon walking beneath a low-hanging suspended block (`is_open_below_rect == true`) will be blocked horizontally if its upper-body leading edge treats all boundaries as 4-sided solids.
   * **Law:** Suspended boundaries with open backdrop underneath are directionally semi-permeable: they arrest downward motion on $L_{\text{top}}$ ($v_y > 0$) and trigger upward impulse transitions $T_{0 \to 1 \to 2}$ on $L_{\text{bottom}}$ ($v_y < 0$), while horizontal wall impedance is sampled along the lower-body support $[y_{\text{feet}} - h_{\text{step}},\, y_{\text{feet}}]$.

4. **Multi-Body Overlap Composites vs. Fundamental Minimal Left Ideals:**
   * **Phenomenon:** When two actors cross paths in a corridor, connected-component residual segmentation yields a transient bounding box of area $\text{area}_{i+j} > \frac{3}{2}\text{area}_0$ and blended color spinor $\frac{1}{2}(\psi_i + \psi_j)$.
   * **Law:** Fundamental holons correspond to persistent minimal left ideals. Compound boxes spanning two established open tracks are factored into per-track sub-boxes prior to greedy assignment (`track_actors_coherent`), preserving trajectory continuity across multi-actor crossings.

5. **Pre-Incidence vs. Post-Incidence Causal Boundary States:**
   * **Phenomenon:** An interactive boundary ($\mathcal{B}_k$) struck early in a gameplay recording spends most of the video in its post-impact spent state ($\epsilon_2$), causing a naive full-stream temporal mode to bake the spent appearance into the initial world atlas.
   * **Law:** Stationary excitable boundaries (`BoundaryBlade`) separate causal epochs: the unmasked pre-incidence window ($t < t_{\text{hit}}$) populates the active State 0 ($\epsilon_0$) atlas texture, while the perimeter mode captures the collapsed State 2 ($\epsilon_2$) structural color, eliminating runtime pixel forgery.

6. **Subterranean Ground-Stripe Exclusion & Support-First Incidence Order:**
   * **Phenomenon:** Under $\pm 1\text{ px}$ camera registration jitter, diagonal high-contrast ground textures generate spurious residual blobs below the surface line (`y >= surface[tx]`) and extend walking actor bounding boxes 3–6 px into the floor. Furthermore, probing lateral wall collisions before updating vertical surface support causes actors stepping onto raised terrain ledges to collide with the ledge underneath their feet.
   * **Law:** Actor residual extraction must clip bounding boxes at the discovered solid terrain contour `pre_surface[tx] - 1` and reject subterranean residuals, while runtime kinematics (`WorldState2D::step`) must resolve vertical surface support coupling prior to lateral boundary reflection.

7. **Multi-Frame Stride Regression vs. Integer Quantization Bias:**
   * **Phenomenon:** Evaluating second-order acceleration differences $\Delta^2 y_t = y(t+1) - 2y(t) + y(t-1)$ at single-frame steps ($\Delta t = 1$) on integer pixel bounding boxes quantizes accelerations to integers $\{0, \pm 1, \pm 2\}$, biasing positive-filtered medians upward by $> 40\%$.
   * **Law:** Field acceleration ($\mathbf{a}_{\text{env}}$) and cruise velocities must be regressed over a multi-frame stride ($\Delta t \ge 4,\; \Delta t^2 = 16$), where sub-pixel accelerations accumulate into multi-pixel displacements that dominate integer rounding noise.

---

## 6. Bonus: Future Work — Extending Implicate Compilation to 3D Worlds

In $\mathbb{G}_{3,0,1}$, 2D orthographic worlds and 3D perspective corridor worlds share the same projective geometric algebra, distinguished only by the camera `Motor` subgroup detected from CFT optical-flow divergence ($\nabla \cdot \mathbf{v}_{\text{flow}}$):
* **Orthographic 2D (`Ortho2D`, $\nabla \cdot \mathbf{v}_{\text{flow}} \approx 0$):** Pure translational camera motor (`pga::integrate_2d_camera`):
  $$M_{\text{ego}}(t) = \exp\left(-\frac{1}{2}(v_x e_{01} + v_y e_{02})\Delta t\right)$$
* **Perspective 3D (`Perspective3D`, $\nabla \cdot \mathbf{v}_{\text{flow}} \neq 0$, e.g. *Wolfenstein 3D*):** Forward translation coupled with horizontal yaw rotation (`pga::integrate_3d_ego`):
  $$M_{\text{ego}}(t) = \exp\left(\frac{1}{2}\Omega \Delta t\, e_{12}\right)\exp\left(-\frac{1}{2}v_{\text{fwd}} \Delta t\, e_{01}\right) \in \text{Spin}(3,0,1)$$

### 6.1 Four-Stage Projectively Rectified CFT for 3D Enfoldment
A naive global 2D FFT across a perspective viewport fails because perspective projection ($y_{\text{screen}} \propto H / z$) chirps spatial frequencies from the horizon to the screen edge, and actor translation smears sprite spectra. Extending `cc enfold` to 3D corridor worlds replaces global 2D FFT registration with a four-stage **projectively rectified CFT**:

1. **Screen-Locked HUD & Weapon Overlay Isolation (Temporal Phase Variance):**
   Rows and foreground regions whose pixel phase remains invariant across horizontal camera pans ($\partial_t \Psi \approx 0$ while global yaw $\Omega(t) \neq 0$) are partitioned first as 2D viewport-locked planes (bottom status bar and centered weapon overlay).
2. **Horizon-Relative Column CFT (3D Depth & Wall Plane Reconstruction):**
   Inside the 3D viewport, vertical columns $x \in [0, W)$ are analyzed relative to the horizon line $y_h$. The symmetric vertical span $h(x)$ of coherent wall texture gives inverse projective depth:
   $$d(x) = \frac{1}{z(x)} \propto h(x)$$
   Piecewise-linear segments of $d(x)$ across adjacent columns directly reconstruct 3D PGA wall and door `Plane`s ($\Pi_k \in \mathbb{G}_{3,0,1}$) and vertical corner `Line`s via the outer product ($L_{k,k+1} = \Pi_k \wedge \Pi_{k+1}$).
3. **Ego-Motion Odometry & Manhattan/Periodicity Locking:**
   Floor-projected CFT phase correlation estimates incremental camera rotors $\Delta M_{\text{ego}}(t) \in \text{Spin}(3,0,1)$. Wall normals are regularized onto orthogonal clusters ($\langle \hat{\mathbf{n}}_i, \hat{\mathbf{n}}_j \rangle \in \{0, \pm 1\}$ via `pga::snap_manhattan_plane`) and wall lengths are quantized to the fundamental tile period $\lambda_{\text{tile}}$ of the wall's horizontal CFT spectrum inside a log-odds occupancy grid, eliminating odometry drift.
4. **Canonical-Frame 3D Actor CFT (Vitality Kernels & State Idempotents):**
   Independent moving entities (guards, projectiles) have their foot depth $z_i(t)$ estimated from projective intersection with $\Pi_{\text{floor}}$. Each actor crop is unwarped into a canonical $64 \times 64$ rest frame before computing its 2D+t CFT kernel $\hat{K}_i(\mathbf{k}, \omega)$—separating spatial silhouette/albedo ($\mathbf{k}$) from periodic walk/attack harmonics ($\omega$) conditioned on relative viewing angle. Discrete behavioral states (patrol, alert, attack, pain, dead; door closed/sliding/open) map to primitive idempotents ($\epsilon_s^2 = \epsilon_s$) connected by transition operators $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$.

### 6.2 3D Projective Incidence, Raycasting, & Active Information Navigation
The core 3D geometric primitives are already verified in `crates/cc_math/src/pga.rs` (`ray_hit_plane`, `ray_hit_plane_pga`, `plane_through_point`, `snap_manhattan_plane`, `integrate_3d_ego`). In a full 3D `cc unfold` runtime, interaction and rendering operate directly in $\mathbb{G}_{3,0,1}$:

* **PGA Ray-Plane Intersection & Line-of-Sight:** Viewport columns and weapon rays intersect wall and door `Plane`s via the PGA regressive product (`Line::antiwedge(ray, plane)`). Actor line-of-sight is evaluated via the join $L_{\text{los}} = P_{\text{player}} \vee P_{\text{enemy}}$ and tested for wall occlusion via $L_{\text{los}} \wedge \Pi_{\text{wall}}$, while sliding doors translate along their PGA `Line` hinge/rail axes.
* **Zero-A* 3D Corridor Navigation:** Over the reconstructed 3D floorplan, the player emits a distance-cone ground while walls and closed doors inject high-impedance repeller ridges into $\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1}$. Autonomous actors follow $-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$ and steer their 3D PGA `Motor`s via bivector spin-torque $\langle \mathcal{Q}_{\text{Cl}} \rangle_2$, rounding corridor corners and flanking obstacles without navmeshes.

| 3D Component Class | How `cc enfold` Extracts It (CFT + PGA + Spinors) | How `cc unfold` Runs It |
| :--- | :--- | :--- |
| **HUD & Weapon Overlay** | CFT detects zero ego-motion parallax on the bottom status bar and centered weapon anchor during camera yaw $\Omega(t) \neq 0$. | Pinned to the 2D screen plane; state variables update via incidence couplings when projectiles or pickups intersect the player `Point`. |
| **Player Ego Movement** | Optical CFT phase flow extracts forward translation speed and yaw angular velocity $M_{\text{ego}}(t) \in \text{Spin}(3,0,1)$. | User input drives $M_{\text{player}}$ (`integrate_3d_ego`); wall `Plane` impedance prevents clipping via $\mathcal{Q}_{\text{Cl}}$ repulsion. |
| **3D Walls & Doors** | Horizon-symmetric vertical spans $h(x) \to 1/z(x)$ + log-odds Manhattan occupancy reconstruct orthogonal 3D PGA `Plane`s ($\Pi_{\text{wall}}, \Pi_{\text{door}}$) and $64 \times 64$ rectified texture tiles. | Rendered via fast PGA ray-plane intersection (`pga::ray_hit_plane`); doors translate along their PGA `Line` rail when triggered. |
| **Enemies & Props** | Independent CFT motion + depth discontinuity isolate billboard actors; canonical $64 \times 64$ 2D+t CFT extracts walk/attack spectral loops. | Enemies navigate the corridor's $\mathcal{Q}_{\text{Cl}}$ potential field toward the player and fire along $P_{\text{enemy}} \vee P_{\text{player}}$. |



