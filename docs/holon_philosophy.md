# From Video & Text to Playable Implicate Worlds
### Ontology and Mathematics of the Clifford World Compiler (`HolonEngine`)

<small>by [Jonathan Dunlap](https://www.linkedin.com/in/jonathandunlap/)</small>

---

## I. The Double Trap of Modern Interactive Media & World Models

Attempts to turn visual media into interactive worlds fail in one of two ways:

1. **The Clockwork Puppet Trap (Manual Game Engines):** Worlds are hand-authored from dead polygons inside an empty coordinate void. Motion requires explicit keyframe timelines, skeletal rigs, and boolean state machines (`if key_collected { open_door() }`).
2. **The Hallucinatory Crutch Trap (Frame-by-Frame Neural World Models):** A diffusion or autoregressive video model predicts the next pixel buffer conditioned on a controller input. Because there is no persistent geometric ground, walls drift when you turn around, object identity evaporates across frames, latency is high, and causality collapses into statistical correlation.

A real world model requires a third foundation: **extracting the Implicate Order from video and text into a deterministic Clifford runtime.**

A single static image does not contain enough information to define space or causality—it is a single frozen *explicate* slice ($t = t_0$) with zero temporal derivative $\partial_t \Psi$. Video and text, by contrast, encode **transitions**: camera parallax, occlusion, invariant HUD overlays, wall planes, actor trajectories, and semantic rules. Local AI tools (`llama.cpp` VLM and embedding models) do not *run* the world; they act at ingest as **diagrammatic transducers** that partner with the **Clifford Fourier Transform (CFT)** and **Active-GATr** to compile raw video and text into a playable $\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$ world model.

---

## II. Ontological Foundations

### 1. Enfoldment and Unfoldment (Bohm & Hiley)
Following David Bohm and Basil Hiley (*The Undivided Universe*, `docs/1011.4031_hiley_clifford_quantum_potential.pdf`), observed video frames are *explicate* projections of an underlying **Implicate Order**—an algebra of process governed by non-commutative transitions $M_{A \to B} = \Psi_B \Psi_A^{-1}$ and Active Information $\mathcal{Q}_{\text{Cl}}$.
* **Enfold (`cc enfold`):** Compiles a folder of video and text into its structural and dynamical invariants (3D PGA geometry, CFT vitality kernels, semantic spinors, and $\mathcal{Q}_{\text{Cl}}$ coupling rules).
* **Unfold (`cc unfold`):** Runs the compiled model forward at hundreds of hertz as an interactive, playable world where user input steers the ego `Motor` and Active Information guides the environment.

### 2. The Diagram vs. The Cliché (Disciplining Local AI)
Gilles Deleuze observed that raw imagery is cluttered with statistical clichés, whereas a *diagram* extracts the underlying intensive forces and relations.
* **The Rule of Non-Crutch AI:** Generative models never sit inside the frame-to-frame runtime loop. If a neural network hallucinates the next frame of a corridor, the world has no ontology.
* **AI + Spectral Math as Ingest Compiler:**
  * **`llama.cpp` VLM (`llama-mtmd-cli` / `llama-server`):** Samples video keyframes to label components (`wall`, `door`, `enemy`, `hud_panel`, `weapon`), bounding boxes, visual descriptions, and state transitions.
  * **`llama.cpp` Embeddings (`llama-embedding`):** Maps component descriptions and text rules into a semantic wording space, whose Gram matrix $S_{ij} = \langle \hat{e}_i, \hat{e}_j \rangle$ projects into Clifford spinor phases.
  * **Clifford Fourier Transform (CFT):** Separates camera ego-motion from world geometry and extracts compact spatiotemporal vitality kernels $\hat{K}(\mathbf{k}, \omega)$ for animated sprites and textures.
  * **Active-GATr:** Discovers the geometric incidence and behavioral laws connecting HUD, 3D space, player, and autonomous actors.

---

## III. Mathematical Architecture: CFT + Active-GATr in $\mathbb{G}_{3,0,1} \otimes \mathbb{G}_{2,0}$

To compile a video (such as *Wolfenstein 3D* gameplay) plus text notes into a playable engine model, every extracted component becomes a **Holon**:

$$\mathcal{H}_i = \left(\underbrace{B_i \in \mathbb{G}_{3,0,1}}_{\text{3D PGA Blade + Motor}},\; \underbrace{\psi_i = R_i e^{\theta_i e_{12}} \in (\mathbb{G}_{2,0}^+)^K}_{\text{Semantic Spinor}},\; \underbrace{\hat{K}_i(\mathbf{k}, \omega)}_{\text{CFT Spectral Kernel}}\right)$$

### 1. Spatiotemporal Decomposition via Clifford Fourier Transform (CFT)
A naive global 2D FFT across a perspective viewport fails because perspective projection ($y_{\text{screen}} \propto H / z$) chirps spatial frequencies from the horizon to the screen edge, and actor translation smears sprite spectra. Instead, `cc enfold` applies **projectively rectified CFT** in four stages:

1. **Screen-Locked HUD Isolation (Temporal Phase Variance):**
   Rows and regions whose pixel phase remains invariant across horizontal camera pans ($\partial_t \Psi \approx 0$ while global yaw $\Omega(t) \neq 0$) are partitioned first as 2D viewport-locked planes (status bar, weapon overlay).
2. **Horizon-Relative Column CFT (3D Depth & Wall Plane Reconstruction):**
   Inside the 3D viewport, vertical columns $x \in [0, W)$ are analyzed relative to the horizon line $y_h$. The symmetric vertical span $h(x)$ of coherent wall texture gives inverse projective depth $d(x) = 1/z(x) \propto h(x)$. Piecewise linear segments of $d(x)$ across adjacent columns directly reconstruct 3D PGA wall and door `Plane`s ($\Pi_k$) and vertical corner `Line`s ($L_{k,k+1} = \Pi_k \wedge \Pi_{k+1}$).
3. **Ego-Motion Odometry + Manhattan/Periodicity Locking:**
   Floor-projected CFT phase correlation estimates incremental camera rotors $\Delta M_{\text{ego}}(t) \in \text{Spin}(3,0,1)$. Active-GATr regularizes wall normals onto orthogonal clusters ($\langle n_i, n_j \rangle \in \{0, \pm 1\}$) and quantizes wall lengths to the fundamental tile period $\lambda_{\text{tile}}$ of the wall's horizontal CFT spectrum, eliminating odometry drift.
4. **Canonical-Frame Actor CFT (Vitality Kernels & State Idempotents):**
   Independent moving entities (guards, projectiles) have their foot depth $z_i(t)$ estimated from intersection with $\Pi_{\text{floor}}$. Each actor crop is unwarped into a canonical $64 \times 64$ rest frame before computing its 2D+t CFT kernel $\hat{K}_i(\mathbf{k}, \omega)$—cleanly separating spatial silhouette/albedo ($\mathbf{k}$) from periodic walk/attack harmonics ($\omega$). Discrete behavioral states (patrol, alert, attack, pain, dead; door closed/sliding/open) are represented as **Clifford Primitive Idempotents** ($\epsilon_s^2 = \epsilon_s$) connected by transition operators $T_{s \to s'} = \epsilon_{s'} M \epsilon_s$.

### 2. Relational Structure Discovery via Active-GATr
Standard Geometric Algebra Transformers (`docs/2302.06594_geometric_clifford_algebra_networks.pdf`) operate on $\mathbb{G}_{3,0,1}$ multivectors using $\text{Pin}(3,0,1)$-equivariant linear layers and the geometric product. **Active-GATr** extends this to bind extracted video/text holons across time:

$$\mathcal{A}_{ij} = \text{Softmax}_j \Big( \alpha \langle q_i, k_j \rangle_{\mathbb{G}_{3,0,1}} + \beta \langle \psi_i, \psi_j \rangle_{\mathbb{G}_{2,0}} + \gamma \langle B_i \vee B_j \rangle_0 + \delta \langle \mathcal{Q}_{\text{Cl}}(i), \mathcal{Q}_{\text{Cl}}(j) \rangle \Big)$$

* **Term 1 ($\langle q_i, k_j \rangle_{\mathbb{G}_{3,0,1}}$):** Orientation and plane-normal alignment (grouping collinear wall segments and orthogonal corridor turns).
* **Term 2 ($\langle \psi_i, \psi_j \rangle_{\mathbb{G}_{2,0}}$):** Wording-space spinor coherence from `llama-embedding` (linking a text rule *"guards shoot when sighting the player"* or *"doors slide open on use"* to the corresponding visual holons).
* **Term 3 ($\langle B_i \vee B_j \rangle_0$):** Projective incidence via the `Antiwedge` (join)—computing line-of-sight rays $L_{\text{los}} = P_{\text{player}} \vee P_{\text{enemy}}$, wall occlusion $L_{\text{los}} \wedge \Pi_{\text{wall}}$, and door hinge axes.
* **Term 4 ($\langle \mathcal{Q}_{\text{Cl}}(i), \mathcal{Q}_{\text{Cl}}(j) \rangle$):** Active Information coupling—inferring which entities act as attractors or repellers in the floorplan field.

### 3. Bohm-Hiley Active Information at Runtime ($\mathcal{Q}_{\text{Cl}}$)
During `cc unfold`, the reconstructed 2D/3D floorplan carries the Clifford field $\Psi(x, t) = R(x, t)\,\mathcal{U}(x, t)$ governed by the **Clifford Quantum Potential**:

$$\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1} = \underbrace{-\frac{\nabla^2 R}{R}}_{\text{Scalar Guidance } \langle \mathcal{Q}_{\text{Cl}} \rangle_0} - \underbrace{\left(2(\nabla \ln R)\mathcal{U}\nabla\widetilde{\mathcal{U}} + (\nabla^2 \mathcal{U})\widetilde{\mathcal{U}}\right)}_{\text{Bivector Spin-Torque } \langle \mathcal{Q}_{\text{Cl}} \rangle_2}$$

