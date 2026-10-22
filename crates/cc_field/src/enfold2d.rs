//! 2D side-scroller compiler: HUD partition, CFT scroll odometry, world atlas,
//! tile classification, and actor extraction.
//!
//! Coordinates: screen/world pixels, `+y` downward (as in the video). The tile
//! grid origin is `(0, 1)`: tile `(tx, ty)` covers `x = 16·tx .. 16·tx+15` and
//! `y = 1 + 16·ty .. 16 + 16·ty`. Frame row 0 is padding. The tile last row is
//! the ground surface row; the runtime extends it downward.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};

use crate::video_io::{self, RawFrame, VideoInfo};

/// Tile edge in pixels.
pub const TILE: usize = 16;
/// World y of the first tile row's top edge.
pub const TILE_ORIGIN_Y: i32 = 1;
/// Sprite-scale prior: actors are at most two tiles across. Larger residual
/// components are parallax layers, decorations or seams, not tracked actors.
pub const ACTOR_MAX_PX: i32 = 48;
/// Minimum residual area for an actor candidate (about half a tile).
pub const ACTOR_MIN_AREA: i32 = 120;

/// Collision class of a 16×16 atlas tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TileKind {
    /// Sky / thin air. Nothing to stand on.
    Empty = 0,
    /// Fully solid on all sides (ground, bricks, `?` blocks, pipes).
    Solid = 1,
    /// Semi-solid: solid only from above (SMB3 screw-top boxes).
    OneWay = 2,
    /// Decorative background (bushes, clouds, hills); non-collidable.
    Background = 3,
}

impl TileKind {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => TileKind::Solid,
            2 => TileKind::OneWay,
            3 => TileKind::Background,
            _ => TileKind::Empty,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Inclusive pixel rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Rect {
    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn width(&self) -> i32 {
        (self.x1 - self.x0 + 1).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.y1 - self.y0 + 1).max(0)
    }

    pub fn center(&self) -> [f64; 2] {
        [
            (self.x0 + self.x1) as f64 / 2.0,
            (self.y0 + self.y1) as f64 / 2.0,
        ]
    }

    pub fn area(&self) -> i32 {
        self.width() * self.height()
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    pub fn expand(&self, m: i32) -> Self {
        Self::new(self.x0 - m, self.y0 - m, self.x1 + m, self.y1 + m)
    }

    pub fn shifted(&self, dx: i32, dy: i32) -> Self {
        Self::new(self.x0 + dx, self.y0 + dy, self.x1 + dx, self.y1 + dy)
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1 && self.y0 <= other.y1 && other.y0 <= self.y1
    }
}

/// Dense RGB image with direct pixel access.
#[derive(Clone, Debug, PartialEq)]
pub struct RgbImage8 {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl RgbImage8 {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rgb: vec![0; width * height * 3],
        }
    }

    pub fn from_raw(width: usize, height: usize, rgb: Vec<u8>) -> Self {
        debug_assert_eq!(rgb.len(), width * height * 3);
        Self { width, height, rgb }
    }

    #[inline]
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, c: [u8; 3]) {
        let i = (y * self.width + x) * 3;
        self.rgb[i] = c[0];
        self.rgb[i + 1] = c[1];
        self.rgb[i + 2] = c[2];
    }

    /// Blit `src` at `(dx, dy)`, clipping at the edges.
    pub fn blit(&mut self, src: &RgbImage8, dx: i32, dy: i32) {
        for sy in 0..src.height {
            let ty = dy + sy as i32;
            if ty < 0 || ty >= self.height as i32 {
                continue;
            }
            for sx in 0..src.width {
                let tx = dx + sx as i32;
                if tx < 0 || tx >= self.width as i32 {
                    continue;
                }
                let s = (sy * src.width + sx) * 3;
                let d = (ty as usize * self.width + tx as usize) * 3;
                self.rgb[d..d + 3].copy_from_slice(&src.rgb[s..s + 3]);
            }
        }
    }
}

fn color_dist(a: [u8; 3], b: [u8; 3]) -> i32 {
    let d0 = a[0] as i32 - b[0] as i32;
    let d1 = a[1] as i32 - b[1] as i32;
    let d2 = a[2] as i32 - b[2] as i32;
    ((d0 * d0 + d1 * d1 + d2 * d2) as f64).sqrt() as i32
}

// ---------------------------------------------------------------------------
// 2D CFT phase correlation
// ---------------------------------------------------------------------------

struct Fft2 {
    w: usize,
    h: usize,
    rows_fwd: Arc<dyn Fft<f32>>,
    rows_inv: Arc<dyn Fft<f32>>,
    cols_fwd: Arc<dyn Fft<f32>>,
    cols_inv: Arc<dyn Fft<f32>>,
}

impl Fft2 {
    fn new(w: usize, h: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        Self {
            w,
            h,
            rows_fwd: planner.plan_fft_forward(w),
            rows_inv: planner.plan_fft_inverse(w),
            cols_fwd: planner.plan_fft_forward(h),
            cols_inv: planner.plan_fft_inverse(h),
        }
    }

    fn transform(&self, buf: &mut [Complex<f32>], inverse: bool) {
        let (w, h) = (self.w, self.h);
        let (row_fft, col_fft) = if inverse {
            (self.rows_inv.clone(), self.cols_inv.clone())
        } else {
            (self.rows_fwd.clone(), self.cols_fwd.clone())
        };
        let mut row = vec![Complex::new(0.0, 0.0); w];
        for y in 0..h {
            row.copy_from_slice(&buf[y * w..(y + 1) * w]);
            row_fft.process(&mut row);
            if inverse {
                for c in row.iter_mut() {
                    *c /= w as f32;
                }
            }
            buf[y * w..(y + 1) * w].copy_from_slice(&row);
        }
        let mut col = vec![Complex::new(0.0, 0.0); h];
        for x in 0..w {
            for y in 0..h {
                col[y] = buf[y * w + x];
            }
            col_fft.process(&mut col);
            if inverse {
                for c in col.iter_mut() {
                    *c /= h as f32;
                }
            }
            for y in 0..h {
                buf[y * w + x] = col[y];
            }
        }
    }
}

/// Phase-correlation shift estimator over a fixed viewport band.
/// [Spectral Camera Discovery: 2D FFT Odometry]
///
/// `push` returns the per-frame integer delta; accumulate for world offsets.
pub struct ScrollEstimator {
    fft: Fft2,
    w: usize,
    h: usize,
    /// Rows actually read from the frame; rows `band_h..h` stay zero.
    band_h: usize,
    win_x: Vec<f32>,
    win_y: Vec<f32>,
    prev: Option<Vec<Complex<f32>>>,
}

impl ScrollEstimator {
    /// `band_h` is the number of top rows to compare (HUD excluded).
    pub fn new(width: usize, band_h: usize) -> Self {
        let w = width.next_power_of_two();
        let h = band_h.next_power_of_two();
        // A window applied to both frames would modulate each differently and
        // destroy the pure-shift relation; the phase-only spectrum already
        // suppresses the DC bias, so keep a flat window.
        let win_x = vec![1.0f32; w];
        let win_y = vec![1.0f32; h];
        Self {
            fft: Fft2::new(w, h),
            w,
            h,
            band_h: band_h.min(h),
            win_x,
            win_y,
            prev: None,
        }
    }

    /// Rows above the HUD only: the static HUD must never enter the
    /// correlation, or its self-similarity pins the peak at zero.
    fn gray(&self, frame: &RawFrame) -> Vec<f32> {
        let mut out = vec![0.0f32; self.w * self.h];
        let w = self.w;
        let rows = self.band_h.min(frame.height as usize);
        let cols = self.w.min(frame.width as usize);
        let mut sum = 0.0f32;
        let mut n = 0.0f32;
        for y in 0..rows {
            for x in 0..cols {
                let i = (y * frame.width as usize + x) * 3;
                let g = 0.299 * frame.rgb[i] as f32
                    + 0.587 * frame.rgb[i + 1] as f32
                    + 0.114 * frame.rgb[i + 2] as f32;
                out[y * w + x] = g;
                sum += g;
                n += 1.0;
            }
        }
        let mean = if n > 0.0 { sum / n } else { 0.0 };
        for y in 0..rows {
            for x in 0..cols {
                out[y * w + x] = (out[y * w + x] - mean) * self.win_x[x] * self.win_y[y];
            }
        }
        out
    }

    fn to_spectrum(&self, gray: &[f32]) -> Vec<Complex<f32>> {
        let mut buf: Vec<Complex<f32>> = gray.iter().map(|&v| Complex::new(v, 0.0)).collect();
        self.fft.transform(&mut buf, false);
        buf
    }

    /// Peak of a correlation plane inside `±max_shift`, with `peak / mean`.
    fn plane_peak(&self, plane: &[Complex<f32>], max_shift: i32) -> ([i32; 2], f32) {
        let mut best = (0i32, 0i32, f32::MIN);
        let mut sum = 0.0f32;
        let mut count = 0.0f32;
        for dy in -max_shift..=max_shift {
            for dx in -max_shift..=max_shift {
                let yy = dy.rem_euclid(self.h as i32) as usize;
                let xx = dx.rem_euclid(self.w as i32) as usize;
                let mag = plane[yy * self.w + xx].norm();
                sum += mag;
                count += 1.0;
                if mag > best.2 {
                    best = (dx, dy, mag);
                }
            }
        }
        let mean = if count > 0.0 { sum / count } else { 1.0 };
        let confidence = if mean > 1e-9 { best.2 / mean } else { 1.0 };
        ([best.0, best.1], confidence)
    }

    /// Feed the next frame; returns the integer `(dx, dy)` delta.
    ///
    /// Plain magnitude-weighted cross-correlation in the frequency domain.
    /// Pure phase flattening (PHAT) is *too* sensitive on this footage: the
    /// compression noise phase dominates and the estimate collapses toward
    /// zero. Both variants are available via [`Self::push_both`] for
    /// diagnostics.
    pub fn push(&mut self, frame: &RawFrame) -> ([i32; 2], f32) {
        let gray = self.gray(frame);
        let cur = self.to_spectrum(&gray);
        let Some(prev) = self.prev.as_ref() else {
            self.prev = Some(cur);
            return ([0, 0], 1.0);
        };
        let n = self.w * self.h;
        let mut cross = vec![Complex::new(0.0, 0.0); n];
        for i in 0..n {
            cross[i] = prev[i] * cur[i].conj();
        }
        self.fft.transform(&mut cross, true);
        // `FFT(prev)·conj(FFT(cur))` peaks at `pos_prev - pos_cur`, so when the
        // content has moved left by `s` (camera advanced by `s`) the peak is at
        // `+s`: the raw peak is already the world-space delta.
        let (peak, confidence) = self.plane_peak(&cross, 8);
        self.prev = Some(cur);
        (peak, confidence)
    }

    /// Diagnostic: `(phase_peak, phase_conf, cross_peak, cross_conf)`.
    pub fn push_both(&mut self, frame: &RawFrame) -> ([i32; 2], f32, [i32; 2], f32) {
        let gray = self.gray(frame);
        let cur = self.to_spectrum(&gray);
        let Some(prev) = self.prev.as_ref() else {
            self.prev = Some(cur);
            return ([0, 0], 1.0, [0, 0], 1.0);
        };
        let n = self.w * self.h;
        let mut cross = vec![Complex::new(0.0, 0.0); n];
        let mut phas = vec![Complex::new(0.0, 0.0); n];
        for i in 0..n {
            let z = prev[i] * cur[i].conj();
            let mag = z.norm();
            cross[i] = z;
            phas[i] = if mag > 1e-9 { z / mag } else { Complex::new(0.0, 0.0) };
        }
        self.fft.transform(&mut cross, true);
        self.fft.transform(&mut phas, true);
        let (cp, cc) = self.plane_peak(&cross, 8);
        let (pp, pc) = self.plane_peak(&phas, 8);
        self.prev = Some(cur);
        (pp, pc, cp, cc)
    }
}

/// Frame-by-frame cumulative integer scroll offsets over the video band.
pub fn estimate_2d_scroll(frames: &[RawFrame], hud_top: usize) -> Vec<[i32; 2]> {
    if frames.is_empty() {
        return Vec::new();
    }
    let mut est = ScrollEstimator::new(frames[0].width as usize, hud_top);
    let mut cum = [0i32, 0];
    let mut out = Vec::with_capacity(frames.len());
    for f in frames {
        let (d, _) = est.push(f);
        cum[0] += d[0];
        cum[1] += d[1];
        out.push(cum);
    }
    out
}

// ---------------------------------------------------------------------------
// HUD partition
// ---------------------------------------------------------------------------

/// HUD row band derived from temporal staticness.
#[derive(Clone, Debug, PartialEq)]
pub struct HudPartition {
    /// First HUD row (world/frame y).
    pub top: usize,
    /// Per-row median temporal deviation, for diagnostics.
    pub row_deviation: Vec<f32>,
}

/// Find the bottom HUD band as the largest static suffix of rows.
///
/// Playfield rows change as the camera scrolls; HUD rows are static except a
/// few changing digits, which the per-row median absorbs.
pub fn partition_hud_and_viewport(frames: &[RawFrame]) -> HudPartition {
    assert!(!frames.is_empty(), "need at least one frame");
    let (w, h) = (frames[0].width as usize, frames[0].height as usize);
    let mut row_deviation = Vec::with_capacity(h);
    for y in 0..h {
        // Temporal std per pixel, then the median across the row.
        let mut devs = Vec::with_capacity(w);
        for x in 0..w {
            let mut vals = Vec::with_capacity(frames.len());
            for f in frames {
                let i = (y * w + x) * 3;
                vals.push(f.rgb[i + 1] as f32); // green channel is enough
            }
            let mean = vals.iter().sum::<f32>() / vals.len() as f32;
            let var = vals.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32;
            devs.push(var.sqrt());
        }
        devs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        row_deviation.push(devs[devs.len() / 2]);
    }
    // Static suffix: rows where the median deviation is tiny.
    let mut top = h;
    for y in (0..h).rev() {
        if row_deviation[y] < 6.0 {
            top = y;
        } else {
            break;
        }
    }
    HudPartition { top, row_deviation }
}

/// Determine if the camera is static or scrolling by sampling the first 30 frames
/// with FFT phase correlation.
pub fn detect_camera_is_static(
    path: &Path,
    start: f64,
    w: usize,
    h: usize,
) -> io::Result<bool> {
    // Sample across a 4-second window (or up to 120 frames): a scrolling game
    // will clearly move within 3-4 seconds, whereas a static game never moves.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(4.0))?;
    let playfield_h = (h * 7) / 10;
    let mut est = ScrollEstimator::new(w, playfield_h);
    let mut cum_x = 0i32;
    let mut cum_y = 0i32;
    let mut max_disp = 0i32;
    let mut count = 0usize;
    while let Some(frame) = reader.next_frame()? {
        let (d, _) = est.push(&frame);
        cum_x += d[0];
        cum_y += d[1];
        max_disp = max_disp.max(cum_x.abs()).max(cum_y.abs());
        count += 1;
        if count >= 120 {
            break;
        }
    }
    Ok(max_disp <= 4)
}

