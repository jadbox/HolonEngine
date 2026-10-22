//! `cc` — HolonEngine model compiler and 2D runtime.
//!
//! ```sh
//! cc enfold example_input2d/smb3_w1_1_256x240.mp4 -o /tmp/opencode/smb3.cc
//! cc inspect /tmp/opencode/smb3.cc --atlas /tmp/opencode/atlas.png
//! cc unfold /tmp/opencode/smb3.cc --frames 600 --capture /tmp/opencode/run -o /tmp/opencode/run.mp4
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cc_field::enfold2d::{self, EnfoldOptions};
use cc_field::model2d::Model2D;
use cc_field::video_io;

const USAGE: &str = "\
cc — 2D game model compiler

USAGE:
  cc enfold <video> [-o model.cc] [--start S] [--duration S]
  cc inspect <model.cc> [--atlas out.png] [--tiles out.png]
  cc unfold <model.cc> [--frames N] [--capture DIR] [-o out.mp4] [--no-video]
";

fn main() {
    if let Err(e) = run() {
        eprintln!("cc: {e}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        print!("{USAGE}");
        return Ok(());
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "enfold" => cmd_enfold(rest),
        "inspect" => cmd_inspect(rest),
        "unfold" => cmd_unfold(rest),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command `{other}`\n{USAGE}"),
        )),
    }
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn positional(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a.starts_with('-') {
            // Options that take a value.
            if matches!(
                a.as_str(),
                "-o" | "--start" | "--duration" | "--frames" | "--capture" | "--atlas" | "--tiles"
            ) {
                skip = true;
            }
            continue;
        }
        out.push(a.as_str());
    }
    out
}

fn cmd_enfold(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(input) = pos.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "enfold needs a video path",
        ));
    };
    let out = opt(args, "-o")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("model.cc"));
    let opts = EnfoldOptions {
        start: opt(args, "--start").and_then(|s| s.parse().ok()),
        duration: opt(args, "--duration").and_then(|s| s.parse().ok()),
        max_frames: None,
        hud_top: None,
    };
    let t0 = Instant::now();
    let level = enfold2d::enfold(Path::new(input), &opts)?;
    let model = Model2D::from_enfolded(&level, input)?;
    model.save(&out)?;
    let bytes = std::fs::metadata(&out)?.len();
    println!(
        "enfolded {input}: {:.2}s video, atlas {}x{}, {} frames, ground_top={}, bg={:?}",
        level.duration,
        level.atlas.image.width,
        level.atlas.image.height,
        level.scroll.len(),
        level.tiles.ground_top,
        level.tiles.background
    );
    println!(
        "wrote {} ({bytes} bytes) in {:.2}s",
        out.display(),
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

fn kind_letter(k: cc_field::TileKind) -> char {
    match k {
        cc_field::TileKind::Empty => '.',
        cc_field::TileKind::Solid => '#',
        cc_field::TileKind::OneWay => '=',
        cc_field::TileKind::Background => 'b',
    }
}

fn cmd_inspect(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(path) = pos.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inspect needs a .cc model",
        ));
    };
    let model = Model2D::load(Path::new(path))?;
    let m = &model.manifest;
    let mut counts = [0usize; 4];
    for &k in &m.kinds {
        counts[(k as usize).min(3)] += 1;
    }
    println!("model   {path} (CC2D v{})", m.version);
    println!(
        "source  {} {:.2}s..{:.2}s @ {:.2} fps",
        m.source,
        m.start,
        m.start + m.duration,
        m.fps
    );
    println!(
        "atlas   {}x{} px, origin {:?}, tile {} px, grid {}x{}",
        m.atlas[0], m.atlas[1], m.origin, m.tile, m.cols, m.rows
    );
    println!(
        "level   viewport {}x{}, hud_top {}, ground_top y={}, drift {:?}, motor {:?}, backdrop {:?}",
        m.viewport[0],
        m.viewport[1],
        m.hud_top,
        m.ground_top,
        m.env_drift,
        m.control_motor,
        m.background
    );
    println!(
        "tiles   empty={} solid={} oneway={} background={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    println!(
        "collide {} solid runs, {} one-way surfaces, {} collectibles, {} boundary blades",
        m.solids.len(),
        m.platforms.len(),
        m.collectibles.len(),
        m.boundaries.len()
    );
    println!("frames  {} scroll samples", m.scroll.len());
    println!(
        "actors  {} tracks, {} sprites",
        m.tracks.len(),
        m.sprites.len()
    );

    // ASCII preview of the collision grid.
    let max_cols = m.cols.min(96) as usize;
    let max_rows = m.rows.min(40) as usize;
    for ty in 0..max_rows {
        let mut line = String::with_capacity(max_cols);
        for tx in 0..max_cols {
            line.push(kind_letter(model.kind(tx, ty)));
        }
        println!("  {ty:>2} {line}");
    }

    if let Some(png) = model.hud_png()? {
        println!(
            "hud     {}x{} px at y={}",
            model.hud.as_ref().map_or(0, |h| h.width),
            model.hud.as_ref().map_or(0, |h| h.height),
            m.hud_top
        );
        if let Some(p) = opt(args, "--hud") {
            std::fs::write(p, png)?;
            println!("hud png -> {p}");
        }
    }
    if let Some(p) = opt(args, "--atlas") {
        let png = model.atlas_png()?;
        std::fs::write(p, png)?;
        println!("atlas png -> {p}");
    }
    if let Some(p) = opt(args, "--tiles") {
        // Overlay tile classes on top of the atlas.
        let mut img = model.atlas.clone();
        let ts = (m.tile as usize).max(4);
        let palette = [
            [0, 0, 0],
            [255, 40, 40],
            [255, 0, 255],
            [40, 40, 255],
        ];
        for ty in 0..m.rows as usize {
            for tx in 0..m.cols as usize {
                let k = model.kind(tx, ty) as usize;
                let y0 = (m.origin_y + (ty * ts) as i32).max(0) as usize;
                for x in tx * ts..((tx + 1) * ts).min(img.width) {
                    for y in y0..(y0 + 2).min(img.height) {
                        img.set_pixel(x, y, palette[k]);
                    }
                }
            }
        }
        video_io::write_png(Path::new(p), img.width as u32, img.height as u32, &img.rgb)?;
        println!("tiles png -> {p}");
    }
    Ok(())
}

/// Stationary boundary holon $\mathcal{B}_k = (L_k, \mathsf{Y}_k, \rho_c)$ with directional
/// admittance and state-idempotent transitions ($\epsilon_0 \to \epsilon_1 \to \epsilon_2$).
struct BoundaryHolon {
    blade: cc_field::model2d::BoundaryBlade,
    cde: cc_math::CliffordDensityElement,
    impulse_timer: usize,
}

/// Mobile left-ideal holon $\mathcal{H}_i$ governed by `CliffordDensityElement`,
/// environmental drift $\mathbf{a}_{\text{env}}$, and $\mathcal{Q}_{\text{Cl}}$ quantum torque.
struct MobileHolon {
    is_primary: bool,
    pos: [f32; 2],
    vel: [f32; 2],
    size: [f32; 2],
    speed: f64,
    bounds_y: [f32; 2],
    on_support: bool,
    phase: f32,
    cde: cc_math::CliffordDensityElement,
    sprite_idx: Option<usize>,
    harmonic_idx: Option<usize>,
    trajectory: std::collections::HashMap<usize, (i32, i32)>,
    spawn_delay: usize,
    respawn_timer: usize,
}

