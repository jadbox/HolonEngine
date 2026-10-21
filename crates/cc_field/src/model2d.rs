//! `.cc` 2D model container.
//!
//! Layout (little-endian):
//! ```text
//! "CC2D" | u32 version | u32 manifest_len | manifest (JSON) | u32 atlas_png_len
//! | atlas (PNG)     | sprite_0 (PNG) | ... | sprite_n (PNG)
//! ```
//! The manifest carries every non-image field (tile classes, scroll, rects,
//! sprite table) so loading needs no game-specific knowledge.

use std::io::{self, Cursor, Read, Write};
use std::path::Path;

use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::enfold2d::{Enfolded2D, Rect, RgbImage8, TileKind};

/// 2D container magic.
pub const MAGIC: &[u8; 4] = b"CC2D";
/// Container version.
pub const VERSION: u32 = 1;

/// One canonical actor sprite in the manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteRecord {
    pub kind: String,
    pub width: u32,
    pub height: u32,
    /// Byte length of the following PNG payload.
    pub bytes: u32,
}

/// A tracked actor, without pixels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackRecord {
    pub kind: String,
    pub frames: u32,
    pub size: [i32; 2],
    /// Index into [`ModelManifest::sprites`].
    pub sprite: Option<u32>,
}

/// Everything in a model that is not pixels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelManifest {
    pub version: u32,
    pub source: String,
    pub start: f64,
    pub duration: f64,
    pub fps: f64,
    pub hud_top: u32,
    /// Playfield size in pixels.
    pub viewport: [u32; 2],
    /// Atlas size in pixels.
    pub atlas: [u32; 2],
    pub origin: [i32; 2],
    pub tile: u32,
    pub origin_y: i32,
    pub ground_top: i32,
    pub background: [u8; 3],
    pub cols: u32,
    pub rows: u32,
    /// `TileKind` bytes, row-major.
    pub kinds: Vec<u8>,
    /// Camera offset per frame, starting at `[0, 0]`.
    pub scroll: Vec<[i32; 2]>,
    /// One-way top surfaces, world pixels: `[x0, y0, x1, y1]`.
    pub platforms: Vec<[i32; 4]>,
    /// Solid tile runs, world pixels: `[x0, y0, x1, y1]`.
    pub solids: Vec<[i32; 4]>,
    pub sprites: Vec<SpriteRecord>,
    pub tracks: Vec<TrackRecord>,
}

/// A decoded model: manifest plus images.
#[derive(Clone, Debug)]
pub struct Model2D {
    pub manifest: ModelManifest,
    pub atlas: RgbImage8,
    /// Static HUD strip (source rows `hud_top..`), if any.
    pub hud: Option<RgbImage8>,
    /// RGBA sprite payloads, parallel to `manifest.sprites`.
    pub sprites: Vec<Vec<u8>>,
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let img = RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "sprite buffer size"))?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(buf.into_inner())
}

fn encode_rgb_as_png(img: &RgbImage8) -> io::Result<Vec<u8>> {
    let rgba: Vec<u8> = img
        .rgb
        .chunks_exact(3)
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect();
    encode_png(img.width as u32, img.height as u32, &rgba)
}

fn decode_rgb_png(bytes: &[u8]) -> io::Result<RgbImage8> {
    let (w, h, rgba) = decode_png(bytes)?;
    let mut img = RgbImage8::new(w as usize, h as usize);
    for (i, px) in rgba.chunks_exact(4).enumerate() {
        img.rgb[i * 3] = px[0];
        img.rgb[i * 3 + 1] = px[1];
        img.rgb[i * 3 + 2] = px[2];
    }
    Ok(img)
}

fn decode_png(bytes: &[u8]) -> io::Result<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|e| io::Error::other(e.to_string()))?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

fn rect4(r: &Rect) -> [i32; 4] {
    [r.x0, r.y0, r.x1, r.y1]
}

