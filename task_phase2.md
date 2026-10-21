# Milestone: Phase 2 — 2D Side-Scroller Compiler & Playable SMB3 Benchmark

**Active:** playable-run polish — player sprite rendering and actor tracks.
Tile classification, the `.cc` model and the `enfold | inspect | unfold`
pipeline are done and gated by tests.

- [X] Fix workspace manifest (remove missing storybook crates)
- [X] `cc_field/src/video_io.rs`
  - [X] ffmpeg raw RGB24 frame reader (`FrameReader`, ranged reads)
  - [X] MP4 writer (libx264)
  - [X] PPM/PNG capture helpers
- [X] `cc_field/src/enfold2d.rs`
  - [X] `partition_hud_and_viewport` (detected top = 193 on 256x240)
  - [X] `detect_playfield_segment` (auto start ≈ 19.0 s)
  - [X] `ScrollEstimator` (FFT cross-correlation; PHAT variant kept for diagnostics only)
  - [X] Odometry validation: pink-box world-x tracker holds 248–256 px over 630 frames
  - [X] `build_2d_world_atlas` (per-pixel colour mode over quantized RGB)
  - [X] Generic tile classifier (`TileOptions`, `learn_background`,
        `extract_2d_tiles_with`)
    - [X] learned backdrop, `bg_tol`
    - [X] per-column ground surface, lattice origin from the ground line
    - [X] one-tile shelves + gap bridging under decorations
    - [X] solids at/below the local surface
    - [X] flat floating components → one-way tops; irregular ones → background
    - [X] all SMB3 colour constants deleted (`BOX_PALETTE`, `is_pipe_green`,
          `is_ground_*`)
  - [O] ghost repair from actor tracks — ±1 SSAD offset refinement + 2-tile
        size prior; residual mask still splits the player by pose
  - [O] `track_actors` / `classify_tracks` — 12 sane tracks (10–24 px), no
        reliable Player class; camera anchor is the useful cue
  - [ ] canonical 32x32 sprite + 2D+t CFT kernel
- [X] `.cc` model format (`cc_field/src/model2d.rs`, CC2D v1)
  - [X] header + JSON manifest + PNG atlas + PNG HUD + sprite PNGs
  - [X] `Model2D::{from_enfolded,to_bytes,from_bytes,save,load,kind,row_y,surface_y}`
  - [X] unit tests: roundtrip, format rejection, surface lookup
- [X] `cc_cli` crate + `cc` binary
  - [X] `cc enfold <video> -o model.cc [--start S] [--duration S]`
  - [X] `cc inspect model.cc` (+ ASCII grid, `--atlas/--tiles/--hud` dumps)
  - [X] `cc unfold model.cc [--frames N] [--capture DIR] [-o out.mp4]`
  - [X] scrolling atlas renderer + static HUD composite
  - [X] gravity + tile collision + one-way platforms
  - [X] scripted auto-run input (`--auto` is the default; trace via `CC_TRACE=1`)
- [X] Tests
  - [X] `smb3_enfold` integration: HUD fidelity, scroll↔atlas MAD, terrain
        layout (floor, shelf, pipe top), track/sprite payloads — 4 pass
  - [X] `tile_classifier` synthetic scene (non-SMB3 palette: shelf, bush,
        cloud, flat float) — 4 pass
  - [ ] synthetic ghost-removal through the full pipeline
  - [ ] playable run test (jump, one-way landing, stomp)
  - [X] headless throughput: 3–16 kHz, gate > 120 Hz
- [X] Visual verification: `/tmp/opencode/run3/frame_00220.png` shows the level,
      HUD strip and the player mid-jump over the pipe; `cc inspect --tiles`
      shows the collision grid
- [ ] Player sprite (currently a flat red box), actor rendering, Q_Cl patrols

## Exit-gate gaps (plan §Phase 2)
- [X] HUD partition: detected at y=193 (exact), HUD image matches frame 0 with
      mean error < 10 / 255.
- [~] Scroll: 722 px total, per-frame steps ≤ 4, frame-vs-atlas MAD < 16.
      Ground truth is the pink-blob tracker (248–256 px over 630 frames), not
      a per-frame displacement trace.
- [~] Atlas ghost freedom: mode atlas + refined registration; clouds and the
      player are removed, but the pipe body / box interiors keep single-frame
      blends where the mode is ambiguous.
- [ ] Playable gate: run + one-way landings work in `cc unfold`; `?`-block
      bumps and Goomba stomps are not modelled (no solid blocks above ground,
      no entities). Decide whether to add a rules file or a block classifier.

## Next

1. Player rendering from a sprite/track (or a minimal silhouette + squash/stretch).
2. Ghost repair with a per-object median instead of the global atlas mode.
3. `play_trace.json` replay for deterministic regression runs.
4. Phase 3 gates from `docs/plan.md` (3D path) once 2D is closed.
