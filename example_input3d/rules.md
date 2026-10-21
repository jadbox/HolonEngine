# Wolfenstein 3D — Episode 1 Floor 1 Implicate World Rules

## Viewport & HUD Invariants
- The outer frame contains left/right black pillarboxes and a teal border surrounding the active 3D viewport.
- The bottom blue status bar (`FLOOR`, `SCORE`, `LIVES`, `PORTRAIT`, `HEALTH`, `AMMO`, `WEAPON`) is screen-locked and invariant under camera rotation and movement.
- The player's hand and pistol sprite is anchored at the bottom center of the 3D viewport and transitions from `idle` to `fire` (muzzle flash + recoil) when attacking.

## 3D Corridor & Architecture
- The ceiling (`#383838`) and floor (`#707070`) meet at the horizontal centerline (horizon $y_h$) of the 3D viewport.
- Walls are orthogonal vertical planes ($\mathbb{G}_{3,0,1}$ `Plane` blades) tiled with repeating 64x64 stone and wood textures.
- Cyan metal doors sit recessed between teal door jambs and slide open along their horizontal wall plane when the player activates `Use` within interaction distance.

## Autonomous Actors & Active Information ($\mathcal{Q}_{\text{Cl}}$)
- Guards patrol or stand watch until an unobstructed line-of-sight ray ($P_{\text{player}} \vee P_{\text{guard}}$) fails to intersect any wall or closed door plane.
- Alerted guards transition to `chase`, following the negative gradient of the player's Clifford Quantum Potential ($-\nabla \langle \mathcal{Q}_{\text{Cl}} \rangle_0$) around wall obstacles and pausing to fire along the line of sight.
- Firing the weapon casts a central ray $L_{\text{aim}}$; intersecting an active guard transitions the guard to `pain` and `dead` while incrementing `SCORE` and dropping ammo pickups.
- Floor pickups (food plates, ammo clips, treasure) are static billboard solitons that transfer health, ammo, or score when intersected by $P_{\text{player}}$.
