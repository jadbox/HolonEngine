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
        "level   viewport {}x{}, hud_top {}, ground_top y={}, gravity {:.2}, backdrop {:?}",
        m.viewport[0], m.viewport[1], m.hud_top, m.ground_top, m.gravity, m.background
    );
    println!(
        "tiles   empty={} solid={} oneway={} background={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    println!(
        "collide {} solid runs, {} one-way surfaces, {} collectibles",
        m.solids.len(),
        m.platforms.len(),
        m.collectibles.len()
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

/// Primary controllable/guided actor holon in a gravitational 2D world.
/// Bounding dimensions `(w, h)` are learned directly from the compiled `.cc` manifest.
struct Player {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    vx: f32,
    vy: f32,
    phase: f32,
    on_ground: bool,
}

const MAX_FALL: f32 = 6.5;
const RUN: f32 = 1.6;
const JUMP: f32 = 6.2;

impl Player {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            vx: 0.0,
            vy: 0.0,
            phase: 0.0,
            on_ground: false,
        }
    }

    fn step(
        &mut self,
        m: &Model2D,
        block_props: &[BlockProp],
        dir: f32,
        jump: bool,
    ) -> Option<usize> {
        // Horizontal: continuous world-space check along the leading edge.
        // Sample the lower/middle body (feet - 2, feet - 8) so overhead floating blocks
        // that still have vertical clearance above the ground can be walked under.
        self.vx = dir * RUN;
        self.x += self.vx;
        self.phase += self.vx.abs() * 0.24;
        if dir != 0.0 {
            let lead = if dir > 0.0 { self.x + self.w - 1.0 } else { self.x };
            let feet = self.y + self.h;
            for sample_y in [feet - 2.0, feet - 8.0, feet - 14.0] {
                if let Some(r) = m.solid_at_world(lead, sample_y) {
                    self.x = if dir > 0.0 {
                        r[0] as f32 - self.w
                    } else {
                        (r[2] + 1) as f32
                    };
                    break;
                }
            }
        }
        self.x = self.x.max(0.0);

        // Vertical: environmental acceleration field from model manifest.
        let g = if m.manifest.gravity > 0.0 {
            m.manifest.gravity * 0.73
        } else {
            0.35
        };
        if jump && self.on_ground {
            self.vy = -JUMP;
            self.on_ground = false;
        }
        self.vy = (self.vy + g).min(MAX_FALL);
        let prev_feet = self.y + self.h;
        self.y += self.vy;
        self.on_ground = false;

        if self.vy >= 0.0 {
            let feet = self.y + self.h;
            let mut landed_top: Option<f32> = None;
            for sample_x in [self.x, self.x + self.w * 0.5, self.x + self.w - 1.0] {
                if let Some(r) = m.solid_at_world(sample_x, feet) {
                    let top = r[1] as f32;
                    let h = (r[3] - r[1] + 1).max(4) as f32;
                    if feet > top && prev_feet <= top + h {
                        landed_top = Some(landed_top.map_or(top, |t| t.min(top)));
                    }
                } else if let Some(r) = m.one_way_at_world(sample_x, feet) {
                    let top = r[1] as f32;
                    if feet > top && prev_feet <= top + 2.0 {
                        landed_top = Some(landed_top.map_or(top, |t| t.min(top)));
                    }
                }
            }
            if let Some(top) = landed_top {
                self.y = top - self.h;
                self.vy = 0.0;
                self.on_ground = true;
            }
            None
        } else {
            let head_y = self.y;
            let mut hit_prop: Option<usize> = None;
            let mut hit_bottom: Option<f32> = None;
            for sample_x in [self.x, self.x + self.w * 0.5, self.x + self.w - 1.0] {
                if let Some(r) = m.solid_at_world(sample_x, head_y) {
                    hit_bottom = Some((r[3] + 1) as f32);
                    break;
                }
                let sx = sample_x.floor() as i32;
                let sy = head_y.floor() as i32;
                if let Some((pi, p)) = block_props.iter().enumerate().find(|(_, p)| {
                    sx >= p.rect[0] && sx <= p.rect[2] && sy >= p.rect[1] && sy <= p.rect[3]
                }) {
                    hit_prop = Some(pi);
                    hit_bottom = Some((p.rect[3] + 1) as f32);
                    break;
                }
            }
            if let Some(bot) = hit_bottom {
                self.y = bot;
                self.vy = 0.0;
            }
            hit_prop
        }
    }
}

/// An autonomous entity in the 2D world model.
///
/// Behaviors are governed by Hiley's `CliffordDensityElement`
/// ($\rho_c = \Phi_L \widetilde{\Phi}_L = \rho U \epsilon_s \widetilde{U}$) and the
/// kinematic excursion invariants learned during `enfold`:
/// - State 0 ($\epsilon_0$): Active forward/outbound branch
/// - State 1 ($\epsilon_1$): Reflected return branch
/// - State 2 ($\epsilon_2$): Post-impact / collapsed branch ($\rho \to 0$)
struct ActorHolon {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    speed: f64,
    bounds_y: [f32; 2],
    cde: cc_math::CliffordDensityElement,
    sprite: Option<image::RgbaImage>,
}

/// A collectible token emitted upon boundary impact (e.g. coin pop).
struct CoinToken {
    x: f32,
    y: f32,
    vy: f32,
    timer: usize,
}