/// Consumable or emitted localized field excitation in the medium.
struct FieldExcitation {
    rect: [i32; 4],
    pos: [f32; 2],
    vel: [f32; 2],
    color: [u8; 3],
    timer: usize,
    anchored: bool,
    active: bool,
}

/// Attached trailing wake emanating from $-\mathbf{s}_i$.
struct AttachedWake {
    pos: [f32; 2],
    vel: [f32; 2],
    color: [u8; 3],
    timer: usize,
    max_timer: usize,
    radius: f32,
}

/// Coherent directional soliton propagating along $+\mathbf{s}_i$.
struct SolitonProjectile {
    pos: [f32; 2],
    vel: [f32; 2],
    color: [u8; 3],
}

/// Radial dispersion burst produced upon holon state collapse $T_{s \to 2}$.
struct RadialDispersion {
    pos: [f32; 2],
    timer: usize,
    max_timer: usize,
    radius: f32,
    color: [u8; 3],
}

/// Unified continuous 2D Implicate World runtime state.
///
/// Steps terrestrial ($a_y > 0$), planar static ($a_y = 0, \Delta \mathbf{X}_{\text{cam}} = 0$),
/// and vertical/scrolling flight ($a_y = 0, \Delta \mathbf{X}_{\text{cam}} \neq 0$) worlds
/// through one continuous dynamical update without per-game branching or procedural pixel shaders.
struct WorldState2D {
    atlas_rgb: Vec<u8>,
    aw: usize,
    ah: usize,
    vw: usize,
    vh: usize,
    env_drift: [f32; 2],
    control_motor: [f32; 2],
    scroll_span: [i32; 2],
    qfield: cc_field::QField,
    basis: cc_math::StateBasis,
    boundaries: Vec<BoundaryHolon>,
    holons: Vec<MobileHolon>,
    excitations: Vec<FieldExcitation>,
    wakes: Vec<AttachedWake>,
    projectiles: Vec<SolitonProjectile>,
    dispersions: Vec<RadialDispersion>,
    cam: [usize; 2],
    impacts: usize,
    excitations_absorbed: usize,
}

impl WorldState2D {
    fn new(model: &Model2D) -> Self {
        let m = &model.manifest;
        let vw = m.viewport[0].max(1) as usize;
        let vh = m.viewport[1].max(1) as usize;
        let aw = model.atlas.width.max(1);
        let ah = model.atlas.height.max(1);

        let ay = if m.env_drift[1] > 0.0 {
            m.env_drift[1]
        } else if m.gravity > 0.0 {
            m.gravity * 0.73
        } else {
            0.0
        };
        let env_drift = [m.env_drift[0], ay];
        let drive_speed = if m.control_motor[0] > 0.0 {
            m.control_motor[0]
        } else {
            1.6
        };
        let launch_impulse = if m.control_motor[1] > 0.0 {
            m.control_motor[1]
        } else if ay > 0.0 {
            (2.0 * ay * 54.0).sqrt()
        } else {
            0.0
        };
        let control_motor = [drive_speed, launch_impulse];

        let min_sx = m.scroll.iter().map(|s| s[0]).min().unwrap_or(0);
        let max_sx = m.scroll.iter().map(|s| s[0]).max().unwrap_or(0);
        let min_sy = m.scroll.iter().map(|s| s[1]).min().unwrap_or(0);
        let max_sy = m.scroll.iter().map(|s| s[1]).max().unwrap_or(0);
        let scroll_span = [max_sx - min_sx, max_sy - min_sy];

        let raw_blades = if m.boundaries.is_empty() {
            model.infer_boundary_blades()
        } else {
            m.boundaries.clone()
        };
        let boundaries: Vec<BoundaryHolon> = raw_blades
            .into_iter()
            .map(|b| BoundaryHolon {
                cde: cc_math::CliffordDensityElement::new(1.0, 0.0, 0, 3, b.frame_color),
                blade: b,
                impulse_timer: 0,
            })
            .collect();

        let mut excitations: Vec<FieldExcitation> = m
            .collectibles
            .iter()
            .map(|&r| FieldExcitation {
                rect: r,
                pos: [(r[0] + r[2]) as f32 * 0.5, (r[1] + r[3]) as f32 * 0.5],
                vel: [0.0, 0.0],
                color: [252, 200, 48],
                timer: usize::MAX,
                anchored: true,
                active: true,
            })
            .collect();

        let mut holons: Vec<MobileHolon> = Vec::new();
        let vertical_corridor = ay <= 0.0 && scroll_span[1] > scroll_span[0] && scroll_span[1] > 8;
        let planar_static = ay <= 0.0 && scroll_span[0] <= 4 && scroll_span[1] <= 4;

        // Instantiate primary holon
        let primary_track = m
            .tracks
            .iter()
            .find(|t| t.kind == "player")
            .or_else(|| m.tracks.first());
        if let Some(pt) = primary_track {
            let (pw, ph) = if ay > 0.0 {
                (
                    (pt.size[0] - 4).max(10) as f32,
                    (pt.size[1] - 2).max(14) as f32,
                )
            } else {
                (pt.size[0].max(8) as f32, pt.size[1].max(8) as f32)
            };
            let (sx, sy) = if ay > 0.0 {
                (32.0f32, (m.ground_top as f32 - ph - 1.0).max(0.0))
            } else if vertical_corridor {
                ((vw as f32) * 0.5 - pw * 0.5, (vh as f32) - ph - 24.0)
            } else {
                (pt.spawn[0] as f32, pt.spawn[1] as f32)
            };
            let trajectory: std::collections::HashMap<usize, (i32, i32)> = pt
                .trajectory
                .iter()
                .map(|p| (p[0] as usize, (p[1], p[2])))
                .collect();
            let theta0 = if vertical_corridor {
                -std::f64::consts::FRAC_PI_2
            } else {
                0.0
            };
            holons.push(MobileHolon {
                is_primary: true,
                pos: [sx, sy],
                vel: [0.0, 0.0],
                size: [pw, ph],
                speed: drive_speed as f64,
                bounds_y: [0.0, ah as f32],
                on_support: false,
                phase: 0.0,
                cde: cc_math::CliffordDensityElement::new(1.0, theta0, 0, 3, pt.color),
                sprite_idx: pt.sprite.map(|i| i as usize),
                harmonic_idx: pt.harmonic_sprite.map(|i| i as usize),
                trajectory,
                spawn_delay: 0,
                respawn_timer: 0,
            });
        } else if ay > 0.0 {
            holons.push(MobileHolon {
                is_primary: true,
                pos: [32.0, (m.ground_top as f32 - 25.0).max(0.0)],
                vel: [0.0, 0.0],
                size: [12.0, 24.0],
                speed: drive_speed as f64,
                bounds_y: [0.0, ah as f32],
                on_support: true,
                phase: 0.0,
                cde: cc_math::CliffordDensityElement::new(1.0, 0.0, 0, 3, [240, 80, 60]),
                sprite_idx: None,
                harmonic_idx: None,
                trajectory: std::collections::HashMap::new(),
                spawn_delay: 0,
                respawn_timer: 0,
            });
        }

        // Instantiate autonomous holons
        let mut auto_idx = 0usize;
        for (ti, t) in m.tracks.iter().enumerate() {
            if (t.kind == "player" && ti == 0) || (ay > 0.0 && t.kind == "player") {
                continue;
            }
            if ay > 0.0 && t.spawn[0] <= 0 {
                continue;
            }
            if vertical_corridor && t.sprite.is_none() {
                continue;
            }
            let vertical_mode = vertical_corridor || t.span[1] > 2 * t.span[0].max(1);
            let theta0 = if vertical_mode {
                -std::f64::consts::FRAC_PI_2
            } else if ay > 0.0 {
                std::f64::consts::PI
            } else {
                0.0
            };
            let hw = t.size[0].max(8) as f32;
            let hh = t.size[1].max(8) as f32;
            let spawn_x = if vertical_corridor {
                if t.spawn[0] > 0 && (t.spawn[0] as usize) < vw {
                    t.spawn[0] as f32
                } else {
                    40.0 + ((auto_idx * 67) % (vw.saturating_sub(80))).max(10) as f32
                }
            } else {
                t.spawn[0] as f32
            };
            let spawn_y = if vertical_corridor {
                -50.0 - (auto_idx as f32) * 45.0
            } else if ay > 0.0 && !vertical_mode {
                let center_x = spawn_x + hw * 0.5;
                let feet_est = t.spawn[1] as f32 + hh;
                let mut best_surface: Option<f32> = None;
                for b in &boundaries {
                    if b.blade.permeability == [0.0, 0.0, 0.0, 0.0]
                        && (center_x as i32) >= b.blade.rect[0]
                        && (center_x as i32) <= b.blade.rect[2]
                    {
                        let top = b.blade.rect[1] as f32;
                        if (top - feet_est).abs() <= 24.0 {
                            best_surface = Some(best_surface.map_or(top, |cur| cur.min(top)));
                        }
                    }
                }
                if best_surface.is_none() {
                    for b in &boundaries {
                        if b.blade.permeability[2] == 0.0
                            && (center_x as i32) >= b.blade.rect[0]
                            && (center_x as i32) <= b.blade.rect[2]
                        {
                            let top = b.blade.rect[1] as f32;
                            if (top - feet_est).abs() <= 24.0 {
                                best_surface = Some(best_surface.map_or(top, |cur| cur.min(top)));
                            }
                        }
                    }
                }
                best_surface.map_or(t.spawn[1] as f32, |top| top - hh)
            } else {
                t.spawn[1] as f32
            };
            let amp_y = (t.span[1].max(16)) as f32;
            let trajectory: std::collections::HashMap<usize, (i32, i32)> = if planar_static {
                t.trajectory
                    .iter()
                    .map(|p| (p[0] as usize, (p[1], p[2])))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };
            let speed = if vertical_corridor {
                1.2 + ((auto_idx % 3) as f64) * 0.35
            } else if vertical_mode {
                0.35
            } else if ay > 0.0 {
                0.5
            } else {
                1.0
            };
            let spawn_delay = if vertical_corridor {
                auto_idx * 35
            } else if planar_static {
                t.trajectory.first().map(|p| p[0].max(0) as usize).unwrap_or(0)
            } else {
                0
            };
            holons.push(MobileHolon {
                is_primary: false,
                pos: [spawn_x, spawn_y],
                vel: [0.0, 0.0],
                size: [hw, hh],
                speed,
                bounds_y: [spawn_y - amp_y, spawn_y],
                on_support: false,
                phase: 0.0,
                cde: cc_math::CliffordDensityElement::new(1.0, theta0, 0, 3, t.color),
                sprite_idx: t.sprite.map(|i| i as usize),
                harmonic_idx: t.harmonic_sprite.map(|i| i as usize),
                trajectory,
                spawn_delay,
                respawn_timer: 0,
            });
            auto_idx += 1;
        }

        // Ensure excitations list is ready for runtime emissions
        excitations.reserve(16);

        Self {
            atlas_rgb: model.atlas.rgb.clone(),
            aw,
            ah,
            vw,
            vh,
            env_drift,
            control_motor,
            scroll_span,
            qfield: model.build_continuous_qfield(64),
            basis: cc_math::StateBasis::new(3),
            boundaries,
            holons,
            excitations,
            wakes: Vec::new(),
            projectiles: Vec::new(),
            dispersions: Vec::new(),
            cam: [0, 0],
            impacts: 0,
            excitations_absorbed: 0,
        }
    }

