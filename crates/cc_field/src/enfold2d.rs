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
pub const ACTOR_MAX_PX: i32 = 40;
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
    Ok((
        s as f64 / fps,
        (e - s) as f64 / fps,
        info,
    ))
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
}

/// Streaming world atlas accumulator.
///
/// Each world pixel keeps a bounded palette of quantized colors; the final
/// value is the mode, which suppresses moving actors far better than a
/// channel-wise median (which produces ghost blends).
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
        for y in 0..rows {
            let wy = y as i32 + off[1];
            for x in 0..fw {
                let wx = x as i32 + off[0];
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
            }
        }
    }

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
    for y in y_start..h {
        for x in 0..w {
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

/// What an extracted track is playing as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorKind {
    Player,
    Goomba,
    Koopa,
    Unknown,
}

impl ActorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorKind::Player => "player",
            ActorKind::Goomba => "goomba",
            ActorKind::Koopa => "koopa",
            ActorKind::Unknown => "unknown",
        }
    }
}

/// Canonical RGBA sprite with per-pixel coverage alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite2D {
    pub width: usize,
    pub height: usize,
    /// RGBA, row-major.
    pub rgba: Vec<u8>,
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
    /// `(frame_index, world_box)` for every detection.
    pub boxes: Vec<(usize, Rect)>,
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
    struct Open {
        boxes: Vec<(usize, Rect)>,
        last: usize,
        last_box: Rect,
    }
    let mut open: Vec<Open> = Vec::new();
    let mut closed: Vec<ActorTrack> = Vec::new();
    for (fi, boxes) in boxes_per_frame.iter().enumerate() {
        let mut used = vec![false; boxes.len()];
        for t in open.iter_mut() {
            if fi - t.last > max_gap {
                continue;
            }
            // Nearest box within reach.
            let tc = t.last_box.center();
            let mut best: Option<(usize, f64)> = None;
            for (bi, b) in boxes.iter().enumerate() {
                if used[bi] {
                    continue;
                }
                let c = b.center();
                let d = ((c[0] - tc[0]).powi(2) + (c[1] - tc[1]).powi(2)).sqrt();
                let size_ok = (b.width() - t.last_box.width()).abs() <= 12
                    && (b.height() - t.last_box.height()).abs() <= 12;
                if d <= 32.0 && size_ok && best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((bi, d));
                }
            }
            if let Some((bi, _)) = best {
                used[bi] = true;
                t.boxes.push((fi, boxes[bi]));
                t.last = fi;
                t.last_box = boxes[bi];
            }
        }
        for (bi, b) in boxes.iter().enumerate() {
            if !used[bi] {
                open.push(Open {
                    boxes: vec![(fi, *b)],
                    last: fi,
                    last_box: *b,
                });
            }
        }
        // Close stale tracks.
        open.retain(|t| {
            if fi - t.last > max_gap {
                closed.push(ActorTrack {
                    kind: ActorKind::Unknown,
                    boxes: t.boxes.clone(),
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
            sprite: None,
        });
    }
    // Drop unstable tracks, sort by length, keep top candidates, and classify.
    closed.retain(|t| t.boxes.len() >= 8 && t.median_size() != (0, 0));
    closed.sort_by_key(|t| std::cmp::Reverse(t.boxes.len()));
    closed.truncate(12);
    classify_tracks(&mut closed);
    closed
}

/// In any side-scroller, the player is the persistent actor across the gameplay
/// segment (most frames alive in human/playable dimensions), whereas enemies exist
/// only locally around their spawn points.
fn classify_tracks(tracks: &mut [ActorTrack]) {
    let mut best: Option<(usize, usize)> = None;
    for (i, t) in tracks.iter().enumerate() {
        let (w, h) = t.median_size();
        if !(8..=30).contains(&w) || !(10..=36).contains(&h) {
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
            let (w, h) = t.median_size();
            if w <= 22 && h <= 22 {
                t.kind = ActorKind::Goomba;
            } else if w <= 24 && h <= 32 {
                t.kind = ActorKind::Koopa;
            }
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
                        if nonbg[y * w + x] {
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
        const MAX_GAP: usize = 8;
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
            if bridge {
                for r in surface_row.iter_mut().take(look).skip(end) {
                    *r = step;
                }
                tx = look;
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
                                    if tx1 >= rx0 && tx0 <= rx1 {
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
                if tx1 >= c.x0 && tx0 <= c.x1 {
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
    let (hud_bottom, hud_top) = if is_static {
        let (hb, ht) = detect_static_borders(path, start, info.width as usize, info.height as usize)?;
        (hb, opts.hud_top.unwrap_or(ht))
    } else {
        let ht = match opts.hud_top {
            Some(t) => t,
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

    // Pass 1: scroll + plain atlas.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut est = ScrollEstimator::new(info.width as usize, hud_top);
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
        }
        frame_idx += 1;
    }
    let n_frames = frame_idx;
    if n_frames == 0 {
        return Err(io::Error::other("no frames decoded in enfold range"));
    }
    let plain_atlas = acc.to_atlas();

    // Pass 2: HUD mode + residual boxes vs the plain atlas.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut boxes_per_frame: Vec<Vec<Rect>> = Vec::with_capacity(n_frames);
    let mut refined: Vec<[i32; 2]> = Vec::with_capacity(n_frames);
    let mut hud_acc: HashMap<(i32, i32), Bucket> = HashMap::new();
    let mut idx = 0usize;
    let actor_min_area = if is_static { 40 } else { ACTOR_MIN_AREA };
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
        let mut boxes = residual_boxes_range(&plain_atlas, &frame, off, hud_bottom, hud_top, 42, 40);
        // Sprite-scale prior: actors are at most two tiles across. Larger
        // residuals are parallax layers (clouds), decorations or seams.
        boxes.retain(|b| {
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

    // HUD image (mode over the segment).
    let hud_w = info.width as usize;
    let hud_h = info.height as usize - hud_top;
    let mut hud = RgbImage8::new(hud_w, hud_h);
    for (&(x, y), b) in &hud_acc {
        if let Some((c, _)) = b.mode() {
            let iy = y - hud_top as i32;
            if x >= 0 && iy >= 0 && (x as usize) < hud_w && (iy as usize) < hud_h {
                hud.set_pixel(x as usize, iy as usize, c);
            }
        }
    }

    // Tracks.
    let mut tracks = track_actors(&boxes_per_frame, 12);

    // Pass 3: masked atlas + sprites.
    let mut reader = video_io::FrameReader::open_range(path, Some(start), Some(duration))?;
    let mut acc2 = AtlasAccumulator::new(hud_top);
    struct SpriteAcc {
        w: usize,
        h: usize,
        buckets: Vec<Bucket>,
        alpha: Vec<u32>,
        total: u32,
    }
    let mut sprite_accs: Vec<Option<SpriteAcc>> = tracks
        .iter()
        .map(|t| {
            let (w, h) = t.median_size();
            if w <= 0 || h <= 0 || w > 48 || h > 48 {
                None
            } else {
                Some(SpriteAcc {
                    w: w as usize,
                    h: h as usize,
                    buckets: vec![Bucket::default(); (w * h) as usize],
                    alpha: vec![0; (w * h) as usize],
                    total: 0,
                })
            }
        })
        .collect();
    // Frame index -> (track index, box) index.
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
        let masked: Vec<Rect> = boxes_per_frame[idx]
            .iter()
            .map(|b| b.expand(2)) // 2px margin ensures no actor edge bleeds into the mode atlas.
            .collect();
        acc2.add_frame(&frame, off, &masked);
        for (ti, b) in &track_at_frame[idx] {
            let Some(sa) = sprite_accs[*ti].as_mut() else {
                continue;
            };
            sa.total += 1;
            // Map the (possibly drifting) box into the median sprite canvas.
            let fw = frame.width as usize;
            for sy in 0..sa.h {
                let fy = b.y0 + (sy as i32 * b.height() / sa.h as i32);
                if fy < 0 || fy >= frame.height as i32 {
                    continue;
                }
                for sx in 0..sa.w {
                    let fx = b.x0 + (sx as i32 * b.width() / sa.w as i32);
                    if fx < 0 || fx >= frame.width as i32 {
                        continue;
                    }
                    let i = (fy as usize * fw + fx as usize) * 3;
                    let c = [frame.rgb[i], frame.rgb[i + 1], frame.rgb[i + 2]];
                    let si = sy * sa.w + sx;
                    sa.buckets[si].add(c);
                    // Alpha: differs from the plain atlas → actor pixel.
                    let wx = fx + off[0];
                    let wy = fy + off[1];
                    if let Some(a) = plain_atlas.pixel(wx, wy) {
                        let d = (c[0] as i32 - a[0] as i32)
                            .abs()
                            .max((c[1] as i32 - a[1] as i32).abs())
                            .max((c[2] as i32 - a[2] as i32).abs());
                        if d > 42 {
                            sa.alpha[si] += 1;
                        }
                    }
                }
            }
        }
        idx += 1;
    }
    let atlas = acc2.to_atlas_with_fallback(&plain_atlas);

    // Resolve sprites.
    for (ti, t) in tracks.iter_mut().enumerate() {
        let Some(sa) = sprite_accs[ti].as_ref() else {
            continue;
        };
        if sa.total == 0 {
            continue;
        }
        let mut rgba = vec![0u8; sa.w * sa.h * 4];
        for i in 0..sa.w * sa.h {
            let (c, _) = sa.buckets[i].mode().unwrap_or(([0, 0, 0], 0));
            let a = (sa.alpha[i] * 255 / sa.total.max(1)).min(255) as u8;
            rgba[i * 4] = c[0];
            rgba[i * 4 + 1] = c[1];
            rgba[i * 4 + 2] = c[2];
            rgba[i * 4 + 3] = a;
        }
        t.sprite = Some(Sprite2D {
            width: sa.w,
            height: sa.h,
            rgba,
        });
    }

    let tile_size = if is_static { 8 } else { 16 };
    let mut tile_opts = TileOptions::default();
    tile_opts.tile_size = tile_size;
    tile_opts.is_static = is_static;
    tile_opts.hud_bottom = hud_bottom;
    tile_opts.hud_top = hud_top;
    let tiles = extract_2d_tiles_with(&atlas, &tile_opts);

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
}