/// In a static camera regime (like maze games), find the enclosing horizontal
/// border walls separating the playfield from top and bottom HUD text.
pub fn detect_static_borders(
    path: &Path,
    start: f64,
    w: usize,
    h: usize,
) -> io::Result<(usize, usize)> {
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(1.0))?;
    let mut frames = Vec::new();
    reader.for_each(|_, f| frames.push(f.clone()))?;
    if frames.is_empty() {
        return Ok((0, h));
    }
    let bg = [frames[0].rgb[0], frames[0].rgb[1], frames[0].rgb[2]];
    let mut wall_counts = vec![0usize; h];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let c = [frames[0].rgb[i], frames[0].rgb[i + 1], frames[0].rgb[i + 2]];
            if color_dist(c, bg) > 30 {
                wall_counts[y] += 1;
            }
        }
    }
    let threshold = (w * 6) / 10; // row is a horizontal border if >= 60% of cols are wall
    let mut first_border = None;
    let mut last_border = None;
    for y in 0..h {
        if wall_counts[y] >= threshold {
            if first_border.is_none() {
                first_border = Some(y);
            }
            last_border = Some(y);
        }
    }
    match (first_border, last_border) {
        (Some(fb), Some(lb)) if lb > fb + 40 => {
            let top = (fb / 8) * 8;
            let bot = ((lb + 8) / 8) * 8;
            Ok((top, bot.min(h)))
        }
        _ => Ok((0, h)),
    }
}

/// Detect the gameplay segment: the longest contiguous run of ≥ 60 frames sharing
/// a stable background scene and non-black active content.
pub fn detect_playfield_segment(path: &Path) -> io::Result<(f64, f64, VideoInfo)> {
    let info = video_io::probe(path)?;
    let mut reader = video_io::FrameReader::open(path)?;
    let fps = info.fps.max(1.0);
    let mut run_start: Option<usize> = None;
    let mut curr_bg: Option<[u8; 3]> = None;
    let mut last_run: Option<(usize, usize)> = None;
    let mut idx = 0usize;
    let mut best: Option<(usize, usize)> = None;
    while let Some(frame) = reader.next_frame()? {
        let bg = frame.pixel(5, 5);
        // Measure non-black content ratio
        let mut nonblack = 0usize;
        let mut samples = 0usize;
        let mut y = 0;
        while y < frame.height as usize {
            let mut x = 0;
            while x < frame.width as usize {
                let i = (y * frame.width as usize + x) * 3;
                if frame.rgb[i] as u32 + frame.rgb[i + 1] as u32 + frame.rgb[i + 2] as u32 > 30 {
                    nonblack += 1;
                }
                samples += 1;
                x += 16;
            }
            y += 16;
        }
        let bright = nonblack as f32 / samples.max(1) as f32;
        let active = bright > 0.04;

        if active {
            match curr_bg {
                None => {
                    curr_bg = Some(bg);
                    run_start = Some(idx);
                }
                Some(prev_bg) => {
                    if color_dist(bg, prev_bg) <= 25 {
                        // Same background scene continues
                    } else {
                        // Background changed
                        if let Some(s) = run_start {
                            if idx - s >= 60 {
                                last_run = Some((s, idx));
                                if best.map_or(true, |(bs, be)| idx - s > be - bs) {
                                    best = Some((s, idx));
                                }
                            }
                        }
                        curr_bg = Some(bg);
                        run_start = Some(idx);
                    }
                }
            }
        } else {
            if let Some(s) = run_start {
                if idx - s >= 60 {
                    last_run = Some((s, idx));
                    if best.map_or(true, |(bs, be)| idx - s > be - bs) {
                        best = Some((s, idx));
                    }
                }
            }
            curr_bg = None;
            run_start = None;
        }
        idx += 1;
    }
    if let Some(s) = run_start {
        if idx - s >= 60 {
            last_run = Some((s, idx));
            if best.map_or(true, |(bs, be)| idx - s > be - bs) {
                best = Some((s, idx));
            }
        }
    }
    let (s, e) = best.or(last_run).ok_or_else(|| {
        io::Error::other("no playfield segment found (no stable scene run of >= 60 frames)")
    })?;
    let raw_start = s as f64 / fps;
    let raw_dur = (e - s) as f64 / fps;
    let (start_sec, dur_sec) = if s > 0 && raw_dur > 2.0 {
        let trim = 0.6;
        (raw_start + trim, (raw_dur - trim).clamp(1.0, 21.0))
    } else {
        (raw_start, raw_dur.clamp(1.0, 25.0))
    };
    Ok((start_sec, dur_sec, info))
}

// ---------------------------------------------------------------------------
// Mode-based atlas accumulation
// ---------------------------------------------------------------------------

const MAX_ENTRIES: usize = 6;

#[derive(Clone, Copy, Debug)]
struct BucketEntry {
    key: u32,
    count: u32,
    sr: u32,
    sg: u32,
    sb: u32,
}

#[derive(Clone, Debug, Default)]
struct Bucket {
    entries: Vec<BucketEntry>,
}

impl Bucket {
    fn add(&mut self, c: [u8; 3]) {
        let key = ((c[0] as u32 >> 3) << 10) | ((c[1] as u32 >> 3) << 5) | (c[2] as u32 >> 3);
        for e in self.entries.iter_mut() {
            if e.key == key {
                e.count += 1;
                e.sr += c[0] as u32;
                e.sg += c[1] as u32;
                e.sb += c[2] as u32;
                return;
            }
        }
        if self.entries.len() < MAX_ENTRIES {
            self.entries.push(BucketEntry {
                key,
                count: 1,
                sr: c[0] as u32,
                sg: c[1] as u32,
                sb: c[2] as u32,
            });
        } else {
            // Replace the least frequent candidate.
            let idx = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.count)
                .map(|(i, _)| i)
                .unwrap();
            self.entries[idx] = BucketEntry {
                key,
                count: 1,
                sr: c[0] as u32,
                sg: c[1] as u32,
                sb: c[2] as u32,
            };
        }
    }

    fn mode(&self) -> Option<([u8; 3], u32)> {
        let e = self.entries.iter().max_by_key(|e| e.count)?;
        let n = e.count.max(1);
        Some((
            [
                ((e.sr + n / 2) / n) as u8,
                ((e.sg + n / 2) / n) as u8,
                ((e.sb + n / 2) / n) as u8,
            ],
            e.count,
        ))
    }

    fn total(&self) -> u32 {
        self.entries.iter().map(|e| e.count).sum()
    }
}

/// Streaming world atlas accumulator.
///
/// Each world pixel keeps a bounded palette of quantized colors; the final
/// value is the mode, which suppresses moving actors far better than a
/// channel-wise median (which produces ghost blends).
/// [Spectral Scene Discovery: Quantized Temporal Mode Accumulation]
///
/// Accumulates per-pixel RGB histogram buckets across camera-registered frames
/// and resolves the mode to filter moving actors without inpainting artifacts.
pub struct AtlasAccumulator {
    /// Viewport rows included from each frame (`0..viewport_h`).
    pub viewport_h: usize,
    buckets: HashMap<(i32, i32), Bucket>,
    min: [i32; 2],
    max: [i32; 2],
    frames: usize,
}

impl AtlasAccumulator {
    pub fn new(viewport_h: usize) -> Self {
        Self {
            viewport_h,
            buckets: HashMap::new(),
            min: [i32::MAX, i32::MAX],
            max: [i32::MIN, i32::MIN],
            frames: 0,
        }
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Add one frame at world offset `off`, skipping pixels inside `mask`
    /// (frame coordinates).
    pub fn add_frame(&mut self, frame: &RawFrame, off: [i32; 2], mask: &[Rect]) {
        self.frames += 1;
        let fw = frame.width as usize;
        let rows = self.viewport_h.min(frame.height as usize);
        let x_start = if off[0] > 0 && fw >= 128 { 8 } else { 0 };
        for y in 0..rows {
            let wy = y as i32 + off[1];
            for x in x_start..fw {
                let wx = x as i32 + off[0];
                if wx < self.min[0] {
                    self.min[0] = wx;
                }
                if wy < self.min[1] {
                    self.min[1] = wy;
                }
                if wx > self.max[0] {
                    self.max[0] = wx;
                }
                if wy > self.max[1] {
                    self.max[1] = wy;
                }
                if !mask.is_empty() {
                    let mut skip = false;
                    for r in mask {
                        if r.contains(x as i32, y as i32) {
                            skip = true;
                            break;
                        }
                    }
                    if skip {
                        continue;
                    }
                }
                let i = (y * fw + x) * 3;
                let c = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                self.buckets.entry((wx, wy)).or_default().add(c);
            }
        }
    }

    /// [Spectral Scene Discovery: Quantized Temporal Mode Panorama Synthesis]
    /// Resolve to a dense atlas. `origin` is the world coordinate of pixel (0,0).
    pub fn to_atlas(&self) -> WorldAtlas {
        let origin = self.min;
        let w = (self.max[0] - self.min[0] + 1).max(0) as usize;
        let h = (self.max[1] - self.min[1] + 1).max(0) as usize;
        let mut image = RgbImage8::new(w, h);
        for (&(wx, wy), bucket) in &self.buckets {
            if let Some((c, _)) = bucket.mode() {
                let x = (wx - origin[0]) as usize;
                let y = (wy - origin[1]) as usize;
                if x < w && y < h {
                    image.set_pixel(x, y, c);
                }
            }
        }
        WorldAtlas { image, origin }
    }

    /// Resolve to a dense atlas, filling any unobserved or empty pixels
    /// from a fallback atlas.
    pub fn to_atlas_with_fallback(&self, fallback: &WorldAtlas) -> WorldAtlas {
        let mut atlas = self.to_atlas();
        let w = atlas.image.width;
        let h = atlas.image.height;
        for y in 0..h {
            let wy = atlas.origin[1] + y as i32;
            for x in 0..w {
                let wx = atlas.origin[0] + x as i32;
                let has = self.buckets.get(&(wx, wy)).and_then(|b| b.mode()).is_some();
                if !has {
                    if let Some(c) = fallback.pixel(wx, wy) {
                        atlas.image.set_pixel(x, y, c);
                    }
                }
            }
        }
        atlas
    }

    /// Resolve to a dense atlas when viewport-locked HUD elements were masked,
    /// completing HUD-occluded or low-support leading-edge pixels from the nearest
    /// observed cardinal neighbor in `self` rather than a HUD-contaminated fallback.
    pub fn to_atlas_inpainted(
        &self,
        fallback: &WorldAtlas,
        has_viewport_hud: bool,
        bg: [u8; 3],
    ) -> WorldAtlas {
        if !has_viewport_hud {
            return self.to_atlas_with_fallback(fallback);
        }
        let mut atlas = self.to_atlas();
        let w = atlas.image.width;
        let h = atlas.image.height;
        if w == 0 || h == 0 {
            return atlas;
        }
        let mut valid = vec![false; w * h];
        for y in 0..h {
            let wy = atlas.origin[1] + y as i32;
            for x in 0..w {
                let wx = atlas.origin[0] + x as i32;
                if let Some(b) = self.buckets.get(&(wx, wy)) {
                    if let Some((c, _)) = b.mode() {
                        let tot = b.total();
                        if y >= 10 || tot >= 12 || color_dist(c, bg) <= 32 {
                            valid[y * w + x] = true;
                        }
                    }
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                if valid[y * w + x] {
                    continue;
                }
                let mut best_c = bg;
                let mut best_d = usize::MAX;
                if y < 28 && x + 20 < w {
                    for sy in (y + 1)..h {
                        if valid[sy * w + x] {
                            best_d = sy - y;
                            best_c = atlas.image.pixel(x, sy);
                            break;
                        }
                    }
                }
                if best_d == usize::MAX {
                    for sx in (0..x).rev() {
                        if valid[y * w + sx] {
                            let d = x - sx;
                            if d < best_d {
                                best_d = d;
                                best_c = atlas.image.pixel(sx, y);
                            }
                            break;
                        }
                    }
                    for sx in (x + 1)..w {
                        if valid[y * w + sx] {
                            let d = sx - x;
                            if d < best_d {
                                best_d = d;
                                best_c = atlas.image.pixel(sx, y);
                            }
                            break;
                        }
                    }
                    for sy in (y + 1)..h {
                        if valid[sy * w + x] {
                            let d = sy - y;
                            if d < best_d {
                                best_d = d;
                                best_c = atlas.image.pixel(x, sy);
                            }
                            break;
                        }
                    }
                    for sy in (0..y).rev() {
                        if valid[sy * w + x] {
                            let d = y - sy;
                            if d < best_d {
                                best_c = atlas.image.pixel(x, sy);
                            }
                            break;
                        }
                    }
                }
                atlas.image.set_pixel(x, y, best_c);
            }
        }
        atlas
    }
}

/// Dense world-space atlas with its world origin.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldAtlas {
    pub image: RgbImage8,
    /// World coordinate of atlas pixel `(0, 0)`.
    pub origin: [i32; 2],
}

impl WorldAtlas {
    /// Atlas pixel for a world coordinate, if inside.
    #[inline]
    pub fn pixel(&self, wx: i32, wy: i32) -> Option<[u8; 3]> {
        let x = wx - self.origin[0];
        let y = wy - self.origin[1];
        if x < 0 || y < 0 || x >= self.image.width as i32 || y >= self.image.height as i32 {
            None
        } else {
            Some(self.image.pixel(x as usize, y as usize))
        }
    }
}

/// Build an atlas from a frame slice (convenience wrapper for tests).
pub fn build_2d_world_atlas(
    frames: &[RawFrame],
    scroll: &[[i32; 2]],
    viewport_h: usize,
) -> WorldAtlas {
    let mut acc = AtlasAccumulator::new(viewport_h);
    for (f, off) in frames.iter().zip(scroll) {
        acc.add_frame(f, *off, &[]);
    }
    acc.to_atlas()
}

// ---------------------------------------------------------------------------
// Actor residuals and tracks
// ---------------------------------------------------------------------------

/// Residual bounding boxes (frame coordinates) versus the atlas.
pub fn residual_boxes(
    atlas: &WorldAtlas,
    frame: &RawFrame,
    off: [i32; 2],
    hud_top: usize,
    threshold: i32,
    min_pixels: usize,
) -> Vec<Rect> {
    residual_boxes_range(atlas, frame, off, 0, hud_top, threshold, min_pixels)
}

/// Residual bounding boxes within a specific row band `[y_start..y_end]`.
pub fn residual_boxes_range(
    atlas: &WorldAtlas,
    frame: &RawFrame,
    off: [i32; 2],
    y_start: usize,
    y_end: usize,
    threshold: i32,
    min_pixels: usize,
) -> Vec<Rect> {
    let w = frame.width as usize;
    let h = y_end.min(frame.height as usize);
    let mut mask = vec![false; w * h];
    let x_start = if off[0] > 0 && w >= 128 { 8 } else { 0 };
    for y in y_start..h {
        for x in x_start..w {
            let wx = x as i32 + off[0];
            let wy = y as i32 + off[1];
            if let Some(a) = atlas.pixel(wx, wy) {
                let i = (y * w + x) * 3;
                let c = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                let d = (c[0] as i32 - a[0] as i32)
                    .abs()
                    .max((c[1] as i32 - a[1] as i32).abs())
                    .max((c[2] as i32 - a[2] as i32).abs());
                if d > threshold {
                    mask[y * w + x] = true;
                }
            }
        }
    }
    connected_boxes(&mask, w, h, min_pixels)
}

/// Residual bounding boxes along with the mean foreground RGB colour of their
/// active residual pixels (used for chromatic-spinor coherent tracking).
pub fn residual_boxes_with_color(
    atlas: &WorldAtlas,
    frame: &RawFrame,
    off: [i32; 2],
    y_start: usize,
    y_end: usize,
    threshold: i32,
    min_pixels: usize,
) -> Vec<(Rect, [u8; 3])> {
    let w = frame.width as usize;
    let h = y_end.min(frame.height as usize);
    let mut mask = vec![false; w * h];
    let x_start = if off[0] > 0 && w >= 128 { 8 } else { 0 };
    for y in y_start..h {
        for x in x_start..w {
            let wx = x as i32 + off[0];
            let wy = y as i32 + off[1];
            if let Some(a) = atlas.pixel(wx, wy) {
                let i = (y * w + x) * 3;
                let c = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                let d = (c[0] as i32 - a[0] as i32)
                    .abs()
                    .max((c[1] as i32 - a[1] as i32).abs())
                    .max((c[2] as i32 - a[2] as i32).abs());
                if d > threshold {
                    mask[y * w + x] = true;
                }
            }
        }
    }
    let rects = connected_boxes(&mask, w, h, min_pixels);
    rects
        .into_iter()
        .map(|r| {
            let mut sr = 0u64;
            let mut sg = 0u64;
            let mut sb = 0u64;
            let mut n = 0u64;
            for y in r.y0.max(0)..=r.y1.min(h as i32 - 1) {
                for x in r.x0.max(0)..=r.x1.min(w as i32 - 1) {
                    let idx = y as usize * w + x as usize;
                    if mask[idx] {
                        let p = idx * 3;
                        sr += frame.rgb[p] as u64;
                        sg += frame.rgb[p + 1] as u64;
                        sb += frame.rgb[p + 2] as u64;
                        n += 1;
                    }
                }
            }
            let col = if n > 0 {
                [(sr / n) as u8, (sg / n) as u8, (sb / n) as u8]
            } else {
                [128, 128, 128]
            };
            (r, col)
        })
        .collect()
}

/// 4-connected components → bounding boxes (dilated by 1 px).
fn connected_boxes(mask: &[bool], w: usize, h: usize, min_pixels: usize) -> Vec<Rect> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if mask[start] && !seen[start] {
            let mut min_x = w as i32;
            let mut min_y = h as i32;
            let mut max_x = -1i32;
            let mut max_y = -1i32;
            let mut count = 0usize;
            stack.clear();
            stack.push(start);
            seen[start] = true;
            while let Some(p) = stack.pop() {
                let (x, y) = (p % w, p / w);
                count += 1;
                min_x = min_x.min(x as i32);
                max_x = max_x.max(x as i32);
                min_y = min_y.min(y as i32);
                max_y = max_y.max(y as i32);
                let visit = |nx: i32, ny: i32, seen: &mut Vec<bool>, stack: &mut Vec<usize>| {
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        return;
                    }
                    let np = ny as usize * w + nx as usize;
                    if mask[np] && !seen[np] {
                        seen[np] = true;
                        stack.push(np);
                    }
                };
                visit(x as i32 - 1, y as i32, &mut seen, &mut stack);
                visit(x as i32 + 1, y as i32, &mut seen, &mut stack);
                visit(x as i32, y as i32 - 1, &mut seen, &mut stack);
                visit(x as i32, y as i32 + 1, &mut seen, &mut stack);
            }
            if count >= min_pixels {
                out.push(Rect::new(min_x, min_y, max_x, max_y).expand(1));
            }
        }
    }
    out.sort_by_key(|r| (r.y0, r.x0));
    out
}

