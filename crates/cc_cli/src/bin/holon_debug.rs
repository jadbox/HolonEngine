use cc_field::model2d::Model2D;
use std::fs;
use std::io;
use std::path::Path;

const HELP: &str = r#"holon-debug: Standalone analysis and inspection tool for HolonEngine (.cc models & visual frames)

USAGE:
    holon-debug <path.cc>                 Top-level overview of model, pregeometry & holons (default)
    holon-debug overview <path.cc>        Top-level overview of world regime, geometry, boundary & mobile holons
    holon-debug model <path.cc>           Detailed manifest, physics priors, grid properties, and atlas layout
    holon-debug tracks <path.cc>          Analyze all observed tracks, learned kinematics, Bohmian momentum, and trajectories
    holon-debug sprites <path.cc>         Inspect compiled canonical sprites, CFT harmonic modes, and alpha statistics
    holon-debug collisions <path.cc>      Inspect solid rects, one-way platforms, boundary blades, and excitability
    holon-debug frame-diff <img1> <img2>  Diff two video frame captures or screenshots (reports bounding boxes, pixel counts, drift)
    holon-debug export <path.cc> <out_dir> Export atlas PNG, HUD PNG, all sprite PNGs, and manifest JSON to disk
    holon-debug help, --help, -h          Print this help message

OPTIONS:
    --threshold <val>                     Color distance threshold for frame-diff (default: 16)
"#;

fn main() {
    if let Err(e) = run() {
        eprintln!("holon-debug: {e}");
        std::process::exit(1);
    }
}

fn clean_path(s: &str) -> &str {
    let mut trimmed = s.trim();
    while (trimmed.starts_with('[') && trimmed.ends_with(']'))
        || (trimmed.starts_with('<') && trimmed.ends_with('>'))
    {
        trimmed = &trimmed[1..trimmed.len() - 1].trim();
    }
    trimmed
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(first) = args.first() else {
        print!("{HELP}");
        return Ok(());
    };
    let rest = &args[1..];
    match first.as_str() {
        "overview" => cmd_overview(rest),
        "model" => cmd_model(rest),
        "tracks" => cmd_tracks(rest),
        "sprites" => cmd_sprites(rest),
        "collisions" => cmd_collisions(rest),
        "frame-diff" => cmd_frame_diff(rest),
        "export" => cmd_export(rest),
        "-h" | "--help" | "help" => {
            print!("{HELP}");
            Ok(())
        }
        _ => {
            let cleaned_first = clean_path(first);
            if cleaned_first.ends_with(".cc")
                || Path::new(cleaned_first).exists()
                || !first.starts_with('-')
            {
                cmd_overview(&args)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown subcommand `{first}`\n\n{HELP}"),
                ))
            }
        }
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
    let mut skip_next = false;
    for a in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a.starts_with("--") || a.starts_with('-') {
            if a == "--threshold" {
                skip_next = true;
            }
            continue;
        }
        out.push(a.as_str());
    }
    out
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn hex_color(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum WorldRegime {
    TerrestrialSideScroller,
    PlanarStaticArena,
    PlanarDynamicFlightVertical,
    PlanarDynamicFlightHorizontal,
}

impl WorldRegime {
    fn label(&self) -> &'static str {
        match self {
            Self::TerrestrialSideScroller => "Terrestrial 2D (Gravity-bound side-scroller)",
            Self::PlanarStaticArena => "Planar Static Arena (Zero-G closed maze)",
            Self::PlanarDynamicFlightVertical => "Planar Dynamic Flight (Vertical-scrolling arena)",
            Self::PlanarDynamicFlightHorizontal => "Planar Dynamic Flight (Horizontal-scrolling arena)",
        }
    }
}

fn infer_regime(gravity: f32, env_drift_y: f32, scroll_dx: i32, scroll_dy: i32) -> WorldRegime {
    if gravity > 0.05 || env_drift_y > 0.05 {
        WorldRegime::TerrestrialSideScroller
    } else if scroll_dx <= 8 && scroll_dy <= 8 {
        WorldRegime::PlanarStaticArena
    } else if scroll_dy > scroll_dx {
        WorldRegime::PlanarDynamicFlightVertical
    } else {
        WorldRegime::PlanarDynamicFlightHorizontal
    }
}