    /// Advance the implicate world state by one tick `frame` under environmental
    /// drift $\mathbf{a}_{\text{env}}$, boundary admittance $\mathsf{Y}_k$, and
    /// $\mathcal{Q}_{\text{Cl}}$ quantum torque.
    fn step(&mut self, model: &Model2D, frame: usize) {
        let m = &model.manifest;
        let ay = self.env_drift[1];
        let drive_speed = self.control_motor[0];
        let launch_impulse = self.control_motor[1];
        let vertical_corridor =
            ay <= 0.0 && self.scroll_span[1] > self.scroll_span[0] && self.scroll_span[1] > 8;
        let planar_static = ay <= 0.0 && self.scroll_span[0] <= 4 && self.scroll_span[1] <= 4;

        // 1. Step stationary boundary holons (impulse relaxation T_{1 -> 2}).
        for b in &mut self.boundaries {
            if b.impulse_timer > 0 {
                b.impulse_timer -= 1;
                if b.impulse_timer == 0 {
                    let _ = b.cde.transition_to(2, &self.basis);
                }
            }
        }

        // 2. Step secondary field excitations, wakes, solitons, and radial dispersions.
        for w in &mut self.wakes {
            w.pos[0] += w.vel[0];
            w.pos[1] += w.vel[1];
            w.timer = w.timer.saturating_sub(1);
        }
        self.wakes.retain(|w| w.timer > 0);

        for p in &mut self.projectiles {
            p.pos[0] += p.vel[0];
            p.pos[1] += p.vel[1];
        }
        self.projectiles
            .retain(|p| p.pos[1] > -10.0 && p.pos[1] < self.vh as f32 + 10.0);

        for tok in &mut self.excitations {
            if !tok.anchored && tok.active {
                tok.pos[0] += tok.vel[0];
                tok.pos[1] += tok.vel[1];
                if ay > 0.0 {
                    tok.vel[1] += 0.25;
                }
                tok.timer = tok.timer.saturating_sub(1);
                if tok.timer == 0 || tok.pos[1] > self.ah.max(self.vh) as f32 + 16.0 {
                    tok.active = false;
                }
            }
        }
        self.excitations.retain(|tok| tok.active);

        for d in &mut self.dispersions {
            d.timer = d.timer.saturating_sub(1);
        }
        self.dispersions.retain(|d| d.timer > 0);

        // 3. Step primary holon (index 0) and autonomous holons.
        let mut prev_primary_feet = 0.0f32;
        if let Some(primary) = self.holons.first() {
            prev_primary_feet = primary.pos[1] + primary.size[1];
        }

        if ay > 0.0 {
            // Terrestrial drift regime: geodesic forward drive + boundary-probed launch motor.
            let mut hit_boundary: Option<usize> = None;
            if let Some(p) = self.holons.first() {
                let px = p.pos[0];
                let py = p.pos[1];
                let pw = p.size[0];
                let ph = p.size[1];
                let ahead = px + pw + 12.0;
                let foot_y = py + ph + 2.0;
                let body_y = py + ph - 8.0;
                let wall = model.solid_at_world(ahead, body_y).is_some();
                let elevated_surface = (1..=4).any(|k| {
                    let probe_y = foot_y - k as f32 * 16.0;
                    model.one_way_at_world(ahead, probe_y).is_some()
                        && !self.boundaries.iter().any(|b| {
                            b.blade.excitable
                                && b.cde.state() == 0
                                && self.excitations_absorbed == 0
                                && ahead.floor() as i32 >= b.blade.rect[0]
                                && ahead.floor() as i32 <= b.blade.rect[2]
                                && probe_y.floor() as i32 >= b.blade.rect[1]
                                && probe_y.floor() as i32 <= b.blade.rect[3]
                        })
                });
                let gap = model.solid_at_world(ahead, foot_y).is_none()
                    && model.one_way_at_world(ahead, foot_y).is_none();
                let center_x = (px + pw * 0.5).floor() as i32;
                let foot_yi = foot_y.floor() as i32;
                let under_excitable = p.on_support
                    && self.excitations_absorbed == 0
                    && self.boundaries.iter().any(|b| {
                        b.blade.excitable
                            && b.cde.state() == 0
                            && center_x >= b.blade.rect[0]
                            && center_x <= b.blade.rect[2]
                            && b.blade.rect[3] < foot_yi
                            && (foot_yi - b.blade.rect[1]) <= 68
                    });
                let approaching_excitable = self.excitations_absorbed == 0
                    && self.boundaries.iter().any(|b| {
                        b.blade.excitable
                            && b.cde.state() == 0
                            && b.blade.rect[0] > center_x
                            && (b.blade.rect[0] - center_x) <= 48
                            && b.blade.rect[3] < foot_yi
                            && (foot_yi - b.blade.rect[1]) <= 68
                    });
                let actor_ahead = !approaching_excitable
                    && self.holons.iter().skip(1).any(|a| {
                        a.cde.state() != 2
                            && (a.pos[0] - (px + pw)) > 40.0
                            && (a.pos[0] - (px + pw)) < 62.0
                            && (a.pos[1] + a.size[1] - (py + ph)).abs() < 24.0
                    });
                let launch = p.on_support
                    && (wall
                        || elevated_surface
                        || actor_ahead
                        || under_excitable
                        || (gap && frame % 3 == 0));

                if let Some(p) = self.holons.first_mut() {
                    p.vel[0] = drive_speed;
                    p.pos[0] += p.vel[0];
                    p.phase += p.vel[0].abs() * 0.24;
                    let lead = p.pos[0] + p.size[0] - 1.0;
                    let feet = p.pos[1] + p.size[1];
                    for sample_y in [feet - 2.0, feet - 8.0, feet - 14.0] {
                        if let Some(r) = model.solid_at_world(lead, sample_y) {
                            p.pos[0] = r[0] as f32 - p.size[0];
                            break;
                        }
                    }
                    p.pos[0] = p.pos[0].max(0.0);

                    if launch && p.on_support {
                        p.vel[1] = -launch_impulse;
                        p.on_support = false;
                    }
                    p.vel[1] = (p.vel[1] + ay).min(6.5);
                    let prev_feet = p.pos[1] + p.size[1];
                    p.pos[1] += p.vel[1];
                    p.on_support = false;

                    if p.vel[1] >= 0.0 {
                        let feet = p.pos[1] + p.size[1];
                        let mut landed_top: Option<f32> = None;
                        for sample_x in
                            [p.pos[0], p.pos[0] + p.size[0] * 0.5, p.pos[0] + p.size[0] - 1.0]
                        {
                            if let Some(r) = model.solid_at_world(sample_x, feet) {
                                let top = r[1] as f32;
                                let h = (r[3] - r[1] + 1).max(4) as f32;
                                if feet > top && prev_feet <= top + h {
                                    landed_top = Some(landed_top.map_or(top, |t| t.min(top)));
                                }
                            } else if let Some(r) = model.one_way_at_world(sample_x, feet) {
                                let top = r[1] as f32;
                                if feet > top && prev_feet <= top + 2.0 {
                                    landed_top = Some(landed_top.map_or(top, |t| t.min(top)));
                                }
                            }
                        }
                        if let Some(top) = landed_top {
                            p.pos[1] = top - p.size[1];
                            p.vel[1] = 0.0;
                            p.on_support = true;
                        }
                    } else {
                        let head_y = p.pos[1];
                        let mut hit_bottom: Option<f32> = None;
                        for sample_x in
                            [p.pos[0], p.pos[0] + p.size[0] * 0.5, p.pos[0] + p.size[0] - 1.0]
                        {
                            if let Some(r) = model.solid_at_world(sample_x, head_y) {
                                hit_bottom = Some((r[3] + 1) as f32);
                                break;
                            }
                            let sx = sample_x.floor() as i32;
                            let sy = head_y.floor() as i32;
                            if let Some((bi, b)) =
                                self.boundaries.iter().enumerate().find(|(_, b)| {
                                    b.blade.permeability[3] == 0.0
                                        && sx >= b.blade.rect[0]
                                        && sx <= b.blade.rect[2]
                                        && sy >= b.blade.rect[1]
                                        && sy <= b.blade.rect[3]
                                })
                            {
                                hit_boundary = Some(bi);
                                hit_bottom = Some((b.blade.rect[3] + 1) as f32);
                                break;
                            }
                        }
                        if let Some(bot) = hit_bottom {
                            p.pos[1] = bot;
                            p.vel[1] = 0.0;
                        }
                    }
                }
            }

            if let Some(bi) = hit_boundary {
                if let Some(b) = self.boundaries.get_mut(bi) {
                    if b.blade.excitable && b.cde.state() == 0 {
                        let _ = b.cde.transition_to(1, &self.basis);
                        b.impulse_timer = 8;
                        self.excitations_absorbed += 1;
                        self.excitations.push(FieldExcitation {
                            rect: b.blade.rect,
                            pos: [b.blade.rect[0] as f32 + 4.0, b.blade.rect[1] as f32 - 4.0],
                            vel: [0.0, -3.5],
                            color: [252, 200, 40],
                            timer: 16,
                            anchored: false,
                            active: true,
                        });
                    }
                }
            }

            // Step autonomous terrestrial holons
            let (px, py, pw, ph, pvy) = self
                .holons
                .first()
                .map(|p| (p.pos[0], p.pos[1], p.size[0], p.size[1], p.vel[1]))
                .unwrap_or((0.0, 0.0, 12.0, 24.0, 0.0));
            let view_right = px + self.vw as f32;
            let mut new_impacts = 0usize;
            let mut bounce_player = false;

            for a in self.holons.iter_mut().skip(1) {
                if a.cde.state() == 2 {
                    continue;
                }
                let [vx_f64, vy_f64] = a.cde.bohm_momentum(a.speed);
                if vy_f64.abs() > vx_f64.abs() {
                    let vy = vy_f64 as f32;
                    if a.pos[1] >= a.bounds_y[1] - 1.0 && (px - a.pos[0]).abs() <= a.size[0] * 1.25
                    {
                        continue;
                    }
                    a.pos[1] = (a.pos[1] + vy).clamp(a.bounds_y[0], a.bounds_y[1]);
                    a.phase += vy.abs() * 0.25;
                    if a.pos[1] <= a.bounds_y[0] {
                        a.cde.reflect_at_boundary([0.0, 1.0], &self.basis);
                    } else if a.pos[1] >= a.bounds_y[1] {
                        a.cde.reflect_at_boundary([0.0, -1.0], &self.basis);
                    }
                    continue;
                }
                if a.pos[0] > view_right {
                    continue;
                }
                let vx = vx_f64 as f32;
                a.pos[0] += vx;
                a.phase += vx.abs() * 0.35;

                // Surface support coupling resolved before lateral wall probe so
                // actors on raised terrain steps never probe into the step below them.
                let center_x = a.pos[0] + a.size[0] * 0.5;
                let feet = a.pos[1] + a.size[1];
                let mut surface_top: Option<f32> = None;
                for b in &self.boundaries {
                    if b.blade.permeability == [0.0, 0.0, 0.0, 0.0]
                        && (center_x as i32) >= b.blade.rect[0]
                        && (center_x as i32) <= b.blade.rect[2]
                    {
                        let top = b.blade.rect[1] as f32;
                        if feet >= top - 8.0 && feet <= top + 24.0 {
                            surface_top = Some(surface_top.map_or(top, |t| t.min(top)));
                        }
                    }
                }
                if surface_top.is_none() {
                    for b in &self.boundaries {
                        if b.blade.permeability[2] == 0.0
                            && (center_x as i32) >= b.blade.rect[0]
                            && (center_x as i32) <= b.blade.rect[2]
                        {
                            let top = b.blade.rect[1] as f32;
                            if feet >= top - 8.0 && feet <= top + 16.0 {
                                surface_top = Some(surface_top.map_or(top, |t| t.min(top)));
                            }
                        }
                    }
                }
                if let Some(top) = surface_top {
                    a.pos[1] = top - a.size[1];
                }

                let check_x = if vx > 0.0 {
                    a.pos[0] + a.size[0] + 1.0
                } else {
                    a.pos[0] - 1.0
                };
                let hit_wall = [a.pos[1] + 2.0, a.pos[1] + a.size[1] * 0.5].iter().any(|&cy| {
                    model.solid_at_world(check_x, cy).is_some()
                        || model.one_way_at_world(check_x, cy).is_some()
                });
                if hit_wall {
                    let normal = if vx > 0.0 { [-1.0, 0.0] } else { [1.0, 0.0] };
                    a.cde.reflect_at_boundary(normal, &self.basis);
                }

                // Top-down relational incidence (P_player ∧ L_actor_top = 0 with v_y > 0)
                let p_feet = py + ph;
                if pvy > 0.0
                    && p_feet >= a.pos[1]
                    && prev_primary_feet <= a.pos[1] + 6.0
                    && (px + pw) > a.pos[0]
                    && px < (a.pos[0] + a.size[0])
                {
                    let _ = a.cde.transition_to(2, &self.basis);
                    a.cde.amplitude = 0.0;
                    bounce_player = true;
                    new_impacts += 1;
                }
            }
            if bounce_player {
                if let Some(p) = self.holons.first_mut() {
                    p.vel[1] = -launch_impulse * 0.75;
                }
            }
            self.impacts += new_impacts;

            // Camera follows primary holon horizontally
            if let Some(p) = self.holons.first() {
                let max_cam = self.aw.saturating_sub(self.vw);
                self.cam[0] = ((p.pos[0] + p.size[0] * 0.5 - self.vw as f32 * 0.5).round()
                    as usize)
                    .min(max_cam);
                self.cam[1] = 0;
            }
        } else if vertical_corridor {
            // Vertical scrolling corridor regime (ay == 0, camera ego-motor along e02)
            let s_idx = frame.min(m.scroll.len().saturating_sub(1));
            let off = m.scroll.get(s_idx).copied().unwrap_or([0, 0]);
            self.cam[0] =
                (off[0] - m.origin[0]).clamp(0, self.aw.saturating_sub(self.vw) as i32) as usize;
            self.cam[1] =
                (off[1] - m.origin[1]).clamp(0, self.ah.saturating_sub(self.vh) as i32) as usize;

            if let Some(p) = self.holons.first_mut() {
                let t_sec = frame as f32 / 30.0;
                let pw = p.size[0];
                let ph = p.size[1];
                let target_px = ((self.vw as f32) * 0.5 - pw * 0.5)
                    + ((self.vw as f32) * 0.35) * (t_sec * 0.8).sin()
                    + 18.0 * (t_sec * 2.1).sin();
                let target_py = (self.vh as f32) - ph - 26.0 + 12.0 * (t_sec * 1.3).cos();
                p.pos[0] = (p.pos[0] + (target_px - p.pos[0]) * 0.12)
                    .clamp(14.0, (self.vw as f32) - pw - 14.0);
                p.pos[1] = (p.pos[1] + (target_py - p.pos[1]) * 0.12)
                    .clamp((self.vh as f32) * 0.35, (self.vh as f32) - ph - 8.0);
                p.phase += 0.35;

                // Attached trailing wake along -s
                self.wakes.push(AttachedWake {
                    pos: [
                        p.pos[0] + pw * 0.5 + ((frame as f32 * 1.7).sin() * pw * 0.22),
                        p.pos[1] + ph + 1.0,
                    ],
                    vel: [0.0, 1.8],
                    color: [140, 220, 255],
                    timer: 8,
                    max_timer: 8,
                    radius: 3.2,
                });

                // Forward soliton bifurcation along +s
                if frame % 7 == 0 {
                    for frac in [0.25f32, 0.75] {
                        self.projectiles.push(SolitonProjectile {
                            pos: [p.pos[0] + pw * frac, p.pos[1] - 2.0],
                            vel: [0.0, -8.0],
                            color: [180, 240, 255],
                        });
                    }
                }
            }

            // Step autonomous holons under Bohm momentum + Q_Cl torque
            for (hi, h) in self.holons.iter_mut().enumerate().skip(1) {
                if h.respawn_timer > 0 {
                    h.respawn_timer -= 1;
                    if h.respawn_timer == 0 {
                        let _ = h.cde.transition_to(0, &self.basis);
                        h.cde.amplitude = 1.0;
                        h.pos[1] = -40.0 - ((hi * 41) % 120) as f32;
                        h.pos[0] = 24.0
                            + ((hi * 79 + frame * 13) % (self.vw.saturating_sub(60))).max(10)
                                as f32;
                    }
                    continue;
                }
                if frame < h.spawn_delay {
                    continue;
                }
                let [vx, vy] = h.cde.bohm_momentum(h.speed);
                h.pos[0] += vx as f32;
                h.pos[1] += (vy as f32).max(1.25);
                h.phase += 0.30;

                let ux = (h.pos[0] / self.vw.max(1) as f32).clamp(0.0, 1.0) as f64;
                let uy = (h.pos[1] / self.vh.max(1) as f32).clamp(0.0, 1.0) as f64;
                let [gx, gy] = self.qfield.gradient_at(ux, uy);
                h.cde.apply_quantum_torque([-gx * 0.001, -gy * 0.001], 0.15);

                if h.pos[0] <= 16.0 {
                    h.pos[0] = 16.0;
                    h.cde.reflect_at_boundary([1.0, 0.0], &self.basis);
                } else if h.pos[0] + h.size[0] >= (self.vw as f32) - 16.0 {
                    h.pos[0] = (self.vw as f32) - 16.0 - h.size[0];
                    h.cde.reflect_at_boundary([-1.0, 0.0], &self.basis);
                }

                if h.pos[1] > self.vh as f32 + 20.0 {
                    h.pos[1] = -35.0;
                    h.pos[0] = 24.0
                        + ((hi * 71 + frame * 17) % (self.vw.saturating_sub(60))).max(10) as f32;
                }

                let mut hit = false;
                self.projectiles.retain(|p| {
                    if hit {
                        return true;
                    }
                    if p.pos[0] >= h.pos[0] - 2.0
                        && p.pos[0] <= h.pos[0] + h.size[0] + 2.0
                        && p.pos[1] >= h.pos[1]
                        && p.pos[1] <= h.pos[1] + h.size[1]
                    {
                        hit = true;
                        false
                    } else {
                        true
                    }
                });

                if hit {
                    let _ = h.cde.transition_to(2, &self.basis);
                    h.cde.amplitude = 0.0;
                    h.respawn_timer = 18;
                    self.impacts += 1;
                    let cx = h.pos[0] + h.size[0] * 0.5;
                    let cy = h.pos[1] + h.size[1] * 0.5;
                    self.dispersions.push(RadialDispersion {
                        pos: [cx, cy],
                        timer: 16,
                        max_timer: 16,
                        radius: 18.0,
                        color: [255, 180, 40],
                    });
                    self.excitations.push(FieldExcitation {
                        rect: [cx as i32 - 4, cy as i32 - 4, cx as i32 + 4, cy as i32 + 4],
                        pos: [cx, cy],
                        vel: [0.0, 0.8],
                        color: [252, 210, 40],
                        timer: 180,
                        anchored: false,
                        active: true,
                    });
                }
            }
        } else if planar_static {
            // Planar zero-drift arena: trajectory + Q_Cl quantum torque continuation
            self.cam = [0, 0];
            for h in &mut self.holons {
                if let Some(&(tx, ty)) = h.trajectory.get(&frame) {
                    let dx = tx as f32 - h.pos[0];
                    let dy = ty as f32 - h.pos[1];
                    let ds = (dx * dx + dy * dy).sqrt();
                    h.phase += ds * 0.40;
                    if ds > 0.5 && dx.abs() < (self.aw as f32 * 0.5) {
                        h.cde.set_heading_angle((dy as f64).atan2(dx as f64));
                    }
                    h.pos = [tx as f32, ty as f32];
                } else {
                    let ux = (h.pos[0] as f64 / self.aw.max(1) as f64).clamp(0.0, 1.0);
                    let uy = (h.pos[1] as f64 / self.ah.max(1) as f64).clamp(0.0, 1.0);
                    let [gx, gy] = self.qfield.gradient_at(ux, uy);
                    h.cde.apply_quantum_torque([-gx * 0.001, -gy * 0.001], 0.2);
                    let [vx, vy] = h.cde.bohm_momentum(h.speed);
                    let ds = (vx * vx + vy * vy).sqrt() as f32;
                    h.phase += ds * 0.40;
                    let nx = (h.pos[0] as f64 + vx).rem_euclid(self.aw.max(1) as f64) as f32;
                    let ny = (h.pos[1] as f64 + vy).clamp(0.0, (self.ah.saturating_sub(1)) as f64)
                        as f32;
                    let cx = nx + h.size[0] * 0.5;
                    let cy = ny + h.size[1] * 0.5;
                    if model.solid_at_world(cx, cy).is_none() {
                        h.pos = [nx, ny];
                    } else {
                        let normal = if vx.abs() >= vy.abs() {
                            [-vx.signum(), 0.0]
                        } else {
                            [0.0, -vy.signum()]
                        };
                        h.cde.reflect_at_boundary(normal, &self.basis);
                    }
                }
            }
        }

        // 4. Universal relational excitation absorption (primary holon ∩ FieldExcitation)
        if let Some(primary) = self.holons.first() {
            let pcx = primary.pos[0] + primary.size[0] * 0.5;
            let pcy = primary.pos[1] + primary.size[1] * 0.5;
            let bg = m.background;
            let radius_sq = if vertical_corridor { 500.0f32 } else { 64.0f32 };
            for tok in &mut self.excitations {
                if !tok.active || (!tok.anchored && ay > 0.0) {
                    continue;
                }
                let dx = pcx - tok.pos[0];
                let dy = pcy - tok.pos[1];
                if dx * dx + dy * dy <= radius_sq {
                    tok.active = false;
                    self.excitations_absorbed += 1;
                    if tok.anchored {
                        let r = tok.rect;
                        for y in (r[1] - 1).max(0)..=(r[3] + 1).min(self.ah as i32 - 1) {
                            for x in (r[0] - 1).max(0)..=(r[2] + 1).min(self.aw as i32 - 1) {
                                let idx = (y as usize * self.aw + x as usize) * 3;
                                self.atlas_rgb[idx] = bg[0];
                                self.atlas_rgb[idx + 1] = bg[1];
                                self.atlas_rgb[idx + 2] = bg[2];
                            }
                        }
                    }
                }
            }
            self.excitations.retain(|tok| tok.active);
        }
    }

