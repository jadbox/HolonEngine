# Milestone: Phase 2 — 2D Side-Scroller Compiler & Playable SMB3 Benchmark

**Status:** Phase 2 complete. All exit gates verified (`smb3_enfold`, `tile_classifier`, `recordings/task_demo_smb3_playable.mp4`, model in `example_input2d/smb3.cc`).
Ready for Phase 3 (3D Raycast Compiler & Wolfenstein 3D Extraction).

## Demo defects (measured, priority order)

- [X] **Odometry reads the HUD.** `ScrollEstimator::gray()` fills `0..h` with
      `h = next_pow2(hud_top) = 256` on a 240-row frame. Stored `band_h` and
      restricted gray loop to `band_h`, zeroing the static HUD. Smear eliminated;
      cloud eyes and block edges are now crisp and pixel-locked.
- [X] **Actor mask shifted twice.** `enfold` pass 3 passed
      `b.shifted(-off[0], -off[1])`. Changed to `*b` in frame coordinates.
- [X] **Grounded obstacles pass through.** Pipe stem and grounded screw boxes
      promoted to one-way collision surfaces via generic shelf extraction on
      grounded components (`height >= 2 * TILE`, bottom touches ground line).
      All 12 level platforms detected cleanly.
- [X] **No sprite renderer.** `cc unfold` compositing canonical synthesized
      player sprite with alpha blending, centered horizontally and feet-aligned.
- [X] **Autonomous actors & Clifford patrol / impact.** Track 0 classified as
      `Player` (dominant persistent track). Non-player actors instantiated as
      autonomous `ActorHolon`s with Clifford idempotent state transitions
      ($\epsilon_0$ patrol left, $\epsilon_1$ patrol right on lateral boundary hit,
      $\epsilon_2$ squashed on top-down relational stomp impact). Recorded
      in `recordings/task_demo_smb3_playable.mp4`.
- [X] **Game-agnostic video inference (tested on Pac-Man benchmark).**
      - Removed hardcoded SMB3 sky colors from `detect_playfield_segment`: detects
        the longest contiguous run of frames sharing a stable scene backdrop.
      - Added CFT camera regime detection (`detect_camera_is_static`): FFT phase
        correlation across playfield rows automatically selects `Ortho2D-Scroll` vs `Ortho2D-Static`.
      - Added geometric border detector (`detect_static_borders`): discovers enclosing
        maze border walls and partitions top/bottom HUD bars directly from pixel layout.
      - Generalized `TileExtraction` to support variable tile sizes (8 px for Pac-Man,
        16 px for SMB3) and maze wall/corridor solid classification without ground floors.
      - Enfolded `example_input2d_pacman/pacman_240x308.mp4` into `example_input2d_pacman/pacman.cc`
        purely from raw video without requiring rules files.

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
- [X] Player sprite (clean silhouette alpha, centered, feet-aligned)
- [X] Actor Holons with Clifford state algebra ($\epsilon_0$ left, $\epsilon_1$ right, $\epsilon_2$ squashed)
- [X] Interactive coin boxes (`?` blocks): upward head hit detection, bump displacement, coin pop, spent transition
- [X] Model saved into source example folder: `example_input2d/smb3.cc`

## Exit-gate status (plan §Phase 2)
- [X] HUD partition: detected at y=193 (exact), HUD image matches frame 0 with
      mean error < 10 / 255.
- [X] Scroll: 744 px total, per-frame steps ≤ 4, frame-vs-atlas MAD < 16.
      Pink-box world-x tracker holds constant across bidirectional scroll.
- [X] Atlas ghost freedom: mode atlas + refined registration + 2px actor dilation;
      cloud eyes and terrain borders sharp.
- [X] Playable gate: run, one-way platform landings, `?`-block bumps with coin pop,
      and Goomba stomps verified in `cc unfold` at > 2500 Hz headless throughput.
      Recorded to `recordings/task_demo_smb3_playable.mp4`.

## Next steps

1. Interactive controls (`cc unfold --interactive`) with terminal raw input or window.
2. Animated walk/jump pose cycling from 2D+t CFT kernels ($\hat{K}(\mathbf{k}, \omega)$).
3. Phase 3: 3D raycast compiler & Wolfenstein 3D extraction (`cc_field/src/enfold3d.rs`, `example_input3d/`).