fn infer_holon_role(
    regime: WorldRegime,
    t: &cc_field::model2d::TrackRecord,
    ground_top: i32,
) -> &'static str {
    if t.kind == "player" {
        return "Player Avatar";
    }
    match regime {
        WorldRegime::TerrestrialSideScroller => {
            let dx = t.span[0];
            let dy = t.span[1];
            let is_near_ground = (t.spawn[1] - ground_top).abs() <= 32;
            if dx >= 20 && dy <= 16 {
                if t.velocity[0] < -0.1 {
                    "Terrestrial Patrol (L)"
                } else if t.velocity[0] > 0.1 {
                    "Terrestrial Patrol (R)"
                } else {
                    "Terrestrial Patrol"
                }
            } else if dy >= 20 && dx <= 16 {
                if t.velocity[1] < -0.1 {
                    "Vertical Ascender"
                } else if t.velocity[1] > 0.1 {
                    "Vertical Descender"
                } else {
                    "Vertical Hazard"
                }
            } else if dx <= 16 && dy <= 32 && is_near_ground {
                "Grounded Hazard"
            } else if dx <= 12 && dy <= 12 {
                "Stationary Sentry"
            } else if dy > 24 && dx > 20 {
                "Dynamic Terrestrial"
            } else {
                "Autonomous Actor"
            }
        }
        WorldRegime::PlanarStaticArena => {
            let dx = t.span[0];
            let dy = t.span[1];
            if dx >= 60 && dy >= 60 {
                "Planar Maze Roamer"
            } else if dx >= 30 && dy <= 16 {
                "Corridor Patrol (H)"
            } else if dy >= 30 && dx <= 16 {
                "Corridor Patrol (V)"
            } else if dx <= 12 && dy <= 12 {
                "Stationary Node"
            } else {
                "Planar Autonomous"
            }
        }
        WorldRegime::PlanarDynamicFlightVertical => {
            if t.velocity[1] > 0.15 {
                "Frontal Interceptor (D)"
            } else if t.velocity[1] < -0.15 {
                "Forward Escort (U)"
            } else if t.velocity[0].abs() > 0.2 {
                "Flanking Fighter"
            } else if t.span[0] <= 12 && t.span[1] <= 12 {
                "Stationary Installation"
            } else {
                "Airborne Entity"
            }
        }
        WorldRegime::PlanarDynamicFlightHorizontal => {
            if t.velocity[0] < -0.15 {
                "Frontal Interceptor (L)"
            } else if t.velocity[0] > 0.15 {
                "Forward Escort (R)"
            } else if t.velocity[1].abs() > 0.2 {
                "Flanking Fighter"
            } else {
                "Airborne Entity"
            }
        }
    }
}

