//! `cc` — cliffordcore model compiler and 2D runtime.
//!
//! ```sh
//! cc enfold example_input2d/smb3_w1_1_256x240.mp4 -o /tmp/opencode/smb3.cc
//! cc inspect /tmp/opencode/smb3.cc --atlas /tmp/opencode/atlas.png
//! cc unfold /tmp/opencode/smb3.cc --frames 600 --capture /tmp/opencode/run -o /tmp/opencode/run.mp4
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cc_field::enfold2d::{self, EnfoldOptions, TILE};
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
        "level   viewport {}x{}, hud_top {}, ground_top y={}, backdrop {:?}",
        m.viewport[0], m.viewport[1], m.hud_top, m.ground_top, m.background
    );
    println!(
        "tiles   empty={} solid={} oneway={} background={}",
        counts[0], counts[1], counts[2], counts[3]
    );
    println!(
        "collide {} solid runs, {} one-way surfaces",
        m.solids.len(),
        m.platforms.len()
    );
    println!("frames  {} scroll samples", m.scroll.len());
    println!(
        "actors  {} tracks, {} sprites",
        m.tracks.len(),
        m.sprites.len()
    );

    // ASCII preview of the collision grid.
    let max_cols = m.cols.min(96) as usize;
    let max_rows = m.rows.min(24) as usize;
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
        let palette = [
            [0, 0, 0],
            [255, 40, 40],
            [255, 0, 255],
            [40, 40, 255],
        ];
        for ty in 0..m.rows as usize {
            for tx in 0..m.cols as usize {
                let k = model.kind(tx, ty) as usize;
                let y0 = (m.origin_y + (ty * TILE) as i32).max(0) as usize;
                for x in tx * TILE..((tx + 1) * TILE).min(img.width) {
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

    fn tile_range(&self, m: &Model2D) -> (i32, i32, i32, i32) {
        let tx0 = (self.x / TILE as f32).floor() as i32;
        let tx1 = ((self.x + PW - 1.0) / TILE as f32).floor() as i32;
        let oy = m.manifest.origin_y;
        let ty0 = ((self.y - oy as f32) / TILE as f32).floor() as i32;
        let ty1 = ((self.y + PH - 1.0 - oy as f32) / TILE as f32).floor() as i32;
        (tx0, ty0, tx1, ty1)
    }

    fn solid(&self, m: &Model2D, tx: i32, ty: i32) -> bool {
        tx >= 0 && ty >= 0 && m.kind(tx as usize, ty as usize) == cc_field::TileKind::Solid
    }

    fn one_way(&self, m: &Model2D, tx: i32, ty: i32) -> bool {
        tx >= 0 && ty >= 0 && m.kind(tx as usize, ty as usize) == cc_field::TileKind::OneWay
    }

    fn step(&mut self, m: &Model2D, dir: f32, jump: bool) -> Option<(usize, usize)> {
        let oy = m.manifest.origin_y as f32;
        let tile = TILE as f32;

        // Horizontal: only the leading edge can collide.
        self.x += dir * RUN;
        if dir != 0.0 {
            let lead = if dir > 0.0 { self.x + PW - 1.0 } else { self.x };
            let tx = (lead / tile).floor() as i32;
            let ty0 = ((self.y - oy) / tile).floor() as i32;
            let ty1 = ((self.y + PH - 1.0 - oy) / tile).floor() as i32;
            for ty in ty0..=ty1 {
                if self.solid(m, tx, ty) {
                    self.x = if dir > 0.0 {
                        tx as f32 * tile - PW
                    } else {
                        (tx as f32 + 1.0) * tile
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
        let tx0 = (self.x / tile).floor() as i32;
        let tx1 = ((self.x + PW - 1.0) / tile).floor() as i32;

        if self.vy >= 0.0 {
            let feet = self.y + PH;
            let ty = ((feet - oy) / tile).floor() as i32;
            let top = oy + ty as f32 * tile;
            let mut landed = false;
            for tx in tx0..=tx1 {
                let solid = self.solid(m, tx, ty);
                let one = self.one_way(m, tx, ty);
                if !solid && !one {
                    continue;
                }
                let tol = if solid { tile } else { 2.0 };
                if feet > top && prev_feet <= top + tol {
                    landed = true;
                    break;
                }
            }
            if landed {
                self.y = top - PH;
                self.vy = 0.0;
                self.on_ground = true;
            }
            None
        } else {
            let ty = ((self.y - oy) / tile).floor() as i32;
            let mut hit = None;
            for tx in tx0..=tx1 {
                let is_solid = self.solid(m, tx, ty);
                let is_block = self.one_way(m, tx, ty)
                    && (ty + 1 < m.manifest.rows as i32
                        && m.kind(tx as usize, (ty + 1) as usize) == cc_field::TileKind::Empty);
                if is_solid || is_block {
                    hit = Some((tx as usize, ty as usize));
                    break;
                }
            }
            if let Some((hx, hy)) = hit {
                self.y = oy + (hy as f32 + 1.0) * tile;
                self.vy = 0.0;
                Some((hx, hy))
            } else {
                None
            }
        }
    }
}

/// An autonomous entity in the 2D world model.
///
/// Behaviors are modeled on high-level geometric invariants and Clifford
/// state idempotents (Bohm-Hiley process ontology) rather than game-specific
/// branching:
/// - State 0: Heading negative along primary axis (e.g. leftward patrol)
/// - State 1: Heading positive along primary axis (e.g. rightward patrol)
/// - State 2: Post-impact / passive state (e.g. squashed, recoiled)
struct ActorHolon {
    x: f32,
    y: f32,
    spawn_y: f32,
    vx: f32,
    w: f32,
    h: f32,
    state: u32,
    is_vertical: bool,
    sprite: Option<image::RgbaImage>,
}

/// A collectible token emitted upon boundary impact (e.g. coin pop).
struct CoinToken {
    x: f32,
    y: f32,
    vy: f32,
    timer: usize,
}

/// An interactive prop block (Bohmian process holon with discrete state transitions).
/// - State 0: Active Question Block (emits token on upward impact)
/// - State 1: Bumping impulse phase
/// - State 2: Spent Empty Block (unreactive)
struct BlockProp {
    tx: usize,
    ty: usize,
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
    let is_static = m.scroll.iter().all(|s| s[0] == 0 && s[1] == 0);
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
        let mut atlas_copy = model.atlas.rgb.clone();
        let track_positions: Vec<std::collections::HashMap<usize, (i32, i32)>> = m
            .tracks
            .iter()
            .map(|t| {
                t.trajectory
                    .iter()
                    .map(|p| (p[0] as usize, (p[1], p[2])))
                    .collect()
            })
            .collect();
        let track_sprites: Vec<Option<image::RgbaImage>> = m
            .tracks
            .iter()
            .map(|t| {
                t.sprite
                    .and_then(|si| model.sprites.get(si as usize))
                    .and_then(|bytes| {
                        image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()
                    })
                    .map(|img| img.to_rgba8())
            })
            .collect();

        let mut last_positions: Vec<Option<(i32, i32)>> = vec![None; m.tracks.len()];
        let mut dots_eaten = 0usize;
        let t0 = Instant::now();
        let mut captured = 0usize;

        for frame in 0..frames {
            // Update track positions for this frame
            for (ti, map) in track_positions.iter().enumerate() {
                if let Some(&pos) = map.get(&frame) {
                    last_positions[ti] = Some(pos);
                }
            }

            // Pac-Man eating dots
            if let Some((px, py)) = last_positions.first().and_then(|p| *p) {
                let cx = px + 8;
                let cy = py + 8;
                for dy in -5..=5 {
                    let y = cy + dy;
                    if y < 0 || y >= ah as i32 {
                        continue;
                    }
                    for dx in -5..=5 {
                        let x = cx + dx;
                        if x < 0 || x >= aw as i32 {
                            continue;
                        }
                        if dx * dx + dy * dy <= 20 {
                            let idx = (y as usize * aw + x as usize) * 3;
                            let c = [atlas_copy[idx], atlas_copy[idx + 1], atlas_copy[idx + 2]];
                            if c[0] > 100 && c[2] < 200 {
                                atlas_copy[idx] = 0;
                                atlas_copy[idx + 1] = 0;
                                atlas_copy[idx + 2] = 0;
                                dots_eaten += 1;
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
                        buf[i] = 0;
                        buf[i + 1] = 0;
                        buf[i + 2] = 0;
                    }
                }
            }

            // Render actors (ghosts first, then player)
            let order = (1..m.tracks.len()).chain(std::iter::once(0));
            for ti in order {
                let Some(sprite) = &track_sprites[ti] else {
                    continue;
                };
                let Some((ax, ay)) = last_positions[ti] else {
                    continue;
                };
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
            "unfolded {frames} frames in {:.3}s ({:.0} Hz, static arena)",
            secs,
            frames as f64 / secs.max(1e-9)
        );
        if let Some((px, py)) = last_positions.first().and_then(|p| *p) {
            println!("player ended at x={px} y={py} (world), dots_eaten={dots_eaten}");
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

    // Instantiate autonomous actor holons from all non-player tracks.
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
            // If track is tall and narrow (e.g. >= 30 px tall, <= 22 px wide), it is vertically anchored (e.g. plant in tube)
            let is_vertical = t.size[1] >= 30 && t.size[0] <= 22;
            ActorHolon {
                x: t.spawn[0] as f32,
                y: t.spawn[1] as f32,
                spawn_y: t.spawn[1] as f32,
                vx: if is_vertical { 0.0 } else { -0.5 },
                w: t.size[0] as f32,
                h: t.size[1] as f32,
                state: 0,
                is_vertical,
                sprite,
            }
        })
        .collect();

    // Interactive block props (floating blocks with open space underneath).
    let mut block_props: std::collections::HashMap<(usize, usize), BlockProp> =
        std::collections::HashMap::new();
    let rows = (ah / TILE).min(m.ground_top as usize / TILE);
    for ty in 0..rows {
        for tx in 0..(aw / TILE) {
            // A bumpable block is a floating platform with empty space underneath it
            if model.kind(tx, ty) == cc_field::TileKind::OneWay
                && ty + 1 < rows
                && model.kind(tx, ty + 1) == cc_field::TileKind::Empty
            {
                block_props.insert(
                    (tx, ty),
                    BlockProp {
                        tx,
                        ty,
                        bump_timer: 0,
                        state: 0,
                    },
                );
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
        // Scripted input: run right, jump when a wall, platform, gap, or actor is ahead.
        let ahead = player.x + PW + 12.0;
        let foot_ty = ((player.y + PH + 2.0 - m.origin_y as f32) / TILE as f32).floor() as i32;
        let tx = (ahead / TILE as f32).floor() as i32;
        let wall = player.solid(&model, tx, foot_ty - 1);
        // A one-way surface can sit a few tiles above the feet (pipe tops, boxes).
        let platform = (1..=4).any(|k| player.one_way(&model, tx, foot_ty - k));
        let gap = !player.solid(&model, tx, foot_ty) && !player.one_way(&model, tx, foot_ty);
        // An actor is ahead on the path -> jump at parabolic distance to land on it.
        let actor_ahead = actors.iter().any(|a| {
            a.state != 2
                && (a.x - (player.x + PW)) > 40.0
                && (a.x - (player.x + PW)) < 62.0
                && (a.y - (player.y + PH)).abs() < 24.0
        });
        // Jump directly under the first active coin box to hit it from below.
        let under_coin_box = player.on_ground
            && coins_collected == 0
            && (1..=4).any(|k| {
                foot_ty - k >= 0
                    && block_props
                        .get(&(
                            ((player.x + PW / 2.0) / TILE as f32).floor() as usize,
                            (foot_ty - k) as usize,
                        ))
                        .map_or(false, |p| p.state == 0)
            });
        let jump = player.on_ground
            && (wall || platform || actor_ahead || under_coin_box || (gap && frame % 3 == 0));
        let hit = player.step(&model, 1.0, jump);
        if let Some((hx, hy)) = hit {
            if let Some(prop) = block_props.get_mut(&(hx, hy)) {
                if prop.state == 0 {
                    prop.state = 1;
                    prop.bump_timer = 8;
                    coins_collected += 1;
                    coins.push(CoinToken {
                        x: hx as f32 * TILE as f32 + 4.0,
                        y: m.origin_y as f32 + hy as f32 * TILE as f32 - 4.0,
                        vy: -3.5,
                        timer: 16,
                    });
                }
            }
        }

        for prop in block_props.values_mut() {
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

        // Clifford-guided patrol and relational incidence resolution.
        let basis = cc_math::idempotent::StateBasis::new(3);
        let view_right = player.x + vw as f32;
        for a in &mut actors {
            if a.state == 2 {
                continue;
            }
            if a.is_vertical {
                // Vertical oscillation: rise from tube, pause, and retract
                let period = 120.0f32;
                let phase = (frame as f32 % period) / period;
                let dist_to_player = (player.x - a.x).abs();
                if phase < 0.4 && dist_to_player > 24.0 {
                    a.y = a.spawn_y - 20.0;
                } else if phase < 0.5 {
                    let t = (phase - 0.4) / 0.1;
                    a.y = a.spawn_y - 20.0 * (1.0 - t);
                } else if phase < 0.9 {
                    a.y = a.spawn_y;
                } else {
                    let t = (phase - 0.9) / 0.1;
                    a.y = a.spawn_y - 20.0 * t;
                }
                continue;
            }
            if a.x > view_right {
                continue;
            }
            a.x += a.vx;
            let check_x = if a.vx > 0.0 { a.x + a.w + 1.0 } else { a.x - 1.0 };
            let atx = (check_x / TILE as f32).floor() as i32;
            let aty = ((a.y + a.h / 2.0 - m.origin_y as f32) / TILE as f32).floor() as i32;
            if player.solid(&model, atx, aty) || player.one_way(&model, atx, aty) {
                // Lateral boundary incidence: idempotent reversal transition T_{0->1} or T_{1->0}
                if a.state == 0 {
                    let _ = basis.get(1);
                    a.state = 1;
                    a.vx = 0.5;
                } else {
                    let _ = basis.get(0);
                    a.state = 0;
                    a.vx = -0.5;
                }
            }

            // Top-down relational incidence: downward player motion onto the actor's top boundary
            let feet = player.y + PH;
            if player.vy > 0.0
                && feet >= a.y
                && prev_feet <= a.y + 6.0
                && (player.x + PW) > a.x
                && player.x < (a.x + a.w)
            {
                // Downward impact: collapse actor to post-impact state (State 2)
                let _ = basis.get(2);
                a.state = 2;
                player.vy = -JUMP * 0.75;
                impacts += 1;
                eprintln!(
                    "Impact at frame {frame}: player landed on actor at x={:.1} y={:.1}",
                    a.x, a.y
                );
            }
        }
        if std::env::var_os("CC_TRACE").is_some() && frame % 25 == 0 {
            let (a, b, c, d) = player.tile_range(&model);
            eprintln!("trace f{frame}: x={:.2} y={:.2} vy={:.2} ground={} tiles=({a},{b})..({c},{d}) solid={}", player.x, player.y, player.vy, player.on_ground, player.solid(&model, (player.x/16.0) as i32, d));
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

        // Render bumping interactive block displacement.
        for prop in block_props.values() {
            if prop.bump_timer > 0 {
                let bx = (prop.tx * TILE) as i32 - cam_x as i32;
                let by = (m.origin_y + prop.ty as i32 * TILE as i32) - cam_y as i32;
                let bw = TILE as i32;
                let bh = TILE as i32;
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
                // Draw tile from model.atlas displaced upward by bump_dy
                for ty in 0..bh {
                    let dy = by + bump_dy + ty;
                    let ay = prop.ty * TILE + ty as usize;
                    if dy >= 0 && dy < vh as i32 && ay < ah {
                        for tx in 0..bw {
                            let dx = bx + tx;
                            let ax = prop.tx * TILE + tx as usize;
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
                let render_h = if a.state == 2 { (sh / 2).max(1) } else { sh };
                let sy_off = (ay + a.h as i32) - render_h;
                let sx_off = ax;
                let walk_flip = !a.is_vertical && a.state != 2 && (frame / 8) % 2 == 1;
                for sy in 0..render_h {
                    let dy = sy_off + sy;
                    if dy < 0 || dy >= vh as i32 {
                        continue;
                    }
                    let src_y = if a.state == 2 { sy * 2 } else { sy };
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
            } else if a.state != 2 {
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
