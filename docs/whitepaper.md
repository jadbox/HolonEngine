# The Clifford Implicate World Compiler (`cliffordcore`)
### Architecture & Systems Whitepaper: Process Ontology, Clifford Density Elements, and Active Information

---

## 1. Foundational Thesis: From Atomism to the Implicate Order

 interactive world engines have historically split into two flawed paradigms:
1. **Atomistic State Machines (Traditional Engines):** Sealed structs sit inside an inert coordinate void, governed by hand-coded boolean flags and game-specific branching (`if is_pacman { eat_dot() } else if is_mario { stomp_goomba() }`).
2. **Frame-by-Frame Neural Hallucination (Autoregressive Video Models):** Pixel buffers are predicted statistically per frame without persistent geometry, conservation laws, or object identity.

`cliffordcore` implements a third architecture grounded in **David Bohm and Basil Hiley's Implicate Order** (*The Undivided Universe*; Hiley & Callaghan 2010, `docs/1011.4031_hiley_clifford_quantum_potential.pdf`) and **Projective Geometric Algebra ($\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$)**:
* **Enfoldment (`cc enfold`):** Compiles raw gameplay video into an invariant **Implicate World Model (`.cc`)**—extracting camera `Motor` odometry, ghost-free world panoramas, oriented PGA boundary blades, chromatic/semantic spinor signatures, and canonical actor processes without game-specific rules.
* **Unfoldment (`cc unfold`):** Runs the compiled `.cc` model forward at thousands of hertz as a unified, zero-AI Clifford dynamical system where every entity is an algebraic process coupled to the surrounding spatial medium.

---

## 2. Core Algebraic Abstractions (`cc_math`)

### 2.1 Minimal Left Ideals & The Clifford Density Element (`CliffordDensityElement`)
Following Hiley & Callaghan (2010, §2.1–2.5), `cliffordcore` dispense with external state vectors and ad-hoc entity flags. Every world entity (player, autonomous actor, interactive block, collectible token) is an **Algebraic Holon** inhabiting a minimal left ideal $\mathcal{I}_i = \mathcal{C}\ell\,\epsilon_i$:

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

