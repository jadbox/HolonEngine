//! Diagnostic: per-band SSD shift between consecutive frames (parallax check).
//!
//! ```sh
//! cargo run --release -p cc_field --example band_dump -- [video] [start] [dur]
//! ```

use std::path::PathBuf;

use cc_field::video_io::{self, RawFrame};

fn gray_band(frame: &RawFrame, y0: usize, y1: usize) -> (Vec<f32>, usize, usize) {
    let w = frame.width as usize;
    let h = (y1.min(frame.height as usize)).saturating_sub(y0);
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = ((y + y0) * w + x) * 3;
            out[y * w + x] = 0.299 * frame.rgb[i] as f32
                + 0.587 * frame.rgb[i + 1] as f32
                + 0.114 * frame.rgb[i + 2] as f32;
        }
    }
    (out, w, h)
}

fn best_ssd(prev: &[f32], cur: &[f32], w: usize, h: usize, m: i32) -> ([i32; 2], f32) {
    let mut best = ([0, 0], f32::MAX);
    for dy in -m..=m {
        for dx in -m..=m {
            let mut sum = 0.0f64;
            let mut n = 0.0f64;
            for y in 0..h as i32 {
                let cy = y + dy;
                if cy < 0 || cy >= h as i32 {
                    continue;
                }
                for x in 0..w as i32 {
                    let cx = x + dx;
                    if cx < 0 || cx >= w as i32 {
                        continue;
                    }
                    let d = (prev[y as usize * w + x as usize] - cur[cy as usize * w + cx as usize])
                        as f64;
                    sum += d * d;
                    n += 1.0;
                }
            }
            let mse = (sum / n.max(1.0)) as f32;
            if mse < best.1 {
                best = ([dx, dy], mse);
            }
        }
    }
    best
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let video = PathBuf::from(
        args.get(1)
            .map(String::as_str)
            .unwrap_or("example_input2d/smb3_w1_1_256x240.mp4"),
    );
    let start: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(19.0);
    let dur: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let bands = [(0usize, 60usize), (60, 190), (0, 190)];

    let mut reader = video_io::FrameReader::open_range(&video, Some(start), Some(dur))?;
    let mut prev: Vec<Option<(Vec<f32>, usize, usize)>> = vec![None; bands.len()];
    let mut idx = 0usize;
    while let Some(frame) = reader.next_frame()? {
        for (bi, (y0, y1)) in bands.iter().enumerate() {
            let (g, w, h) = gray_band(&frame, *y0, *y1);
            if let Some((p, pw, ph)) = &prev[bi] {
                let (shift, mse) = best_ssd(p, &g, *pw, *ph, 8);
                let world = [-shift[0], -shift[1]];
                if bi == 2 {
                    let mse0 = {
                        let (_, m) = best_ssd(p, &g, *pw, *ph, 0);
                        m
                    };
                    eprintln!(
                        "f{idx} band[{y0}..{y1}) shift={world:?} mse={mse:.1} mse@0={mse0:.1}"
                    );
                }
            }
            prev[bi] = Some((g, w, h));
        }
        idx += 1;
    }
    Ok(())
}