fn cmd_overview(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(raw_path) = pos.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: holon-debug overview <path.cc>\n       holon-debug <path.cc>",
        ));
    };
    let path_str = clean_path(raw_path);
    let path = Path::new(path_str);
    let model = Model2D::load(path)?;
    let m = &model.manifest;
    let file_size = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);

    // 1. Camera odometry & traversal calculations
    let (min_sx, max_sx, min_sy, max_sy) = if !m.scroll.is_empty() {
        let min_x = m.scroll.iter().map(|s| s[0]).min().unwrap_or(0);
        let max_x = m.scroll.iter().map(|s| s[0]).max().unwrap_or(0);
        let min_y = m.scroll.iter().map(|s| s[1]).min().unwrap_or(0);
        let max_y = m.scroll.iter().map(|s| s[1]).max().unwrap_or(0);
        (min_x, max_x, min_y, max_y)
    } else {
        (0, 0, 0, 0)
    };
    let scroll_dx = max_sx - min_sx;
    let scroll_dy = max_sy - min_sy;

    // 2. Regime determination
    let regime = infer_regime(m.gravity, m.env_drift[1], scroll_dx, scroll_dy);

    // 3. Lattice kinds count
    let mut counts = [0usize; 4];
    for &k in &m.kinds {
        counts[(k as usize).min(3)] += 1;
    }
    let total_tiles = m.kinds.len().max(1);
    let empty_pct = (counts[0] as f64 * 100.0) / total_tiles as f64;
    let solid_pct = (counts[1] as f64 * 100.0) / total_tiles as f64;
    let oneway_pct = (counts[2] as f64 * 100.0) / total_tiles as f64;
    let bg_pct = (counts[3] as f64 * 100.0) / total_tiles as f64;

    // Visual bar (36 chars)
    let bar_total = 36usize;
    let n_empty = ((counts[0] as f64 / total_tiles as f64) * bar_total as f64).round() as usize;
    let n_solid = ((counts[1] as f64 / total_tiles as f64) * bar_total as f64).round() as usize;
    let n_oneway = ((counts[2] as f64 / total_tiles as f64) * bar_total as f64).round() as usize;
    let n_bg = bar_total.saturating_sub(n_empty + n_solid + n_oneway);
    let lattice_bar = format!(
        "[{}{}{}{}]",
        ".".repeat(n_empty),
        "#".repeat(n_solid),
        "=".repeat(n_oneway),
        "b".repeat(n_bg)
    );

    // 4. Boundary blades breakdown
    let excitable_blades = m.boundaries.iter().filter(|b| b.excitable).count();
    let rigid_blades = m.boundaries.len().saturating_sub(excitable_blades);

    // 5. Partition tracks into primary player vs autonomous holons
    let player_tracks: Vec<(usize, &cc_field::model2d::TrackRecord)> = m
        .tracks
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == "player" || (*i == 0 && !m.tracks.iter().any(|tr| tr.kind == "player")))
        .collect();
    let primary_player = player_tracks.iter().max_by_key(|(_, t)| t.frames).copied();

    let autonomous_tracks: Vec<(usize, &cc_field::model2d::TrackRecord)> = m
        .tracks
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            if let Some((pi, _)) = primary_player {
                *i != pi && t.kind != "player"
            } else {
                *i != 0 && t.kind != "player"
            }
        })
        .collect();

    // 6. Aggregate sprite statistics
    let mut total_pixels = 0u64;
    let mut opaque_pixels = 0u64;
    let mut semi_pixels = 0u64;
    let mut clear_pixels = 0u64;
    let mut min_w = u32::MAX;
    let mut max_w = 0u32;
    let mut min_h = u32::MAX;
    let mut max_h = 0u32;

    for (i, rec) in m.sprites.iter().enumerate() {
        min_w = min_w.min(rec.width);
        max_w = max_w.max(rec.width);
        min_h = min_h.min(rec.height);
        max_h = max_h.max(rec.height);
        if let Ok((w, h, rgba)) = model.sprite_rgba(i) {
            let px_count = (w * h) as u64;
            total_pixels += px_count;
            for px in rgba.chunks_exact(4) {
                match px[3] {
                    255 => opaque_pixels += 1,
                    0 => clear_pixels += 1,
                    _ => semi_pixels += 1,
                }
            }
        }
    }
    let (opaque_pct, semi_pct, clear_pct) = if total_pixels > 0 {
        (
            (opaque_pixels as f64 * 100.0) / total_pixels as f64,
            (semi_pixels as f64 * 100.0) / total_pixels as f64,
            (clear_pixels as f64 * 100.0) / total_pixels as f64,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    println!("===================================================================================================");
    println!(" HOLONENGINE MODEL OVERVIEW: {} (CC2D v{}, {})", path_str, m.version, format_bytes(file_size));
    println!("===================================================================================================");

    // Section 1: Ingest & World Configuration
    println!();
    println!("--- 1. RECORDING & WORLD CONFIGURATION");
    println!("  Source Stream:   {} ({:.2}s..{:.2}s, duration: {:.2}s @ {:.2} fps)",
        m.source, m.start, m.start + m.duration, m.duration, m.fps);
    println!("  World Regime:    {}", regime.label());
    println!("  Viewport:        {} x {} px", m.viewport[0], m.viewport[1]);
    println!("  World Atlas:     {} x {} px (origin: {:?})", m.atlas[0], m.atlas[1], m.origin);
    println!("  Tile Lattice:    {} x {} px, Grid: {} cols x {} rows ({} total cells)",
        m.tile, m.tile, m.cols, m.rows, m.cols * m.rows);
    println!("  Backdrop:        RGB{:?}  {}", m.background, hex_color(m.background));
    if let Some(ref hud) = model.hud {
        println!("  HUD Strip:       y >= {} px (height: {} px, dimensions: {} x {} px)",
            m.hud_top, hud.height, hud.width, hud.height);
    } else {
        println!("  HUD Strip:       None detected (full-screen playfield)");
    }
    if regime == WorldRegime::TerrestrialSideScroller {
        println!("  Ground Plane:    y = {} px (Lattice Row {})", m.ground_top, m.ground_top / (m.tile as i32).max(1));
    }

    // Section 2: Active Information & Physics Priors
    println!();
    println!("--- 2. ACTIVE INFORMATION & ENVIRONMENTAL PRIORS");
    if regime == WorldRegime::TerrestrialSideScroller {
        println!("  Gravity Field:   g_y = {:.4} px/frame² (unbiased free-fall acceleration)", m.gravity);
        println!("  Environmental:   env_drift = [{:.4}, {:.4}] px/frame²", m.env_drift[0], m.env_drift[1]);
        println!("  Control Motor:   lateral drive = {:.2} px/frame ({:.1} px/s), launch impulse J_⊥ = {:.2} px/frame",
            m.control_motor[0], m.control_motor[0] as f64 * m.fps, m.control_motor[1]);
    } else {
        println!("  Gravity Field:   Planar / Zero-G (g = 0.0000 px/frame²)");
        println!("  Environmental:   env_drift = [{:.4}, {:.4}] px/frame²", m.env_drift[0], m.env_drift[1]);
        println!("  Control Motor:   cruising velocity = {:.2} px/frame ({:.1} px/s)",
            m.control_motor[0], m.control_motor[0] as f64 * m.fps);
    }
    println!("  Camera Odometry: {} recorded viewport poses", m.scroll.len());
    let cam_behavior = if scroll_dx <= 8 && scroll_dy <= 8 {
        "Static Viewport (zero pan/drift)".to_string()
    } else if scroll_dy > scroll_dx {
        format!("Vertical Scroll Traversal (y=[{}..{}], Δy={} px)", min_sy, max_sy, scroll_dy)
    } else {
        let has_reverse = m.scroll.windows(2).any(|w| w[1][0] < w[0][0]);
        let mode_desc = if has_reverse { "Bidirectional Horizontal Scroll" } else { "Unidirectional Horizontal Scroll" };
        format!("{} (x=[{}..{}], Δx={} px)", mode_desc, min_sx, max_sx, scroll_dx)
    };
    println!("  Camera Movement: {}", cam_behavior);

    // Section 3: Pregeometry & Boundary Structure
    println!();
    println!("--- 3. IMPLICATE PREGEOMETRY & BOUNDARIES");
    println!("  Lattice Breakdown:  {}", lattice_bar);
    println!("    • Empty:      {:<5} ({:.1}%) - Passable medium", counts[0], empty_pct);
    println!("    • Solid:      {:<5} ({:.1}%) - Impassable boundary terrain", counts[1], solid_pct);
    println!("    • One-Way:    {:<5} ({:.1}%) - Top-landing collision platforms", counts[2], oneway_pct);
    println!("    • Background: {:<5} ({:.1}%) - Non-collidable scenery", counts[3], bg_pct);
    println!("  Geometric Blades:   {} solid terrain runs, {} one-way platforms", m.solids.len(), m.platforms.len());
    println!("  Boundary Blades:    {} total ({} excitable ε0→ε1→ε2 transitions, {} rigid)",
        m.boundaries.len(), excitable_blades, rigid_blades);
    if !m.collectibles.is_empty() {
        println!("  Field Excitations:  {} consumable / localized items (collectibles)", m.collectibles.len());
    }

    // Section 4: Primary Guided Holon
    println!();
    println!("--- 4. PRIMARY GUIDED HOLON (PLAYER AVATAR)");
    if let Some((p_idx, p)) = primary_player {
        let p_pct = (p.frames as f64 * 100.0) / m.scroll.len().max(1) as f64;
        let t_range = if let (Some(first), Some(last)) = (p.trajectory.first(), p.trajectory.last()) {
            format!("frames {}..{} ({} waypoints)", first[0], last[0], p.trajectory.len())
        } else {
            format!("{} frames recorded", p.frames)
        };
        let atlas_w_cov = (p.span[0] as f64 * 100.0) / (m.atlas[0] as f64).max(1.0);
        let spr_desc = p.sprite.map_or("none".to_string(), |s| {
            if let Some(r) = m.sprites.get(s as usize) {
                format!("Sprite #{:<2} ({}x{} px)", s, r.width, r.height)
            } else {
                format!("Sprite #{s}")
            }
        });
        let harm_desc = p.harmonic_sprite.map_or("none".to_string(), |h| {
            if let Some(r) = m.sprites.get(h as usize) {
                format!("Sprite #{:<2} ({}x{} px, 1st CFT harmonic walk mode)", h, r.width, r.height)
            } else {
                format!("Sprite #{h}")
            }
        });

        println!("  Track Index:     H{:02} (Kind: \"{}\")", p_idx, p.kind);
        println!("  Lifespan:        {} frames alive ({:.1}% of recording session), {}", p.frames, p_pct, t_range);
        println!("  Spawn Origin:    ({}, {}) world coordinates", p.spawn[0], p.spawn[1]);
        println!("  Physical BBox:   {} x {} px", p.size[0], p.size[1]);
        println!("  Observed Span:   ΔX = {} px, ΔY = {} px ({:.1}% level width traversed)",
            p.span[0], p.span[1], atlas_w_cov);
        println!("  Characteristic:  velocity = [{:.2}, {:.2}] px/frame", p.velocity[0], p.velocity[1]);
        println!("  Chromatic Spinor: RGB{:?}  {}", p.color, hex_color(p.color));
        println!("  Visual Kernels:  Base DC: {} | Vitality: {}", spr_desc, harm_desc);

        // Mention secondary player segments if any exist
        let other_players: Vec<usize> = player_tracks.iter().map(|(i, _)| *i).filter(|i| *i != p_idx).collect();
        if !other_players.is_empty() {
            let seg_str: Vec<String> = other_players
                .iter()
                .filter_map(|&oi| m.tracks.get(oi).map(|ot| format!("H{:02} ({} frames)", oi, ot.frames)))
                .collect();
            println!("  Other Segments:  {}", seg_str.join(", "));
        }
    } else {
        println!("  No designated player holon detected in recording.");
    }

    // Section 5: Autonomous Process Holons
    println!();
    println!("--- 5. AUTONOMOUS PROCESS HOLONS ({})", autonomous_tracks.len());
    if autonomous_tracks.is_empty() {
        println!("  No autonomous holon tracks recorded.");
    } else {
        println!("  {:<4} | {:<24} | {:<10} | {:<13} | {:<13} | {:<17} | {:<10} | {:<11}",
            "ID", "Inferred Role", "Spawn", "Span (ΔX,ΔY)", "Vel (vx,vy)", "Lifespan", "Signature", "Sprites");
        println!("  -----+--------------------------+------------+---------------+---------------+-------------------+------------+------------");
        for &(i, t) in &autonomous_tracks {
            let role = infer_holon_role(regime, t, m.ground_top);
            let id_s = format!("H{:02}", i);
            let spawn_s = format!("{},{}", t.spawn[0], t.spawn[1]);
            let span_s = format!("{}x{} px", t.span[0], t.span[1]);
            let vel_s = format!("{:.2},{:.2}", t.velocity[0], t.velocity[1]);
            let life_s = if let (Some(f), Some(l)) = (t.trajectory.first(), t.trajectory.last()) {
                format!("{:>3} f ({}..{})", t.frames, f[0], l[0])
            } else {
                format!("{:>3} frames", t.frames)
            };
            let col_hex = hex_color(t.color);
            let spr_s = match (t.sprite, t.harmonic_sprite) {
                (Some(s), Some(h)) => format!("S{:02} + S{:02}", s, h),
                (Some(s), None) => format!("S{:02}", s),
                (None, Some(h)) => format!("harm S{:02}", h),
                (None, None) => "-".to_string(),
            };
            println!("  {:<4} | {:<24} | {:<10} | {:<13} | {:<13} | {:<17} | {:<10} | {:<11}",
                id_s, role, spawn_s, span_s, vel_s, life_s, col_hex, spr_s);
        }
    }

    // Section 6: Visual Memory & Canonical Sprites
    println!();
    println!("--- 6. VISUAL MEMORY & CANONICAL SPRITES");
    let base_sprites = m.tracks.iter().filter_map(|t| t.sprite).count();
    let harm_sprites = m.tracks.iter().filter_map(|t| t.harmonic_sprite).count();
    println!("  Sprite Library:  {} compiled canonical sprites ({} DC appearance modes, {} CFT harmonic vitality modes)",
        m.sprites.len(), base_sprites, harm_sprites);
    if !m.sprites.is_empty() {
        println!("  Dimensions:      widths: {}..{} px, heights: {}..{} px", min_w, max_w, min_h, max_h);
        println!("  Alpha Keying:    {:.1}% opaque, {:.1}% semi-transparent, {:.1}% keyed transparent",
            opaque_pct, semi_pct, clear_pct);
    }
    println!("  Visual Assets:   atlas: {}x{} px ({} px²)", m.atlas[0], m.atlas[1], m.atlas[0] * m.atlas[1]);
    if let Some(ref hud) = model.hud {
        println!("                   hud:   {}x{} px ({} px²)", hud.width, hud.height, hud.width * hud.height);
    }

    // Section 7: Navigation & Deeper Inspection
    println!();
    println!("--- 7. DEEP-DIVE INSPECTION COMMANDS");
    println!("  holon-debug tracks {:<15} Analyze full trajectory waypoints, excursions & velocities", path_str);
    println!("  holon-debug sprites {:<14} View canonical sprite dimensions, byte lengths & alpha statistics", path_str);
    println!("  holon-debug collisions {:<11} Inspect exact solid bounding rects, platforms & boundary blades", path_str);
    println!("  holon-debug export {:<15} Export atlas PNG, HUD PNG, sprite sheets & manifest JSON", format!("{} <dir>", path_str));
    println!("  holon unfold {:<21} Launch interactive Clifford dynamical simulation", path_str);
    println!("===================================================================================================");

    Ok(())
}

fn cmd_model(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(raw_path) = pos.first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug model <path.cc>"));
    };
    let path = Path::new(clean_path(raw_path));
    let model = Model2D::load(path)?;
    let m = &model.manifest;

    let mut counts = [0usize; 4];
    for &k in &m.kinds {
        counts[(k as usize).min(3)] += 1;
    }

    println!("================================================================================");
    println!(" HolonEngine Model Manifest: {}", path.display());
    println!("================================================================================");
    println!("Version:         CC2D v{}", m.version);
    println!("Source Stream:   {} ({:.2}s..{:.2}s @ {:.2} fps)", m.source, m.start, m.start + m.duration, m.fps);
    println!("Viewport:        {} x {} px", m.viewport[0], m.viewport[1]);
    println!("World Atlas:     {} x {} px, origin: {:?}", m.atlas[0], m.atlas[1], m.origin);
    println!("Tile Lattice:    {} px, Grid: {} cols x {} rows", m.tile, m.cols, m.rows);
    println!("Ground Top:      y = {} px", m.ground_top);
    println!("HUD Top:         y = {} px (strip: {} px)", m.hud_top, model.hud.as_ref().map_or(0, |h| h.height));
    println!("Backdrop RGB:    {:?}", m.background);
    println!("Learned Drift:   env_drift = {:?} (gravity: {:.4})", m.env_drift, m.gravity);
    println!("Learned Motor:   control_motor = {:?}", m.control_motor);
    println!("Scroll Samples:  {} recorded viewport offsets", m.scroll.len());
    println!("--------------------------------------------------------------------------------");
    println!("Lattice Kinds:   empty={} solid={} oneway={} background={}", counts[0], counts[1], counts[2], counts[3]);
    println!("Geometry Blades: {} solid runs, {} one-way platforms, {} boundary blades, {} collectibles",
        m.solids.len(), m.platforms.len(), m.boundaries.len(), m.collectibles.len());
    println!("Holon Entities:  {} kinematic tracks, {} compiled sprites", m.tracks.len(), m.sprites.len());
    println!("================================================================================");
    Ok(())
}

