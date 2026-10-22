# `holon-debug` — Inspection & Analysis Tool

Standalone diagnostic and inspection tool for HolonEngine compiled world models (`.cc`) and visual media frames.

---

## 1. Quick Start & Subcommands

```bash
# Top-level overview (alias)
holon-debug <model.cc>

# Top-level overview (explicit)
holon-debug overview <model.cc>

# Bracket notation is automatically stripped (e.g., in prompts or notes)
holon-debug [[model.cc]]
holon-debug overview [[model.cc]]
```

### Full Subcommand Reference

| Command | Description |
| :--- | :--- |
| `holon-debug <path.cc>` | Default alias for `overview`. |
| `holon-debug overview <path.cc>` | Top-level summary of world regime, geometry, boundary blades, and holons. |
| `holon-debug tracks <path.cc>` | Detailed track list with spawn origins, spans, velocities, and waypoints. |
| `holon-debug sprites <path.cc>` | Canonical sprite library, dimensions, byte sizes, and alpha statistics. |
| `holon-debug collisions <path.cc>` | Bounding boxes for solid runs, one-way platforms, and excitable boundary blades. |
| `holon-debug frame-diff <img1> <img2>` | Pixel-level difference analysis between video frames (`--threshold 16`). |
| `holon-debug export <path.cc> <out_dir>` | Extract atlas PNG, HUD PNG, sprite sheets, and JSON manifest to disk. |

---

## 2. Overview Structure

When running `holon-debug overview <model.cc>`, the tool produces a structured 7-part report:

```text
===================================================================================================
 HOLONENGINE MODEL OVERVIEW: <path.cc> (CC2D v1, <size>)
===================================================================================================

--- 1. RECORDING & WORLD CONFIGURATION
  Source Stream:   Input video path, time range, duration, and sampling FPS.
  World Regime:    Inferred regime (Terrestrial 2D, Planar Static Arena, or Planar Dynamic Flight).
  Viewport:        Playfield dimensions in pixels.
  World Atlas:     Global composite panorama dimensions and coordinate origin.
  Tile Lattice:    Tile size in px and grid column/row counts.
  Backdrop:        Dominant background color (RGB and Hex).
  HUD Strip:       HUD bar placement (y threshold and height) or full-screen indication.
  Ground Plane:    Dominant ground height and corresponding lattice row (terrestrial worlds).

--- 2. ACTIVE INFORMATION & ENVIRONMENTAL PRIORS
  Gravity Field:   Unbiased vertical free-fall acceleration (g_y in px/frame²).
  Environmental:   Environmental drift acceleration vector [ax, ay].
  Control Motor:   Learned motor basis (lateral drive speed and launch impulse J_⊥).
  Camera Odometry: Total recorded frame offsets and scrolling behavior (static, vertical, bidirectional).

--- 3. IMPLICATE PREGEOMETRY & BOUNDARIES
  Lattice Breakdown: Compact visual bar and percentage breakdown:
                     • Empty (Passable medium)
                     • Solid (Impassable terrain)
                     • One-Way (Top-landing platforms)
                     • Background (Non-collidable scenery)
  Geometric Blades:  Count of merged solid runs and one-way surfaces.
  Boundary Blades:   Stationary boundary holons (B_k) split into excitable (ε0→ε1→ε2) vs rigid.
  Field Excitations: Consumable localized items and collectibles (e.g., maze pellets).

--- 4. PRIMARY GUIDED HOLON (PLAYER AVATAR)
  Track Index:     Identifier (e.g., H00) and classification kind.
  Lifespan:        Active frame count, percentage of session, and frame index span.
  Spawn Origin:    Initial world coordinates (x, y).
  Physical BBox:   Bounding box dimensions (w x h).
  Observed Span:   Excursion envelope (ΔX x ΔY) and percentage of atlas width traversed.
  Characteristic:  Mean velocity vector [vx, vy] in px/frame.
  Chromatic Spinor: Intrinsic RGB color signature and Hex representation.
  Visual Kernels:  DC mode sprite ID (appearance) and 1st CFT harmonic sprite ID (vitality/walk).

--- 5. AUTONOMOUS PROCESS HOLONS
  Formatted table listing all autonomous environmental entities (H_i):
  • ID:             Holon index (e.g., H01, H02).
  • Inferred Role:  Behavioral classification (Terrestrial Patrol, Planar Maze Roamer, Frontal Interceptor).
  • Spawn:          Initial spawn coordinates.
  • Span:           Observed world-space excursion (ΔX x ΔY).
  • Vel:            Characteristic velocity vector [vx, vy].
  • Lifespan:       Active frame count and time window.
  • Signature:      Intrinsic color hex code.
  • Sprites:        Associated base and harmonic sprite IDs.

--- 6. VISUAL MEMORY & CANONICAL SPRITES
  Sprite Library:  Total compiled sprites, DC mode count, and CFT harmonic mode count.
  Dimensions:      Width and height ranges.
  Alpha Keying:    Aggregate pixel opacity breakdown (opaque, semi-transparent, keyed transparent).
  Visual Assets:   Pixel surface area for atlas and HUD components.

--- 7. DEEP-DIVE INSPECTION COMMANDS
  Command shortcuts for drilling down into tracks, sprites, collisions, export, or unfolding simulation.
===================================================================================================
```

---

## 3. Benchmark Examples

### Super Mario Bros. 3 (`smb3.cc`)
- **Regime**: `Terrestrial 2D (Gravity-bound side-scroller)` ($g_y \approx 0.3504\text{ px/frame}^2$, launch impulse $J_{\perp} \approx 6.20\text{ px/frame}$)
- **Odometry**: Bidirectional horizontal scroll across 744 px.
- **Boundaries**: 16 boundary blades (7 excitable mystery/brick blocks, 9 rigid ground/pipe boundaries).
- **Holons**: Primary Mario holon (H00) + 7 autonomous patrol holons (Goombas, Koopas, Piranha plant).

### Pac-Man (`pacman.cc`)
- **Regime**: `Planar Static Arena (Zero-G closed maze)` ($g = 0.0$, static camera drift $\le 4\text{ px}$)
- **Boundaries**: 538 rigid maze boundary blades + 133 collectible field excitations (pellets/energizers).
- **Holons**: Primary Pac-Man holon (H00, yellow `#CAC71A`) + 4 planar maze roamers (Clyde, Inky, Pinky, Blinky).

### Raptor: Call of the Shadows (`raptor.cc`)
- **Regime**: `Planar Dynamic Flight (Vertical-scrolling arena)` ($g = 0.0$, camera $\Delta y = 275\text{ px}$)
- **Holons**: Primary player jet (H00) + 8 airborne holons (frontal interceptors, forward escorts, flanking fighters).
