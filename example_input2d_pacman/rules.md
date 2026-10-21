# Pac-Man (Ortho2D-Static) — Rule Specification

## 1. Space & Camera Regime
- **Regime:** `Ortho2D-Static` (zero cumulative camera displacement: $\max |X_{\text{cam}}| < 8$, $\max |Y_{\text{cam}}| < 8$).
- **Gravity:** None ($\mathbf{g} = 0$). Actor kinematics are planar and non-gravitational.
- **HUD Partitioning:**
  - Top HUD bar: $y < 24$ (`1UP`, `HIGH SCORE`, `2UP`).
  - Bottom HUD bar: $y > 288$ (lives counter, stage fruit indicators).
  - Viewport / Playfield: $y \in [24, 288]$, $x \in [0, 240]$.

## 2. Geometry & Navigation Grid
- **Lattice:** $8 \times 8$ px cells ($28 \times 36$ tile grid).
- **Collision Blades:** Blue maze boundary lines ($L_{\text{wall}}$).
  - Arrest perpendicular velocity: $\mathbf{v} \cdot \hat{n}_{\text{wall}} = 0$.
  - Free tangential passage along corridor axes.
- **Warp Tunnels:** Bilateral boundary mapping: crossing $x < 0$ wraps to $x = W - 1$ at tunnel row.

## 3. Kinematics & Rotor Steering ($\mathbb{G}_{2,0}^+$)
- **Heading Rotors:** Discrete 4-way direction rotors $R_\theta = e^{\theta e_{12} / 2}$:
  - Right: $\theta = 0^\circ$
  - Up: $\theta = 90^\circ$
  - Left: $\theta = 180^\circ$
  - Down: $\theta = 270^\circ$
- Direction changes occur at corridor intersection nodes where lateral boundary $L_{\text{wall}}$ is absent.

## 4. Entity Holons & State Idempotents
- **Consumable Props (Dots & Energizers):**
  - Dot Holon: small $2 \times 2$ or $4 \times 4$ prop with idempotent $\epsilon_{\text{dot}}$.
  - Incidence: when Pac-Man point $P_{\text{player}}$ intersects dot region, triggers nilpotent consumption $T_{\text{dot} \to 0}$: dot disappears, score increments.
- **Autonomous Actors (Ghosts):**
  - Inhabit minimal left ideals with state basis $\mathcal{B} = \{\epsilon_{\text{chase}}, \epsilon_{\text{scatter}}, \epsilon_{\text{frightened}}, \epsilon_{\text{eyes}}\}$.
  - State transitions governed by timer / energizer trigger:
    $T_{\text{chase} \to \text{frightened}} = \epsilon_{\text{frightened}} M \epsilon_{\text{chase}}$.
  - Relational incidence:
    - Pac-Man $\cap$ Normal Ghost $\implies$ Pac-Man life loss.
    - Pac-Man $\cap$ Frightened Ghost $\implies$ Ghost collapses to $\epsilon_{\text{eyes}}$, returns to ghost house.

## 5. Active Information Field ($\mathcal{Q}_{\text{Cl}}$)
- Ghost navigation is guided by distance-cone potential gradients:
  - Blinky (Red): targets player position directly ($-\nabla \mathcal{Q}_{\text{target}}$).
  - Pinky (Pink): targets 4 tiles ahead of player heading.
  - Inky (Cyan): reflects Blinky vector across player offset.
  - Clyde (Orange): approaches player when far ($d > 8$ tiles), scatters to corner when near ($d \le 8$ tiles).