fn cmd_tracks(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(raw_path) = pos.first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug tracks <path.cc>"));
    };
    let path = Path::new(clean_path(raw_path));
    let model = Model2D::load(path)?;
    let m = &model.manifest;

    println!("=================================================================================================");
    println!(" Kinematic Tracks Analysis: {} ({} tracks total)", path.display(), m.tracks.len());
    println!("=================================================================================================");
    println!(" {:<3} | {:<10} | {:<10} | {:<9} | {:<6} | {:<15} | {:<13} | {:<7} | {:<7} | {:<12}",
        "ID", "Kind", "Spawn", "Span", "Frames", "Velocity", "Color", "Sprite", "Harmonic", "Time Range");
    println!("-------------------------------------------------------------------------------------------------");
    for (i, t) in m.tracks.iter().enumerate() {
        let spawn_str = format!("{},{}", t.spawn[0], t.spawn[1]);
        let span_str = format!("{},{}", t.span[0], t.span[1]);
        let vel_str = format!("{:.2},{:.2}", t.velocity[0], t.velocity[1]);
        let col_str = format!("[{},{},{}]", t.color[0], t.color[1], t.color[2]);
        let spr_str = t.sprite.map_or("-".to_string(), |s| s.to_string());
        let harm_str = t.harmonic_sprite.map_or("-".to_string(), |h| h.to_string());
        let t_range = if let (Some(first), Some(last)) = (t.trajectory.first(), t.trajectory.last()) {
            format!("{}..{} ({})", first[0], last[0], t.trajectory.len())
        } else {
            "-".to_string()
        };
        println!(" {:<3} | {:<10} | {:<10} | {:<9} | {:<6} | {:<15} | {:<13} | {:<7} | {:<7} | {:<12}",
            i, t.kind, spawn_str, span_str, t.frames, vel_str, col_str, spr_str, harm_str, t_range);
    }
    println!("=================================================================================================");
    Ok(())
}