impl Model2D {
    /// Freeze an enfolded level into a model (sprite PNGs are encoded here).
    pub fn from_enfolded(level: &Enfolded2D, source: &str) -> io::Result<Self> {
        let mut sprites = Vec::new();
        let mut records = Vec::new();
        let mut tracks = Vec::new();
        for t in &level.tracks {
            let (w, h) = t.median_size();
            let mut sprite = None;
            if let Some(s) = &t.sprite {
                if s.width > 0 && s.height > 0 {
                    let png = encode_png(s.width as u32, s.height as u32, &s.rgba)?;
                    records.push(SpriteRecord {
                        kind: t.kind.as_str().to_string(),
                        width: s.width as u32,
                        height: s.height as u32,
                        bytes: png.len() as u32,
                    });
                    sprite = Some(sprites.len() as u32);
                    sprites.push(png);
                }
            }
            tracks.push(TrackRecord {
                kind: t.kind.as_str().to_string(),
                frames: t.boxes.len() as u32,
                size: [w, h],
                sprite,
            });
        }
        let manifest = ModelManifest {
            version: VERSION,
            source: source.to_string(),
            start: level.start,
            duration: level.duration,
            fps: level.fps,
            hud_top: level.hud_top as u32,
            viewport: [level.info.width, level.info.height],
            atlas: [
                level.atlas.image.width as u32,
                level.atlas.image.height as u32,
            ],
            origin: level.atlas.origin,
            tile: crate::enfold2d::TILE as u32,
            origin_y: level.tiles.origin_y,
            ground_top: level.tiles.ground_top,
            background: level.tiles.background,
            cols: level.tiles.cols as u32,
            rows: level.tiles.rows as u32,
            kinds: level.tiles.kinds.iter().map(|k| k.as_u8()).collect(),
            scroll: level.scroll.clone(),
            platforms: level.tiles.platforms.iter().map(rect4).collect(),
            solids: level.tiles.solids.iter().map(rect4).collect(),
            sprites: records,
            tracks,
        };
        Ok(Model2D {
            manifest,
            atlas: level.atlas.image.clone(),
            hud: Some(level.hud.clone()),
            // PNG payloads kept encoded; decoded lazily by `sprite_rgba`.
            sprites,
        })
    }

    /// Serialize to the `.cc` byte layout.
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let manifest = serde_json::to_vec(&self.manifest).map_err(io::Error::other)?;
        let atlas_png = self.atlas_png()?;
        let mut out = Vec::with_capacity(manifest.len() + atlas_png.len() + 64);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        out.extend_from_slice(&manifest);
        out.extend_from_slice(&(atlas_png.len() as u32).to_le_bytes());
        out.extend_from_slice(&atlas_png);
        let hud_png = match &self.hud {
            Some(hud) => encode_rgb_as_png(hud)?,
            None => Vec::new(),
        };
        out.extend_from_slice(&(hud_png.len() as u32).to_le_bytes());
        out.extend_from_slice(&hud_png);
        for (rec, png) in self.manifest.sprites.iter().zip(&self.sprites) {
            if png.len() as u32 != rec.bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sprite length disagrees with manifest",
                ));
            }
            out.extend_from_slice(png);
        }
        Ok(out)
    }

    /// Parse a `.cc` payload.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(bytes);
        let mut magic = [0u8; 4];
        cur.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a CC2D model",
            ));
        }
        let version = read_u32(&mut cur)?;
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported CC2D version {version}"),
            ));
        }
        let manifest_len = read_u32(&mut cur)? as usize;
        let mut manifest_bytes = vec![0u8; manifest_len];
        cur.read_exact(&mut manifest_bytes)?;
        let manifest: ModelManifest =
            serde_json::from_slice(&manifest_bytes).map_err(io::Error::other)?;
        let atlas_len = read_u32(&mut cur)? as usize;
        let mut atlas_png = vec![0u8; atlas_len];
        cur.read_exact(&mut atlas_png)?;
        let (aw, ah, rgba) = decode_png(&atlas_png)?;
        let mut atlas = RgbImage8::new(aw as usize, ah as usize);
        for (i, px) in rgba.chunks_exact(4).enumerate() {
            atlas.rgb[i * 3] = px[0];
            atlas.rgb[i * 3 + 1] = px[1];
            atlas.rgb[i * 3 + 2] = px[2];
        }
        let hud_len = read_u32(&mut cur)? as usize;
        let hud = if hud_len == 0 {
            None
        } else {
            let mut png = vec![0u8; hud_len];
            cur.read_exact(&mut png)?;
            Some(decode_rgb_png(&png)?)
        };
        let mut sprites = Vec::new();
        for rec in &manifest.sprites {
            let mut png = vec![0u8; rec.bytes as usize];
            cur.read_exact(&mut png)?;
            sprites.push(png);
        }
        Ok(Model2D {
            manifest,
            atlas,
            hud,
            sprites,
        })
    }

    /// Write a `.cc` file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let bytes = self.to_bytes()?;
        let mut f = io::BufWriter::new(std::fs::File::create(path)?);
        f.write_all(&bytes)?;
        f.flush()
    }

    /// Read a `.cc` file.
    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// PNG bytes of the atlas (re-encoded on demand).
    pub fn atlas_png(&self) -> io::Result<Vec<u8>> {
        encode_rgb_as_png(&self.atlas)
    }

    /// PNG bytes of the HUD strip (re-encoded on demand).
    pub fn hud_png(&self) -> io::Result<Option<Vec<u8>>> {
        self.hud.as_ref().map(encode_rgb_as_png).transpose()
    }

    /// Decode sprite `i` to RGBA.
    pub fn sprite_rgba(&self, i: usize) -> io::Result<(u32, u32, Vec<u8>)> {
        let png = self
            .sprites
            .get(i)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "sprite index"))?;
        decode_png(png)
    }

    /// Class of a tile.
    pub fn kind(&self, tx: usize, ty: usize) -> TileKind {
        let (cols, rows) = (self.manifest.cols as usize, self.manifest.rows as usize);
        if tx < cols && ty < rows {
            TileKind::from_u8(self.manifest.kinds[ty * cols + tx])
        } else {
            TileKind::Empty
        }
    }

    /// Top edge (world y) of a tile row.
    pub fn row_y(&self, ty: usize) -> i32 {
        self.manifest.origin_y + ty as i32 * self.manifest.tile as i32
    }

    /// Ground surface per tile column, in world y.
    pub fn surface_y(&self, tx: usize) -> i32 {
        let cols = self.manifest.cols as usize;
        let rows = self.manifest.rows as usize;
        if cols == 0 {
            return self.manifest.ground_top;
        }
        let mut ty = rows;
        for candidate in 0..rows {
            if self.kind(tx, candidate) == TileKind::Solid {
                ty = candidate;
                break;
            }
        }
        self.row_y(ty)
    }
}