* **Zero-A* Enemy Navigation:** The player emits a global distance-cone ground (`docs/discovery_page_ground.md`), while walls and closed doors inject high-impedance repeller ridges. Enemies follow $-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$ and steer their PGA `Motor`s via $\langle \mathcal{Q}_{\text{Cl}} \rangle_2$, naturally rounding corridor corners and flanking obstacles without navmeshes.
* **Soliton Projectiles & CFT Playback:** Projectiles and shockwaves propagate through the split-step CFT lattice as self-focusing solitons, while actor sprites synthesise their animation frames directly from their enfolded CFT kernels $\hat{K}_i(\mathbf{k}, \omega)$ conditioned on relative viewing angle.

---

## IV. Two-Stage Benchmark: 2D (*Super Mario Bros. 3*) $\to$ 3D (*Wolfenstein 3D*)

In $\mathbb{G}_{3,0,1}$, 2D side-scrollers and 3D corridor worlds share one mathematical representation, distinguished only by the camera `Motor` subgroup detected from CFT optical-flow divergence ($\nabla \cdot \mathbf{v}_{\text{flow}}$):

1. **`example_input2d/` — *Super Mario Bros. 3* World 1-1 (`Ortho2D`, $\nabla \cdot \mathbf{v}_{\text{flow}} \approx 0$):**
   * **Camera & Level Atlas:** CFT cross-power spectrum recovers sub-pixel camera scroll $(X_{\text{cam}}(t), Y_{\text{cam}}(t))$. Taking the temporal median in world coordinates removes transient sprites and stitches a ghost-free level panorama.
   * **Tile Boundaries & One-Way Platforms:** Bragg peaks in the atlas CFT lock the $16 \times 16$ tile lattice. Observed velocity arrests classify solid 4-sided tiles vs. one-way top-surface PGA `Line`s (pastel shadow-boxes).
   * **Actors & Active Information:** Residual motion isolates Mario, Goombas, Koopas, and `?` blocks into canonical $32 \times 32$ 2D+t CFT vitality kernels and state idempotents ($\epsilon_{\text{small}} \leftrightarrow \epsilon_{\text{super}}$, $\epsilon_{\text{walk}} \to \epsilon_{\text{shell}}$).