fn cmd_sprites(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(raw_path) = pos.first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug sprites <path.cc>"));
    };
    let path = Path::new(clean_path(raw_path));
    let model = Model2D::load(path)?;

    println!("=================================================================================================");
    println!(" Compiled Sprites Analysis: {} ({} sprites)", path.display(), model.manifest.sprites.len());
    println!("=================================================================================================");
    println!(" {:<3} | {:<15} | {:<9} | {:<7} | {:<10} | {:<9} | {:<9} | {:<9} | Used By Tracks",
        "ID", "Kind", "Size", "Bytes", "Alpha (min..max)", "Opaque", "Semi", "Clear");
    println!("-------------------------------------------------------------------------------------------------");
    for (i, rec) in model.manifest.sprites.iter().enumerate() {
        let size_str = format!("{}x{}", rec.width, rec.height);
        let mut min_a = 255u8;
        let mut max_a = 0u8;
        let mut opaque = 0usize;
        let mut semi = 0usize;
        let mut clear = 0usize;

        if let Ok((w, h, rgba)) = model.sprite_rgba(i) {
            let n = (w * h) as usize;
            for px in 0..n {
                let a = rgba[px * 4 + 3];
                min_a = min_a.min(a);
                max_a = max_a.max(a);
                if a == 255 {
                    opaque += 1;
                } else if a > 0 {
                    semi += 1;
                } else {
                    clear += 1;
                }
            }
        }
        let alpha_str = format!("{}..{}", min_a, max_a);
        let mut users = Vec::new();
        for (ti, t) in model.manifest.tracks.iter().enumerate() {
            if t.sprite == Some(i as u32) {
                users.push(format!("t{}(base)", ti));
            }
            if t.harmonic_sprite == Some(i as u32) {
                users.push(format!("t{}(harm)", ti));
            }
        }
        let users_str = if users.is_empty() { "-".to_string() } else { users.join(", ") };
        println!(" {:<3} | {:<15} | {:<9} | {:<7} | {:<16} | {:<9} | {:<9} | {:<9} | {}",
            i, rec.kind, size_str, rec.bytes, alpha_str, opaque, semi, clear, users_str);
    }
    println!("=================================================================================================");
    Ok(())
}