fn read_u32(cur: &mut Cursor<&[u8]>) -> io::Result<u32> {
    let mut b = [0u8; 4];
    cur.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Model2D {
        let mut atlas = RgbImage8::new(4, 4);
        for i in 0..16 {
            atlas.rgb[i * 3] = i as u8;
            atlas.rgb[i * 3 + 1] = (i * 2) as u8;
            atlas.rgb[i * 3 + 2] = (i * 3) as u8;
        }
        let sprite = encode_png(2, 2, &[255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 9, 9, 9, 255])
            .unwrap();
        Model2D {
            manifest: ModelManifest {
                version: VERSION,
                source: "test".into(),
                start: 1.0,
                duration: 2.0,
                fps: 30.0,
                hud_top: 3,
                viewport: [4, 4],
                atlas: [4, 4],
                origin: [0, 0],
                tile: 16,
                origin_y: 1,
                ground_top: 4,
                background: [1, 2, 3],
                cols: 1,
                rows: 1,
                kinds: vec![TileKind::Solid.as_u8()],
                scroll: vec![[0, 0], [1, 0]],
                platforms: vec![[0, 0, 3, 0]],
                solids: vec![[0, 4, 3, 7]],
                sprites: vec![SpriteRecord {
                    kind: "player".into(),
                    width: 2,
                    height: 2,
                    bytes: sprite.len() as u32,
                }],
                tracks: vec![TrackRecord {
                    kind: "player".into(),
                    frames: 2,
                    size: [2, 2],
                    sprite: Some(0),
                }],
            },
            atlas,
            hud: None,
            sprites: vec![sprite],
        }
    }

    #[test]
    fn roundtrip() {
        let m = sample();
        let bytes = m.to_bytes().unwrap();
        let back = Model2D::from_bytes(&bytes).unwrap();
        assert_eq!(back.manifest.kinds, m.manifest.kinds);
        assert_eq!(back.manifest.scroll, m.manifest.scroll);
        assert_eq!(back.atlas, m.atlas);
        let (w, h, rgba) = back.sprite_rgba(0).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba.len(), 16);
    }

    #[test]
    fn rejects_other_formats() {
        let err = Model2D::from_bytes(b"nope").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let mut bytes = sample().to_bytes().unwrap();
        bytes[0] = b'X';
        let err = Model2D::from_bytes(&bytes).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn surface_lookup() {
        let m = sample();
        assert_eq!(m.row_y(0), 1);
        assert_eq!(m.surface_y(0), 1);
        assert_eq!(m.kind(0, 0), TileKind::Solid);
        assert_eq!(m.kind(5, 5), TileKind::Empty);
    }
}
