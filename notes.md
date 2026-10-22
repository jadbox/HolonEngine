# HolonEngine — Builder Notes

## Benchmark facts (verified by frame inspection)
- Prefer `example_input2d/smb3_w1_1_256x240.mp4` (native 256x240) over the 320x240 upscale. Tiles are exactly 16x16.
- Gameplay starts ~19.0 s. 0–18.5 s is title/world-map/blank, so `enfold` must auto-detect the playfield segment.
- HUD band starts at y=193 (not 192). Bottom rows 224–239 are black. Plan's `y >= 192` is a 1 px approximation; use detected top.
- Tile lattice: `origin_y = ground_top % 16`. SMB3 atlas has `origin_y = 0`, ground top y=176 (tile row 11), block top edges at `16*k`.
- **The ground is not flat**: at world x≈608 the surface steps up one tile (y=176 → y=160, tile row 10) and stays raised. Ground detection must be per tile column, never one global `ground_top`.
- Camera scroll is bidirectional: right from 19 s, back left ~24.5–28 s, right again afterwards. No death/restart. Scroll total 19–40 s ≈ 722–744 px (estimator vs brute-force SSD).
- Channel-wise temporal median produces ghost blends. Per-pixel **color mode** over quantized RGB removes nearly all actor ghosts.

## Scroll odometry (important)
- Use **plain magnitude-weighted FFT cross-correlation**, not PHAT. On this footage PHAT (phase-only) agrees with brute-force SSD on 59.4 % of frames and undercounts cumulatively (209 vs 744 px); plain cross-power agrees on 92.7 % (721 vs 744 px).
- Remaining disagreements are all ±1–2 px at slow-motion transitions; cumulative drift < 0.1 px/frame. Good enough for integer atlas placement.
- Independent ground truth: track a uniquely coloured blob (the pink screw box) and check `screen_x + cumulative_scroll` stays constant. It does (248–256 px over 630 frames, including the leftward segment). This is a cheap, robust odometry test that avoids the periodic-texture aliasing trap.
- 1 s / 5 s-gap SSD alignment is **not** a valid ground truth: the 16-px ground texture repeats and aliases by ±16/32 px. Per-frame deltas (≤8 px) are unambiguous.

## Tile / object classification (final, colour-agnostic)
- Learn the backdrop = mode of the atlas top band (5-bit RGB buckets, bucket-mean colour).
- `nonbg = color_dist(pixel, backdrop) > bg_tol` (24).
- Per pixel column, `dense_start[x]` = top of the non-background run that reaches the atlas floor.
- Ground line = **lowest** y whose `dense_start` occurs in ≥ `max(8, W/12)` columns. SMB3: y=176 → `origin_y = ground_top % 16 = 0`, `ground_row = 11`. Lattice origin is derived from the ground line, not assumed.
- Per tile column, snap the dominant `dense_start` to the lattice (round). Accept only a **one-tile step** (`ground_row - 1`); taller content falls back to the ground row.
- Bridge two visible pieces of the same shelf across gaps ≤ 8 columns whose step-row tile is dense (bushes hide the shelf line). **No open-ended growth**: a one-tile bush is structurally identical to a one-tile step, and growing shelves terraformed the whole level in the box cluster.
- Drop runs shorter than `min_run` (3) columns — but never rewrite the ground row itself (doing so filled a lone ground column between two steps and propagated the step leftward).
- Solid = all tiles at/below `surface_row[tx]`.
- One-way = connected components *above* the local surface that are filled (≥0.6 bbox fill), ≥12×10 px, and flat-topped (≥80 % of columns within ±1 px of the median top). Clouds/bushes fail flatness; a flat rectangle with a grounded base is folded into terrain instead.
- Remaining non-background tiles → Background (scenery, no collision).
- Tunables: `TileOptions { bg_tol, min_run, min_area, min_w, min_h, min_fill, min_flat }`, `extract_2d_tiles_with(atlas, &opts)`.
- Known limits: a *wide* flat-topped decoration sitting on the ground is indistinguishable from a one-tile ledge; a decor-hidden shelf is only reconstructed where both sides are visible. Per-game fixes belong in a rules file, never in `cc_field`.
- RGB-palette proximity trap: sky `(173,228,241)` is only ~20 away from the pastel-blue box `(186,220,254)`. Never test against game palettes; compare to the *learned* backdrop.