2. **`example_input3d/` — *Wolfenstein 3D* Episode 1 Floor 1 (`Perspective3D`, $\nabla \cdot \mathbf{v}_{\text{flow}} \neq 0$):**

| Component Class | How `cc enfold` Extracts It (VLM + Embeddings + CFT + GATr) | How `cc unfold` Runs It |
| :--- | :--- | :--- |
| **HUD Layout** | CFT detects zero ego-motion parallax on the bottom status bar and centered weapon anchor; VLM labels `health`, `ammo`, `face`, `weapon`. | Pinned to the 2D screen plane; state variables update via Active-GATr couplings when projectiles or pickups intersect the player `Point`. |
| **Player Movement** | Optical CFT phase flow extracts forward translation speed and yaw angular velocity $M_{\text{ego}}(t) \in \text{Spin}(3,0,1)$. | User keyboard/CLI input drives $M_{\text{player}}$; wall `Plane` impedance prevents clipping via $\mathcal{Q}_{\text{Cl}}$ repulsion. |
| **3D Walls & Doors** | Horizon-symmetric vertical spans $h(x) \to 1/z(x)$ + log-odds Manhattan occupancy reconstruct orthogonal 3D PGA `Plane`s ($\Pi_{\text{wall}}, \Pi_{\text{door}}$) and $64 \times 64$ texture tiles. | Rendered via fast PGA ray-plane intersection (`Line::vee(ray, plane)`); doors translate along their PGA `Line` rail when triggered. |
| **Enemies & Props** | VLM boxes + independent CFT motion isolate guards; canonical $64 \times 64$ 2D+t CFT extracts walk/attack spectral loops; GATr links enemy pursuit to player line-of-sight. | Enemies navigate the corridor's $\mathcal{Q}_{\text{Cl}}$ potential field toward the player and fire along $P_{\text{enemy}} \vee P_{\text{player}}$. |

---

## V. First Principles Summary

1. **Video + Text In, Playable Model Out:** Static images lack temporal transition structure; video provides the CFT motion invariants and text provides the semantic wording-space spinors.
2. **AI Compiles, Math Runs:** `llama.cpp` (VLM + embeddings) runs only during `cc enfold`. `cc unfold` is a pure Rust Clifford runtime running at hundreds of hertz.
3. **Geometry Is Explicit:** 3D space is modeled as exact $\mathbb{G}_{3,0,1}$ blades (`Plane` walls, `Line` rays/hinges, `Point` actors, `Motor` poses), never blurry neural latent tensors.
4. **Guidance by Active Information:** Autonomous behaviors emerge from $\mathcal{Q}_{\text{Cl}} = -(\nabla^2 \Psi)\Psi^{-1}$ over the reconstructed space and semantic phase alignment.