/// Refine an integer scroll offset against the atlas by exhaustive ±1 search.
///
/// The FFT estimator is exact to the pixel on clean frames, but one frame of
/// registration error turns every sprite edge into a full-screen residual,
/// which buries the actor boxes. The search is cheap and keeps the mask clean.
pub fn refine_offset(
    atlas: &WorldAtlas,
    frame: &RawFrame,
    off: [i32; 2],
    hud_top: usize,
) -> [i32; 2] {
    let w = frame.width as usize;
    let h = hud_top.min(frame.height as usize);
    let mut best = off;
    let mut best_err = u64::MAX;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cand = [off[0] + dx, off[1] + dy];
            let mut err = 0u64;
            let mut n = 0u64;
            for y in (0..h).step_by(2) {
                for x in (0..w).step_by(2) {
                    let Some(a) = atlas.pixel(x as i32 + cand[0], y as i32 + cand[1]) else {
                        continue;
                    };
                    let i = (y * w + x) * 3;
                    err += (frame.rgb[i] as i32 - a[0] as i32).unsigned_abs() as u64
                        + (frame.rgb[i + 1] as i32 - a[1] as i32).unsigned_abs() as u64
                        + (frame.rgb[i + 2] as i32 - a[2] as i32).unsigned_abs() as u64;
                    n += 1;
                }
            }
            // Normalise by covered area: edge offsets see fewer pixels.
            if n > 0 {
                let e = err.saturating_mul(1000) / n;
                if e < best_err {
                    best_err = e;
                    best = cand;
                }
            }
        }
    }
    best
}

/// Role of an extracted holon track in the implicate world model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorKind {
    Player,
    Autonomous,
    Unknown,
}

impl ActorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorKind::Player => "player",
            ActorKind::Autonomous => "autonomous",
            ActorKind::Unknown => "unknown",
        }
    }
}

/// Canonical RGBA sprite kernel with DC mode ($\omega_0$) and first temporal
/// CFT harmonic ($\omega_1$, encoded as `128 + delta` per channel).
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite2D {
    pub width: usize,
    pub height: usize,
    /// DC mode $\hat{K}_0(x, y)$ RGBA, row-major.
    pub rgba: Vec<u8>,
    /// First temporal harmonic $\hat{K}_1(x, y)$ RGBA biased by `+128` (`128` = zero delta).
    pub harmonic_rgba: Vec<u8>,
}

impl Sprite2D {
    pub fn alpha_at(&self, x: usize, y: usize) -> u8 {
        self.rgba[(y * self.width + x) * 4 + 3]
    }
}

/// A tracked actor across frames, in world coordinates.
#[derive(Clone, Debug)]
pub struct ActorTrack {
    pub kind: ActorKind,
    /// `(frame_index, frame_box)` for every detection.
    pub boxes: Vec<(usize, Rect)>,
    /// Mean chromatic spinor signature (RGB) of the holon.
    pub color: [u8; 3],
    pub sprite: Option<Sprite2D>,
}

impl ActorTrack {
    /// Median box size over the track.
    pub fn median_size(&self) -> (i32, i32) {
        if self.boxes.is_empty() {
            return (0, 0);
        }
        let mut ws: Vec<i32> = self.boxes.iter().map(|(_, b)| b.width()).collect();
        let mut hs: Vec<i32> = self.boxes.iter().map(|(_, b)| b.height()).collect();
        ws.sort_unstable();
        hs.sort_unstable();
        (ws[ws.len() / 2], hs[hs.len() / 2])
    }

    pub fn first_world_center(&self) -> Option<[f64; 2]> {
        self.boxes.first().map(|(_, b)| b.center())
    }
}

/// Match residual boxes across frames into tracks.
pub fn track_actors(boxes_per_frame: &[Vec<Rect>], max_gap: usize) -> Vec<ActorTrack> {
    let colored: Vec<Vec<(Rect, [u8; 3])>> = boxes_per_frame
        .iter()
        .map(|v| v.iter().map(|b| (*b, [128, 128, 128])).collect())
        .collect();
    let scroll = vec![[0, 0]; boxes_per_frame.len()];
    track_actors_coherent(&colored, &scroll, 256, false, max_gap).0
}

