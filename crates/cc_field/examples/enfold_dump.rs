//! Diagnostic: run the 2D compiler on the SMB3 sample and dump atlas,
//! tile-classification overlay, tile JSON, and a manifest summary.
//!
//! ```sh
//! cargo run -p cc_field --example enfold_dump -- [video] [start] [dur] [out_dir]
//! ```

use std::path::PathBuf;

use cc_field::enfold2d;
use cc_field::video_io;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let video = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("example_input2d/smb3_w1_1_256x240.mp4"),
    );
    let start: Option<f64> = args.get(2).and_then(|s| s.parse().ok());
    let dur: Option<f64> = args.get(3).and_then(|s| s.parse().ok());
    let out = PathBuf::from(
        args.get(4)
            .map(String::as_str)
            .unwrap_or("/tmp/opencode/enfold"),
    );
    std::fs::create_dir_all(&out)?;

    let opts = enfold2d::EnfoldOptions {
        start,
        duration: dur,
        max_frames: None,
        hud_top: None,
    };
    let t0 = std::time::Instant::now();
    let level = enfold2d::enfold(&video, &opts)?;
    eprintln!(
        "enfold: {:.2}s  start={:.2}s dur={:.2}s hud_top={} atlas={}x{} frames={}",
        t0.elapsed().as_secs_f64(),
        level.start,
        level.duration,
        level.hud_top,
        level.atlas.image.width,
        level.atlas.image.height,
        level.scroll.len()
    );
    eprintln!(
        "tiles: {}x{} ({} px/tile), origin_y={}, bg={:?}, ground_top={}, boxes={}, platforms={}, solids={}",
        level.tiles.cols,
        level.tiles.rows,
        enfold2d::TILE,
        level.tiles.origin_y,
        level.tiles.background,
        level.tiles.ground_top,
        level.tiles.boxes.len(),
        level.tiles.platforms.len(),
        level.tiles.solids.len()
    );
    for (i, t) in level.tracks.iter().enumerate() {
        let (w, h) = t.median_size();
        let mut wx = 0.0;
        let mut sx = 0.0;
        let mut n = 0.0;
        let mut wy = 0.0;
        for (fi, b) in &t.boxes {
            let s = level.scroll.get(*fi).copied().unwrap_or([0, 0]);
            wx += b.center()[0] + s[0] as f64;
            wy += b.center()[1] + s[1] as f64;
            sx += b.center()[0];
            n += 1.0;
        }
        if n > 0.0 {
            eprintln!(
                "track {i}: kind={} frames={} size={}x{} world=({:.0},{:.0}) screen_x={:.0} first={:?} last_f={:?}",
                t.kind.as_str(),
                t.boxes.len(),
                w,
                h,
                wx / n,
                wy / n,
                sx / n,
                t.boxes.first().map(|b| b.0),
                t.boxes.last().map(|b| b.0)
            );
        }
    }

    video_io::write_png(&out.join("atlas.png"), level.atlas.image.width as u32, level.atlas.image.height as u32, &level.atlas.image.rgb.clone())?;
    video_io::write_png(&out.join("hud.png"), level.hud.width as u32, level.hud.height as u32, &level.hud.rgb.clone())?;

    // Tile overlay: color each tile by class.
    let mut overlay = level.atlas.image.clone();
    let palette: [[u8; 3]; 4] = [
        [0, 0, 0],     // empty
        [255, 40, 40], // solid
        [255, 0, 255], // one-way
        [40, 40, 255], // background
    ];
    for ty in 0..level.tiles.rows {
        for tx in 0..level.tiles.cols {
            let k = level.tiles.kind(tx, ty);
            let y0 = (level.tiles.origin_y + (ty * enfold2d::TILE) as i32) as usize;
            for x in tx * enfold2d::TILE..(tx + 1) * enfold2d::TILE {
                for y in y0..y0 + 2 {
                    if x < overlay.width && y < overlay.height {
                        overlay.set_pixel(x, y, palette[k as usize]);
                    }
                }
            }
            if ty == level.tiles.rows - 1 {
                for y in (y0 + enfold2d::TILE).saturating_sub(2)..y0 + enfold2d::TILE {
                    for x in tx * enfold2d::TILE..(tx + 1) * enfold2d::TILE {
                        if x < overlay.width && y < overlay.height {
                            overlay.set_pixel(x, y, palette[k as usize]);
                        }
                    }
                }
            }
        }
    }
    video_io::write_png(
        &out.join("tiles.png"),
        overlay.width as u32,
        overlay.height as u32,
        &overlay.rgb,
    )?;

    // Tile JSON.
    let mut kinds = String::from("[\n");
    for ty in 0..level.tiles.rows {
        if ty > 0 {
            kinds.push_str(",\n");
        }
        kinds.push('[');
        for tx in 0..level.tiles.cols {
            if tx > 0 {
                kinds.push(',');
            }
            kinds.push_str(&format!("{}", level.tiles.kind(tx, ty) as u8));
        }
        kinds.push(']');
    }
    kinds.push_str("\n]\n");
    std::fs::write(out.join("tiles.json"), kinds)?;

    // Scroll summary.
    let mut scroll = String::new();
    let mut acc = [0i32; 2];
    for (i, s) in level.scroll.iter().enumerate() {
        acc[0] += s[0] - if i == 0 { 0 } else { level.scroll[i - 1][0] };
        acc[1] += s[1] - if i == 0 { 0 } else { level.scroll[i - 1][1] };
        scroll.push_str(&format!("{},{},{}\n", i, s[0], s[1]));
    }
    std::fs::write(out.join("scroll.csv"), scroll)?;

    let stats = level.scroll.iter().fold((0, 0), |mut a, s| {
        a.0 = a.0.max(s[0]);
        a.1 = a.1.min(s[1]);
        a
    });
    eprintln!("scroll: max_x={} min_y={}", stats.0, stats.1);

    // Solid/one-way/platform rectangles.
    let mut rects = String::new();
    for r in &level.tiles.platforms {
        rects.push_str(&format!(
            "oneway {},{},{},{}\n",
            r.x0, r.y0, r.x1, r.y1
        ));
    }
    for r in &level.tiles.boxes {
        rects.push_str(&format!("box {},{},{},{}\n", r.x0, r.y0, r.x1, r.y1));
    }
    std::fs::write(out.join("rects.txt"), rects)?;

    // Sprite dump.
    for (i, t) in level.tracks.iter().enumerate() {
        if let Some(s) = &t.sprite {
            let path = out.join(format!("sprite_{i}_{}.png", t.kind.as_str()));
            let mut rgba = image::RgbaImage::new(s.width as u32, s.height as u32);
            for y in 0..s.height {
                for x in 0..s.width {
                    let i = (y * s.width + x) * 4;
                    rgba.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([s.rgba[i], s.rgba[i + 1], s.rgba[i + 2], s.rgba[i + 3]]),
                    );
                }
            }
            rgba.save_with_format(&path, image::ImageFormat::Png)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    println!("dumped to {}", out.display());
    Ok(())
}
