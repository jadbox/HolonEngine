//! `ffmpeg`-backed video IO: raw RGB24 frame reader, H.264 writer, PNG/PPM capture.
//!
//! No image codec is linked in for video; `ffmpeg`/`ffprobe` are expected on
//! `PATH`. Frames are plain `RGB24` buffers in scanline order.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Stream metadata from `ffprobe`.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// Frames per second (rational converted to `f64`).
    pub fps: f64,
    /// Frame count from the container, `0` when unknown.
    pub frames: u64,
}

impl VideoInfo {
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 3
    }
}

fn run_capture(cmd: &mut Command) -> io::Result<String> {
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "command failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Probe stream `width,height,fps,frames`.
pub fn probe(path: &Path) -> io::Result<VideoInfo> {
    let out = run_capture(
        Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,r_frame_rate,nb_frames",
                "-of",
                "csv=p=0",
            ])
            .arg(path),
    )?;
    let mut it = out.split(',');
    let width: u32 = it
        .next()
        .ok_or_else(|| io::Error::other("ffprobe: missing width"))?
        .trim()
        .parse()
        .map_err(|_| io::Error::other("ffprobe: bad width"))?;
    let height: u32 = it
        .next()
        .ok_or_else(|| io::Error::other("ffprobe: missing height"))?
        .trim()
        .parse()
        .map_err(|_| io::Error::other("ffprobe: bad height"))?;
    let rate = it.next().unwrap_or("30/1").trim();
    let fps = match rate.split_once('/') {
        Some((n, d)) => n.parse::<f64>().unwrap_or(30.0) / d.parse::<f64>().unwrap_or(1.0),
        None => rate.parse().unwrap_or(30.0),
    };
    let frames: u64 = it.next().unwrap_or("0").trim().parse().unwrap_or(0);
    Ok(VideoInfo {
        width,
        height,
        fps,
        frames,
    })
}

/// One decoded frame.
#[derive(Clone, Debug, PartialEq)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

impl RawFrame {
    pub fn new(width: u32, height: u32, rgb: Vec<u8>) -> Self {
        debug_assert_eq!(rgb.len(), width as usize * height as usize * 3);
        Self { width, height, rgb }
    }

    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }
}

/// Streaming raw RGB24 reader over a span of an ffmpeg-decodable video.
pub struct FrameReader {
    child: Child,
    stdout: ChildStdout,
    width: u32,
    height: u32,
    buf: Vec<u8>,
    done: bool,
}

impl FrameReader {
    /// Open the whole stream.
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_range(path, None, None)
    }

    /// Open `path`, seeking to `start` seconds and reading at most `duration`.
    pub fn open_range(path: &Path, start: Option<f64>, duration: Option<f64>) -> io::Result<Self> {
        let info = probe(path)?;
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-v").arg("error");
        if let Some(s) = start {
            cmd.arg("-ss").arg(format!("{s:.6}"));
        }
        cmd.arg("-i").arg(path);
        if let Some(d) = duration {
            cmd.arg("-t").arg(format!("{d:.6}"));
        }
        cmd.args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .stdin(Stdio::null());
        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("ffmpeg: no stdout"))?;
        Ok(Self {
            child,
            stdout,
            width: info.width,
            height: info.height,
            buf: vec![0u8; info.frame_bytes()],
            done: false,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Read the next frame, `None` at end of stream.
    pub fn next_frame(&mut self) -> io::Result<Option<RawFrame>> {
        if self.done {
            return Ok(None);
        }
        match read_exact_or_eof(&mut self.stdout, &mut self.buf)? {
            false => {
                self.done = true;
                Ok(None)
            }
            true => Ok(Some(RawFrame {
                width: self.width,
                height: self.height,
                rgb: self.buf.clone(),
            })),
        }
    }

    /// Sweep the whole span, yielding frames to `f`.
    pub fn for_each<F>(&mut self, mut f: F) -> io::Result<usize>
    where
        F: FnMut(usize, &RawFrame),
    {
        let mut n = 0;
        while let Some(frame) = self.next_frame()? {
            f(n, &frame);
            n += 1;
        }
        Ok(n)
    }
}

impl Drop for FrameReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_exact_or_eof(r: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => {
                if filled == 0 {
                    return Ok(false);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated frame",
                ));
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

/// H.264 MP4 writer fed with raw RGB24 frames.
pub struct VideoWriter {
    child: Child,
    stdin: Option<ChildStdin>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    written: u64,
}

impl VideoWriter {
    pub fn create(path: &Path, width: u32, height: u32, fps: f64) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut child = Command::new("ffmpeg")
            .arg("-y")
            .arg("-v")
            .arg("error")
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24"])
            .arg("-s")
            .arg(format!("{width}x{height}"))
            .arg("-r")
            .arg(format!("{fps}"))
            .arg("-i")
            .arg("-")
            .arg("-an")
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("ffmpeg: no stdin"))?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            width,
            height,
            fps,
            written: 0,
        })
    }

    pub fn write_frame(&mut self, rgb: &[u8]) -> io::Result<()> {
        let expect = self.width as usize * self.height as usize * 3;
        if rgb.len() != expect {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("frame has {} bytes, expected {expect}", rgb.len()),
            ));
        }
        self.stdin
            .as_mut()
            .ok_or_else(|| io::Error::other("writer already finished"))?
            .write_all(rgb)?;
        self.written += 1;
        Ok(())
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    /// Close stdin and wait for the encoder.
    pub fn finish(&mut self) -> io::Result<()> {
        drop(self.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            return Err(io::Error::other("ffmpeg encoder failed"));
        }
        Ok(())
    }
}