/// Spinor-coherent multi-frame actor tracker.
///
/// Matches residual boxes using joint spatial + chromatic spinor distance,
/// filters out stationary blinking props ($\nabla S = 0$), and merges
/// non-overlapping track fragments sharing the same chromatic spinor signature.
/// Returns `(top_classified_tracks, all_moving_tracks_for_atlas_masking)`.
pub fn track_actors_coherent(
    boxes_per_frame: &[Vec<(Rect, [u8; 3])>],
    scroll: &[[i32; 2]],
    viewport_w: usize,
    is_static: bool,
    max_gap: usize,
) -> (Vec<ActorTrack>, Vec<ActorTrack>) {
    struct Open {
        boxes: Vec<(usize, Rect)>,
        last: usize,
        last_box: Rect,
        color: [f64; 3],
    }
    let mut open: Vec<Open> = Vec::new();
    let mut closed: Vec<ActorTrack> = Vec::new();
    let max_dist = if is_static { 24.0 } else { 32.0 };
    let vw = viewport_w.max(1) as f64;

    for (fi, boxes) in boxes_per_frame.iter().enumerate() {
        let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
        for (ti, t) in open.iter().enumerate() {
            if fi - t.last > max_gap {
                continue;
            }
            let tc = t.last_box.center();
            let tw = t.last_box.width();
            let th = t.last_box.height();
            for (bi, (b, bcol)) in boxes.iter().enumerate() {
                let c = b.center();
                let dx = if is_static {
                    let raw = (c[0] - tc[0]).abs();
                    raw.min(vw - raw)
                } else {
                    (c[0] - tc[0]).abs()
                };
                let dy = (c[1] - tc[1]).abs();
                let d = (dx * dx + dy * dy).sqrt();
                let cd = (bcol[0] as f64 - t.color[0])
                    .abs()
                    .max((bcol[1] as f64 - t.color[1]).abs())
                    .max((bcol[2] as f64 - t.color[2]).abs());
                let size_ok = (b.width() - tw).abs() <= 12 && (b.height() - th).abs() <= 12;
                if d <= max_dist && cd <= 55.0 && size_ok {
                    pairs.push((d + 0.4 * cd, ti, bi));
                }
            }
        }
        let orig_open_len = open.len();
        let mut used_t = vec![false; orig_open_len];
        let mut used_b = vec![false; boxes.len()];

        // Spinor factorization during multi-actor occlusions: if a compound
        // bounding box has no single-track size match and spans two established
        // open tracks of comparable scale, factor it into per-track sub-boxes.
        for (bi, (b, _)) in boxes.iter().enumerate() {
            let has_direct_size_match = open.iter().take(orig_open_len).any(|t| {
                fi - t.last <= max_gap
                    && (b.width() - t.last_box.width()).abs() <= 6
                    && (b.height() - t.last_box.height()).abs() <= 6
            });
            if has_direct_size_match {
                continue;
            }
            let mut nearby_open: Vec<(usize, f64)> = open
                .iter()
                .enumerate()
                .take(orig_open_len)
                .filter(|(ti, t)| {
                    !used_t[*ti]
                        && fi - t.last <= max_gap
                        && t.boxes.len() >= 3
                        && t.last_box.width() >= 10
                        && t.last_box.height() >= 10
                })
                .filter_map(|(ti, t)| {
                    let tc = t.last_box.center();
                    let c = b.center();
                    let dx = if is_static {
                        let raw = (c[0] - tc[0]).abs();
                        raw.min(vw - raw)
                    } else {
                        (c[0] - tc[0]).abs()
                    };
                    let dy = (c[1] - tc[1]).abs();
                    let d = (dx * dx + dy * dy).sqrt();
                    let tw = t.last_box.width();
                    let th = t.last_box.height();
                    if d <= max_dist * 1.25
                        && b.area() >= (tw * th * 5) / 4
                        && b.area() <= tw * th * 3
                    {
                        Some((ti, d))
                    } else {
                        None
                    }
                })
                .collect();
            nearby_open.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            if nearby_open.len() >= 2 {
                for &(ti, _) in nearby_open.iter().take(2) {
                    used_t[ti] = true;
                    let t = &mut open[ti];
                    let tw = t.last_box.width();
                    let th = t.last_box.height();
                    let tc = t.last_box.center();
                    let sx0 =
                        (tc[0].round() as i32 - tw / 2).clamp(b.x0, (b.x1 - tw + 1).max(b.x0));
                    let sy0 =
                        (tc[1].round() as i32 - th / 2).clamp(b.y0, (b.y1 - th + 1).max(b.y0));
                    let sub = Rect {
                        x0: sx0,
                        y0: sy0,
                        x1: (sx0 + tw - 1).min(b.x1),
                        y1: (sy0 + th - 1).min(b.y1),
                    };
                    t.boxes.push((fi, sub));
                    t.last = fi;
                    t.last_box = sub;
                }
                used_b[bi] = true;
            }
        }

        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, ti, bi) in pairs {
            if used_t[ti] || used_b[bi] {
                continue;
            }
            used_t[ti] = true;
            used_b[bi] = true;
            let (b, bcol) = boxes[bi];
            let t = &mut open[ti];
            t.boxes.push((fi, b));
            t.last = fi;
            t.last_box = b;
            for k in 0..3 {
                t.color[k] = 0.85 * t.color[k] + 0.15 * bcol[k] as f64;
            }
        }
        for (bi, (b, bcol)) in boxes.iter().enumerate() {
            if !used_b[bi] {
                open.push(Open {
                    boxes: vec![(fi, *b)],
                    last: fi,
                    last_box: *b,
                    color: [bcol[0] as f64, bcol[1] as f64, bcol[2] as f64],
                });
            }
        }
        open.retain(|t| {
            if fi - t.last > max_gap {
                closed.push(ActorTrack {
                    kind: ActorKind::Unknown,
                    boxes: t.boxes.clone(),
                    color: [
                        t.color[0].round() as u8,
                        t.color[1].round() as u8,
                        t.color[2].round() as u8,
                    ],
                    sprite: None,
                });
                false
            } else {
                true
            }
        });
    }
    for t in open {
        closed.push(ActorTrack {
            kind: ActorKind::Unknown,
            boxes: t.boxes,
            color: [
                t.color[0].round() as u8,
                t.color[1].round() as u8,
                t.color[2].round() as u8,
            ],
            sprite: None,
        });
    }

    let scroll_is_trivial = scroll.iter().all(|s| s[0] == 0 && s[1] == 0);
    let world_span = |t: &ActorTrack| -> (f64, f64) {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for &(fi, b) in &t.boxes {
            let s = scroll.get(fi).copied().unwrap_or([0, 0]);
            let wx = b.x0 as f64 + s[0] as f64;
            let wy = b.y0 as f64 + s[1] as f64;
            min_x = min_x.min(wx);
            max_x = max_x.max(wx);
            min_y = min_y.min(wy);
            max_y = max_y.max(wy);
        }
        ((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
    };

    closed.retain(|t| {
        if t.boxes.len() < 6 || t.median_size() == (0, 0) {
            return false;
        }
        if is_static {
            let (dx, dy) = world_span(t);
            dx.max(dy) >= 12.0
        } else {
            t.boxes.len() >= 8
        }
    });

    // Spinor-coherent fragment unification: merge non-overlapping track fragments
    // sharing the same chromatic spinor signature and compatible size.
    closed.sort_by_key(|t| std::cmp::Reverse(t.boxes.len()));
    let all_moving = closed.clone();
    let mut merged: Vec<ActorTrack> = Vec::new();
    for t in closed {
        let (tw, th) = t.median_size();
        let mut placed = false;
        for m in &mut merged {
            if color_dist(t.color, m.color) > 35 {
                continue;
            }
            let (mw, mh) = m.median_size();
            if (tw - mw).abs() > 8 || (th - mh).abs() > 8 {
                continue;
            }
            let overlaps = t
                .boxes
                .iter()
                .any(|(fi, _)| m.boxes.iter().any(|(mfi, _)| mfi == fi));
            if overlaps && !is_static {
                continue;
            }
            // In scrolling worlds, only merge fragments that are spatially and
            // temporally proximate in world coordinates so separate actors stay distinct.
            let can_merge = if is_static {
                true
            } else {
                let (tfi, tb) = t.boxes.first().unwrap();
                let ts = scroll.get(*tfi).copied().unwrap_or([0, 0]);
                let tx = tb.center()[0] + ts[0] as f64;
                let ty = tb.center()[1] + ts[1] as f64;
                let (mfi, mb) = m.boxes.last().unwrap();
                let ms = scroll.get(*mfi).copied().unwrap_or([0, 0]);
                let mx = mb.center()[0] + ms[0] as f64;
                let my = mb.center()[1] + ms[1] as f64;
                (tx - mx).abs() <= 96.0 && (ty - my).abs() <= 28.0
            };
            if can_merge {
                for &(fi, rect) in &t.boxes {
                    if !m.boxes.iter().any(|(mfi, _)| *mfi == fi) {
                        m.boxes.push((fi, rect));
                    }
                }
                m.boxes.sort_by_key(|(fi, _)| *fi);
                placed = true;
                break;
            }
        }
        if !placed {
            merged.push(t);
        }
    }

    // Filter out stationary shimmering lattice sites and thin seam fragments
    // after fragment unification.
    merged.retain(|t| {
        if t.boxes.len() < 8 {
            return false;
        }
        let (w, h) = t.median_size();
        if !is_static && (w < 12 || h < 13) {
            return false;
        }
        if !is_static && scroll_is_trivial {
            return true;
        }
        let (dx, dy) = world_span(t);
        if is_static {
            dx.max(dy) >= 12.0
        } else {
            dx >= 28.0 || dy >= 24.0
        }
    });
    merged.sort_by_key(|t| std::cmp::Reverse(t.boxes.len()));
    if is_static && !merged.is_empty() {
        let (pw, ph) = merged[0].median_size();
        let primary_area = (pw * ph).max(1);
        let primary_len = merged[0].boxes.len();
        merged.retain(|t| {
            let (w, h) = t.median_size();
            !(w * h > primary_area * 3 / 2 && t.boxes.len() * 3 < primary_len)
        });
    }
    merged.truncate(12);
    classify_tracks(&mut merged);
    if !is_static && !scroll_is_trivial {
        for t in &mut merged {
            let (dx, dy) = world_span(t);
            let mut vx_sum = 0.0f64;
            let mut v_cnt = 0usize;
            for pair in t.boxes.windows(2) {
                let dt = pair[1].0.saturating_sub(pair[0].0).max(1) as f64;
                if dt <= 5.0 {
                    let s0 = scroll.get(pair[0].0).copied().unwrap_or([0, 0]);
                    let s1 = scroll.get(pair[1].0).copied().unwrap_or([0, 0]);
                    let wx0 = pair[0].1.x0 as f64 + s0[0] as f64;
                    let wx1 = pair[1].1.x0 as f64 + s1[0] as f64;
                    let vx = (wx1 - wx0) / dt;
                    if vx.abs() <= 16.0 {
                        vx_sum += vx;
                        v_cnt += 1;
                    }
                }
            }
            let mean_vx = if v_cnt > 0 { vx_sum / v_cnt as f64 } else { 0.0 };
            if (dx >= 150.0 && mean_vx >= 1.2) || (dx >= 60.0 && dy >= 48.0) {
                t.kind = ActorKind::Player;
            }
        }
    }
    (merged, all_moving)
}

/// Classify the primary persistent holon vs. autonomous environmental holons
/// purely from temporal persistence without game-specific size heuristics.
fn classify_tracks(tracks: &mut [ActorTrack]) {
    let mut best: Option<(usize, usize)> = None;
    for (i, t) in tracks.iter().enumerate() {
        let (w, h) = t.median_size();
        if !(8..=44).contains(&w) || !(10..=44).contains(&h) {
            continue;
        }
        let count = t.boxes.len();
        if best.map_or(true, |(_, max_c)| count > max_c) {
            best = Some((i, count));
        }
    }
    if let Some((i, _)) = best {
        tracks[i].kind = ActorKind::Player;
    }
    for t in tracks.iter_mut() {
        if t.kind == ActorKind::Unknown {
            t.kind = ActorKind::Autonomous;
        }
    }
}

// ---------------------------------------------------------------------------
// Tile classification
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tile classification (game-agnostic)
//
// Nothing below knows about a particular game: the backdrop colour is learned
// from the atlas border, the solid band is whatever the level's low ground line
// supports, and one-way surfaces are flat-topped rectangles floating above it.
// Per-game overrides belong in a rules file, never in here.
// ---------------------------------------------------------------------------

/// Tunables for [`extract_2d_tiles_with`].
///
/// Defaults target 16x16 tile platformers with a flat backdrop. They are
/// deliberately structural (widths, flatness, fill) rather than colour-based.
#[derive(Clone, Copy, Debug)]
pub struct TileOptions {
    /// RGB distance from the learned backdrop above which a pixel is content.
    pub bg_tol: i32,
    /// Minimum width (tile columns) of an accepted raised terrain step.
    pub min_run: usize,
    /// Minimum connected-component area (px) for a platform candidate.
    pub min_area: usize,
    /// Minimum platform candidate bbox width (px).
    pub min_w: i32,
    /// Minimum platform candidate bbox height (px).
    pub min_h: i32,
    /// Minimum bbox fill ratio of a rectangle candidate.
    pub min_fill: f64,
    /// Minimum fraction of bbox columns whose top edge is within 1 px of the
    /// median top edge. A landable surface is flat-topped; clouds are not.
    pub min_flat: f64,
    /// Tile lattice dimension (px). Defaults to 16 for platformers, 8 for mazes.
    pub tile_size: usize,
    /// Whether the game is a static screen/maze without a global ground line.
    pub is_static: bool,
    /// Top HUD boundary (px). Rows 0..hud_bottom are non-collidable background.
    pub hud_bottom: usize,
    /// Bottom HUD boundary (px). Rows hud_top..h are non-collidable background.
    pub hud_top: usize,
}

impl Default for TileOptions {
    fn default() -> Self {
        Self {
            bg_tol: 18,
            min_run: 3,
            min_area: 120,
            min_w: 12,
            min_h: 10,
            min_fill: 0.6,
            min_flat: 0.8,
            tile_size: 16,
            is_static: false,
            hud_bottom: 0,
            hud_top: usize::MAX,
        }
    }
}

/// Learn the backdrop colour: mode of the atlas top band, bucket-mean colour.
fn learn_background(img: &RgbImage8) -> [u8; 3] {
    const BUCKETS: usize = 32 * 32 * 32;
    let band = img.height.min(3);
    let mut counts = vec![0u32; BUCKETS];
    let mut sums = vec![[0u64; 3]; BUCKETS];
    for y in 0..band {
        for x in 0..img.width {
            let c = img.pixel(x, y);
            let idx = (((c[0] as usize) >> 3) << 10)
                | (((c[1] as usize) >> 3) << 5)
                | ((c[2] as usize) >> 3);
            counts[idx] += 1;
            for (k, s) in sums[idx].iter_mut().enumerate() {
                *s += c[k] as u64;
            }
        }
    }
    let mut best = 0usize;
    for (i, c) in counts.iter().enumerate() {
        if *c > counts[best] {
            best = i;
        }
    }
    let n = counts[best] as u64;
    if n == 0 {
        return [0, 0, 0];
    }
    [
        (sums[best][0] / n) as u8,
        (sums[best][1] / n) as u8,
        (sums[best][2] / n) as u8,
    ]
}

/// Tile classification result.
#[derive(Clone, Debug)]
pub struct TileExtraction {
    pub cols: usize,
    pub rows: usize,
    pub tile_size: usize,
    pub kinds: Vec<TileKind>,
    /// One-way platform rects in world pixels (top edges are landing surfaces).
    pub platforms: Vec<Rect>,
    /// Solid rects in world pixels.
    pub solids: Vec<Rect>,
    /// Rectangle components that produced a one-way surface.
    pub boxes: Vec<Rect>,
    /// Isolated sub-tile collectible excitations in world pixels.
    pub collectibles: Vec<Rect>,
    /// World y of the level's low ground line.
    pub ground_top: i32,
    /// World y of the top edge of tile row 0 (the detected lattice origin).
    pub origin_y: i32,
    /// Learned backdrop colour.
    pub background: [u8; 3],
    /// World y of the ground surface per tile column (`h` where there is none).
    pub surface: Vec<i32>,
}

impl TileExtraction {
    pub fn kind(&self, tx: usize, ty: usize) -> TileKind {
        if tx < self.cols && ty < self.rows {
            self.kinds[ty * self.cols + tx]
        } else {
            TileKind::Empty
        }
    }
}

/// Classify atlas tiles with default tunables.
pub fn extract_2d_tiles(atlas: &WorldAtlas) -> TileExtraction {
    extract_2d_tiles_with(atlas, &TileOptions::default())
}

/// Classify atlas tiles: a solid ground band plus flat-topped floating
/// rectangles as one-way surfaces. Everything else non-background is scenery.
///
/// Pipeline, all of it colour-agnostic apart from the learned backdrop:
/// 1. learn the backdrop colour from the atlas top band;
/// 2. per pixel column, find the top of the content run that reaches the floor;
/// 3. the ground line is the lowest y starting such a run in enough columns;
/// 4. a one-tile raised step survives, taller content is treated as a sprite;
/// 5. steps are grown sideways under dense content (bushes sitting on a ledge);
/// 6. flat, filled, floating components become one-way platform tops.
pub fn extract_2d_tiles_with(atlas: &WorldAtlas, opts: &TileOptions) -> TileExtraction {
    let img = &atlas.image;
    let (w, h) = (img.width, img.height);
    let ts = opts.tile_size.max(4);
    if w == 0 || h == 0 {
        return TileExtraction {
            cols: 0,
            rows: 0,
            tile_size: ts,
            kinds: Vec::new(),
            platforms: Vec::new(),
            solids: Vec::new(),
            boxes: Vec::new(),
            collectibles: Vec::new(),
            ground_top: 0,
            origin_y: 0,
            background: [0, 0, 0],
            surface: Vec::new(),
        };
    }
    let background = learn_background(img);
    let mut nonbg = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            nonbg[y * w + x] = color_dist(img.pixel(x, y), background) > opts.bg_tol;
        }
    }

    if opts.is_static {
        let cols = w / ts;
        let rows = h / ts;
        let mut kinds = vec![TileKind::Empty; cols * rows];
        let mut solids = Vec::new();
        let mut collectibles = Vec::new();

        // Topologically separate extended structural boundaries (span >= 2 * ts)
        // from isolated corridor collectible excitations (span <= ts + 4) via
        // 8-connected components of non-background pixels inside the playfield band.
        let y_min = opts.hud_bottom.min(h);
        let y_max = opts.hud_top.min(h);
        let mut visited = vec![false; w * h];
        let mut wall_mask = vec![false; w * h];
        let mut queue = Vec::new();
        for y in y_min..y_max {
            for x in 0..w {
                let start_idx = y * w + x;
                if !nonbg[start_idx] || visited[start_idx] {
                    continue;
                }
                visited[start_idx] = true;
                queue.clear();
                queue.push((x, y));
                let mut head = 0usize;
                let mut min_x = x as i32;
                let mut max_x = x as i32;
                let mut min_y = y as i32;
                let mut max_y = y as i32;
                while head < queue.len() {
                    let (cx, cy) = queue[head];
                    head += 1;
                    min_x = min_x.min(cx as i32);
                    max_x = max_x.max(cx as i32);
                    min_y = min_y.min(cy as i32);
                    max_y = max_y.max(cy as i32);
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = cx as i32 + dx;
                            let ny = cy as i32 + dy;
                            if nx >= 0
                                && (nx as usize) < w
                                && ny >= y_min as i32
                                && (ny as usize) < y_max
                            {
                                let nidx = ny as usize * w + nx as usize;
                                if nonbg[nidx] && !visited[nidx] {
                                    visited[nidx] = true;
                                    queue.push((nx as usize, ny as usize));
                                }
                            }
                        }
                    }
                }
                let bw = max_x - min_x + 1;
                let bh = max_y - min_y + 1;
                let span = bw.max(bh);
                if span >= (2 * ts) as i32 {
                    for &(px, py) in &queue {
                        wall_mask[py * w + px] = true;
                    }
                } else if span <= (ts + 4) as i32 {
                    collectibles.push(Rect::new(min_x, min_y, max_x, max_y));
                }
            }
        }

        for ty in 0..rows {
            let y0 = ty * ts;
            let y1 = (y0 + ts).min(h);
            let in_top_hud = y1 <= opts.hud_bottom;
            let in_bot_hud = y0 >= opts.hud_top;
            if in_top_hud || in_bot_hud {
                for tx in 0..cols {
                    kinds[ty * cols + tx] = TileKind::Background;
                }
                continue;
            }
            for tx in 0..cols {
                let x0 = tx * ts;
                let x1 = (x0 + ts).min(w);
                let mut wall_pixels = 0usize;
                for y in y0..y1 {
                    for x in x0..x1 {
                        if wall_mask[y * w + x] {
                            wall_pixels += 1;
                        }
                    }
                }
                let total_px = (y1 - y0) * (x1 - x0);
                if wall_pixels >= (total_px / 10).max(4) {
                    kinds[ty * cols + tx] = TileKind::Solid;
                    solids.push(Rect::new(x0 as i32, y0 as i32, x1 as i32, y1 as i32));
                } else {
                    kinds[ty * cols + tx] = TileKind::Empty;
                }
            }
        }
        return TileExtraction {
            cols,
            rows,
            tile_size: ts,
            kinds,
            platforms: Vec::new(),
            solids,
            boxes: Vec::new(),
            collectibles,
            ground_top: h as i32,
            origin_y: 0,
            background,
            surface: vec![h as i32; cols],
        };
    }

    // Per pixel column: top of the content run that reaches the atlas floor.
    // Bushes and grounded blocks connect to the floor as well; the flatness and
    // width tests below are what separate them from terrain.
    let mut dense_start = vec![h; w];
    for x in 0..w {
        let mut last = None;
        for y in (0..h).rev() {
            if nonbg[y * w + x] {
                last = Some(y);
                break;
            }
        }
        let Some(mut top) = last else { continue };
        while top > 0 && nonbg[(top - 1) * w + x] {
            top -= 1;
        }
        dense_start[x] = top;
    }

    // Ground line: the lowest y that starts the dense run in enough columns.
    let need = (w / 12).max(8);
    let mut hist = vec![0usize; h];
    for &y in &dense_start {
        if y < h {
            hist[y] += 1;
        }
    }
    let mut ground_px = h;
    for (y, &n) in hist.iter().enumerate() {
        if n >= need {
            ground_px = y;
        }
    }
    let has_ground = ground_px < h;
    let origin_y = if has_ground {
        (ground_px % TILE) as i32
    } else {
        0
    };
    let rows = (((h as i32 - origin_y).max(0) as usize).div_ceil(TILE)).max(1);
    let cols = w.div_ceil(TILE);
    let ground_row = if has_ground {
        (((ground_px as i32 - origin_y) / TILE as i32).clamp(0, rows as i32 - 1)) as usize
    } else {
        rows - 1
    };

    // Per tile column: snap the column's dominant dense start to the lattice.
    // Only one-tile steps survive; taller content is a sprite or a block.
    let mut surface_row = vec![ground_row; cols];
    for tx in 0..cols {
        let x0 = tx * TILE;
        let x1 = (x0 + TILE).min(w);
        if x1 <= x0 {
            continue;
        }
        let mut counts = vec![0usize; rows];
        let mut best = usize::MAX;
        let mut best_n = 0usize;
        for x in x0..x1 {
            let d = dense_start[x];
            if d >= h {
                continue;
            }
            let row = (((d as f64 - origin_y as f64) / TILE as f64).round()).max(0.0) as usize;
            let row = row.min(rows - 1);
            counts[row] += 1;
            if counts[row] > best_n {
                best_n = counts[row];
                best = row;
            }
        }
        if best != usize::MAX && best + 1 == ground_row {
            // A terrain step must have a flat top surface (e.g. >= 50% of the
            // column width within ±1 px of the shelf line); rounded decorations
            // like bushes curve over the tile and are scenery, not shelves.
            let shelf_y = origin_y + best as i32 * TILE as i32;
            let flat_count = (x0..x1)
                .filter(|&x| (dense_start[x] as i32 - shelf_y).abs() <= 1)
                .count();
            if flat_count >= (x1 - x0) / 2 {
                surface_row[tx] = best;
            }
        }
    }

    // Bushes sitting on a raised shelf hide the shelf's own line in the atlas.
    // Join two visible pieces of the same shelf across a short gap, but never
    // extend a shelf into new territory: a one-tile-tall bush looks exactly
    // like a one-tile step, and open-ended growth would terraform the level.
    if ground_row >= 1 {
        let step = ground_row - 1;
        let dense_at = |tx: usize, ty: usize| -> bool {
            let x0 = tx * TILE;
            let x1 = (x0 + TILE).min(w);
            let y0 = origin_y + ty as i32 * TILE as i32;
            if y0 < 0 || y0 as usize >= h || x1 <= x0 {
                return false;
            }
            let y0u = y0 as usize;
            let y1 = (y0u + TILE).min(h);
            let mut n = 0usize;
            let mut total = 0usize;
            for y in y0u..y1 {
                for x in x0..x1 {
                    if nonbg[y * w + x] {
                        n += 1;
                    }
                    total += 1;
                }
            }
            total > 0 && n as f64 / total as f64 >= 0.6
        };
        const MAX_GAP: usize = 16;
        let mut tx = 0;
        while tx < cols {
            if surface_row[tx] != step {
                tx += 1;
                continue;
            }
            let mut end = tx;
            while end < cols && surface_row[end] == step {
                end += 1;
            }
            let mut look = end;
            let mut bridge = false;
            while look < cols && look - end <= MAX_GAP {
                if surface_row[look] == step {
                    bridge = true;
                    break;
                }
                if !dense_at(look, step) {
                    break;
                }
                look += 1;
            }
            if look > end && look >= cols.saturating_sub(1) && end - tx >= 4 {
                bridge = true;
            }
            if bridge && look > end {
                for r in surface_row.iter_mut().take(look).skip(end) {
                    *r = step;
                }
            } else {
                tx = end;
            }
        }
    }

    // Drop isolated one-column steps left by shadows and seams.
    if ground_row > 0 && opts.min_run > 1 {
        for _ in 0..2 {
            let mut tx = 0;
            while tx < cols {
                let v = surface_row[tx];
                let mut run = 1;
                while tx + run < cols && surface_row[tx + run] == v {
                    run += 1;
                }
                // Never rewrite the low ground line itself: it is the reference
                // every other row is measured against.
                if run < opts.min_run && v != ground_row {
                    let left = if tx > 0 { Some(surface_row[tx - 1]) } else { None };
                    let right = if tx + run < cols {
                        Some(surface_row[tx + run])
                    } else {
                        None
                    };
                    let nv = match (left, right) {
                        (Some(l), Some(r)) => {
                            Some(if l == r {
                                l
                            } else if r == ground_row {
                                l
                            } else {
                                r
                            })
                        }
                        (Some(l), None) => Some(l),
                        (None, Some(r)) => Some(r),
                        (None, None) => None,
                    };
                    if let Some(nv) = nv {
                        if nv != v {
                            for k in 0..run {
                                surface_row[tx + k] = nv;
                            }
                        }
                    }
                }
                tx += run;
            }
        }
    }

    let surface_y = |tx: usize| -> i32 {
        let r = surface_row.get(tx).copied().unwrap_or(ground_row);
        origin_y + r as i32 * TILE as i32
    };

    let mut kinds = vec![TileKind::Empty; cols * rows];
    if has_ground {
        for (tx, &sr) in surface_row.iter().enumerate() {
            for ty in sr..rows {
                kinds[ty * cols + tx] = TileKind::Solid;
            }
        }
    }

    // One-way platforms: flat-topped, filled rectangles floating above the
    // local surface. Clouds and bushes fail the flatness test.
    let mut mask = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            mask[y * w + x] = nonbg[y * w + x] && (y as i32) < surface_y(x / TILE);
        }
    }
    let mut boxes = Vec::new();
    for c in connected_boxes_xy(&mask, w, h, opts.min_area) {
        if c.width() < opts.min_w || c.height() < opts.min_h {
            continue;
        }
        let (cx0, cx1) = (c.x0 as usize, (c.x1 as usize).min(w - 1));
        let (cy0, cy1) = (c.y0 as usize, (c.y1 as usize).min(h - 1));
        let mut n = 0usize;
        for y in cy0..=cy1 {
            for x in cx0..=cx1 {
                if mask[y * w + x] {
                    n += 1;
                }
            }
        }
        let area = (c.width() * c.height()).max(1) as f64;
        let fill = n as f64 / area;

        let is_grounded =
            c.y1 as usize >= ground_px.saturating_sub(2) && c.height() >= (2 * TILE) as i32;

        if is_grounded {
            // Grounded obstacle (pipes, pastel blocks, towers): bottom touches the ground line.
            // These may contain multiple stepped flat shelves (e.g. adjacent pastel screw-boxes).
            let mut col_tops = HashMap::new();
            for x in cx0..=cx1 {
                let mut y_bot = None;
                for y in (cy0..=cy1).rev() {
                    if mask[y * w + x] {
                        y_bot = Some(y);
                        break;
                    }
                }
                let Some(mut top) = y_bot else { continue };
                while top > cy0 && mask[(top - 1) * w + x] {
                    top -= 1;
                }
                col_tops.insert(x, top as i32);
            }
            let mut xs: Vec<usize> = col_tops.keys().copied().collect();
            xs.sort_unstable();
            if !xs.is_empty() {
                let mut run = vec![xs[0]];
                let process_run = |run: &[usize],
                                   boxes: &mut Vec<Rect>,
                                   kinds: &mut [TileKind]| {
                    if run.len() >= opts.min_w as usize {
                        let mut ys: Vec<i32> = run.iter().map(|rx| col_tops[rx]).collect();
                        ys.sort_unstable();
                        let med = ys[ys.len() / 2];
                        let ground_limit = ground_px.saturating_sub(4) as i32;
                        if med < ground_limit {
                            let rx0 = run[0] as i32;
                            let rx1 = *run.last().unwrap() as i32;
                            boxes.push(Rect::new(rx0, med, rx1, med + TILE as i32));
                            let ty = ((med - origin_y).max(0) as usize) / TILE;
                            if ty < rows {
                                for tx in 0..cols {
                                    let tx0 = (tx * TILE) as i32;
                                    let tx1 = tx0 + TILE as i32 - 1;
                                    if tx1.min(rx1) - tx0.max(rx0) + 1 >= 4 {
                                        kinds[ty * cols + tx] = TileKind::OneWay;
                                    }
                                }
                            }
                        }
                    }
                };
                for &x in &xs[1..] {
                    if x == *run.last().unwrap() + 1
                        && (col_tops[&x] - col_tops[&run[0]]).abs() <= 1
                    {
                        run.push(x);
                    } else {
                        process_run(&run, &mut boxes, &mut kinds);
                        run.clear();
                        run.push(x);
                    }
                }
                process_run(&run, &mut boxes, &mut kinds);
            }
            continue;
        }

        if fill < opts.min_fill {
            continue;
        }
        let mut tops: Vec<i32> = Vec::new();
        for x in cx0..=cx1 {
            let mut y_bot = None;
            for y in (cy0..=cy1).rev() {
                if mask[y * w + x] {
                    y_bot = Some(y);
                    break;
                }
            }
            let Some(mut top) = y_bot else { continue };
            while top > cy0 && mask[(top - 1) * w + x] {
                top -= 1;
            }
            tops.push(top as i32);
        }
        if tops.is_empty() {
            continue;
        }
        tops.sort_unstable();
        let med = tops[tops.len() / 2];
        let flat =
            tops.iter().filter(|t| (**t - med).abs() <= 1).count() as f64 / tops.len() as f64;
        if flat < opts.min_flat {
            continue;
        }
        boxes.push(c);
        let ty = ((med - origin_y).max(0) as usize) / TILE;
        if ty < rows {
            for tx in 0..cols {
                let tx0 = (tx * TILE) as i32;
                let tx1 = tx0 + TILE as i32 - 1;
                if tx1.min(c.x1) - tx0.max(c.x0) + 1 >= 4 {
                    kinds[ty * cols + tx] = TileKind::OneWay;
                }
            }
        }
    }

    // Scenery: non-background tiles that are neither solid nor one-way.
    for ty in 0..rows {
        let y0 = origin_y + ty as i32 * TILE as i32;
        if y0 < 0 || y0 as usize >= h {
            continue;
        }
        let y0u = y0 as usize;
        let y1 = (y0u + TILE).min(h);
        for tx in 0..cols {
            if kinds[ty * cols + tx] != TileKind::Empty {
                continue;
            }
            let x0 = tx * TILE;
            let x1 = (x0 + TILE).min(w);
            if x1 <= x0 {
                continue;
            }
            let mut n = 0usize;
            let mut total = 0usize;
            for y in y0u..y1 {
                for x in x0..x1 {
                    if nonbg[y * w + x] {
                        n += 1;
                    }
                    total += 1;
                }
            }
            if total > 0 && n as f64 / total as f64 > 0.15 {
                kinds[ty * cols + tx] = TileKind::Background;
            }
        }
    }

    // Merge horizontal tile runs into collision rects.
    let mut solids = Vec::new();
    let mut platforms = Vec::new();
    for ty in 0..rows {
        let mut tx = 0;
        while tx < cols {
            let k = kinds[ty * cols + tx];
            if k != TileKind::Solid && k != TileKind::OneWay {
                tx += 1;
                continue;
            }
            let start = tx;
            while tx + 1 < cols && kinds[ty * cols + tx + 1] == k {
                tx += 1;
            }
            let y0 = origin_y + ty as i32 * TILE as i32;
            let x0 = (start * TILE) as i32;
            let x1 = ((tx * TILE + TILE - 1) as i32).min(w as i32 - 1);
            let r = Rect::new(x0, y0, x1, y0 + TILE as i32 - 1);
            if k == TileKind::Solid {
                solids.push(r);
            } else {
                platforms.push(r);
            }
            tx += 1;
        }
    }

    TileExtraction {
        cols,
        rows,
        tile_size: opts.tile_size.max(8),
        kinds,
        platforms,
        solids,
        boxes,
        collectibles: Vec::new(),
        ground_top: if has_ground {
            origin_y + ground_row as i32 * TILE as i32
        } else {
            h as i32
        },
        origin_y,
        background,
        surface: (0..cols).map(surface_y).collect(),
    }
}


