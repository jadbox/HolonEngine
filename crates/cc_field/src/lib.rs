//! Continuous page lattice: quantum-potential sampling and split-step CFT.

pub mod enfold2d;
pub mod model2d;
pub mod potential;
pub mod video_io;
pub mod wave;

pub use enfold2d::{
    ActorKind, ActorTrack, AtlasAccumulator, Enfolded2D, EnfoldOptions, Rect, RgbImage8,
    Sprite2D, TileExtraction, TileKind, WorldAtlas, enfold, extract_2d_tiles,
    estimate_2d_scroll, partition_hud_and_viewport,
};
pub use potential::QField;
pub use rustfft::num_complex::Complex;
pub use wave::{WaveSolver, add_drive, blur3, disk_mask, disk_potential, mass, peak, sponge};