| Incidence Event | Geometric Predicate ($\mathbb{G}_{3,0,1}$) | Algebraic Operator ($T_{s \to s'}$) | Emergent Behavior Across Genres |
| :--- | :--- | :--- | :--- |
| **Boundary Reflection** | $P_i \wedge L_{\text{wall}} = 0,\; \langle \mathbf{s}_i, \hat{\mathbf{n}} \rangle < 0$ | $U_i \mapsto -\hat{\mathbf{n}} U_i \hat{\mathbf{n}},\; T_{0 \leftrightarrow 1}$ | Platformer enemy reverses patrol at pipe/ledge; maze actor turns at corridor wall. |
| **Top-Down Incidence** | $P_{\text{actor}} \wedge L_{\text{target,top}} = 0,\; \mathbf{P}_B \cdot e_2 > 0$ | Target $T_{s \to 2}$ (collapse), Actor $P_{B,y} \mapsto -\eta P_{B,y}$ | Stomping an enemy collapses it to passive state $\epsilon_2$ and imparts upward restitution. |
| **Upward Boundary Impulse** | $P_{\text{actor}} \wedge L_{\text{prop,bottom}} = 0,\; \mathbf{P}_B \cdot e_2 < 0$ | Prop $T_{0 \to 1 \to 2}$, emits excitation token | Striking a floating block from below triggers harmonic displacement and token emission. |
| **Field Excitation Absorption** | $P_{\text{actor}} \cap B_{\text{excitation}} \neq \varnothing$ | Excitation $T_{0 \to 2}$ ($\rho \to 0$) | Passing through a corridor dot, pellet, or coin absorbs the excitation into actor energy. |

---

## 3. Active Information & The Clifford Quantum Potential (`cc_field::potential`)

During `cc unfold`, the spatial lattice carries a real-time amplitude field $\Psi(x, y, t) = R(x, y, t)\,\mathcal{U}(x, y, t)$ governed by the **Clifford Quantum Potential**:

$$\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1} = \underbrace{-\frac{\nabla^2 R}{R}}_{\text{Scalar Guidance } \langle \mathcal{Q}_{\text{Cl}} \rangle_0} - \underbrace{\left(2(\nabla \ln R)\mathcal{U}\nabla\widetilde{\mathcal{U}} + (\nabla^2 \mathcal{U})\widetilde{\mathcal{U}}\right)}_{\text{Bivector Spin-Torque } \langle \mathcal{Q}_{\text{Cl}} \rangle_2}$$

* **Scale-Invariant Valley Guidance:** Because $\langle \mathcal{Q}_{\text{Cl}} \rangle_0 = -\nabla^2 R / R$ normalizes spatial curvature by amplitude, guidance depends solely on the *geometric form* of the environment.
* **Boundary Impedance & Attractor Basins:** `TileKind::Solid` cells clamp $R \to R_{\text{floor}}$, forming steep positive $\mathcal{Q}_{\text{Cl}}$ repeller ridges along walls and pipes. Open corridors (`TileKind::Empty`) and active collectible excitations inject smooth positive amplitude domes, carving negative-$\mathcal{Q}_{\text{Cl}}$ valleys through the level.

---

## 4. Game-Agnostic Spatiotemporal Compiler (`cc_field::enfold2d`)

`cc enfold` extracts the complete `.cc` world model directly from raw video without relying on per-game rules files or palette constants:

1. **Scene Stability Segmentation (`detect_playfield_segment`):**
   Scans the video stream for the longest contiguous run of active frames sharing a stable border backdrop mode, automatically stripping title screens, menus, and attract-mode cuts.
2. **Spectral Camera Regime Discovery (`detect_camera_is_static`, `ScrollEstimator`):**
   Evaluates 2D FFT cross-power phase correlation across the playfield band. Near-zero cumulative drift ($\max |\Delta \mathbf{X}_{\text{cam}}| \le 4\text{ px}$) selects the static arena regime (`Ortho2D-Static`), while translational flow selects scrolling odometry (`Ortho2D-Scroll`) refined to pixel lock via $\pm 1\text{ px}$ local SSAD.
3. **Topological Boundary & Collectible Separation (`extract_2d_tiles_with`):**
   * Learns the backdrop color from the border mode.
   * In scrolling worlds with a continuous floor (`has_ground`), extracts the per-column ground surface, raised terrain steps, and flat-topped `OneWay` shelves (pipes, platforms, floating blocks) while relegating curved shapes (clouds, hills, bushes) to `Background`.
   * In floorless / planar worlds, segments 8-connected components of non-background pixels: extended structural boundaries ($\max(w, h) \ge 2 \times \text{tile\_size}$) become 4-sided `Solid` walls, while small isolated corridor components ($\max(w, h) \le 12\text{ px}$) are extracted as discrete **Collectible Field Excitations** (`collectibles`).
4. **Mobile Process vs. Stationary Site Separation & Spinor-Coherent Tracking (`track_actors`):**
   * Residual bounding boxes carry their mean foreground chromatic spinor $\psi_b$.
   * Frame-to-frame assignment minimizes a joint spatial + spinor-coherence cost $d(\mathbf{x}_a, \mathbf{x}_b) + \lambda \|\psi_a - \psi_b\|$ under both Euclidean and toroidal (warp-tunnel) metrics.
   * Non-overlapping track fragments with matching spinor coherence ($\|\psi_a - \psi_b\| \le \tau_\psi$) are unified into a single persistent `ActorTrack`.
   * Processes with world displacement $\max(\Delta X, \Delta Y) \ge 10\text{ px}$ ($\mathbf{P}_B \neq 0$) are compiled into mobile `ActorTrack`s and masked out of the static background atlas in Pass 3; stationary blinking sites ($\mathbf{P}_B = 0$) remain anchored in the world lattice.
5. **Inferred Field Constants (`infer_world_gravity`):**
   Analyzes vertical acceleration $\Delta^2 y_t$ and ground-surface support across extracted tracks to infer whether the world is governed by downward gravity ($g_y > 0$) or planar zero-gravity kinematics ($g_y = 0$).