## Model and runtime
- `.cc` (CC2D v1): `"CC2D" | u32 version | u32 manifest_len | JSON manifest | u32 atlas_png_len | PNG | u32 hud_png_len (0 = none) | PNG | sprite PNGs`. The manifest lists tile kinds, scroll, platforms, solid runs, sprites and tracks; images stay PNG so the container is small (166 KB for the 978×193 SMB3 atlas + HUD + 9 sprites).
- Player collision: leading-edge-only horizontal resolution, move-then-resolve vertically, one-way only when falling with `prev_feet <= top + 2`, solid landing tolerance one tile. Resolving *all* overlapped tiles per axis teleports the player through the floor; the leading-edge rule does not.
- `cc unfold` is headless at 3–16 kHz (release); rendering = atlas blit + static HUD composite + player marker.

## Actor tracking and Hiley process ontology
- Residuals compare each frame to the atlas at its scroll offset. Off-by-one registration turns every sprite edge into a full-screen residual: refine the FFT offset with an exhaustive ±1 SSAD search (normalised by covered area) before masking.
- `ACTOR_MAX_PX = 40` (up from 32): captures tall 2-tile sprites (Piranha plant 18x34, Super Mario, Koopas) while rejecting clouds (w>=32 with irregular tops).
- Player classification: in any side-scroller, the player is the dominant persistent actor alive across the segment (`t.boxes.len()`), while enemies exist only locally around spawn points. Track 0 is classified as `Player` (499 frames alive).
- Autonomous entities as **Algebraic Holons** (Basil Hiley):
  Entities are not boolean bags of properties, but processes inhabiting minimal left ideals $\mathcal{I}_i = \mathcal{C}\ell \epsilon_i$ governed by state idempotents ($\epsilon_s^2 = \epsilon_s$):
  - State 0: Patrol negative direction ($\epsilon_0$)
  - State 1: Patrol positive direction ($\epsilon_1$)
  - State 2: Post-impact / passive state ($\epsilon_2$)
  - Lateral boundary incidence (wall hit) triggers idempotent reversal transition $T_{0 \leftrightarrow 1}$.
  - Top-down relational incidence (downward player motion onto actor's top boundary) collapses the actor to State 2 (stomp / squash) and imparts an upward restitution impulse to the player.
  - Generically applies to any 2D game without ad-hoc per-game branching.

## Grounded obstacles and multi-level platforms
- Obstacles standing on the floor (pipes, pastel blocks, pyramids) often have multiple stepped flat tops within a single connected component.
- Grounded obstacles (`height >= 2 * TILE`, bottom touches ground line) are segmented into horizontal flat shelves of width $\ge 12$ px.
- This promotes pipe tops, pink screw-box tops, blue screw-box tops, and terrace blocks to one-way collision surfaces while leaving curved green hills as non-collidable background, without hardcoded game palettes.

## Static 2D arenas and mazes (Pac-Man)
- Detect camera regime via FFT phase correlation across playfield rows over a 3-4 second window: cumulative drift $\le 4$ px classifies the game as `Ortho2D-Static`.
- In a static regime, cumulative scroll is identically $[0, 0]$; the temporal mode over the stream builds a clean actor-free panorama of the entire arena.
- Enclosing maze walls are discovered directly by locating rows with horizontal wall runs spanning $\ge 60\%$ of columns, neatly isolating the playfield from top and bottom HUD text without hardcoded row numbers.
- **Topological Wall vs. Collectible Separation:** Naive per-tile pixel counting (`wall_pixels >= 4`) misclassifies corridor tiles containing $2 \times 2$ dots or $8 \times 8$ energizers as `TileKind::Solid`. Instead, segment 8-connected components of `nonbg` inside the playfield: components with bounding-box span $\max(w, h) \ge 2 \times \text{tile\_size}$ form `wall_mask` (`TileKind::Solid`), while isolated components with $\max(w, h) \le \text{tile\_size} + 4$ are extracted into `ModelManifest::collectibles`.
- **Spinor-Coherent Actor Tracking & Composite Rejection:** Match residual boxes across frames using joint spatial + chromatic spinor distance (`residual_boxes_with_color`, `track_actors_coherent`) with toroidal horizontal wrap in static arenas. Unify non-overlapping track fragments sharing `color_dist <= 35` and compatible size (`<= 8 px`), and reject transient two-actor overlap composites ($\text{area} > 1.5 \times \text{primary\_area}$ and $\text{frames} < \text{primary\_frames} / 3$).
- **Learned Kinematic Excursion Tensors (`TrackRecord::span`):** Do not use sprite-size heuristics (`h >= 30 && w <= 22`) to infer vertical vs. horizontal behavior. Instead, orient each holon's `CliffordDensityElement` rotor from its observed world-space excursion $[\Delta X, \Delta Y]$: if $\Delta Y > 2\,\Delta X$, initialize along $-e_2$ and reflect across $[Y_0 - \Delta Y, Y_0]$; otherwise initialize along $e_1$.

## Architecture decisions
- `cc_field` owns video IO, 2D enfold, atlas, tiles, actor tracks, sprite CFT, and the `.cc` (CC2D) model format.
- `cc_math` owns G(2,0) `Mv2`, `Q_Cl`, PGA line/plane incidence, Manhattan snap, camera motors, state idempotents.
- `cc_cli` provides `cc enfold | inspect | unfold`.
- Model = header + JSON manifest + PNG atlas + PNG sprite sheets. No serde on huge arrays.
- Runtime is zero-AI: deterministic scripted/terminal input, tile/PGA collision, Q_Cl diagnostics.
- Engine must stay game-agnostic: colours, palettes and object classes are *learned* or supplied by a per-game rules file, never baked into `cc_field`.

## Pitfalls
- Don't use `example_input2d/smb3_w1_1.mp4` for extraction; it is 720p and slow. Fallback only.
- Accumulate integer shifts to preserve pixel art; subpixel peaks are diagnostics only.
- The 320x240 video is horizontally stretched (tile width 20). Tests may use it for HUD only.
- `enfold_dump` emits valid JSON (`tiles.json`); the trailing-comma bug is fixed.
- `clifford 0.3` Plane sign conventions are split: `from_normal_and_distance` and
  `signed_distance` use `n·x + dist = 0`, but `Line ⊻ Plane` reads `dist` as
  `n·x = dist`. Use `pga::plane_through_point` and the analytic ray hit.
- A 2D spinor space has at most two orthogonal rank-1 projectors; `StateBasis::new(n)`
  uses diagonal units `E_kk` in dimension `n` for `n > 2`.
- **Pre-Incidence vs. Post-Incidence Atlas Mode (`init_acc`):** When an interactive block is hit early in a video (e.g. frame 45 out of 630), the global temporal mode reflects its *spent* state ($\epsilon_2$). Accumulating an unmasked pre-incidence window (`init_frames = 25`) restores the genuine initial State 0 texture ($\epsilon_0$) into the atlas while `BoundaryBlade::frame_color` learns the spent State 2 perimeter color—eliminating any need for procedural block glyphs at runtime.
- **Multi-Frame Stride for Kinematic Regression (`infer_world_kinematics`):** Second-order differences $\Delta^2 y_t$ on integer-quantized bounding boxes at $\Delta t = 1$ suffer from integer truncation bias. Using a 4-frame half-window ($\Delta t = 4,\; \Delta t^2 = 16$) recovers unbiased free-fall acceleration $a_y \approx 0.35\text{ px/frame}^2$ and launch impulse $J_{\perp} = \sqrt{2 a_y \Delta h} \approx 6.2\text{ px/frame}$.