fn cmd_collisions(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    let Some(raw_path) = pos.first() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug collisions <path.cc>"));
    };
    let path = Path::new(clean_path(raw_path));
    let model = Model2D::load(path)?;
    let m = &model.manifest;

    println!("================================================================================");
    println!(" Collision Boundaries: {} ({} blades, {} platforms, {} solids)", path.display(), m.boundaries.len(), m.platforms.len(), m.solids.len());
    println!("================================================================================");
    println!("--- Solids (World Rectangles) ---");
    for (i, s) in m.solids.iter().enumerate() {
        let w = s[2] - s[0] + 1;
        let h = s[3] - s[1] + 1;
        println!("  Solid {:<2}: x=[{}..{}], y=[{}..{}], ({}x{} px)", i, s[0], s[2], s[1], s[3], w, h);
    }
    println!("--- One-Way Platforms ---");
    for (i, p) in m.platforms.iter().enumerate() {
        let w = p[2] - p[0] + 1;
        let h = p[3] - p[1] + 1;
        println!("  Platform {:<2}: x=[{}..{}], y=[{}..{}], ({}x{} px)", i, p[0], p[2], p[1], p[3], w, h);
    }
    println!("--- Boundary Blades ---");
    for (i, b) in m.boundaries.iter().enumerate() {
        let r = b.rect;
        let w = r[2] - r[0] + 1;
        let h = r[3] - r[1] + 1;
        let perm_str = format!("[{}, {}, {}, {}]", b.permeability[0], b.permeability[1], b.permeability[2], b.permeability[3]);
        let col_str = format!("[{}, {}, {}]", b.frame_color[0], b.frame_color[1], b.frame_color[2]);
        println!("  Blade {:<2}: rect=[{}..{}, {}..{}] ({}x{}), perm={}, excitable={}, frame_color={}",
            i, r[0], r[2], r[1], r[3], w, h, perm_str, b.excitable, col_str);
    }
    println!("================================================================================");
    Ok(())
}

