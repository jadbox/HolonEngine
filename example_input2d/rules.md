# Super Mario Bros. 3 — World 1-1 Implicate World Rules (2D Benchmark)

## Viewport, Camera & HUD Invariants
- The bottom black-bordered HUD panel (`WORLD 1`, `[P-Meter >>> P]`, `Coins $`, `Lives M`, `Score`, `Timer`, and `3 Goal Cards`) is screen-locked at the bottom of the frame ($y \ge 192$ in $256 \times 240$ / $320 \times 240$) and invariant under camera scrolling.
- The upper playfield is a 2D orthographic plane governed by a translational camera `Motor` $M_{\text{ego}}(t) = \exp(-\frac{1}{2}(v_x e_{01} + v_y e_{02})\Delta t)$.

## 2D Tile Lattice & Oriented PGA Boundaries
- The static world consists of a $16 \times 16$ tile lattice (ground strip, question `?` blocks, brick blocks, green pipes, and pastel screw-corner shadow boxes).
- Ground tiles, bricks, question blocks, and green pipes are full 4-sided high-impedance barriers ($V_{\text{impedance}} = \infty$).
- Pastel screw-corner boxes (pink, blue, orange, white) are **one-way top-surface PGA lines**: Mario and actors pass freely through their bottom and sides when jumping upward ($v_y < 0$), and land solidly on their top horizontal edge when falling ($v_y \ge 0$).

## Autonomous Actors, State Idempotents & Active Information ($\mathcal{Q}_{\text{Cl}}$)
- **Mario (`player`):** Controlled via horizontal thrust ($v_x$), jump impulse ($v_{y0}$), and gravity ($g$). Holds state idempotents $\epsilon_{\text{small}} \leftrightarrow \epsilon_{\text{super}} \leftrightarrow \epsilon_{\text{raccoon}}$ with canonical $32 \times 32$ 2D+t CFT walk/jump/tail-wag vitality kernels.
- **Question Blocks (`question_block`):** Striking the bottom edge ($P_{\text{head}} \wedge L_{\text{bottom}} = 0$ with $v_y < 0$) transitions $\epsilon_{\text{question}} \to \epsilon_{\text{empty}}$, increments `Coins`/`Score` or spawns a `SuperMushroom` / `SuperLeaf`.
- **Goombas & Koopa Troopas (`enemy`):** Drift along horizontal platform valleys of $\langle \mathcal{Q}_{\text{Cl}} \rangle_0$ and reverse horizontal spinor direction when intersecting pipe or step boundaries.
  - Top stomp incidence ($P_{\text{mario\_foot}} \wedge L_{\text{enemy\_top}} = 0$ with $v_y > 0$) transitions Goomba $\epsilon_{\text{walk}} \to \epsilon_{\text{flat}}$ and Koopa $\epsilon_{\text{walk}} \to \epsilon_{\text{shell\_idle}} \to \epsilon_{\text{shell\_kick}}$.
  - Lateral contact transitions Mario down one power state ($\epsilon_{\text{raccoon}} \to \epsilon_{\text{super}} \to \epsilon_{\text{small}} \to \epsilon_{\text{dead}}$).
- **Venus Fire Trap (`piranha_plant`):** Emerges periodically from green pipe top lines and fires projectiles along the PGA join line $P_{\text{plant}} \vee P_{\text{mario}}$.
