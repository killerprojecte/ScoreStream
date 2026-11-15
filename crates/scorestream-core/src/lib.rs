//! Exact musical domain values, the normalized 1.1 AST, and semantic validation.
//! No UI, storage, network, or playback policy belongs in this crate.
#![forbid(unsafe_code)]
pub mod instruments;
mod metadata;
mod model;
pub mod navigation_extension;
pub mod notation;
mod rational;
mod registry;
pub mod repeat_traversal;
pub mod score;
mod validate;
pub use metadata::*;
pub use model::*;
pub use rational::*;
pub use registry::{attribute_type, is_text_attribute};
pub use validate::validate;