fn cmd_frame_diff(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    if pos.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug frame-diff <img1.png> <img2.png> [--threshold 16]"));
    }
    let p1 = Path::new(pos[0]);
    let p2 = Path::new(pos[1]);
    let thresh: u32 = opt(args, "--threshold").and_then(|v| v.parse().ok()).unwrap_or(16);

    let img1 = image::open(p1).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?.to_rgb8();
    let img2 = image::open(p2).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?.to_rgb8();

    if img1.dimensions() != img2.dimensions() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("dimension mismatch: {:?} vs {:?}", img1.dimensions(), img2.dimensions()),
        ));
    }
    let (w, h) = img1.dimensions();
    let total = (w * h) as usize;
    let mut diff_cnt = 0usize;
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut sum_dr = 0u64;
    let mut sum_dg = 0u64;
    let mut sum_db = 0u64;

    for y in 0..h {
        for x in 0..w {
            let p1_px = img1.get_pixel(x, y);
            let p2_px = img2.get_pixel(x, y);
            let dr = (p1_px[0] as i32 - p2_px[0] as i32).abs() as u32;
            let dg = (p1_px[1] as i32 - p2_px[1] as i32).abs() as u32;
            let db = (p1_px[2] as i32 - p2_px[2] as i32).abs() as u32;
            let max_d = dr.max(dg).max(db);
            if max_d >= thresh {
                diff_cnt += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                sum_dr += dr as u64;
                sum_dg += dg as u64;
                sum_db += db as u64;
            }
        }
    }

    println!("================================================================================");
    println!(" Visual Frame Diff Analysis");
    println!("   Image 1: {}", pos[0]);
    println!("   Image 2: {}", pos[1]);
    println!("   Dimensions: {} x {} px, Threshold: {}", w, h, thresh);
    println!("================================================================================");
    println!("Differing Pixels: {} / {} ({:.2}%)", diff_cnt, total, (diff_cnt as f64 * 100.0) / total as f64);
    if diff_cnt > 0 {
        let avg_dr = sum_dr as f64 / diff_cnt as f64;
        let avg_dg = sum_dg as f64 / diff_cnt as f64;
        let avg_db = sum_db as f64 / diff_cnt as f64;
        println!("Bounding Box:     x=[{}..{}] (w={}), y=[{}..{}] (h={})",
            min_x, max_x, max_x - min_x + 1, min_y, max_y, max_y - min_y + 1);
        println!("Mean Delta RGB:   ΔR={:.1}, ΔG={:.1}, ΔB={:.1}", avg_dr, avg_dg, avg_db);
    } else {
        println!("Images are visually identical under threshold {thresh}.");
    }
    println!("================================================================================");
    Ok(())
}