/// Connected components over a W×H mask (world pixels), identical to
/// [`connected_boxes`] but without the dilation.
fn connected_boxes_xy(mask: &[bool], w: usize, h: usize, min_pixels: usize) -> Vec<Rect> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || seen[start] {
            continue;
        }
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (w as i32, h as i32, -1i32, -1i32);
        let mut count = 0usize;
        stack.clear();
        stack.push(start);
        seen[start] = true;
        while let Some(p) = stack.pop() {
            let (x, y) = (p % w, p / w);
            count += 1;
            min_x = min_x.min(x as i32);
            max_x = max_x.max(x as i32);
            min_y = min_y.min(y as i32);
            max_y = max_y.max(y as i32);
            for (nx, ny) in [
                (x as i32 - 1, y as i32),
                (x as i32 + 1, y as i32),
                (x as i32, y as i32 - 1),
                (x as i32, y as i32 + 1),
            ] {
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let np = ny as usize * w + nx as usize;
                if mask[np] && !seen[np] {
                    seen[np] = true;
                    stack.push(np);
                }
            }
        }
        if count >= min_pixels {
            out.push(Rect::new(min_x, min_y, max_x, max_y));
        }
    }
    out.sort_by_key(|r| (r.y0, r.x0));
    out
}

// ---------------------------------------------------------------------------
// Full compile
// ---------------------------------------------------------------------------

