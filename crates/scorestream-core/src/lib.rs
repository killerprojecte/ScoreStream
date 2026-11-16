//! Exact musical domain values, the normalized 1.1 AST, and semantic validation.
//! Pure, explicitly selected performance semantics may live here; UI, storage,
//! network, audio gates, devices, and playback runtime behavior do not.
#![forbid(unsafe_code)]
pub mod articulation_extension;
pub mod articulation_model;
pub mod beam_extension;
pub mod fermata_extension;
pub mod fermata_timing;
pub mod grace_extension;
pub mod instrument_extension;
pub mod instrument_metadata;
pub mod instruments;
mod metadata;
mod model;
pub mod navigation;
pub mod navigation_extension;
pub mod notation;
pub mod notation_extension;
pub mod notation_order;
mod rational;
mod registry;
pub mod repeat_traversal;
pub mod score;
pub mod slur_extension;
pub mod tempo_clock;
mod validate;
pub use metadata::*;
pub use model::*;
pub use rational::*;
pub use registry::{attribute_type, is_text_attribute};
pub use validate::validate;
