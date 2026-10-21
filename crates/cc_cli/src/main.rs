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

/// A minimal AABB player: gravity, solid tiles, one-way surfaces.
struct Player {
    x: f32,
    y: f32,
    vy: f32,
    on_ground: bool,
}

const PW: f32 = 12.0;
const PH: f32 = 16.0;
const GRAVITY: f32 = 0.35;
const MAX_FALL: f32 = 6.5;
const RUN: f32 = 1.6;
const JUMP: f32 = 6.2;

impl Player {
    fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            vy: 0.0,
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
        self.x += dir * RUN;
        if dir != 0.0 {
            let lead = if dir > 0.0 { self.x + PW - 1.0 } else { self.x };
            for sample_y in [self.y + 1.0, self.y + PH * 0.5, self.y + PH - 1.0] {
                if let Some(r) = m.solid_at_world(lead, sample_y) {
                    self.x = if dir > 0.0 {
                        r[0] as f32 - PW
                    } else {
                        (r[2] + 1) as f32
                    };
                    break;
                }
            }
        }
        self.x = self.x.max(0.0);

        // Vertical.
        if jump && self.on_ground {
            self.vy = -JUMP;
            self.on_ground = false;
        }
        self.vy = (self.vy + GRAVITY).min(MAX_FALL);
        let prev_feet = self.y + PH;
        self.y += self.vy;
        self.on_ground = false;

        if self.vy >= 0.0 {
            let feet = self.y + PH;
            let mut landed_top: Option<f32> = None;
            for sample_x in [self.x, self.x + PW * 0.5, self.x + PW - 1.0] {
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
                self.y = top - PH;
                self.vy = 0.0;
                self.on_ground = true;
            }
            None
        } else {
            let head_y = self.y;
            let mut hit_prop: Option<usize> = None;
            let mut hit_bottom: Option<f32> = None;
            for sample_x in [self.x, self.x + PW * 0.5, self.x + PW - 1.0] {
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
    let frames: usize = opt(args, "--frames")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| if is_static { m.scroll.len() } else { 600 });
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

    let spawn_x = 32.0f32;
    let spawn_y = (m.ground_top as f32 - PH - 1.0).max(0.0);
    let mut player = Player::new(spawn_x, spawn_y);

    // Load canonical player sprite if present in the model.
    let player_sprite: Option<image::RgbaImage> = model
        .manifest
        .tracks
        .iter()
        .find(|t| t.kind == "player")
        .and_then(|t| {
            let si = t.sprite? as usize;
            let bytes = model.sprites.get(si)?;
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()
                .map(|img| img.to_rgba8())
        });

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
        let prev_feet = player.y + PH;
        // Continuous world-space probes ahead and above the player holon.
        let ahead = player.x + PW + 12.0;
        let foot_y = player.y + PH + 2.0;
        let body_y = player.y + PH * 0.5;
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
        let center_x = (player.x + PW * 0.5).floor() as i32;
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
                    && (a.x - (player.x + PW)) > 40.0
                    && (a.x - (player.x + PW)) < 62.0
                    && (a.y - (player.y + PH)).abs() < 24.0
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
            let feet = player.y + PH;
            if player.vy > 0.0
                && feet >= a.y
                && prev_feet <= a.y + 6.0
                && (player.x + PW) > a.x
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
        let cam_x = ((player.x + PW / 2.0 - vw as f32 / 2.0).round() as usize).min(max_cam);
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
                let bh = (prop.rect[3] - prop.rect[1] + 1).max(1);
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
            let sy_off = (py + PH as i32) - sh;
            let sx_off = px + (PW as i32 - sw) / 2;
            let run_flip = player.on_ground && (frame / 6) % 2 == 1;
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
                    let src_x = if run_flip { sw - 1 - sx } else { sx };
                    let p = sprite.get_pixel(src_x as u32, sy as u32);
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
            for y in py.max(0)..(py + PH as i32).min(vh as i32) {
                for x in px.max(0)..(px + PW as i32).min(vw as i32) {
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