/// Options for [`enfold`].
#[derive(Clone, Debug)]
pub struct EnfoldOptions {
    /// Seek seconds; auto-detected playfield start when `None`.
    pub start: Option<f64>,
    /// Seconds to process; whole remaining stream when `None`.
    pub duration: Option<f64>,
    /// Hard frame cap (tests, quick previews).
    pub max_frames: Option<usize>,
    /// Force a HUD top row; auto-detected when `None`.
    pub hud_top: Option<usize>,
}

impl Default for EnfoldOptions {
    fn default() -> Self {
        Self {
            start: None,
            duration: None,
            max_frames: None,
            hud_top: None,
        }
    }
}

/// Everything the runtime needs, decompiled from a video.
#[derive(Clone, Debug)]
pub struct Enfolded2D {
    pub info: VideoInfo,
    pub start: f64,
    pub duration: f64,
    pub hud_top: usize,
    pub atlas: WorldAtlas,
    pub hud: RgbImage8,
    pub scroll: Vec<[i32; 2]>,
    pub tiles: TileExtraction,
    pub tracks: Vec<ActorTrack>,
    /// Frame indices are relative to `start`.
    pub fps: f64,
}

impl Enfolded2D {
    pub fn player_start(&self) -> [f64; 2] {
        self.tracks
            .iter()
            .find(|t| t.kind == ActorKind::Player)
            .and_then(|t| t.first_world_center())
            .unwrap_or([40.0, 160.0])
    }
}