impl Drop for VideoWriter {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Write a binary PPM (`P6`).
pub fn write_ppm(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut f = io::BufWriter::new(std::fs::File::create(path)?);
    write!(f, "P6\n{width} {height}\n255\n")?;
    f.write_all(rgb)?;
    f.flush()
}

/// Write an RGB PNG via the `image` crate (no ffmpeg round-trip).
pub fn write_png(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let img = image::RgbImage::from_raw(width, height, rgb.to_vec())
        .ok_or_else(|| io::Error::other("buffer size does not match dimensions"))?;
    img.save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| io::Error::other(e.to_string()))
}

/// Run one `ffmpeg -ss t -frames:v 1` extraction to PNG.
pub fn extract_frame_png(video: &Path, t: f64, out: &Path) -> io::Result<()> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-ss")
        .arg(format!("{t:.6}"))
        .arg("-i")
        .arg(video)
        .args(["-frames:v", "1"])
        .arg(out)
        .status()?;
    if !status.success() {
        return Err(io::Error::other("ffmpeg frame extraction failed"));
    }
    Ok(())
}

/// Default output directory for diagnostics.
pub fn scratch_dir() -> PathBuf {
    let base = std::env::var_os("TMPDIR").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(base).join("opencode")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_video() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../example_input2d/smb3_w1_1_256x240.mp4")
    }

    #[test]
    fn probe_reports_native_geometry() {
        let info = probe(&sample_video()).unwrap();
        assert_eq!(info.width, 256);
        assert_eq!(info.height, 240);
        assert!((info.fps - 30.0).abs() < 0.01, "fps = {}", info.fps);
        assert!(info.frames > 2000, "frames = {}", info.frames);
    }

    #[test]
    fn reads_frames_from_a_range() {
        let mut reader =
            FrameReader::open_range(&sample_video(), Some(20.0), Some(1.0 / 30.0 * 3.0)).unwrap();
        let mut sizes = Vec::new();
        reader
            .for_each(|_, f| {
                sizes.push(f.rgb.len());
            })
            .unwrap();
        assert!((3..=3).contains(&sizes.len()), "got {} frames", sizes.len());
        assert!(sizes.iter().all(|s| *s == 256 * 240 * 3));
    }

    #[test]
    fn ppm_and_png_round_trip() {
        let dir = std::env::temp_dir().join("cc_video_io_test");
        std::fs::create_dir_all(&dir).unwrap();
        let rgb: Vec<u8> = (0..16 * 8 * 3).map(|i| (i * 7 % 251) as u8).collect();
        let ppm = dir.join("t.ppm");
        write_ppm(&ppm, 16, 8, &rgb).unwrap();
        let back = std::fs::read(&ppm).unwrap();
        assert!(back.starts_with(b"P6\n16 8\n255\n"));
        assert_eq!(&back[12..], &rgb[..]);
        let png = dir.join("t.png");
        write_png(&png, 16, 8, &rgb).unwrap();
        let img = image::open(&png).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (16, 8));
        assert_eq!(img.into_raw(), rgb);
    }
}
