use std::fs;
use std::path::Path;

/// High-level game invariants and rules parsed from an optional `rules.md`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rules2D {
    pub regime: Option<String>,
    pub gravity: Option<f32>,
    /// Bottom HUD band start (y coordinate, e.g. 192 in SMB3, 288 in Pac-Man).
    pub hud_top: Option<usize>,
    /// Top HUD band end (y coordinate, e.g. 24 in Pac-Man).
    pub hud_bottom: Option<usize>,
    pub viewport_y0: Option<usize>,
    pub viewport_y1: Option<usize>,
    pub tile_size: Option<usize>,
}

impl Rules2D {
    /// Try finding a `rules.md` file adjacent to the video file or in its parent.
    pub fn try_find_for_video(video_path: &Path) -> Option<Self> {
        let dir = video_path.parent()?;
        let candidate1 = dir.join("rules.md");
        if candidate1.exists() {
            return Self::from_file(&candidate1).ok();
        }
        let candidate2 = dir.parent()?.join("rules.md");
        if candidate2.exists() {
            return Self::from_file(&candidate2).ok();
        }
        None
    }

    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Ok(Self::from_str(&content))
    }

    pub fn from_str(s: &str) -> Self {
        let mut r = Self::default();
        for line in s.lines() {
            let l = line.trim();
            // Regime: Ortho2D-Static or Ortho2D-Scroll
            if l.contains("Regime") {
                if l.contains("Ortho2D-Static") {
                    r.regime = Some("Ortho2D-Static".to_string());
                } else if l.contains("Ortho2D") {
                    r.regime = Some("Ortho2D-Scroll".to_string());
                }
            }
            // Gravity: None (g = 0)
            if l.contains("Gravity") {
                if l.contains("None") || l.contains("g = 0") || l.contains("g = 0.0") {
                    r.gravity = Some(0.0);
                }
            }
            // Lattice: 8 x 8 or 16 x 16
            if l.contains("Lattice") || l.contains("tile lattice") {
                if l.contains('8')
                    && (l.contains("8 x 8") || l.contains("8x8") || l.contains("8 × 8"))
                {
                    r.tile_size = Some(8);
                } else if l.contains("16")
                    && (l.contains("16 x 16") || l.contains("16x16") || l.contains("16 × 16"))
                {
                    r.tile_size = Some(16);
                }
            }
            // Top HUD bar: y < 24
            if l.contains("Top HUD") || (l.contains("top") && l.contains("HUD")) {
                if let Some(pos) = l.find("y < ") {
                    let num_str: String = l[pos + 4..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(val) = num_str.parse() {
                        r.hud_bottom = Some(val);
                    }
                }
            }
            // Bottom HUD bar: y > 288 or y >= 192
            if l.contains("Bottom HUD") || l.contains("bottom HUD") || l.contains("HUD panel") {
                for pat in &["y > ", "y >= ", "y \\ge ", "y >="] {
                    if let Some(pos) = l.find(pat) {
                        let num_str: String = l[pos + pat.len()..]
                            .chars()
                            .take_while(|c| c.is_ascii_digit())
                            .collect();
                        if let Ok(val) = num_str.parse() {
                            r.hud_top = Some(val);
                            break;
                        }
                    }
                }
            }
            // Standalone y >= 192 or y \ge 192
            if r.hud_top.is_none() {
                for pat in &["y >= ", "y \\ge ", "y >="] {
                    if let Some(pos) = l.find(pat) {
                        let num_str: String = l[pos + pat.len()..]
                            .chars()
                            .take_while(|c| c.is_ascii_digit())
                            .collect();
                        if let Ok(val) = num_str.parse() {
                            r.hud_top = Some(val);
                            break;
                        }
                    }
                }
            }
            // Viewport: y \in [24, 288]
            if l.contains("Viewport") && l.contains('[') {
                if let Some(open) = l.find('[') {
                    if let Some(close) = l[open..].find(']') {
                        let inner = &l[open + 1..open + close];
                        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
                        if parts.len() == 2 {
                            if let (Ok(y0), Ok(y1)) = (parts[0].parse(), parts[1].parse()) {
                                r.viewport_y0 = Some(y0);
                                r.viewport_y1 = Some(y1);
                            }
                        }
                    }
                }
            }
        }
        r
    }

    pub fn is_static(&self) -> bool {
        self.regime.as_deref() == Some("Ortho2D-Static")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_smb3_rules() {
        let path = Path::new("example_input2d/rules.md");
        if path.exists() {
            let r = Rules2D::from_file(path).unwrap();
            assert_eq!(r.hud_top, Some(192));
            assert_eq!(r.tile_size, Some(16));
            assert_eq!(r.regime, Some("Ortho2D-Scroll".into()));
        }
    }

    #[test]
    fn parses_pacman_rules() {
        let path = Path::new("example_input2d_pacman/rules.md");
        if path.exists() {
            let r = Rules2D::from_file(path).unwrap();
            assert_eq!(r.regime, Some("Ortho2D-Static".into()));
            assert_eq!(r.gravity, Some(0.0));
            assert_eq!(r.hud_bottom, Some(24));
            assert_eq!(r.hud_top, Some(288));
            assert_eq!(r.viewport_y0, Some(24));
            assert_eq!(r.viewport_y1, Some(288));
            assert_eq!(r.tile_size, Some(8));
        }
    }
}