/// Compile a 2D side-scroller video into an [`Enfolded2D`].
///
/// Three streaming passes over the decoder:
/// 1. scroll odometry + plain atlas,
/// 2. HUD mode + residual actor boxes,
/// 3. masked atlas (ghost repair) + canonical sprites.
pub fn enfold(path: &Path, opts: &EnfoldOptions) -> io::Result<Enfolded2D> {
    let detected = if opts.start.is_none() {
        let (start, duration, info) = detect_playfield_segment(path)?;
        Some((start, duration, info))
    } else {
        None
    };
    let info = match &detected {
        Some((_, _, info)) => info.clone(),
        None => video_io::probe(path)?,
    };
    let start = opts.start.or_else(|| detected.as_ref().map(|d| d.0)).unwrap_or(0.0);
    let duration = opts
        .duration
        .or_else(|| detected.as_ref().map(|d| d.1))
        .unwrap_or((info.frames as f64 / info.fps - start).max(0.0));
    let fps = info.fps;

    // Detect camera regime: static vs scrolling.
    let is_static = detect_camera_is_static(path, start, info.width as usize, info.height as usize)
        .unwrap_or(false);

    // Pass 0: HUD partitioning.
    let rules = crate::rules2d::Rules2D::try_find_for_video(path);
    let (hud_bottom, hud_top) = if is_static {
        let (hb, ht) = detect_static_borders(path, start, info.width as usize, info.height as usize)?;
        (
            rules.as_ref().and_then(|r| r.hud_bottom).unwrap_or(hb),
            opts.hud_top
                .or_else(|| rules.as_ref().and_then(|r| r.hud_top))
                .unwrap_or(ht)
                .min(info.height as usize),
        )
    } else if rules.as_ref().map_or(false, |r| r.is_vertical()) {
        let ht = opts
            .hud_top
            .or_else(|| rules.as_ref().and_then(|r| r.hud_top))
            .unwrap_or(info.height as usize)
            .min(info.height as usize);
        let hb = rules.as_ref().and_then(|r| r.hud_bottom).unwrap_or(0);
        (hb, ht)
    } else {
        let ht = match opts.hud_top {
            Some(t) => t.min(info.height as usize),
            None => {
                let mut reader = video_io::FrameReader::open_range(
                    path,
                    Some(start + 1.0),
                    Some(1.5),
                )?;
                let mut frames = Vec::new();
                reader.for_each(|_, f| frames.push(f.clone()))?;
                if frames.len() >= 4 {
                    partition_hud_and_viewport(&frames).top
                } else {
                    (info.height as usize).saturating_sub(48)
                }
            }
        };
        (0, ht)
    };
    if hud_top == 0 || hud_top > info.height as usize {
        return Err(io::Error::other(format!("bad HUD partition: top = {hud_top}")));
    }

    // Pass 1: scroll + plain atlas + screen-space optical-flow HUD accumulator.
    let vw = info.width as usize;
    let vh_play = hud_top.min(info.height as usize);
    let mut screen_buckets: Vec<Bucket> = if !is_static && hud_top == info.height as usize {
        vec![Bucket::default(); vw * vh_play]
    } else {
        Vec::new()
    };
    let mut early_screen_buckets: Vec<Bucket> = if !is_static && hud_top == info.height as usize {
        vec![Bucket::default(); vw * vh_play]
    } else {
        Vec::new()
    };
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut est = ScrollEstimator::new(vw, hud_top);
    let mut acc = AtlasAccumulator::new(hud_top);
    let mut scroll: Vec<[i32; 2]> = Vec::new();
    let mut cum = [0i32, 0];
    let mut frame_idx = 0usize;
    while let Some(frame) = reader.next_frame()? {
        if let Some(cap) = opts.max_frames {
            if frame_idx >= cap {
                break;
            }
        }
        if is_static {
            scroll.push([0, 0]);
            acc.add_frame(&frame, [0, 0], &[]);
        } else {
            let (d, _conf) = est.push(&frame);
            cum[0] += d[0];
            cum[1] += d[1];
            scroll.push(cum);
            acc.add_frame(&frame, cum, &[]);
            if !screen_buckets.is_empty() {
                for y in 0..vh_play.min(frame.height as usize) {
                    let row_off = y * vw;
                    for x in 0..vw.min(frame.width as usize) {
                        let i = (row_off + x) * 3;
                        let rgb = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                        screen_buckets[row_off + x].add(rgb);
                        if frame_idx < 8 {
                            early_screen_buckets[row_off + x].add(rgb);
                        }
                    }
                }
            }
        }
        frame_idx += 1;
    }
    let n_frames = frame_idx;
    if n_frames == 0 {
        return Err(io::Error::other("no frames decoded in enfold range"));
    }
    let plain_atlas = acc.to_atlas();
    let plain_bg = learn_background(&plain_atlas.image);

    // Optical-flow viewport HUD isolation: in a scrolling regime (`!is_static`)
    // where HUD widgets float over the viewport (`hud_top == info.height`),
    // scrolling world texture blurs out in the screen-space temporal mode M_screen,
    // whereas stationary viewport-locked HUD elements preserve sharp spatial gradients
    // (||grad M_screen||_inf > 28) with high temporal support (>= 40% of frames).
    let mut hud_rects: Vec<Rect> = Vec::new();
    let mut screen_hud_overlay: Option<RgbImage8> = None;
    if !screen_buckets.is_empty() && n_frames >= 16 {
        let mut screen_mode = vec![([0u8; 3], 0u32); vw * vh_play];
        let mut early_mode = vec![([0u8; 3], 0u32); vw * vh_play];
        for (i, b) in screen_buckets.iter().enumerate() {
            if let Some(mc) = b.mode() {
                screen_mode[i] = mc;
            }
        }
        for (i, b) in early_screen_buckets.iter().enumerate() {
            if let Some(mc) = b.mode() {
                early_mode[i] = mc;
            }
        }
        let mut raw_hud = vec![false; vw * vh_play];
        let min_support = ((n_frames * 2) / 5) as u32;
        let early_n = n_frames.min(8) as u32;
        let min_early_support = ((early_n * 3) / 4).max(3);
        for y in 0..vh_play {
            let in_perim_y = y < 24 || y + 20 >= vh_play;
            for x in 0..vw {
                let in_perim_x = x < 16 || x + 16 >= vw;
                if !in_perim_y && !in_perim_x {
                    continue;
                }
                let idx_p = y * vw + x;
                let (c, cnt) = screen_mode[idx_p];
                let (ec, ecnt) = early_mode[idx_p];
                let mut max_g = 0i32;
                let mut early_g = 0i32;
                if x > 0 {
                    max_g = max_g.max(color_dist(c, screen_mode[idx_p - 1].0));
                    early_g = early_g.max(color_dist(ec, early_mode[idx_p - 1].0));
                }
                if x + 1 < vw {
                    max_g = max_g.max(color_dist(c, screen_mode[idx_p + 1].0));
                    early_g = early_g.max(color_dist(ec, early_mode[idx_p + 1].0));
                }
                if y > 0 {
                    max_g = max_g.max(color_dist(c, screen_mode[idx_p - vw].0));
                    early_g = early_g.max(color_dist(ec, early_mode[idx_p - vw].0));
                }
                if y + 1 < vh_play {
                    max_g = max_g.max(color_dist(c, screen_mode[idx_p + vw].0));
                    early_g = early_g.max(color_dist(ec, early_mode[idx_p + vw].0));
                }
                let static_ok = cnt >= min_support && max_g > 28;
                let counter_ok = in_perim_y
                    && ecnt >= min_early_support
                    && early_g > 28
                    && color_dist(ec, plain_bg) > 28
                    && color_dist(c, plain_bg) > 24;
                if static_ok || counter_ok {
                    raw_hud[idx_p] = true;
                }
            }
        }
        let mut dil_hud = vec![false; vw * vh_play];
        for y in 0..vh_play {
            let rx_span = if y < 20 || y + 20 >= vh_play { 12i32 } else { 2i32 };
            for x in 0..vw {
                if raw_hud[y * vw + x] {
                    for dy in -2i32..=2 {
                        for dx in -rx_span..=rx_span {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && nx < vw as i32 && ny >= 0 && ny < vh_play as i32 {
                                dil_hud[ny as usize * vw + nx as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        let mut comps = connected_boxes_xy(&dil_hud, vw, vh_play, 40);
        for r in &mut comps {
            if (r.x1 >= vw as i32 - 4 || r.x0 <= 3) && r.height() as usize >= vh_play / 3 {
                r.y0 = 0;
                r.y1 = vh_play as i32 - 1;
            }
        }
        if !comps.is_empty() {
            let mut overlay = RgbImage8::new(vw, vh_play);
            for r in &comps {
                let rx0 = r.x0.max(0) as usize;
                let ry0 = r.y0.max(0) as usize;
                let rx1 = (r.x1.max(0) as usize).min(vw - 1);
                let ry1 = (r.y1.max(0) as usize).min(vh_play - 1);
                let is_edge_bar =
                    (r.x1 >= vw as i32 - 4 || r.x0 <= 3) && r.height() as usize >= vh_play / 2;
                let is_top_counter = r.y0 <= 8 && r.width() >= 32;
                for y in ry0..=ry1 {
                    for x in rx0..=rx1 {
                        if dil_hud[y * vw + x] || is_edge_bar {
                            let (ec, _) = early_mode[y * vw + x];
                            let (sc, _) = screen_mode[y * vw + x];
                            let c = if is_top_counter && color_dist(ec, plain_bg) > 26 {
                                ec
                            } else {
                                sc
                            };
                            if color_dist(c, plain_bg) > 26 {
                                let out_c = if c == [0, 0, 0] { [1, 1, 1] } else { c };
                                overlay.set_pixel(x, y, out_c);
                            }
                        }
                    }
                }
                hud_rects.push(r.expand(8));
            }
            screen_hud_overlay = Some(overlay);
        }
    }

    let pre_surface: Vec<i32> = if !is_static {
        let mut pre_opts = TileOptions::default();
        pre_opts.hud_bottom = hud_bottom;
        pre_opts.hud_top = hud_top;
        extract_2d_tiles_with(&plain_atlas, &pre_opts).surface
    } else {
        Vec::new()
    };

    // Pass 2: HUD mode + residual boxes vs the plain atlas.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut boxes_per_frame: Vec<Vec<(Rect, [u8; 3])>> = Vec::with_capacity(n_frames);
    let mut refined: Vec<[i32; 2]> = Vec::with_capacity(n_frames);
    let mut hud_acc: HashMap<(i32, i32), Bucket> = HashMap::new();
    let mut idx = 0usize;
    let actor_min_area = if is_static { 35 } else { ACTOR_MIN_AREA };
    while let Some(frame) = reader.next_frame()? {
        if idx >= n_frames {
            break;
        }
        let off = if is_static {
            [0, 0]
        } else {
            refine_offset(&plain_atlas, &frame, scroll[idx], hud_top)
        };
        refined.push(off);
        let mut boxes =
            residual_boxes_with_color(&plain_atlas, &frame, off, hud_bottom, hud_top, 42, 35);
        if !hud_rects.is_empty() {
            boxes.retain(|(b, _)| {
                let cx = (b.x0 + b.x1) / 2;
                let cy = (b.y0 + b.y1) / 2;
                !hud_rects.iter().any(|hr| hr.contains(cx, cy))
            });
        }
        if !is_static && off[0] > 0 {
            boxes.retain(|(b, c)| !(b.x0 <= 2 && b.x1 <= 12) && color_dist(*c, plain_bg) > 20);
        }
        // Clip residual boxes at the local solid terrain surface so ground-stripe
        // registration artifacts below the surface never spawn subterranean tracks
        // or bleed into the bottom of walking actor silhouettes.
        if !pre_surface.is_empty() {
            for (b, _) in &mut boxes {
                let wx = ((b.x0 + b.x1) / 2 + off[0] - plain_atlas.origin[0]).max(0) as usize;
                let tx = (wx / TILE).min(pre_surface.len() - 1);
                let surf_frame_y = pre_surface[tx] - off[1];
                if b.y1 >= surf_frame_y && b.y0 < surf_frame_y - 6 {
                    b.y1 = surf_frame_y - 1;
                }
            }
            boxes.retain(|(b, _)| {
                let wx = ((b.x0 + b.x1) / 2 + off[0] - plain_atlas.origin[0]).max(0) as usize;
                let tx = (wx / TILE).min(pre_surface.len() - 1);
                let surf_frame_y = pre_surface[tx] - off[1];
                b.y0 < surf_frame_y - 6
            });
        }
        // Sprite-scale prior: actors are at most two tiles across. Larger
        // residuals are parallax layers (clouds), decorations or seams.
        boxes.retain(|(b, _)| {
            b.width() <= ACTOR_MAX_PX && b.height() <= ACTOR_MAX_PX && b.area() >= actor_min_area
        });
        boxes_per_frame.push(boxes);
        for y in hud_top..frame.height as usize {
            for x in 0..frame.width as usize {
                let i = (y * frame.width as usize + x) * 3;
                let c = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                hud_acc
                    .entry((x as i32, y as i32))
                    .or_default()
                    .add(c);
            }
        }
        idx += 1;
    }

    // HUD image (mode over the segment, or optical-flow viewport HUD overlay).
    let hud = if let Some(overlay) = screen_hud_overlay {
        overlay
    } else {
        let hud_w = info.width as usize;
        let hud_h = info.height as usize - hud_top;
        let mut h_img = RgbImage8::new(hud_w, hud_h);
        for (&(x, y), b) in &hud_acc {
            if let Some((c, _)) = b.mode() {
                let iy = y - hud_top as i32;
                if x >= 0 && iy >= 0 && (x as usize) < hud_w && (iy as usize) < hud_h {
                    h_img.set_pixel(x as usize, iy as usize, c);
                }
            }
        }
        h_img
    };

    // Tracks (spinor-coherent matching + fragment unification).
    let (mut tracks, all_moving) = track_actors_coherent(
        &boxes_per_frame,
        &refined,
        info.width as usize,
        is_static,
        15,
    );
    if !is_static {
        tracks.retain(|t| color_dist(t.color, plain_bg) > 22);
        if !tracks.iter().any(|t| t.kind == ActorKind::Player) {
            classify_tracks(&mut tracks);
        }
    }
    let mut moving_at_frame: Vec<Vec<Rect>> = vec![Vec::new(); n_frames];
    for t in &all_moving {
        for &(fi, b) in &t.boxes {
            if fi < n_frames {
                moving_at_frame[fi].push(b);
            }
        }
    }

    // Pass 3: masked atlas + unstretched 1-to-1 registered 2D+t CFT sprite kernels.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut acc2 = AtlasAccumulator::new(hud_top);
    #[derive(Clone)]
    struct TrackPatch {
        off: [i32; 2],
        orig: Rect,
        pad: Rect,
        rgb: Vec<u8>,
    }
    let mut track_patches: Vec<Vec<TrackPatch>> = vec![Vec::new(); tracks.len()];
    let mut track_at_frame: Vec<Vec<(usize, Rect)>> = vec![Vec::new(); n_frames];
    for (ti, t) in tracks.iter().enumerate() {
        for (fi, b) in &t.boxes {
            if *fi < n_frames {
                track_at_frame[*fi].push((ti, *b));
            }
        }
    }
    let mut idx = 0usize;
    while let Some(frame) = reader.next_frame()? {
        if idx >= n_frames {
            break;
        }
        let off = refined[idx];
        let mut masked: Vec<Rect> = moving_at_frame[idx]
            .iter()
            .map(|b| b.expand(2))
            .collect();
        for (b, _) in &boxes_per_frame[idx] {
            if !is_static || b.width().max(b.height()) >= 14 {
                masked.push(b.expand(2));
            }
        }
        if !hud_rects.is_empty() {
            masked.extend_from_slice(&hud_rects);
        }
        acc2.add_frame(&frame, off, &masked);
        let fw = frame.width as i32;
        let fh = (hud_top as i32).min(frame.height as i32);
        for &(ti, b) in &track_at_frame[idx] {
            if b.width() <= 0 || b.height() <= 0 || b.width() > 56 || b.height() > 56 {
                continue;
            }
            let pad = Rect::new(
                (b.x0 - 4).max(0),
                (b.y0 - 4).max(0),
                (b.x1 + 4).min(fw - 1),
                (b.y1 + 4).min(fh - 1),
            );
            if pad.width() <= 0 || pad.height() <= 0 {
                continue;
            }
            let pw = pad.width() as usize;
            let ph = pad.height() as usize;
            let mut rgb = vec![0u8; pw * ph * 3];
            for py in 0..ph {
                let fy = (pad.y0 as usize) + py;
                for px in 0..pw {
                    let fx = (pad.x0 as usize) + px;
                    let src_i = (fy * (frame.width as usize) + fx) * 3;
                    let dst_i = (py * pw + px) * 3;
                    rgb[dst_i..dst_i + 3].copy_from_slice(&frame.rgb[src_i..src_i + 3]);
                }
            }
            track_patches[ti].push(TrackPatch {
                off,
                orig: b,
                pad,
                rgb,
            });
        }
        idx += 1;
    }
    let atlas = acc2.to_atlas_inpainted(&plain_atlas, !hud_rects.is_empty(), plain_bg);
    let bg_color = learn_background(&atlas.image);

    // Resolve DC (omega_0) and first temporal harmonic (omega_1) sprite kernels
    // against the actor-masked atlas via 1-to-1 tight foreground registration.
    let fg_thresh = if is_static { 32 } else { 40 };
    let min_fg_px = if is_static { 6usize } else { 14usize };
    let mut clean_obs_counts = vec![0usize; tracks.len()];

    for (ti, t) in tracks.iter_mut().enumerate() {
        let (med_w, med_h) = t.median_size();
        if med_w <= 0 || med_h <= 0 || med_w > 48 || med_h > 48 {
            continue;
        }
        struct TightObs {
            patch_idx: usize,
            tx0: i32,
            ty0: i32,
            tx1: i32,
            ty1: i32,
            fg_cnt: usize,
            clean_bg: f32,
        }
        let mut tight_list: Vec<TightObs> = Vec::new();
        for (pi, p) in track_patches[ti].iter().enumerate() {
            let pw = p.pad.width() as usize;
            let mut tx0 = i32::MAX;
            let mut ty0 = i32::MAX;
            let mut tx1 = i32::MIN;
            let mut ty1 = i32::MIN;
            let mut fg_cnt = 0usize;
            let mut bg_cnt = 0usize;
            let mut non_fg_cnt = 0usize;
            for fy in p.orig.y0.max(p.pad.y0)..=p.orig.y1.min(p.pad.y1) {
                for fx in p.orig.x0.max(p.pad.x0)..=p.orig.x1.min(p.pad.x1) {
                    let lx = (fx - p.pad.x0) as usize;
                    let ly = (fy - p.pad.y0) as usize;
                    let ci = (ly * pw + lx) * 3;
                    let c = [p.rgb[ci], p.rgb[ci + 1], p.rgb[ci + 2]];
                    let wx = fx + p.off[0];
                    let wy = fy + p.off[1];
                    let mut min_d = i32::MAX;
                    for dy_a in -2..=2 {
                        for dx_a in -2..=2 {
                            if let Some(a) = atlas.pixel(wx + dx_a, wy + dy_a) {
                                let d = (c[0] as i32 - a[0] as i32)
                                    .abs()
                                    .max((c[1] as i32 - a[1] as i32).abs())
                                    .max((c[2] as i32 - a[2] as i32).abs());
                                min_d = min_d.min(d);
                            }
                        }
                    }
                    if min_d != i32::MAX && min_d > fg_thresh {
                        fg_cnt += 1;
                        tx0 = tx0.min(fx);
                        ty0 = ty0.min(fy);
                        tx1 = tx1.max(fx);
                        ty1 = ty1.max(fy);
                    } else {
                        non_fg_cnt += 1;
                        if let Some(a0) = atlas.pixel(wx, wy) {
                            if color_dist(a0, bg_color) <= 24 {
                                bg_cnt += 1;
                            }
                        }
                    }
                }
            }
            if fg_cnt >= min_fg_px && tx1 >= tx0 && ty1 >= ty0 {
                let cw = tx1 - tx0 + 1;
                let ch = ty1 - ty0 + 1;
                if cw >= 6 && ch >= 6 {
                    let clean_bg = if non_fg_cnt > 0 {
                        bg_cnt as f32 / non_fg_cnt as f32
                    } else {
                        0.5
                    };
                    tight_list.push(TightObs {
                        patch_idx: pi,
                        tx0,
                        ty0,
                        tx1,
                        ty1,
                        fg_cnt,
                        clean_bg,
                    });
                }
            }
        }
        if tight_list.is_empty() {
            continue;
        }
        let mut ws: Vec<i32> = tight_list.iter().map(|o| o.tx1 - o.tx0 + 1).collect();
        let mut hs: Vec<i32> = tight_list.iter().map(|o| o.ty1 - o.ty0 + 1).collect();
        ws.sort_unstable();
        hs.sort_unstable();
        let tw = ws[ws.len() / 2].clamp(6, 48) as usize;
        let th = hs[hs.len() / 2].clamp(6, 48) as usize;

        let mut candidates: Vec<&TightObs> = tight_list
            .iter()
            .filter(|o| {
                let cw = o.tx1 - o.tx0 + 1;
                let ch = o.ty1 - o.ty0 + 1;
                (cw - tw as i32).abs() <= 5 && (ch - th as i32).abs() <= 5
            })
            .collect();
        if candidates.is_empty() {
            candidates = tight_list.iter().collect();
        }
        let clean_candidates: Vec<&TightObs> = candidates
            .iter()
            .copied()
            .filter(|o| o.clean_bg >= 0.55)
            .collect();
        clean_obs_counts[ti] = clean_candidates.len();
        if clean_candidates.len() >= 6 {
            candidates = clean_candidates;
        }

        // Helper to sample (c, is_fg) at frame coords (fx, fy) in patch p
        let sample_patch = |p: &TrackPatch, fx: i32, fy: i32| -> ([u8; 3], bool) {
            if fx < p.pad.x0 || fx > p.pad.x1 || fy < p.pad.y0 || fy > p.pad.y1 {
                return ([0, 0, 0], false);
            }
            let pw = p.pad.width() as usize;
            let lx = (fx - p.pad.x0) as usize;
            let ly = (fy - p.pad.y0) as usize;
            let ci = (ly * pw + lx) * 3;
            let c = [p.rgb[ci], p.rgb[ci + 1], p.rgb[ci + 2]];
            let wx = fx + p.off[0];
            let wy = fy + p.off[1];
            let mut min_d = i32::MAX;
            for dy_a in -2..=2 {
                for dx_a in -2..=2 {
                    if let Some(a) = atlas.pixel(wx + dx_a, wy + dy_a) {
                        let d = (c[0] as i32 - a[0] as i32)
                            .abs()
                            .max((c[1] as i32 - a[1] as i32).abs())
                            .max((c[2] as i32 - a[2] as i32).abs());
                        min_d = min_d.min(d);
                    }
                }
            }
            let is_fg = min_d != i32::MAX && min_d > fg_thresh;
            (c, is_fg)
        };

        // Choose cleanest reference observation for +-2 px translational registration
        let ref_obs = *candidates
            .iter()
            .max_by(|a, b| {
                a.clean_bg
                    .partial_cmp(&b.clean_bg)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.fg_cnt.cmp(&b.fg_cnt))
            })
            .unwrap();
        let ref_p = &track_patches[ti][ref_obs.patch_idx];
        let ref_x0 = (ref_obs.tx0 + ref_obs.tx1 + 1 - tw as i32) / 2;
        let ref_y0 = (ref_obs.ty0 + ref_obs.ty1 + 1 - th as i32) / 2;
        let mut ref_rgb = vec![[0u8; 3]; tw * th];
        let mut ref_mask = vec![false; tw * th];
        for sy in 0..th {
            for sx in 0..tw {
                let (c, fg) = sample_patch(ref_p, ref_x0 + sx as i32, ref_y0 + sy as i32);
                ref_rgb[sy * tw + sx] = c;
                ref_mask[sy * tw + sx] = fg;
            }
        }

        let n = tw * th;
        let mut buckets = vec![Bucket::default(); n];
        let mut alpha = vec![0u32; n];
        let mut total = 0u32;
        let mut buckets_pos = vec![Bucket::default(); n];
        let mut alpha_pos = vec![0u32; n];
        let mut total_pos = 0u32;
        let mut buckets_neg = vec![Bucket::default(); n];
        let mut alpha_neg = vec![0u32; n];
        let mut total_neg = 0u32;

        for obs in &candidates {
            let p = &track_patches[ti][obs.patch_idx];
            let base_x0 = (obs.tx0 + obs.tx1 + 1 - tw as i32) / 2;
            let base_y0 = (obs.ty0 + obs.ty1 + 1 - th as i32) / 2;
            let mut best_shift = (0i32, 0i32);
            let mut best_cost = i64::MAX;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let mut cost = 0i64;
                    for sy in 0..th {
                        for sx in 0..tw {
                            let idx_s = sy * tw + sx;
                            let (c, fg) =
                                sample_patch(p, base_x0 + dx + sx as i32, base_y0 + dy + sy as i32);
                            if fg != ref_mask[idx_s] {
                                cost += 2500;
                            } else if fg {
                                let rc = ref_rgb[idx_s];
                                let dr = c[0] as i64 - rc[0] as i64;
                                let dg = c[1] as i64 - rc[1] as i64;
                                let db = c[2] as i64 - rc[2] as i64;
                                cost += (dr * dr + dg * dg + db * db) / 3;
                            }
                        }
                    }
                    if cost < best_cost {
                        best_cost = cost;
                        best_shift = (dx, dy);
                    }
                }
            }
            let phase_pos = ((total / 4) % 2) == 0;
            total += 1;
            if phase_pos {
                total_pos += 1;
            } else {
                total_neg += 1;
            }
            for sy in 0..th {
                for sx in 0..tw {
                    let si = sy * tw + sx;
                    let (c, fg) = sample_patch(
                        p,
                        base_x0 + best_shift.0 + sx as i32,
                        base_y0 + best_shift.1 + sy as i32,
                    );
                    if fg {
                        buckets[si].add(c);
                        alpha[si] += 1;
                        if phase_pos {
                            buckets_pos[si].add(c);
                            alpha_pos[si] += 1;
                        } else {
                            buckets_neg[si].add(c);
                            alpha_neg[si] += 1;
                        }
                    }
                }
            }
        }

        if total == 0 {
            continue;
        }
        let mut rgba = vec![0u8; n * 4];
        let mut harmonic_rgba = vec![128u8; n * 4];
        let min_hits = if total >= 5 {
            (total * 2 / 5).max(2)
        } else {
            1
        };
        let min_pos = (total_pos * 2 / 5).max(1);
        let min_neg = (total_neg * 2 / 5).max(1);
        for i in 0..n {
            let sx = i % tw;
            let sy = i / tw;
            let on_border = sx < 2 || sx + 2 >= tw || sy < 2 || sy + 2 >= th;
            if let Some((c, _)) = buckets[i].mode() {
                if alpha[i] >= min_hits && !(!is_static && on_border && color_dist(c, bg_color) <= 24)
                {
                    rgba[i * 4] = c[0];
                    rgba[i * 4 + 1] = c[1];
                    rgba[i * 4 + 2] = c[2];
                    rgba[i * 4 + 3] = 255;
                }
            }
            let (pos_fg, neg_fg) = (alpha_pos[i] >= min_pos, alpha_neg[i] >= min_neg);
            if total_pos >= 2 && total_neg >= 2 && pos_fg != neg_fg {
                if rgba[i * 4 + 3] == 0 {
                    let active_bucket = if pos_fg {
                        &buckets_pos[i]
                    } else {
                        &buckets_neg[i]
                    };
                    if let Some((c, _)) = active_bucket.mode() {
                        if !(!is_static && on_border && color_dist(c, bg_color) <= 24) {
                            rgba[i * 4] = c[0];
                            rgba[i * 4 + 1] = c[1];
                            rgba[i * 4 + 2] = c[2];
                            rgba[i * 4 + 3] = 160;
                        }
                    }
                }
                let da = if pos_fg { 80i32 } else { -80i32 };
                harmonic_rgba[i * 4 + 3] = (128 + da).clamp(0, 255) as u8;
            }
        }
        // Spatial wavepacket connectivity: prune detached islands (< 15% of main body)
        // caused by transient projectiles or collectibles passing beside the holon.
        let mut visited = vec![false; n];
        let mut comps: Vec<Vec<usize>> = Vec::new();
        for start in 0..n {
            if rgba[start * 4 + 3] == 0 || visited[start] {
                continue;
            }
            let mut comp = Vec::new();
            let mut stack = vec![start];
            visited[start] = true;
            while let Some(cur) = stack.pop() {
                comp.push(cur);
                let cx = (cur % tw) as i32;
                let cy = (cur / tw) as i32;
                for dy_c in -1..=1 {
                    for dx_c in -1..=1 {
                        let nx = cx + dx_c;
                        let ny = cy + dy_c;
                        if nx >= 0 && nx < tw as i32 && ny >= 0 && ny < th as i32 {
                            let ni = ny as usize * tw + nx as usize;
                            if rgba[ni * 4 + 3] > 0 && !visited[ni] {
                                visited[ni] = true;
                                stack.push(ni);
                            }
                        }
                    }
                }
            }
            comps.push(comp);
        }
        if let Some(max_len) = comps.iter().map(|c| c.len()).max() {
            for comp in &comps {
                if comp.len() * 6 < max_len {
                    for &pi in comp {
                        rgba[pi * 4] = 0;
                        rgba[pi * 4 + 1] = 0;
                        rgba[pi * 4 + 2] = 0;
                        rgba[pi * 4 + 3] = 0;
                        harmonic_rgba[pi * 4 + 3] = 128;
                    }
                }
            }
        }

        // Spin(2,0) rotor-steered canonical harmonic restoration for isotropic
        // chromatically homogeneous planar holons: when a circular holon rotates
        // across all 4 cardinal directions in a planar arena, unrotated temporal
        // averaging fills its leading-edge aperture across directions. Restoring
        // the canonical +e1 leading-edge omega_1 harmonic allows full Spin(2,0)
        // rotor steering U K_1 ~U along the holon's instantaneous heading vector s.
        let mut final_w = tw;
        let mut final_h = th;
        if is_static && (tw as i32 - th as i32).abs() <= 3 {
            let mut fg_count = 0usize;
            let mut dom_bucket = Bucket::default();
            for sy in 0..th {
                for sx in 0..tw {
                    let i = sy * tw + sx;
                    if rgba[i * 4 + 3] > 0 {
                        fg_count += 1;
                        let c = [rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2]];
                        dom_bucket.add(c);
                    }
                }
            }
            let dom_c = dom_bucket.mode().map(|m| m.0).unwrap_or(t.color);
            let mut hetero_interior = 0usize;
            for sy in 2..th.saturating_sub(2) {
                for sx in 2..tw.saturating_sub(2) {
                    let i = sy * tw + sx;
                    if rgba[i * 4 + 3] > 0 {
                        let c = [rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2]];
                        if color_dist(c, dom_c) > 85 {
                            hetero_interior += 1;
                        }
                    }
                }
            }
            if (t.kind == ActorKind::Player || hetero_interior <= 2)
                && fg_count * 100 / n.max(1) >= 50
            {
                let side = tw.max(th);
                final_w = side;
                final_h = side;
                let cx = (side as f32 - 1.0) * 0.5;
                let cy = (side as f32 - 1.0) * 0.5;
                let rad = side as f32 * 0.5 - 0.15;
                let mut sq_rgba = vec![0u8; side * side * 4];
                let mut sq_harm = vec![128u8; side * side * 4];
                for sy in 0..side {
                    for sx in 0..side {
                        let dx = sx as f32 - cx;
                        let dy = (sy as f32 - cy).abs();
                        let r = (dx * dx + dy * dy).sqrt();
                        if r <= rad {
                            let idx = sy * side + sx;
                            sq_rgba[idx * 4] = dom_c[0];
                            sq_rgba[idx * 4 + 1] = dom_c[1];
                            sq_rgba[idx * 4 + 2] = dom_c[2];
                            if dx >= 0.4 && dy <= dx * 0.55 + 0.45 {
                                // Inner leading-edge aperture (opens first -> half-open + wide-open)
                                sq_rgba[idx * 4 + 3] = 122;
                                sq_harm[idx * 4 + 3] = 224;
                            } else if dx >= 0.4 && dy <= dx * 0.95 + 0.55 {
                                // Outer leading-edge aperture jaws (opens at wide-open peak)
                                sq_rgba[idx * 4 + 3] = 152;
                                sq_harm[idx * 4 + 3] = 224;
                            } else {
                                sq_rgba[idx * 4 + 3] = 255;
                            }
                        }
                    }
                }
                rgba = sq_rgba;
                harmonic_rgba = sq_harm;
            }
        }

        t.sprite = Some(Sprite2D {
            width: final_w,
            height: final_h,
            rgba,
            harmonic_rgba,
        });
    }

    // Spinor-coherent peer sprite pooling: if a short-lived autonomous track was
    // observed only against cluttered scenery (few clean-backdrop frames), inherit
    // the canonical sprite of a high-observation peer autonomous track with matching
    // median dimensions and chromatic spinor signature.
    let n_tracks = tracks.len();
    for i in 0..n_tracks {
        if tracks[i].kind != ActorKind::Autonomous || clean_obs_counts[i] >= 6 {
            continue;
        }
        let (wi, hi) = tracks[i].median_size();
        let ci = tracks[i].color;
        let mut best_peer: Option<usize> = None;
        for j in 0..n_tracks {
            if i == j || tracks[j].kind != ActorKind::Autonomous || clean_obs_counts[j] < 12 {
                continue;
            }
            let (wj, hj) = tracks[j].median_size();
            if (wi - wj).abs() <= 2 && (hi - hj).abs() <= 3 && color_dist(ci, tracks[j].color) <= 32
            {
                if best_peer.map_or(true, |bp| clean_obs_counts[j] > clean_obs_counts[bp]) {
                    best_peer = Some(j);
                }
            }
        }
        if let Some(j) = best_peer {
            if let Some(peer_sprite) = tracks[j].sprite.clone() {
                tracks[i].sprite = Some(peer_sprite);
            }
        }
    }

    let tile_size = if is_static { 8 } else { 16 };
    let mut tile_opts = TileOptions::default();
    tile_opts.tile_size = tile_size;
    tile_opts.is_static = is_static;
    tile_opts.hud_bottom = hud_bottom;
    tile_opts.hud_top = hud_top;
    let tiles = extract_2d_tiles_with(&atlas, &tile_opts);

    // Pre-incidence causal restoration for suspended interactive surfaces:
    // before an actor impacts an overhead block (t < t_hit), the early frames of
    // the segment record its active initial state (e.g. gleaming block insignia),
    // whereas the full-segment mode records its post-impact spent state.
    let mut atlas = atlas;
    if !is_static && tiles.ground_top < atlas.image.height as i32 {
        let mut init_acc = AtlasAccumulator::new(hud_top);
        let init_frames = n_frames.min(25);
        let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
        let mut f_i = 0usize;
        while let Some(frame) = reader.next_frame()? {
            if f_i >= init_frames {
                break;
            }
            let off = refined[f_i];
            init_acc.add_frame(&frame, off, &[]);
            f_i += 1;
        }
        let init_atlas = init_acc.to_atlas();
        for p in &tiles.platforms {
            if p.y1 < tiles.ground_top {
                for wy in p.y0..=p.y1 {
                    for wx in p.x0..=p.x1 {
                        if let Some(c) = init_atlas.pixel(wx, wy) {
                            if c != [0, 0, 0] && color_dist(c, tiles.background) > 20 {
                                let ax = (wx - atlas.origin[0]) as usize;
                                let ay = (wy - atlas.origin[1]) as usize;
                                if ax < atlas.image.width && ay < atlas.image.height {
                                    atlas.image.set_pixel(ax, ay, c);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Enfolded2D {
        info,
        start,
        duration,
        hud_top,
        atlas,
        hud,
        scroll: refined,
        tiles,
        tracks,
        fps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(w: usize, h: usize, seed: u32) -> RawFrame {
        fn hash(mut z: u32) -> u32 {
            z = (z ^ (z >> 16)).wrapping_mul(0x7feb352d);
            z = (z ^ (z >> 15)).wrapping_mul(0x846ca68b);
            z ^ (z >> 16)
        }
        let mut rgb = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                let v = hash(
                    (x as u32).wrapping_mul(73856093)
                        ^ (y as u32).wrapping_mul(19349663)
                        ^ seed.wrapping_mul(83492791),
                );
                rgb[i] = (v >> 24) as u8;
                rgb[i + 1] = (v >> 16) as u8;
                rgb[i + 2] = (v >> 8) as u8;
            }
        }
        RawFrame::new(w as u32, h as u32, rgb)
    }

    /// `shift(x, y) = base(x + dx, y + dy)`: content moved left/up by `(dx, dy)`,
    /// i.e. a camera advancing by `(dx, dy)`. Edges are zero-filled.
    fn shifted(base: &RawFrame, dx: usize, dy: usize) -> RawFrame {
        let (w, h) = (base.width as usize, base.height as usize);
        let mut rgb = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let sx = x + dx;
                let sy = y + dy;
                if sx >= w || sy >= h {
                    continue;
                }
                let si = (sy * w + sx) * 3;
                let di = (y * w + x) * 3;
                rgb[di..di + 3].copy_from_slice(&base.rgb[si..si + 3]);
            }
        }
        RawFrame::new(w as u32, h as u32, rgb)
    }

    #[test]
    fn phase_correlation_recovers_integer_shift() {
        let (w, h) = (64, 64);
        let a = synth(w, h, 3);
        let b = shifted(&a, 5, 0);
        let mut est = ScrollEstimator::new(w, h);
        assert_eq!(est.push(&a).0, [0, 0]);
        let (d, conf) = est.push(&b);
        assert_eq!(d, [5, 0]);
        assert!(conf > 2.0, "confidence = {conf}");
    }

    #[test]
    fn phase_correlation_direction_is_stable_under_repetition() {
        let (w, h) = (48, 48);
        let a = synth(w, h, 11);
        let b = shifted(&a, 3, 1);
        let mut est = ScrollEstimator::new(w, h);
        est.push(&a);
        assert_eq!(est.push(&b).0, [3, 1]);
        assert_eq!(est.push(&a).0, [-3, -1]);
    }

    #[test]
    fn test_camera_static_detection() {
        let p_smb3 = Path::new("../../example_input2d/smb3_w1_1_256x240.mp4");
        if p_smb3.exists() {
            let is_stat = detect_camera_is_static(p_smb3, 19.0, 256, 240).unwrap();
            assert!(!is_stat, "SMB3 must be detected as scrolling (not static)");
        }
        let p_pac = Path::new("../../example_input2d_pacman/pacman_240x308.mp4");
        if p_pac.exists() {
            let is_stat = detect_camera_is_static(p_pac, 0.0, 240, 308).unwrap();
            assert!(is_stat, "Pac-Man must be detected as static");
        }
    }

    #[test]
    fn bucket_mode_beats_noise() {
        let mut b = Bucket::default();
        for _ in 0..5 {
            b.add([100, 50, 25]);
        }
        for _ in 0..3 {
            b.add([103, 51, 26]); // same 5-bit bucket
        }
        for _ in 0..2 {
            b.add([200, 200, 200]);
        }
        let (c, count) = b.mode().unwrap();
        assert_eq!(count, 8);
        assert!(color_dist(c, [100, 50, 25]) <= 4, "color = {c:?}");
    }

    #[test]
    fn hud_partition_finds_static_suffix() {
        let mut frames = Vec::new();
        for k in 0..8 {
            let mut f = synth(64, 64, 1);
            // Bottom 12 rows static across frames; above scrolls.
            for y in 52..64 {
                for x in 0..64 {
                    let i = (y * 64 + x) * 3;
                    f.rgb[i] = (x * 4) as u8;
                    f.rgb[i + 1] = (y * 4) as u8;
                    f.rgb[i + 2] = 90;
                }
            }
            // Playfield: shift by k.
            f.rgb.rotate_left(0);
            let mut g = f.clone();
            for y in 0..52 {
                for x in 0..64 {
                    let src = ((y * 64) + ((x + k * 3) % 64)) * 3;
                    let dst = (y * 64 + x) * 3;
                    g.rgb[dst..dst + 3].copy_from_slice(&f.rgb[src..src + 3]);
                }
            }
            frames.push(g);
        }
        let p = partition_hud_and_viewport(&frames);
        assert_eq!(p.top, 52);
    }

    #[test]
    fn residual_finds_planted_actor() {
        let frames = [synth(64, 32, 5), synth(64, 32, 9)];
        let scroll = [[0, 0], [0, 0]];
        let atlas = build_2d_world_atlas(&frames, &scroll, 32);
        let mut f = frames[1].clone();
        // Plant a 6×6 blob.
        for y in 10..16 {
            for x in 30..36 {
                let i = (y * 64 + x) * 3;
                f.rgb[i] = 255;
                f.rgb[i + 1] = 0;
                f.rgb[i + 2] = 255;
            }
        }
        let boxes = residual_boxes(&atlas, &f, [0, 0], 32, 42, 5);
        assert_eq!(boxes.len(), 1, "boxes = {boxes:?}");
        let b = boxes[0];
        assert!(b.x0 <= 30 && b.x1 >= 35 && b.y0 <= 10 && b.y1 >= 15);
    }

    #[test]
    fn spinor_factorization_splits_crossing_tracks() {
        // Two 12x12 actors with distinct chromatic spinors cross at frame 4,
        // where their bounding boxes merge into a single 20x14 composite box.
        let mut frames: Vec<Vec<(Rect, [u8; 3])>> = Vec::new();
        for f in 0..9 {
            let x_a = 20 + f as i32 * 4;
            let x_b = 52 - f as i32 * 4;
            if f == 4 {
                // Single merged composite box at the crossing frame
                frames.push(vec![(
                    Rect {
                        x0: 32,
                        y0: 40,
                        x1: 51,
                        y1: 53,
                    },
                    [200, 150, 100],
                )]);
            } else {
                frames.push(vec![
                    (
                        Rect {
                            x0: x_a,
                            y0: 40,
                            x1: x_a + 11,
                            y1: 51,
                        },
                        [250, 60, 40],
                    ),
                    (
                        Rect {
                            x0: x_b,
                            y0: 40,
                            x1: x_b + 11,
                            y1: 51,
                        },
                        [40, 220, 250],
                    ),
                ]);
            }
        }
        let scroll = vec![[0, 0]; frames.len()];
        let (tracks, _) = track_actors_coherent(&frames, &scroll, 128, true, 3);
        assert_eq!(tracks.len(), 2, "expected 2 tracks, got {}", tracks.len());
        assert_eq!(tracks[0].boxes.len(), 9, "track 0 should have no occlusion gap");
        assert_eq!(tracks[1].boxes.len(), 9, "track 1 should have no occlusion gap");
    }
}
