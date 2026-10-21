//! Gate test for the 2D compiler on the SMB3 World 1-1 fixture.
//!
//! Run with `cargo test --release -p cc_field --test smb3_enfold`; the
//! pipeline decodes 630 frames three times, so debug builds take a while.
//!
//! Assertions are structural (registration consistency, terrain layout, HUD
//! fidelity), not per-pixel golden images.

use std::path::Path;

use cc_field::enfold2d::{self, EnfoldOptions, TILE};
use cc_field::video_io::FrameReader;
use cc_field::TileKind;

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example_input2d/smb3_w1_1_256x240.mp4")
}

/// The pipeline decodes 630 frames; share one result across tests.
fn compiled() -> &'static enfold2d::Enfolded2D {
    static LEVEL: std::sync::OnceLock<enfold2d::Enfolded2D> = std::sync::OnceLock::new();
    LEVEL.get_or_init(|| {
        let video = fixture();
        assert!(video.exists(), "missing fixture {}", video.display());
        let opts = EnfoldOptions {
            start: Some(19.0),
            duration: Some(21.0),
            max_frames: None,
            hud_top: None,
        };
        enfold2d::enfold(&video, &opts).expect("enfold")
    })
}

#[test]
fn hud_and_viewport_partition() {
    let level = compiled();
    assert_eq!(level.hud_top, 193, "SMB3 HUD starts at y=193");
    assert_eq!(level.info.width, 256);
    assert_eq!(level.info.height, 240);
    assert_eq!(level.hud.width, 256);
    assert_eq!(level.hud.height, 240 - 193);

    // The HUD is static: the first decoded frame must match it almost exactly.
    let mut reader = FrameReader::open_range(&fixture(), Some(19.0), Some(0.5)).unwrap();
    let first = reader.next_frame().unwrap().unwrap();
    // Bucket means are quantized, and sprites composite over the strip, so
    // compare permissively: mean error plus the fraction of near matches.
    let mut err = 0u64;
    let mut near = 0usize;
    let mut total = 0usize;
    for y in 193..240 {
        for x in 0..256 {
            let i = (y * 256 + x) * 3;
            let h = ((y - 193) * 256 + x) * 3;
            let mut worst = 0i32;
            for c in 0..3 {
                let d = (first.rgb[i + c] as i32 - level.hud.rgb[h + c] as i32).abs();
                err += d as u64;
                worst = worst.max(d);
            }
            if worst <= 24 {
                near += 1;
            }
            total += 1;
        }
    }
    let mad = err as f64 / (total * 3) as f64;
    let ratio = near as f64 / total as f64;
    assert!(mad < 10.0, "HUD mean error {mad:.1} >= 10");
    assert!(ratio > 0.93, "HUD near-match {ratio:.3} <= 0.93");
}

#[test]
fn scroll_is_consistent_with_the_atlas() {
    let level = compiled();
    let atlas = &level.atlas.image;
    let (aw, ah) = (atlas.width, atlas.height);
    let vw = level.info.width as usize;
    let vh = ah.min(level.info.height as usize);

    assert_eq!(level.scroll.first().copied(), Some([0, 0]));
    assert!(level.scroll.iter().all(|s| s[1] == 0), "no vertical scroll");
    let xs: Vec<i32> = level.scroll.iter().map(|s| s[0]).collect();
    assert_eq!(xs.len(), 630);
    let max_x = *xs.iter().max().unwrap();
    let min_x = *xs.iter().min().unwrap();
    // The clip covers more than two screens and returns left afterwards.
    assert!(max_x > 600, "max camera x {max_x} <= 600");
    assert!(
        max_x - min_x > 400,
        "camera range {} too small",
        max_x - min_x
    );
    for w in xs.windows(2) {
        let dx = w[1] - w[0];
        assert!(dx.abs() <= 6, "implausible frame step {dx} at {w:?}");
    }

    // Registration quality: each frame should equal the atlas shifted by the
    // reported offset. A wrong offset blurs the atlas and raises this error.
    let mut reader = FrameReader::open_range(&fixture(), Some(19.0), Some(21.0)).unwrap();
    let mut sum = 0u64;
    let mut count = 0u64;
    let mut frame = 0usize;
    while let Some(raw) = reader.next_frame().unwrap() {
        if frame >= xs.len() {
            break;
        }
        if frame % 15 != 0 {
            frame += 1;
            continue;
        }
        let off = xs[frame].max(0) as usize;
        // Sparse grid: full frames cost more than the signal is worth.
        for y in (0..vh).step_by(4) {
            for x in (0..vw).step_by(4) {
                let sx = (off + x).min(aw - 1);
                let a = (y * aw + sx) * 3;
                let f = (y * vw + x) * 3;
                for c in 0..3 {
                    sum += (raw.rgb[f + c] as i32 - atlas.rgb[a + c] as i32).unsigned_abs() as u64;
                    count += 1;
                }
            }
        }
        frame += 1;
    }
    let mad = sum as f64 / count.max(1) as f64;
    assert!(mad < 16.0, "atlas MAD {mad:.1} >= 16");
}

#[test]
fn terrain_layout() {
    let level = compiled();
    let tiles = &level.tiles;
    assert_eq!(tiles.origin_y, 0);
    assert_eq!(tiles.ground_top, 176);
    assert_eq!(tiles.rows, 13);
    assert!((tiles.cols as usize * TILE) >= level.atlas.image.width);

    let (cols, rows) = (tiles.cols, tiles.rows);
    let at = |tx: usize, ty: usize| tiles.kinds[ty * cols + tx];

    // The floor is continuous under the whole level.
    for tx in 0..cols {
        assert_eq!(
            at(tx, 11),
            TileKind::Solid,
            "ground hole at column {tx}"
        );
    }
    // The one-tile raised shelf near the middle is solid...
    let shelf: Vec<usize> = (0..cols)
        .filter(|&tx| at(tx, 10) == TileKind::Solid)
        .collect();
    assert!(!shelf.is_empty(), "raised shelf not detected");
    assert!(
        shelf.iter().all(|&tx| tx >= 20),
        "shelf leaked into the block cluster: {shelf:?}"
    );
    // ...while the decorated boxes stay background (they are not terrain).
    assert_eq!(at(30, 10), TileKind::Background);
    assert_eq!(at(2, 10), TileKind::Background);

    // The pipe top is a one-way surface, not solid terrain.
    assert_eq!(at(21, 8), TileKind::OneWay);
    assert_eq!(at(22, 8), TileKind::OneWay);
    assert!(
        tiles.platforms.len() >= 4,
        "only {} platforms",
        tiles.platforms.len()
    );
}

#[test]
fn sprites_and_tracks() {
    let level = compiled();
    assert!(!level.tracks.is_empty());
    assert!(level.tracks.len() <= 12, "track cap broken");
    for t in &level.tracks {
        if let Some(s) = &t.sprite {
            assert!(s.width > 0 && s.height > 0);
            assert_eq!(s.rgba.len(), s.width * s.height * 4);
        }
    }
}
