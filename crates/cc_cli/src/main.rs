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

    fn step(&mut self, m: &Model2D, dir: f32, jump: bool) {
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
        } else {
            let ty = ((self.y - oy) / tile).floor() as i32;
            for tx in tx0..=tx1 {
                if self.solid(m, tx, ty) {
                    self.y = oy + (ty as f32 + 1.0) * tile;
                    self.vy = 0.0;
                    break;
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
        .unwrap_or(600);
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

    let spawn_x = 32.0f32;
    let spawn_y = (m.ground_top as f32 - PH - 1.0).max(0.0);
    let mut player = Player::new(spawn_x, spawn_y);

    let mut writer = match (&out, no_video) {
        (Some(p), false) => Some(video_io::VideoWriter::create(p, vw as u32, vh as u32, 30.0)?),
        _ => None,
    };

    let t0 = Instant::now();
    let mut captured = 0usize;
    for frame in 0..frames {
        // Scripted input: run right, jump when a wall or a gap is ahead.
        let ahead = player.x + PW + RUN * 2.0;
        let foot_ty = ((player.y + PH + 2.0 - m.origin_y as f32) / TILE as f32).floor() as i32;
        let tx = (ahead / TILE as f32).floor() as i32;
        let wall = player.solid(&model, tx, foot_ty - 1);
        // A one-way surface can sit a few tiles above the feet (pipe tops).
        let platform = (1..=4).any(|k| player.one_way(&model, tx, foot_ty - k));
        let gap = !player.solid(&model, tx, foot_ty) && !player.one_way(&model, tx, foot_ty);
        let jump = player.on_ground && (wall || platform || (gap && frame % 3 == 0));
        player.step(&model, 1.0, jump);
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

        // Player box (placeholder until sprite classification is solid).
        let px = player.x as i32 - cam_x as i32;
        let py = player.y as i32 - cam_y as i32;
        for y in py.max(0)..(py + PH as i32).min(vh as i32) {
            for x in px.max(0)..(px + PW as i32).min(vw as i32) {
                let i = (y as usize * vw + x as usize) * 3;
                buf[i] = 235;
                buf[i + 1] = 60;
                buf[i + 2] = 60;
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
        "player ended at x={:.1} y={:.1} (world), on_ground={}",
        player.x, player.y, player.on_ground
    );
    if let Some(dir) = &capture {
        println!("captured {captured} frames -> {}", dir.display());
    }
    if let Some(p) = &out {
        println!("video -> {}", p.display());
    }
    Ok(())
}