/// An interactive prop block (Bohmian process holon with discrete state transitions)
/// represented by its continuous world-space bounding rect `rect = [x0, y0, x1, y1]`:
/// - State 0: Active interactive prop (emits token on upward impact)
/// - State 1: Harmonic bumping impulse phase
/// - State 2: Spent / collapsed block (unreactive)
struct BlockProp {
    rect: [i32; 4],
    bump_timer: usize,
    state: u32,
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
    let is_static = m.gravity <= 0.0 && m.scroll.iter().all(|s| s[0] == 0 && s[1] == 0);
    let is_flight = m.gravity <= 0.0 && !m.scroll.iter().all(|s| s[0] == 0 && s[1] == 0);
    let frames: usize = opt(args, "--frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| if is_static || is_flight { m.scroll.len().max(600) } else { 600 });
    let capture = opt(args, "--capture").map(PathBuf::from);
    let no_video = args.iter().any(|a| a == "--no-video");
    let out = opt(args, "-o").map(PathBuf::from);
    if let Some(dir) = &capture {
        std::fs::create_dir_all(dir)?;
    }

    let vw = m.viewport[0] as usize;
    let vh = m.viewport[1] as usize;
    let aw = model.atlas.width;
    let ah = model.atlas.height;
    let mut buf = vec![0u8; vw * vh * 3];

    let mut writer = match (&out, no_video) {
        (Some(p), false) => Some(video_io::VideoWriter::create(p, vw as u32, vh as u32, 30.0)?),
        _ => None,
    };

    if is_static {
        struct PlanarHolon {
            pos: Option<(f64, f64)>,
            size: [f64; 2],
            cde: cc_math::CliffordDensityElement,
            trajectory: std::collections::HashMap<usize, (i32, i32)>,
            sprite: Option<image::RgbaImage>,
            phase: f32,
        }

        // Build the continuous Clifford Quantum Potential field Q_Cl = -(∇²R)/R
        // directly from world-space geometry (`solids`, `platforms`, `collectibles`),
        // requiring zero tile-grid assumptions.
        let qfield = model.build_continuous_qfield(64);
        let basis = cc_math::StateBasis::new(3);

        let mut atlas_copy = model.atlas.rgb.clone();
        let mut collectibles: Vec<([i32; 4], bool)> =
            m.collectibles.iter().map(|&r| (r, true)).collect();
        let mut holons: Vec<PlanarHolon> = m
            .tracks
            .iter()
            .map(|t| {
                let trajectory: std::collections::HashMap<usize, (i32, i32)> = t
                    .trajectory
                    .iter()
                    .map(|p| (p[0] as usize, (p[1], p[2])))
                    .collect();
                let sprite = t
                    .sprite
                    .and_then(|si| model.sprites.get(si as usize))
                    .and_then(|bytes| {
                        image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()
                    })
                    .map(|img| img.to_rgba8());
                PlanarHolon {
                    pos: None,
                    size: [t.size[0].max(8) as f64, t.size[1].max(8) as f64],
                    cde: cc_math::CliffordDensityElement::new(1.0, 0.0, 0, 3, t.color),
                    trajectory,
                    sprite,
                    phase: 0.0,
                }
            })
            .collect();

        let bg = m.background;
        let mut absorbed = 0usize;
        let t0 = Instant::now();
        let mut captured = 0usize;

        for frame in 0..frames {
            for h in &mut holons {
                if let Some(&(tx, ty)) = h.trajectory.get(&frame) {
                    if let Some((px, py)) = h.pos {
                        let dx = tx as f64 - px;
                        let dy = ty as f64 - py;
                        let ds = (dx * dx + dy * dy).sqrt() as f32;
                        h.phase += ds * 0.40;
                        if dx * dx + dy * dy > 0.25 && dx.abs() < (aw as f64 * 0.5) {
                            h.cde.set_heading_angle(dy.atan2(dx));
                        }
                    }
                    h.pos = Some((tx as f64, ty as f64));
                } else if let Some((px, py)) = h.pos {
                    // Unfold across trajectory gaps using Q_Cl quantum torque + Bohm momentum.
                    let ux = (px / aw.max(1) as f64).clamp(0.0, 1.0);
                    let uy = (py / ah.max(1) as f64).clamp(0.0, 1.0);
                    let [gx, gy] = qfield.gradient_at(ux, uy);
                    h.cde.apply_quantum_torque([-gx * 0.001, -gy * 0.001], 0.2);
                    let [vx, vy] = h.cde.bohm_momentum(1.0);
                    let ds = (vx * vx + vy * vy).sqrt() as f32;
                    h.phase += ds * 0.40;
                    let nx = (px + vx).rem_euclid(aw.max(1) as f64);
                    let ny = (py + vy).clamp(0.0, (ah.saturating_sub(1)) as f64);
                    let cx = (nx + h.size[0] * 0.5) as f32;
                    let cy = (ny + h.size[1] * 0.5) as f32;
                    if model.solid_at_world(cx, cy).is_none() {
                        h.pos = Some((nx, ny));
                    } else {
                        let normal = if vx.abs() >= vy.abs() {
                            [-vx.signum(), 0.0]
                        } else {
                            [0.0, -vy.signum()]
                        };
                        h.cde.reflect_at_boundary(normal, &basis);
                    }
                }
            }

            // Relational excitation absorption: when the primary holon intersects an
            // active collectible excitation, transition the collectible to its spent state
            // and clear its bounding box to the learned world background.
            if let Some(primary) = holons.first() {
                if let Some((px, py)) = primary.pos {
                    let pcx = px + primary.size[0] * 0.5;
                    let pcy = py + primary.size[1] * 0.5;
                    for (rect, active) in &mut collectibles {
                        if !*active {
                            continue;
                        }
                        let cx = (rect[0] + rect[2]) as f64 * 0.5;
                        let cy = (rect[1] + rect[3]) as f64 * 0.5;
                        let dx = pcx - cx;
                        let dy = pcy - cy;
                        if dx * dx + dy * dy <= 49.0 {
                            *active = false;
                            absorbed += 1;
                            for y in (rect[1] - 1).max(0)..=(rect[3] + 1).min(ah as i32 - 1) {
                                for x in (rect[0] - 1).max(0)..=(rect[2] + 1).min(aw as i32 - 1) {
                                    let idx = (y as usize * aw + x as usize) * 3;
                                    atlas_copy[idx] = bg[0];
                                    atlas_copy[idx + 1] = bg[1];
                                    atlas_copy[idx + 2] = bg[2];
                                }
                            }
                        }
                    }
                }
            }

            // Render background atlas + HUD
            let hud_top = m.hud_top as usize;
            for y in 0..vh {
                for x in 0..vw {
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
                    } else if x < aw && y < ah {
                        let s = (y * aw + x) * 3;
                        buf[i] = atlas_copy[s];
                        buf[i + 1] = atlas_copy[s + 1];
                        buf[i + 2] = atlas_copy[s + 2];
                    } else {
                        buf[i] = bg[0];
                        buf[i + 1] = bg[1];
                        buf[i + 2] = bg[2];
                    }
                }
            }

            // Render holons (autonomous actors first, then primary holon)
            let order = (1..holons.len()).chain(std::iter::once(0));
            for ti in order {
                let Some(h) = holons.get(ti) else {
                    continue;
                };
                let Some(sprite) = &h.sprite else {
                    continue;
                };
                let Some((fx, fy)) = h.pos else {
                    continue;
                };
                let ax = fx.round() as i32;
                let ay = fy.round() as i32;
                let sw = sprite.width() as i32;
                let sh = sprite.height() as i32;

                // Heading rotor and vitality harmonics
                let [hx, hy] = h.cde.heading_vector();
                let theta = (hy as f32).atan2(hx as f32);
                let moving = hx * hx + hy * hy > 0.04;
                let chomp_open = if moving {
                    ((h.phase).sin().abs()) * 0.95
                } else {
                    0.20
                };
                let cx = sw as f32 * 0.5;
                let cy = sh as f32 * 0.5;

                let eye_dx = if hx > 0.3 { 1 } else if hx < -0.3 { -1 } else { 0 };
                let eye_dy = if hy > 0.3 { 1 } else if hy < -0.3 { -1 } else { 0 };
                let skirt_shift = ((h.phase * 0.4).sin().signum().max(0.0)) as i32;

                for sy in 0..sh {
                    let dy = ay + sy;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    for sx in 0..sw {
                        let dx = ax + sx;
                        if dx < 0 || dx >= vw as i32 {
                            continue;
                        }

                        // Primary holon: dynamic wedge chomp aligned with heading rotor
                        if ti == 0 && chomp_open > 0.08 {
                            let diff_x = sx as f32 - cx;
                            let diff_y = sy as f32 - cy;
                            let dist = (diff_x * diff_x + diff_y * diff_y).sqrt();
                            if dist >= 1.5 && dist <= cx.max(cy) + 2.0 {
                                let ang = diff_y.atan2(diff_x);
                                let d_theta = (ang - theta + std::f32::consts::PI)
                                    .rem_euclid(2.0 * std::f32::consts::PI)
                                    - std::f32::consts::PI;
                                if d_theta.abs() < chomp_open {
                                    continue; // open mouth wedge
                                }
                            }
                        }

                        // Autonomous holons: skirt wave at bottom boundary
                        let sample_x = if ti > 0 && sy >= sh - 3 {
                            (sx + skirt_shift).rem_euclid(sw)
                        } else {
                            sx
                        };

                        let mut p = sprite.get_pixel(sample_x as u32, sy as u32);

                        // Autonomous holons: gaze shift according to heading
                        if ti > 0 && (eye_dx != 0 || eye_dy != 0) {
                            let is_eye = p[0] > 200 && p[1] > 200 && p[2] > 200;
                            let is_pupil = p[0] < 60 && p[1] < 60 && p[2] > 140;
                            if is_eye || is_pupil {
                                let shifted_x = (sx - eye_dx).clamp(0, sw - 1);
                                let shifted_y = (sy - eye_dy).clamp(0, sh - 1);
                                let sp = sprite.get_pixel(shifted_x as u32, shifted_y as u32);
                                if sp[0] < 60 && sp[1] < 60 && sp[2] > 140 {
                                    p = sp; // shifted pupil
                                }
                            }
                        }

                        let alpha = p[3] as u32;
                        if alpha >= 100 {
                            let i = (dy as usize * vw + dx as usize) * 3;
                            let inv = 255 - alpha;
                            buf[i] = ((p[0] as u32 * alpha + buf[i] as u32 * inv) / 255) as u8;
                            buf[i + 1] =
                                ((p[1] as u32 * alpha + buf[i + 1] as u32 * inv) / 255) as u8;
                            buf[i + 2] =
                                ((p[2] as u32 * alpha + buf[i + 2] as u32 * inv) / 255) as u8;
                        }
                    }
                }
            }

            if let Some(w) = writer.as_mut() {
                w.write_frame(&buf)?;
            }
            if let Some(dir) = &capture {
                if frame % 10 == 0 || frame + 1 == frames {
                    let p = dir.join(format!("frame_{frame:05}.png"));
                    video_io::write_png(&p, vw as u32, vh as u32, &buf)?;
                    captured += 1;
                }
            }
        }
        let secs = t0.elapsed().as_secs_f64();
        if let Some(w) = writer.as_mut() {
            w.finish()?;
        }
        println!(
            "unfolded {frames} frames in {:.3}s ({:.0} Hz, planar Q_Cl arena)",
            secs,
            frames as f64 / secs.max(1e-9)
        );
        if let Some((px, py)) = holons.first().and_then(|h| h.pos) {
            println!(
                "player ended at x={:.0} y={:.0} (world), collectibles_absorbed={absorbed}",
                px, py
            );
        }
        if let Some(dir) = &capture {
            println!("captured {captured} frames -> {}", dir.display());
        }
        if let Some(p) = &out {
            println!("video -> {}", p.display());
        }
        return Ok(());
    }