    /// Render the current world state into `buf` (`vw * vh * 3` RGB24) using
    /// camera-transformed atlas blitting, boundary holon states, and 2D+t CFT
    /// spectral sprite evaluation.
    fn render(&self, model: &Model2D, frame: usize, buf: &mut [u8]) {
        let m = &model.manifest;
        let vw = self.vw;
        let vh = self.vh;
        let aw = self.aw;
        let ah = self.ah;
        let cam_x = self.cam[0];
        let cam_y = self.cam[1];
        let hud_top = m.hud_top as usize;
        let bg = m.background;

        // 1. Background atlas + static HUD strip
        for y in 0..vh {
            let sy = cam_y + y;
            for x in 0..vw {
                let sx = cam_x + x;
                let i = (y * vw + x) * 3;
                let src = model.hud.as_ref().and_then(|hud| {
                    let hy = y.checked_sub(hud_top)?;
                    if hy < hud.height && x < hud.width {
                        Some((hy * hud.width + x) * 3)
                    } else {
                        None
                    }
                });
                if let Some(s) = src {
                    let hud = model.hud.as_ref().unwrap();
                    buf[i] = hud.rgb[s];
                    buf[i + 1] = hud.rgb[s + 1];
                    buf[i + 2] = hud.rgb[s + 2];
                } else if sx < aw && sy < ah {
                    let s = (sy * aw + sx) * 3;
                    buf[i] = self.atlas_rgb[s];
                    buf[i + 1] = self.atlas_rgb[s + 1];
                    buf[i + 2] = self.atlas_rgb[s + 2];
                } else {
                    buf[i] = bg[0];
                    buf[i + 1] = bg[1];
                    buf[i + 2] = bg[2];
                }
            }
        }

        // 2. Stationary excitable boundary holons (drawn directly from atlas pixels + learned perimeter color)
        let pulse = ((frame as f32 * 0.25).cos() * 14.0).round() as i32;
        for b in &self.boundaries {
            if !b.blade.excitable {
                continue;
            }
            let r = b.blade.rect;
            let bx = r[0] - cam_x as i32;
            let by = r[1] - cam_y as i32;
            let bw = (r[2] - r[0] + 1).max(1);
            let bh = (r[3] - r[1] + 1).max(1);
            if bx + bw <= 0 || bx >= vw as i32 || by + bh <= 0 || by >= vh as i32 {
                continue;
            }
            if b.impulse_timer > 0 {
                let bump_dy = (-(b.impulse_timer as f32 * (8.0 - b.impulse_timer as f32)) / 3.0)
                    .round() as i32;
                for ty in 0..=bh {
                    let dy = by + ty;
                    if dy >= 0 && dy < vh as i32 {
                        for tx in 0..bw {
                            let dx = bx + tx;
                            if dx >= 0 && dx < vw as i32 {
                                let idx = (dy as usize * vw + dx as usize) * 3;
                                buf[idx] = bg[0];
                                buf[idx + 1] = bg[1];
                                buf[idx + 2] = bg[2];
                            }
                        }
                    }
                }
                for ty in 0..=bh {
                    let dy = by + bump_dy + ty;
                    let ay_src = (r[1] + ty).max(0) as usize;
                    if dy >= 0 && dy < vh as i32 && ay_src < ah {
                        for tx in 0..bw {
                            let dx = bx + tx;
                            let ax_src = (r[0] + tx).max(0) as usize;
                            if dx >= 0 && dx < vw as i32 && ax_src < aw {
                                let idx = (dy as usize * vw + dx as usize) * 3;
                                let s = (ay_src * aw + ax_src) * 3;
                                buf[idx] = self.atlas_rgb[s];
                                buf[idx + 1] = self.atlas_rgb[s + 1];
                                buf[idx + 2] = self.atlas_rgb[s + 2];
                            }
                        }
                    }
                }
            } else if b.cde.state() == 0 {
                // Active state: modulate actual atlas texture luminance via CFT idle harmonic
                for ty in 1..(bh - 1).max(1) {
                    let dy = by + ty;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    for tx in 1..(bw - 1).max(1) {
                        let dx = bx + tx;
                        if dx < 0 || dx >= vw as i32 {
                            continue;
                        }
                        let idx = (dy as usize * vw + dx as usize) * 3;
                        for c in 0..3 {
                            buf[idx + c] = (buf[idx + c] as i32 + pulse).clamp(0, 255) as u8;
                        }
                    }
                }
            } else {
                // Collapsed state (state == 2): attenuate interior toward learned perimeter frame color
                let fc = b.blade.frame_color;
                for ty in 1..(bh - 1).max(1) {
                    let dy = by + ty;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    for tx in 1..(bw - 1).max(1) {
                        let dx = bx + tx;
                        if dx < 0 || dx >= vw as i32 {
                            continue;
                        }
                        let idx = (dy as usize * vw + dx as usize) * 3;
                        buf[idx] = fc[0];
                        buf[idx + 1] = fc[1];
                        buf[idx + 2] = fc[2];
                    }
                }
            }
        }

        // 3. Attached process wakes
        for w in &self.wakes {
            let wx = w.pos[0].round() as i32;
            let wy = w.pos[1].round() as i32;
            let rad = (w.radius * (w.timer as f32 / w.max_timer as f32)).max(1.0) as i32;
            let alpha_scale = w.timer as f32 / w.max_timer as f32;
            for dy in -rad..=rad {
                let py_pix = wy + dy;
                if py_pix < 0 || py_pix >= vh as i32 {
                    continue;
                }
                for dx in -rad..=rad {
                    let px_pix = wx + dx;
                    if px_pix < 0 || px_pix >= vw as i32 {
                        continue;
                    }
                    if dx * dx + dy * dy <= rad * rad {
                        let idx = (py_pix as usize * vw + px_pix as usize) * 3;
                        for c in 0..3 {
                            buf[idx + c] = (buf[idx + c] as f32 * (1.0 - alpha_scale * 0.6)
                                + w.color[c] as f32 * alpha_scale * 0.6)
                                as u8;
                        }
                    }
                }
            }
        }

        // 4. Emitted non-anchored field excitations
        for tok in &self.excitations {
            if tok.anchored || !tok.active {
                continue;
            }
            let screen_coords = self.env_drift[1] <= 0.0 && self.scroll_span[1] > 8;
            let tx = tok.pos[0].round() as i32 - if screen_coords { 0 } else { cam_x as i32 };
            let ty = tok.pos[1].round() as i32 - if screen_coords { 0 } else { cam_y as i32 };
            let rad = 5i32;
            for dy in -rad..=rad {
                let py_pix = ty + dy;
                if py_pix < 0 || py_pix >= vh as i32 {
                    continue;
                }
                for dx in -rad..=rad {
                    let px_pix = tx + dx;
                    if px_pix < 0 || px_pix >= vw as i32 {
                        continue;
                    }
                    let dist = dx.abs() + dy.abs();
                    if dist <= rad + 1 {
                        let idx = (py_pix as usize * vw + px_pix as usize) * 3;
                        let rgb = if dist <= 1 {
                            [255u8, 255, 230]
                        } else {
                            tok.color
                        };
                        buf[idx] = rgb[0];
                        buf[idx + 1] = rgb[1];
                        buf[idx + 2] = rgb[2];
                    }
                }
            }
        }

        // 5. Mobile holons (autonomous first, then primary holon) evaluated via 2D+t CFT kernels
        let screen_space_actors =
            self.env_drift[1] <= 0.0 && self.scroll_span[1] > self.scroll_span[0] && self.scroll_span[1] > 8;
        let order = (1..self.holons.len()).chain(std::iter::once(0));
        for hi in order {
            let Some(h) = self.holons.get(hi) else {
                continue;
            };
            if h.respawn_timer > 0 || frame < h.spawn_delay {
                continue;
            }
            let ax = h.pos[0].round() as i32 - if screen_space_actors { 0 } else { cam_x as i32 };
            let ay = h.pos[1].round() as i32 - if screen_space_actors { 0 } else { cam_y as i32 };
            if ax + h.size[0] as i32 <= 0
                || ax >= vw as i32
                || ay + h.size[1] as i32 <= 0
                || ay >= vh as i32
            {
                continue;
            }
            let [hx, hy] = h.cde.heading_vector();
            let flip_x = if !h.is_primary && self.env_drift[1] > 0.0 && hx.abs() >= hy.abs() {
                let is_bilateral = (h.size[0] - h.size[1]).abs() <= 4.0;
                if is_bilateral {
                    h.cde.state() != 2 && (h.phase * 0.9).sin() < 0.0
                } else {
                    hx > 0.25
                }
            } else {
                !screen_space_actors && hx < -0.25 && hx.abs() >= hy.abs()
            };
            let eval_phase = if h.cde.state() == 2 { 0.0 } else { h.phase };
            if let Some(si) = h.sprite_idx {
                if let Some((sw_u, sh_u, rgba)) =
                    model.evaluate_sprite_cft(si, h.harmonic_idx, eval_phase, flip_x)
                {
                    let sw = sw_u as i32;
                    let sh = sh_u as i32;
                    let state = h.cde.state();
                    let render_h = if state == 2 && self.env_drift[1] > 0.0 {
                        (sh / 2).max(1)
                    } else {
                        sh
                    };
                    let sy_off = if self.env_drift[1] > 0.0 {
                        (ay + h.size[1] as i32) - render_h
                    } else {
                        ay
                    };
                    let sx_off = if h.is_primary && self.env_drift[1] > 0.0 {
                        ax + (h.size[0] as i32 - sw) / 2
                    } else {
                        ax
                    };
                    for sy in 0..render_h {
                        let dy = sy_off + sy;
                        if dy < 0 || dy >= vh as i32 {
                            continue;
                        }
                        let src_y = if state == 2 && self.env_drift[1] > 0.0 {
                            (sy * 2).min(sh - 1)
                        } else {
                            sy
                        };
                        for sx in 0..sw {
                            let dx = sx_off + sx;
                            if dx < 0 || dx >= vw as i32 {
                                continue;
                            }
                            let p_idx = ((src_y * sw + sx) * 4) as usize;
                            let alpha = rgba[p_idx + 3] as u32;
                            if alpha >= 96 {
                                let i = (dy as usize * vw + dx as usize) * 3;
                                let inv = 255 - alpha;
                                buf[i] =
                                    ((rgba[p_idx] as u32 * alpha + buf[i] as u32 * inv) / 255) as u8;
                                buf[i + 1] = ((rgba[p_idx + 1] as u32 * alpha
                                    + buf[i + 1] as u32 * inv)
                                    / 255) as u8;
                                buf[i + 2] = ((rgba[p_idx + 2] as u32 * alpha
                                    + buf[i + 2] as u32 * inv)
                                    / 255) as u8;
                            }
                        }
                    }
                    continue;
                }
            }
            if h.cde.state() != 2 {
                let c = h.cde.signature;
                let rgb = [
                    (c[0].s * 255.0).clamp(40.0, 255.0) as u8,
                    (c[1].s * 255.0).clamp(40.0, 255.0) as u8,
                    (c[2].s * 255.0).clamp(40.0, 255.0) as u8,
                ];
                for y in ay.max(0)..(ay + h.size[1] as i32).min(vh as i32) {
                    for x in ax.max(0)..(ax + h.size[0] as i32).min(vw as i32) {
                        let i = (y as usize * vw + x as usize) * 3;
                        buf[i] = rgb[0];
                        buf[i + 1] = rgb[1];
                        buf[i + 2] = rgb[2];
                    }
                }
            }
        }

        // 6. Radial dispersions
        for d in &self.dispersions {
            let bx = d.pos[0].round() as i32;
            let by = d.pos[1].round() as i32;
            let progress = 1.0 - (d.timer as f32 / d.max_timer as f32);
            let radius = (6.0 + progress * d.radius) as i32;
            for dy in -radius..=radius {
                let py_pix = by + dy;
                if py_pix < 0 || py_pix >= vh as i32 {
                    continue;
                }
                for dx in -radius..=radius {
                    let px_pix = bx + dx;
                    if px_pix < 0 || px_pix >= vw as i32 {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy;
                    let r2 = radius * radius;
                    if d2 <= r2 {
                        let idx = (py_pix as usize * vw + px_pix as usize) * 3;
                        let frac = (d2 as f32 / r2 as f32).sqrt();
                        let rgb = if frac < 0.35 {
                            [255u8, 255, 240]
                        } else if frac < 0.7 {
                            d.color
                        } else {
                            [220u8, 60, 20]
                        };
                        for c in 0..3 {
                            buf[idx + c] = ((buf[idx + c] as u32 + rgb[c] as u32) / 2) as u8;
                        }
                    }
                }
            }
        }

        // 7. Directional solitons
        for p in &self.projectiles {
            let bx = p.pos[0].round() as i32;
            let by = p.pos[1].round() as i32;
            for dy in -4i32..=4 {
                let py_pix = by + dy;
                if py_pix < 0 || py_pix >= vh as i32 {
                    continue;
                }
                for dx in -1i32..=1 {
                    let px_pix = bx + dx;
                    if px_pix < 0 || px_pix >= vw as i32 {
                        continue;
                    }
                    let idx = (py_pix as usize * vw + px_pix as usize) * 3;
                    let rgb = if dx == 0 && dy.abs() <= 2 {
                        [255u8, 255, 255]
                    } else {
                        p.color
                    };
                    buf[idx] = rgb[0];
                    buf[idx + 1] = rgb[1];
                    buf[idx + 2] = rgb[2];
                }
            }
        }
    }
}

fn cmd_unfold(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(path) = pos.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unfold needs a .cc model",
        ));
    };
    let model = Model2D::load(Path::new(path))?;
    let m = &model.manifest;
    let frames: usize = opt(args, "--frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            if m.env_drift[1] <= 0.0 && m.gravity <= 0.0 {
                m.scroll.len().max(600)
            } else {
                600
            }
        });
    let capture = opt(args, "--capture").map(PathBuf::from);
    let no_video = args.iter().any(|a| a == "--no-video");
    let out = opt(args, "-o").map(PathBuf::from);
    if let Some(dir) = &capture {
        std::fs::create_dir_all(dir)?;
    }

    let mut world = WorldState2D::new(&model);
    let mut buf = vec![0u8; world.vw * world.vh * 3];
    let mut writer = match (&out, no_video) {
        (Some(p), false) => Some(video_io::VideoWriter::create(
            p,
            world.vw as u32,
            world.vh as u32,
            30.0,
        )?),
        _ => None,
    };

    let t0 = Instant::now();
    let mut captured = 0usize;
    for frame in 0..frames {
        world.step(&model, frame);
        if writer.is_some() || capture.is_some() {
            world.render(&model, frame, &mut buf);
        }
        if let Some(w) = writer.as_mut() {
            w.write_frame(&buf)?;
        }
        if let Some(dir) = &capture {
            if frame % 10 == 0 || frame + 1 == frames {
                let p = dir.join(format!("frame_{frame:05}.png"));
                video_io::write_png(&p, world.vw as u32, world.vh as u32, &buf)?;
                captured += 1;
            }
        }
    }

    let secs = t0.elapsed().as_secs_f64();
    if let Some(w) = writer.as_mut() {
        w.finish()?;
    }
    let [px, py] = world
        .holons
        .first()
        .map(|h| h.pos)
        .unwrap_or([0.0, 0.0]);
    println!(
        "unfolded {frames} frames in {:.3}s ({:.0} Hz, unified implicate WorldState2D)",
        secs,
        frames as f64 / secs.max(1e-9)
    );
    println!(
        "primary holon ended at x={:.1} y={:.1}, impacts={}, excitations_absorbed={}",
        px, py, world.impacts, world.excitations_absorbed
    );
    if let Some(dir) = &capture {
        println!("captured {captured} frames -> {}", dir.display());
    }
    if let Some(p) = &out {
        println!("video -> {}", p.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_field::enfold2d::RgbImage8;
    use cc_field::model2d::{BoundaryBlade, ModelManifest, TrackRecord, VERSION};

    #[test]
    fn unified_world_steps_terrestrial_and_planar_regimes() {
        let atlas = RgbImage8::new(128, 128);
        let terrestrial = Model2D {
            manifest: ModelManifest {
                version: VERSION,
                source: "synth".into(),
                start: 0.0,
                duration: 2.0,
                fps: 30.0,
                hud_top: 120,
                viewport: [64, 64],
                atlas: [128, 128],
                origin: [0, 0],
                tile: 16,
                origin_y: 0,
                ground_top: 100,
                background: [0, 0, 0],
                gravity: 0.35,
                env_drift: [0.0, 0.35],
                control_motor: [1.6, 6.0],
                cols: 0,
                rows: 0,
                kinds: Vec::new(),
                scroll: vec![[0, 0]; 30],
                platforms: vec![[40, 72, 55, 87]],
                solids: vec![[0, 100, 127, 127]],
                collectibles: Vec::new(),
                boundaries: vec![
                    BoundaryBlade {
                        rect: [0, 100, 127, 127],
                        permeability: [0.0, 0.0, 0.0, 0.0],
                        excitable: false,
                        frame_color: [100, 80, 60],
                    },
                    BoundaryBlade {
                        rect: [40, 72, 55, 87],
                        permeability: [1.0, 1.0, 0.0, 0.0],
                        excitable: true,
                        frame_color: [140, 100, 50],
                    },
                ],
                sprites: Vec::new(),
                tracks: vec![TrackRecord {
                    kind: "player".into(),
                    frames: 10,
                    size: [14, 24],
                    spawn: [32, 76],
                    span: [60, 32],
                    velocity: [1.6, 0.0],
                    color: [240, 80, 80],
                    trajectory: Vec::new(),
                    sprite: None,
                    harmonic_sprite: None,
                }],
            },
            atlas,
            hud: None,
            sprites: Vec::new(),
        };
        let mut world = WorldState2D::new(&terrestrial);
        let mut buf = vec![0u8; 64 * 64 * 3];
        for f in 0..40 {
            world.step(&terrestrial, f);
            world.render(&terrestrial, f, &mut buf);
        }
        assert!(
            world.holons[0].pos[0] > 32.0,
            "primary holon should advance under control motor"
        );
    }
}
