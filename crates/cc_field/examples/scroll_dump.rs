//! Diagnostic: compare CFT phase-correlation scroll vs brute-force SSD.
//!
//! ```sh
//! cargo run --release -p cc_field --example scroll_dump -- [video] [start] [dur] [hud_top]
//! ```

use std::path::PathBuf;

use cc_field::enfold2d::ScrollEstimator;
use cc_field::video_io::{self, RawFrame};

fn gray(frame: &RawFrame, hud_top: usize) -> Vec<f32> {
    let w = frame.width as usize;
    let h = hud_top.min(frame.height as usize);
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            out[y * w + x] = 0.299 * frame.rgb[i] as f32
                + 0.587 * frame.rgb[i + 1] as f32
                + 0.114 * frame.rgb[i + 2] as f32;
        }
    }
    out
}

/// Brute-force integer SSD over `[-m, m]` on the overlapping region.
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
                    let p = prev[(y as usize) * w + x as usize];
                    let c = cur[(cy as usize) * w + cx as usize];
                    let d = (p - c) as f64;
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
    let dur: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(21.0);
    let hud_top: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(193);
    let (w, h) = (256usize, hud_top);

    let mut reader = video_io::FrameReader::open_range(&video, Some(start), Some(dur))?;
    let mut est = ScrollEstimator::new(w, hud_top);
    let mut prev_gray: Option<Vec<f32>> = None;
    let mut cft_cum = [0i32, 0];
    let mut xcor_cum = [0i32, 0];
    let mut ssd_cum = [0i32, 0];
    let mut idx = 0usize;
    let mut agree = 0usize;
    let mut xagree = 0usize;
    while let Some(frame) = reader.next_frame()? {
        let g = gray(&frame, hud_top);
        let (phas, pconf, cross, cconf) = est.push_both(&frame);
        cft_cum[0] += phas[0];
        cft_cum[1] += phas[1];
        xcor_cum[0] += cross[0];
        xcor_cum[1] += cross[1];
        if let Some(p) = &prev_gray {
            let (ssd, mse) = best_ssd(p, &g, w, h, 6);
            // SSD argmin is the displacement of the *content*: if content moved
            // left by s, cur(x) = prev(x+s), so `cur` matches `prev` shifted by
            // -s. Convert to world delta = -argmin.
            let world = [-ssd[0], -ssd[1]];
            ssd_cum[0] += world[0];
            ssd_cum[1] += world[1];
            if world == phas {
                agree += 1;
            }
            if world == cross {
                xagree += 1;
            }
            if world != cross {
                eprintln!(
                    "MISMATCH f{idx}: phas={phas:?}({pconf:.0}) cross={cross:?}({cconf:.0}) ssd={world:?} mse={mse:.0}"
                );
            }
            if idx % 100 == 0 {
                eprintln!(
                    "f{idx}: phas={phas:?}({pconf:.0}) cross={cross:?}({cconf:.0}) ssd={world:?} mse={mse:.0} cum: phas={cft_cum:?} cross={xcor_cum:?} ssd={ssd_cum:?}"
                );
            }
        }
        prev_gray = Some(g);
        idx += 1;
    }
    eprintln!(
        "frames={idx} agree_phase={agree} ({:.1}%) agree_cross={xagree} ({:.1}%) phas_cum={cft_cum:?} cross_cum={xcor_cum:?} ssd_cum={ssd_cum:?}",
        100.0 * agree as f64 / idx.max(1) as f64,
        100.0 * xagree as f64 / idx.max(1) as f64,
    );
    Ok(())
}