    if is_flight {
        /// An autonomous entity in the continuous spatial medium.
        struct AutonomousHolon {
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            speed: f64,
            cde: cc_math::CliffordDensityElement,
            sprite: Option<image::RgbaImage>,
            spawn_timer: usize,
            explosion_timer: usize,
        }

        /// An emitted directional soliton or projectile propagating along a heading vector.
        struct SolitonProjectile {
            pos: [f32; 2],
            vel: [f32; 2],
            color: [u8; 3],
        }

        /// An attached trailing field excitation (wake, thruster trail, or dispersion)
        /// emanating opposite to a holon's heading vector.
        struct AttachedWake {
            pos: [f32; 2],
            vel: [f32; 2],
            color: [u8; 3],
            timer: usize,
            max_timer: usize,
            radius: f32,
        }

        /// A localized consumable field excitation (prop, token, or energy node).
        struct FieldExcitation {
            pos: [f32; 2],
            vel: [f32; 2],
            color: [u8; 3],
            timer: usize,
        }

        /// A radial dispersion burst produced when a holon undergoes a state transition.
        struct RadialDispersion {
            pos: [f32; 2],
            timer: usize,
            max_timer: usize,
            radius: f32,
            color: [u8; 3],
        }

        let qfield = model.build_continuous_qfield(64);
        let basis = cc_math::StateBasis::new(3);

        let player_track = model.manifest.tracks.iter().find(|t| t.kind == "player");
        let player_sprite: Option<image::RgbaImage> = player_track.and_then(|t| {
            let si = t.sprite? as usize;
            let bytes = model.sprites.get(si)?;
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()
                .map(|img| img.to_rgba8())
        });
        let (pw, ph) = player_track
            .map(|t| (t.size[0].max(16) as f32, t.size[1].max(20) as f32))
            .unwrap_or((26.0, 36.0));

        let mut holons: Vec<AutonomousHolon> = model
            .manifest
            .tracks
            .iter()
            .filter(|t| t.kind != "player" && t.sprite.is_some())
            .enumerate()
            .map(|(idx, t)| {
                let sprite = t.sprite.and_then(|si| {
                    let bytes = model.sprites.get(si as usize)?;
                    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                        .ok()
                        .map(|img| img.to_rgba8())
                });
                let theta0 = -std::f64::consts::FRAC_PI_2;
                let cde = cc_math::CliffordDensityElement::new(1.0, theta0, 0, 3, t.color);
                let w = t.size[0].max(16) as f32;
                let h = t.size[1].max(16) as f32;
                let sx = if t.spawn[0] > 0 && (t.spawn[0] as usize) < vw {
                    t.spawn[0] as f32
                } else {
                    40.0 + ((idx * 67) % (vw.saturating_sub(80))).max(10) as f32
                };
                AutonomousHolon {
                    x: sx,
                    y: -50.0 - (idx as f32) * 45.0,
                    w,
                    h,
                    speed: 1.2 + ((idx % 3) as f64) * 0.35,
                    cde,
                    sprite,
                    spawn_timer: idx * 35,
                    explosion_timer: 0,
                }
            })
            .collect();

        let mut projectiles: Vec<SolitonProjectile> = Vec::new();
        let mut wakes: Vec<AttachedWake> = Vec::new();
        let mut excitations: Vec<FieldExcitation> = Vec::new();
        let mut dispersions: Vec<RadialDispersion> = Vec::new();
        let mut enemies_downed = 0usize;
        let mut tokens_absorbed = 0usize;
        let mut score = 0u64;

        let mut px = (vw as f32) * 0.5 - pw * 0.5;
        let mut py = (vh as f32) - ph - 24.0;
        let t0 = Instant::now();
        let mut captured = 0usize;

        for frame in 0..frames {
            // Primary holon continuous maneuvering across open airspace
            let t_sec = frame as f32 / 30.0;
            let target_px = ((vw as f32) * 0.5 - pw * 0.5)
                + ((vw as f32) * 0.35) * (t_sec * 0.8).sin()
                + 18.0 * (t_sec * 2.1).sin();
            let target_py = (vh as f32) - ph - 26.0 + 12.0 * (t_sec * 1.3).cos();
            px += (target_px - px) * 0.12;
            py += (target_py - py) * 0.12;
            px = px.clamp(14.0, (vw as f32) - pw - 14.0);
            py = py.clamp((vh as f32) * 0.35, (vh as f32) - ph - 8.0);

            // Emit trailing process wake attached to the holon's trailing boundary
            wakes.push(AttachedWake {
                pos: [
                    px + pw * 0.5 + ((frame as f32 * 1.7).sin() * pw * 0.22),
                    py + ph + 1.0,
                ],
                vel: [0.0, 1.8],
                color: [140, 220, 255],
                timer: 8,
                max_timer: 8,
                radius: 3.2,
            });

            // Emit directed soliton projectiles along forward heading
            if frame % 7 == 0 {
                projectiles.push(SolitonProjectile {
                    pos: [px + pw * 0.25, py - 2.0],
                    vel: [0.0, -8.0],
                    color: [180, 240, 255],
                });
                projectiles.push(SolitonProjectile {
                    pos: [px + pw * 0.75, py - 2.0],
                    vel: [0.0, -8.0],
                    color: [180, 240, 255],
                });
            }

            // Step attached process wakes
            for w in &mut wakes {
                w.pos[0] += w.vel[0];
                w.pos[1] += w.vel[1];
                w.timer = w.timer.saturating_sub(1);
            }
            wakes.retain(|w| w.timer > 0);

            // Step directional solitons
            for p in &mut projectiles {
                p.pos[0] += p.vel[0];
                p.pos[1] += p.vel[1];
            }
            projectiles.retain(|p| p.pos[1] > -10.0);

            // Step consumable field excitations
            for tok in &mut excitations {
                tok.pos[0] += tok.vel[0];
                tok.pos[1] += tok.vel[1];
                tok.timer = tok.timer.saturating_sub(1);
            }
            excitations.retain(|tok| tok.timer > 0 && tok.pos[1] < vh as f32 + 10.0);

            // Step radial dispersions
            for d in &mut dispersions {
                d.timer = d.timer.saturating_sub(1);
            }
            dispersions.retain(|d| d.timer > 0);

            // Camera odometry from ego motor
            let s_idx = frame.min(m.scroll.len().saturating_sub(1));
            let off = m.scroll[s_idx];
            let cam_x = (off[0] - m.origin[0]).clamp(0, aw.saturating_sub(vw) as i32) as usize;
            let cam_y = (off[1] - m.origin[1]).clamp(0, ah.saturating_sub(vh) as i32) as usize;

            // Step autonomous holons
            for (hi, h) in holons.iter_mut().enumerate() {
                if h.explosion_timer > 0 {
                    h.explosion_timer -= 1;
                    if h.explosion_timer == 0 {
                        // Re-initialize holon state upon process cycle
                        let _ = h.cde.transition_to(0, &basis);
                        h.cde.amplitude = 1.0;
                        h.y = -40.0 - ((hi * 41) % 120) as f32;
                        h.x = 24.0 + ((hi * 79 + frame * 13) % (vw.saturating_sub(60))).max(10) as f32;
                    }
                    continue;
                }
                if frame < h.spawn_timer {
                    continue;
                }

                // Bohm momentum step
                let [vx, vy] = h.cde.bohm_momentum(h.speed);
                h.x += vx as f32;
                h.y += (vy as f32).max(1.25);

                // Q_Cl quantum torque steering
                let ux = (h.x / vw.max(1) as f32).clamp(0.0, 1.0) as f64;
                let uy = (h.y / vh.max(1) as f32).clamp(0.0, 1.0) as f64;
                let [gx, gy] = qfield.gradient_at(ux, uy);
                h.cde.apply_quantum_torque([-gx * 0.001, -gy * 0.001], 0.15);

                // Boundary reflection at lateral walls
                if h.x <= 16.0 {
                    h.x = 16.0;
                    h.cde.reflect_at_boundary([1.0, 0.0], &basis);
                } else if h.x + h.w >= (vw as f32) - 16.0 {
                    h.x = (vw as f32) - 16.0 - h.w;
                    h.cde.reflect_at_boundary([-1.0, 0.0], &basis);
                }

                // Recycle at top if past bottom of viewport
                if h.y > vh as f32 + 20.0 {
                    h.y = -35.0;
                    h.x = 24.0 + ((hi * 71 + frame * 17) % (vw.saturating_sub(60))).max(10) as f32;
                }

                // Projectile incidence: T_{active -> collapsed}
                let mut hit = false;
                projectiles.retain(|p| {
                    if hit {
                        return true;
                    }
                    if p.pos[0] >= h.x - 2.0
                        && p.pos[0] <= h.x + h.w + 2.0
                        && p.pos[1] >= h.y
                        && p.pos[1] <= h.y + h.h
                    {
                        hit = true;
                        false // absorb projectile
                    } else {
                        true
                    }
                });

                if hit {
                    let _ = h.cde.transition_to(2, &basis);
                    h.cde.amplitude = 0.0;
                    h.explosion_timer = 18;
                    enemies_downed += 1;
                    score += 250;
                    dispersions.push(RadialDispersion {
                        pos: [h.x + h.w * 0.5, h.y + h.h * 0.5],
                        timer: 16,
                        max_timer: 16,
                        radius: 18.0,
                        color: [255, 180, 40],
                    });
                    excitations.push(FieldExcitation {
                        pos: [h.x + h.w * 0.5, h.y + h.h * 0.5],
                        vel: [0.0, 0.8],
                        color: [252, 210, 40],
                        timer: 180,
                    });
                }
            }

            // Primary holon contact with field excitations: absorption T_{excitation -> 0}
            let pcx = px + pw * 0.5;
            let pcy = py + ph * 0.5;
            excitations.retain(|tok| {
                let dx = pcx - tok.pos[0];
                let dy = pcy - tok.pos[1];
                if dx * dx + dy * dy <= 500.0 {
                    tokens_absorbed += 1;
                    score += 500;
                    false // absorbed
                } else {
                    true
                }
            });

            // Render world atlas
            for y in 0..vh {
                let sy = (cam_y + y).min(ah.saturating_sub(1));
                for x in 0..vw {
                    let sx = (cam_x + x).min(aw.saturating_sub(1));
                    let i = (y * vw + x) * 3;
                    let s = (sy * aw + sx) * 3;
                    buf[i] = model.atlas.rgb[s];
                    buf[i + 1] = model.atlas.rgb[s + 1];
                    buf[i + 2] = model.atlas.rgb[s + 2];
                }
            }

            // Render attached process wakes
            for w in &wakes {
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
                            let c = w.color;
                            buf[idx] = (buf[idx] as f32 * (1.0 - alpha_scale * 0.6)
                                + c[0] as f32 * alpha_scale * 0.6) as u8;
                            buf[idx + 1] = (buf[idx + 1] as f32 * (1.0 - alpha_scale * 0.6)
                                + c[1] as f32 * alpha_scale * 0.6) as u8;
                            buf[idx + 2] = (buf[idx + 2] as f32 * (1.0 - alpha_scale * 0.6)
                                + c[2] as f32 * alpha_scale * 0.6) as u8;
                        }
                    }
                }
            }

            // Render consumable field excitations
            for tok in &excitations {
                let tx = tok.pos[0].round() as i32;
                let ty = tok.pos[1].round() as i32;
                let rot = (frame * 6) % 360;
                let r_rad = (rot as f32).to_radians();
                let scale = 0.8 + 0.2 * (r_rad * 2.0).sin();
                let rad = (5.0 * scale) as i32;
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

            // Render autonomous holons
            for h in &holons {
                if h.explosion_timer > 0 || frame < h.spawn_timer {
                    continue;
                }
                let ax = h.x.round() as i32;
                let ay = h.y.round() as i32;
                if let Some(sprite) = &h.sprite {
                    let sw = sprite.width() as i32;
                    let sh = sprite.height() as i32;
                    for sy in 0..sh {
                        let dy = ay + sy;
                        if dy < 0 || dy >= vh as i32 {
                            continue;
                        }
                        for sx in 0..sw {
                            let dx = ax + sx;
                            if dx < 0 || dx >= vw as i32 {
                                continue;
                            }
                            let p = sprite.get_pixel(sx as u32, sy as u32);
                            let alpha = p[3] as u32;
                            if alpha >= 90 {
                                let i = (dy as usize * vw + dx as usize) * 3;
                                let inv = 255 - alpha;
                                buf[i] = ((p[0] as u32 * alpha + buf[i] as u32 * inv) / 255) as u8;
                                buf[i + 1] =
                                    ((p[1] as u32 * alpha + buf[i + 1] as u32 * inv) / 255) as u8;
                                buf[i + 2] =
                                    ((p[2] as u32 * alpha + buf[i + 2] as u32 * inv) / 255) as u8;
                            }
                        }
                    }
                } else {
                    for y in ay.max(0)..(ay + h.h as i32).min(vh as i32) {
                        for x in ax.max(0)..(ax + h.w as i32).min(vw as i32) {
                            let i = (y as usize * vw + x as usize) * 3;
                            buf[i] = 180;
                            buf[i + 1] = 60;
                            buf[i + 2] = 60;
                        }
                    }
                }
            }

            // Render radial dispersions (state transition energy release)
            for d in &dispersions {
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
                            buf[idx] = ((buf[idx] as u32 + rgb[0] as u32) / 2) as u8;
                            buf[idx + 1] = ((buf[idx + 1] as u32 + rgb[1] as u32) / 2) as u8;
                            buf[idx + 2] = ((buf[idx + 2] as u32 + rgb[2] as u32) / 2) as u8;
                        }
                    }
                }
            }

            // Render directional solitons
            for p in &projectiles {
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

            // Render primary holon
            let pl_x = px.round() as i32;
            let pl_y = py.round() as i32;
            if let Some(sprite) = &player_sprite {
                let sw = sprite.width() as i32;
                let sh = sprite.height() as i32;
                for sy in 0..sh {
                    let dy = pl_y + sy;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    for sx in 0..sw {
                        let dx = pl_x + sx;
                        if dx < 0 || dx >= vw as i32 {
                            continue;
                        }
                        let p = sprite.get_pixel(sx as u32, sy as u32);
                        let alpha = p[3] as u32;
                        if alpha >= 90 {
                            let i = (dy as usize * vw + dx as usize) * 3;
                            let inv = 255 - alpha;
                            buf[i] = ((p[0] as u32 * alpha + buf[i] as u32 * inv) / 255) as u8;
                            buf[i + 1] =
                                ((p[1] as u32 * alpha + buf[i + 1] as u32 * inv) / 255) as u8;
                            buf[i + 2] =
                                ((p[2] as u32 * alpha + buf[i + 2] as u32 * inv) / 255) as u8;
                        }
                    }
                }
            } else {
                for y in pl_y.max(0)..(pl_y + ph as i32).min(vh as i32) {
                    for x in pl_x.max(0)..(pl_x + pw as i32).min(vw as i32) {
                        let i = (y as usize * vw + x as usize) * 3;
                        buf[i] = 70;
                        buf[i + 1] = 180;
                        buf[i + 2] = 240;
                    }
                }
            }

            // Render HUD overlay (top status / side status)
            if let Some(hud) = &model.hud {
                let hud_top = m.hud_top as usize;
                if hud_top < vh && hud.height > 0 {
                    for y in 0..hud.height.min(vh.saturating_sub(hud_top)) {
                        let dy = hud_top + y;
                        for x in 0..hud.width.min(vw) {
                            let s = (y * hud.width + x) * 3;
                            let i = (dy * vw + x) * 3;
                            buf[i] = hud.rgb[s];
                            buf[i + 1] = hud.rgb[s + 1];
                            buf[i + 2] = hud.rgb[s + 2];
                        }
                    }
                }
            }

            if let Some(w) = writer.as_mut() {
                w.write_frame(&buf)?;
            }
            if let Some(dir) = &capture {
                if frame % 10 == 0 || frame + 1 == frames {
                    let p = dir.join(format!("frame_{frame:05}.png"));
                    video_io::write_png(&p, vw as u32, vh as u32, &buf)?;
                    captured += 1;
                }
            }
        }

        let secs = t0.elapsed().as_secs_f64();
        if let Some(w) = writer.as_mut() {
            w.finish()?;
        }
        println!(
            "unfolded {frames} frames in {:.3}s ({:.0} Hz, planar Q_Cl flight corridor)",
            secs,
            frames as f64 / secs.max(1e-9)
        );
        println!(
            "player ended at x={:.1} y={:.1} (world), enemies_downed={enemies_downed}, bonus_collected={tokens_absorbed}, score={score}",
            px, py
        );
        if let Some(dir) = &capture {
            println!("captured {captured} frames -> {}", dir.display());
        }
        if let Some(p) = &out {
            println!("video -> {}", p.display());
        }
        return Ok(());
    }

    // Load canonical player sprite and learned bounding dimensions from the model.
    let player_track = model.manifest.tracks.iter().find(|t| t.kind == "player");
    let player_sprite: Option<image::RgbaImage> = player_track.and_then(|t| {
        let si = t.sprite? as usize;
        let bytes = model.sprites.get(si)?;
        image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .ok()
            .map(|img| img.to_rgba8())
    });
    let (pw, ph) = player_track
        .map(|t| ((t.size[0] - 4).max(10) as f32, (t.size[1] - 2).max(14) as f32))
        .unwrap_or((12.0, 24.0));
    let spawn_x = 32.0f32;
    let spawn_y = (m.ground_top as f32 - ph - 1.0).max(0.0);
    let mut player = Player::new(spawn_x, spawn_y, pw, ph);

    // Instantiate autonomous actor holons from all non-player tracks using their
    // learned world-space excursion tensor `t.span` to orient the initial spinor rotor.
    let mut actors: Vec<ActorHolon> = model
        .manifest
        .tracks
        .iter()
        .filter(|t| t.kind != "player" && t.spawn[0] > 0)
        .map(|t| {
            let sprite = t.sprite.and_then(|si| {
                let bytes = model.sprites.get(si as usize)?;
                image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                    .ok()
                    .map(|img| img.to_rgba8())
            });
            // Principal mobility axis from observed world-space excursion [dx, dy]:
            // if dy > 2 * dx, the process oscillates along e2 (-π/2); otherwise along e1 (π).
            let vertical_mode = t.span[1] > 2 * t.span[0].max(1);
            let theta0 = if vertical_mode {
                -std::f64::consts::FRAC_PI_2
            } else {
                std::f64::consts::PI
            };
            let cde = cc_math::CliffordDensityElement::new(1.0, theta0, 0, 3, t.color);
            let spawn_y = t.spawn[1] as f32;
            let amp_y = (t.span[1].max(16)) as f32;
            ActorHolon {
                x: t.spawn[0] as f32,
                y: spawn_y,
                w: t.size[0] as f32,
                h: t.size[1] as f32,
                speed: if vertical_mode { 0.35 } else { 0.5 },
                bounds_y: [spawn_y - amp_y, spawn_y],
                cde,
                sprite,
            }
        })
        .collect();

    // Interactive block props: discovered directly from continuous `m.platforms`
    // segments that have open backdrop space underneath (`is_open_below_rect`),
    // subdivided into square interactive units of side `h = r[3] - r[1] + 1`.
    let mut block_props: Vec<BlockProp> = Vec::new();
    for &r in &m.platforms {
        if r[3] < m.ground_top && model.is_open_below_rect(r) {
            let side = (r[3] - r[1] + 1).max(8);
            let mut sx = r[0];
            while sx <= r[2] {
                let ex = (sx + side - 1).min(r[2]);
                block_props.push(BlockProp {
                    rect: [sx, r[1], ex, r[3]],
                    bump_timer: 0,
                    state: 0,
                });
                sx += side;
            }
        }
    }
    let mut coins: Vec<CoinToken> = Vec::new();
    let mut coins_collected = 0usize;
    let mut impacts = 0usize;
    let t0 = Instant::now();
    let mut captured = 0usize;
    for frame in 0..frames {
        let prev_feet = player.y + player.h;
        // Continuous world-space probes ahead and above the player holon.
        let ahead = player.x + player.w + 12.0;
        let foot_y = player.y + player.h + 2.0;
        let body_y = player.y + player.h - 8.0;
        let wall = model.solid_at_world(ahead, body_y).is_some();
        let platform = (1..=4).any(|k| {
            let probe_y = foot_y - k as f32 * 16.0;
            model.one_way_at_world(ahead, probe_y).is_some()
                && !block_props.iter().any(|p| {
                    p.state == 0
                        && coins_collected == 0
                        && ahead.floor() as i32 >= p.rect[0]
                        && ahead.floor() as i32 <= p.rect[2]
                        && probe_y.floor() as i32 >= p.rect[1]
                        && probe_y.floor() as i32 <= p.rect[3]
                })
        });
        let gap = model.solid_at_world(ahead, foot_y).is_none()
            && model.one_way_at_world(ahead, foot_y).is_none();
        let center_x = (player.x + player.w * 0.5).floor() as i32;
        let foot_yi = foot_y.floor() as i32;
        let under_coin_box = player.on_ground
            && coins_collected == 0
            && block_props.iter().any(|p| {
                p.state == 0
                    && center_x >= p.rect[0]
                    && center_x <= p.rect[2]
                    && p.rect[3] < foot_yi
                    && (foot_yi - p.rect[1]) <= 68
            });
        let approaching_coin_box = coins_collected == 0
            && block_props.iter().any(|p| {
                p.state == 0
                    && p.rect[0] > center_x
                    && (p.rect[0] - center_x) <= 48
                    && p.rect[3] < foot_yi
                    && (foot_yi - p.rect[1]) <= 68
            });
        // An actor is ahead on the path -> jump at parabolic distance to land on it.
        let actor_ahead = !approaching_coin_box
            && actors.iter().any(|a| {
                a.cde.state() != 2
                    && (a.x - (player.x + player.w)) > 40.0
                    && (a.x - (player.x + player.w)) < 62.0
                    && (a.y + a.h - (player.y + player.h)).abs() < 24.0
            });
        let jump = player.on_ground
            && (wall || platform || actor_ahead || under_coin_box || (gap && frame % 3 == 0));
        let hit = player.step(&model, &block_props, 1.0, jump);
        if let Some(pi) = hit {
            if let Some(prop) = block_props.get_mut(pi) {
                if prop.state == 0 {
                    prop.state = 1;
                    prop.bump_timer = 8;
                    coins_collected += 1;
                    coins.push(CoinToken {
                        x: prop.rect[0] as f32 + 4.0,
                        y: prop.rect[1] as f32 - 4.0,
                        vy: -3.5,
                        timer: 16,
                    });
                }
            }
        }

        for prop in &mut block_props {
            if prop.bump_timer > 0 {
                prop.bump_timer -= 1;
                if prop.bump_timer == 0 {
                    prop.state = 2; // Collapse to spent empty block
                }
            }
        }
        for c in &mut coins {
            c.y += c.vy;
            c.vy += 0.25;
            c.timer = c.timer.saturating_sub(1);
        }
        coins.retain(|c| c.timer > 0);

        // Clifford-guided Bohm momentum evolution and boundary reflection.
        let basis = cc_math::idempotent::StateBasis::new(3);
        let view_right = player.x + vw as f32;
        for a in &mut actors {
            if a.cde.state() == 2 {
                continue;
            }
            let [vx_f64, vy_f64] = a.cde.bohm_momentum(a.speed);
            if vy_f64.abs() > vx_f64.abs() {
                // e2-oriented process: step vertically between learned excursion bounds
                // and reflect rotor U -> N U ~N at upper/lower limits.
                let vy = vy_f64 as f32;
                if a.y >= a.bounds_y[1] - 1.0 && (player.x - a.x).abs() <= a.w * 1.25 {
                    continue;
                }
                a.y = (a.y + vy).clamp(a.bounds_y[0], a.bounds_y[1]);
                if a.y <= a.bounds_y[0] {
                    a.cde.reflect_at_boundary([0.0, 1.0], &basis);
                } else if a.y >= a.bounds_y[1] {
                    a.cde.reflect_at_boundary([0.0, -1.0], &basis);
                }
                continue;
            }
            if a.x > view_right {
                continue;
            }
            let vx = vx_f64 as f32;
            a.x += vx;
            let check_x = if vx > 0.0 { a.x + a.w + 1.0 } else { a.x - 1.0 };
            let check_y = a.y + a.h * 0.5;
            if model.solid_at_world(check_x, check_y).is_some()
                || model.one_way_at_world(check_x, check_y).is_some()
            {
                // Lateral boundary incidence: rotor reflection U -> N U ~N and idempotent transition T_{0<->1}
                let normal = if vx > 0.0 { [-1.0, 0.0] } else { [1.0, 0.0] };
                a.cde.reflect_at_boundary(normal, &basis);
            }

            // Top-down relational incidence: downward player motion onto the actor's top boundary
            let feet = player.y + player.h;
            if player.vy > 0.0
                && feet >= a.y
                && prev_feet <= a.y + 6.0
                && (player.x + player.w) > a.x
                && player.x < (a.x + a.w)
            {
                // Downward impact: collapse actor CDE to post-impact state (State 2, amplitude -> 0)
                let _ = a.cde.transition_to(2, &basis);
                a.cde.amplitude = 0.0;
                player.vy = -JUMP * 0.75;
                impacts += 1;
                eprintln!(
                    "Impact at frame {frame}: player landed on actor at x={:.1} y={:.1}",
                    a.x, a.y
                );
            }
        }
        if std::env::var_os("CC_TRACE").is_some() && frame % 25 == 0 {
            eprintln!(
                "trace f{frame}: x={:.2} y={:.2} vy={:.2} ground={}",
                player.x, player.y, player.vy, player.on_ground
            );
        }

        // Camera follows the player, clamped to the atlas.
        let max_cam = aw.saturating_sub(vw);
        let cam_x = ((player.x + player.w / 2.0 - vw as f32 / 2.0).round() as usize).min(max_cam);
        let cam_y = 0usize.min(ah.saturating_sub(vh));

        let hud_top = m.hud_top as usize;
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
                    buf[i] = model.atlas.rgb[s];
                    buf[i + 1] = model.atlas.rgb[s + 1];
                    buf[i + 2] = model.atlas.rgb[s + 2];
                } else {
                    buf[i] = 12;
                    buf[i + 1] = 16;
                    buf[i + 2] = 28;
                }
            }
        }

        // Render bumping interactive block displacement in continuous world coordinates.
        for prop in &block_props {
            if prop.bump_timer > 0 {
                let bx = prop.rect[0] - cam_x as i32;
                let by = prop.rect[1] - cam_y as i32;
                let bw = (prop.rect[2] - prop.rect[0] + 1).max(1);
                let bh = (prop.rect[3] - prop.rect[1] + 2).max(1);
                if bx + bw <= 0 || bx >= vw as i32 || by + bh <= 0 || by >= vh as i32 {
                    continue;
                }
                // Parabolic upward bump displacement
                let bump_dy = (-(prop.bump_timer as f32 * (8.0 - prop.bump_timer as f32)) / 3.0)
                    .round() as i32;
                let bg = m.background;
                // Clear original location to learned backdrop
                for ty in 0..bh {
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
                // Draw region from model.atlas displaced upward by bump_dy
                for ty in 0..bh {
                    let dy = by + bump_dy + ty;
                    let ay = (prop.rect[1] + ty).max(0) as usize;
                    if dy >= 0 && dy < vh as i32 && ay < ah {
                        for tx in 0..bw {
                            let dx = bx + tx;
                            let ax = (prop.rect[0] + tx).max(0) as usize;
                            if dx >= 0 && dx < vw as i32 && ax < aw {
                                let idx = (dy as usize * vw + dx as usize) * 3;
                                let s = (ay * aw + ax) * 3;
                                buf[idx] = model.atlas.rgb[s];
                                buf[idx + 1] = model.atlas.rgb[s + 1];
                                buf[idx + 2] = model.atlas.rgb[s + 2];
                            }
                        }
                    }
                }
            }
        }

        // Render emitted coin tokens.
        for c in &coins {
            let cx = c.x as i32 - cam_x as i32;
            let cy = c.y as i32 - cam_y as i32;
            let cw = 8i32;
            let ch = 14i32;
            for py in 0..ch {
                let dy = cy + py;
                if dy < 0 || dy >= vh as i32 {
                    continue;
                }
                let ry = (py as f32 - ch as f32 / 2.0) / (ch as f32 / 2.0);
                for px in 0..cw {
                    let dx = cx + px;
                    if dx < 0 || dx >= vw as i32 {
                        continue;
                    }
                    let rx = (px as f32 - cw as f32 / 2.0) / (cw as f32 / 2.0);
                    if rx * rx + ry * ry <= 1.0 {
                        let i = (dy as usize * vw + dx as usize) * 3;
                        let is_rim = rx * rx + ry * ry > 0.65;
                        let is_shine = rx.abs() < 0.25 && ry < 0.2;
                        let rgb = if is_shine {
                            [255u8, 255, 210] // gleaming coin shine
                        } else if is_rim {
                            [184u8, 115, 0] // dark gold rim
                        } else {
                            [252u8, 198, 3] // rich gold body
                        };
                        buf[i] = rgb[0];
                        buf[i + 1] = rgb[1];
                        buf[i + 2] = rgb[2];
                    }
                }
            }
        }

        // Render player sprite (or fallback to collision box).
        let px = player.x as i32 - cam_x as i32;
        let py = player.y as i32 - cam_y as i32;
        if let Some(sprite) = &player_sprite {
            let sw = sprite.width() as i32;
            let sh = sprite.height() as i32;
            let sy_off = (py + player.h as i32) - sh;
            let sx_off = px + (player.w as i32 - sw) / 2;
            let is_jumping = !player.on_ground;
            let walk_cycle = ((player.phase * 0.8).sin() * 1.5).round() as i32;
            for sy in 0..sh {
                let dy = sy_off + sy;
                if dy < 0 || dy >= vh as i32 {
                    continue;
                }
                for sx in 0..sw {
                    let dx = sx_off + sx;
                    if dx < 0 || dx >= vw as i32 {
                        continue;
                    }
                    // Harmonic vitality stride / jump pose
                    let mut src_x = sx;
                    let mut src_y = sy;
                    if is_jumping {
                        if sy < sh / 2 && sy > 1 {
                            src_y = (sy + 1).min(sh - 1);
                        }
                    } else if player.on_ground && player.vx.abs() > 0.1 {
                        let is_leg = sy >= (sh * 3) / 4;
                        if is_leg {
                            src_x = (sx + walk_cycle).clamp(0, sw - 1);
                        }
                    }
                    let p = sprite.get_pixel(src_x as u32, src_y as u32);
                    let a = p[3] as u32;
                    if a >= 100 {
                        let i = (dy as usize * vw + dx as usize) * 3;
                        let inv = 255 - a;
                        buf[i] = ((p[0] as u32 * a + buf[i] as u32 * inv) / 255) as u8;
                        buf[i + 1] = ((p[1] as u32 * a + buf[i + 1] as u32 * inv) / 255) as u8;
                        buf[i + 2] = ((p[2] as u32 * a + buf[i + 2] as u32 * inv) / 255) as u8;
                    }
                }
            }
        } else {
            for y in py.max(0)..(py + player.h as i32).min(vh as i32) {
                for x in px.max(0)..(px + player.w as i32).min(vw as i32) {
                    let i = (y as usize * vw + x as usize) * 3;
                    buf[i] = 235;
                    buf[i + 1] = 60;
                    buf[i + 2] = 60;
                }
            }
        }

        // Render autonomous actor holons.
        for a in &actors {
            let ax = a.x as i32 - cam_x as i32;
            let ay = a.y as i32 - cam_y as i32;
            if ax + a.w as i32 <= 0
                || ax >= vw as i32
                || ay + a.h as i32 <= 0
                || ay >= vh as i32
            {
                continue;
            }
            if let Some(sprite) = &a.sprite {
                let sw = sprite.width() as i32;
                let sh = sprite.height() as i32;
                let state = a.cde.state();
                let render_h = if state == 2 { (sh / 2).max(1) } else { sh };
                let sy_off = (ay + a.h as i32) - render_h;
                let sx_off = ax;
                let [hx, hy] = a.cde.heading_vector();
                let walk_flip = hx.abs() >= hy.abs() && state != 2 && (frame / 8) % 2 == 1;
                for sy in 0..render_h {
                    let dy = sy_off + sy;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    let src_y = if state == 2 { sy * 2 } else { sy };
                    for sx in 0..sw {
                        let dx = sx_off + sx;
                        if dx < 0 || dx >= vw as i32 {
                            continue;
                        }
                        let src_x = if walk_flip { sw - 1 - sx } else { sx };
                        let p = sprite.get_pixel(src_x as u32, src_y as u32);
                        let alpha = p[3] as u32;
                        if alpha >= 100 {
                            let i = (dy as usize * vw + dx as usize) * 3;
                            let inv = 255 - alpha;
                            buf[i] = ((p[0] as u32 * alpha + buf[i] as u32 * inv) / 255) as u8;
                            buf[i + 1] =
                                ((p[1] as u32 * alpha + buf[i + 1] as u32 * inv) / 255) as u8;
                            buf[i + 2] =
                                ((p[2] as u32 * alpha + buf[i + 2] as u32 * inv) / 255) as u8;
                        }
                    }
                }
            } else if a.cde.state() != 2 {
                for y in ay.max(0)..(ay + a.h as i32).min(vh as i32) {
                    for x in ax.max(0)..(ax + a.w as i32).min(vw as i32) {
                        let i = (y as usize * vw + x as usize) * 3;
                        buf[i] = 160;
                        buf[i + 1] = 80;
                        buf[i + 2] = 40;
                    }
                }
            }
        }

        if let Some(w) = writer.as_mut() {
            w.write_frame(&buf)?;
        }
        if let Some(dir) = &capture {
            if frame % 10 == 0 || frame + 1 == frames {
                let p = dir.join(format!("frame_{frame:05}.png"));
                video_io::write_png(&p, vw as u32, vh as u32, &buf)?;
                captured += 1;
            }
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    if let Some(w) = writer.as_mut() {
        w.finish()?;
    }
    println!(
        "unfolded {frames} frames in {:.3}s ({:.0} Hz, headless)",
        secs,
        frames as f64 / secs.max(1e-9)
    );
    println!(
        "player ended at x={:.1} y={:.1} (world), on_ground={}, impacts={}, coins={}",
        player.x, player.y, player.on_ground, impacts, coins_collected
    );
    if let Some(dir) = &capture {
        println!("captured {captured} frames -> {}", dir.display());
    }
    if let Some(p) = &out {
        println!("video -> {}", p.display());
    }
    Ok(())
}