fn cmd_export(args: &[String]) -> io::Result<()> {
    let pos = positional(args);
    if pos.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "usage: holon-debug export <path.cc> <out_dir>"));
    }
    let model_path = Path::new(clean_path(pos[0]));
    let out_dir = Path::new(pos[1]);
    fs::create_dir_all(out_dir)?;

    let model = Model2D::load(model_path)?;
    println!("Exporting {} to {}", model_path.display(), out_dir.display());

    // Export atlas
    let atlas_png = model.atlas_png()?;
    let atlas_path = out_dir.join("atlas.png");
    fs::write(&atlas_path, &atlas_png)?;
    println!("  -> Wrote atlas: {}", atlas_path.display());

    // Export HUD if present
    if let Ok(Some(hud_png)) = model.hud_png() {
        let hud_path = out_dir.join("hud.png");
        fs::write(&hud_path, &hud_png)?;
        println!("  -> Wrote HUD: {}", hud_path.display());
    }

    // Export all sprites
    for (i, rec) in model.manifest.sprites.iter().enumerate() {
        if let Some(png_bytes) = model.sprites.get(i) {
            let fname = format!("sprite_{:02}_{}.png", i, rec.kind);
            let sp_path = out_dir.join(&fname);
            fs::write(&sp_path, png_bytes)?;
            println!("  -> Wrote sprite {:<2}: {} ({}x{})", i, fname, rec.width, rec.height);
        }
    }

    // Export manifest JSON
    let manifest_json = serde_json::to_string_pretty(&model.manifest).map_err(io::Error::other)?;
    let manifest_path = out_dir.join("manifest.json");
    fs::write(&manifest_path, manifest_json)?;
    println!("  -> Wrote manifest: {}", manifest_path.display());
    println!("Export completed successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_path() {
        assert_eq!(clean_path("smb3.cc"), "smb3.cc");
        assert_eq!(clean_path("[smb3.cc]"), "smb3.cc");
        assert_eq!(clean_path("[[smb3.cc]]"), "smb3.cc");
        assert_eq!(clean_path("<smb3.cc>"), "smb3.cc");
        assert_eq!(clean_path("  [[  model.cc  ]]  "), "model.cc");
    }

    #[test]
    fn test_infer_regime() {
        assert_eq!(infer_regime(0.35, 0.35, 744, 0), WorldRegime::TerrestrialSideScroller);
        assert_eq!(infer_regime(0.0, 0.0, 0, 0), WorldRegime::PlanarStaticArena);
        assert_eq!(infer_regime(0.0, 0.0, 0, 275), WorldRegime::PlanarDynamicFlightVertical);
        assert_eq!(infer_regime(0.0, 0.0, 300, 20), WorldRegime::PlanarDynamicFlightHorizontal);
    }

    #[test]
    fn test_overview_command_and_alias() {
        for path in ["smb3.cc", "pacman.cc", "raptor.cc"] {
            if Path::new(path).exists() {
                assert!(cmd_overview(&[path.to_string()]).is_ok());
                assert!(cmd_overview(&[format!("[[{}]]", path)]).is_ok());
            }
        }
    }
}
