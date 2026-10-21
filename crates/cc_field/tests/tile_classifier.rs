//! The tile classifier must work on any side-scroller, not just SMB3.
//!
//! A synthetic scene with none of SMB3's palette exercises the whole generic
//! pipeline: learned backdrop, per-column ground line, one-tile shelf, a
//! grounded bush (scenery), a round cloud (scenery) and a flat floating
//! platform (one-way).

use cc_field::enfold2d::{extract_2d_tiles_with, TileOptions, WorldAtlas, TILE};
use cc_field::{RgbImage8, TileKind};

const W: usize = 160;
const H: usize = 64;

fn scene() -> RgbImage8 {
    let sky = [90, 140, 200];
    let soil = [120, 80, 40];
    let mut img = RgbImage8::new(W, H);
    img.rgb
        .chunks_exact_mut(3)
        .for_each(|p| p.copy_from_slice(&sky));

    // Ground band: y 48..64.
    for y in 48..H {
        for x in 0..W {
            img.set_pixel(x, y, soil);
        }
    }
    // Raised shelf: one tile up, three tiles wide (x 64..112).
    for y in 32..48 {
        for x in 64..112 {
            img.set_pixel(x, y, soil);
        }
    }
    // Grounded bush, two tiles wide with a rounded top.
    for x in 16..48 {
        let dx = (x as f64 - 32.0) / 16.0;
        let top = (44.0 - 6.0 * (1.0 - dx * dx)) as usize;
        for y in top..48 {
            img.set_pixel(x, y, [40, 160, 60]);
        }
    }
    // Round cloud above the playfield; must stay scenery.
    for x in 8..40 {
        let dx = (x as f64 - 24.0) / 16.0;
        let top = (14.0 - 8.0 * (1.0 - dx * dx).sqrt()) as usize;
        for y in top..20 {
            img.set_pixel(x, y, [250, 250, 250]);
        }
    }
    // Flat floating platform: one-way surface at y = 16 (on the lattice).
    for y in 16..28 {
        for x in 120..152 {
            img.set_pixel(x, y, [200, 60, 180]);
        }
    }
    img
}

fn atlas() -> WorldAtlas {
    WorldAtlas {
        image: scene(),
        origin: [0, 0],
    }
}

#[test]
fn learns_backdrop_and_ground_line() {
    let t = extract_2d_tiles_with(&atlas(), &TileOptions::default());
    assert_eq!((t.cols, t.rows), (W / TILE, H / TILE));
    assert_eq!(t.origin_y, 0);
    assert_eq!(t.ground_top, 48);
    let bg = t.background;
    assert!((bg[0] as i32 - 90).abs() <= 8, "backdrop {bg:?}");
    assert!((bg[1] as i32 - 140).abs() <= 8, "backdrop {bg:?}");
    assert!((bg[2] as i32 - 200).abs() <= 8, "backdrop {bg:?}");
}

#[test]
fn shelf_is_solid_only_above_its_own_columns() {
    let t = extract_2d_tiles_with(&atlas(), &TileOptions::default());
    let at = |tx: usize, ty: usize| t.kinds[ty * t.cols + tx];
    for tx in 0..t.cols {
        assert_eq!(at(tx, 3), TileKind::Solid, "ground hole at {tx}");
    }
    for tx in 4..=6 {
        assert_eq!(at(tx, 2), TileKind::Solid, "shelf missing at {tx}");
    }
    // Above the low ground, outside the shelf: nothing solid.
    for tx in [0usize, 1, 2, 8, 9] {
        assert_ne!(at(tx, 2), TileKind::Solid, "phantom shelf at {tx}");
    }
    assert!(t
        .solids
        .iter()
        .any(|r| r.x0 == 4 * TILE as i32 && r.y0 == 32));
}

#[test]
fn flat_float_is_one_way_clouds_and_bushes_are_scenery() {
    let t = extract_2d_tiles_with(&atlas(), &TileOptions::default());
    let at = |tx: usize, ty: usize| t.kinds[ty * t.cols + tx];
    // Platform top at y=16 → row 1, columns 7..=9.
    for tx in 7..=9 {
        assert_eq!(at(tx, 1), TileKind::OneWay, "platform missing at {tx}");
    }
    assert_eq!(t.platforms.len(), 1, "{:?}", t.platforms);
    assert_eq!(t.platforms[0].y0, 16);
    // The cloud and the bush are decoration, not collision.
    for tx in 0..=2 {
        assert_ne!(at(tx, 1), TileKind::OneWay, "cloud became a platform");
    }
    for tx in 1..=2 {
        assert_ne!(at(tx, 2), TileKind::Solid, "bush became terrain");
    }
    assert!(t.boxes.iter().any(|b| b.x0 == 120 && b.y0 == 16));
}

#[test]
fn threshold_options_are_respected() {
    // A tight background tolerance classifies the sky as content, so the
    // ground line is no longer the lowest dense band: the classifier must
    // still produce a valid grid rather than panicking.
    let opts = TileOptions {
        bg_tol: 1,
        ..TileOptions::default()
    };
    let t = extract_2d_tiles_with(&atlas(), &opts);
    assert_eq!(t.cols, W / TILE);
    assert_eq!(t.kinds.len(), t.cols * t.rows);
}
