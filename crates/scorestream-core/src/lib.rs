//! Exact musical domain values, the normalized 1.1 AST, and semantic validation.
//! No UI, storage, network, or playback policy belongs in this crate.
#![forbid(unsafe_code)]
mod model;
mod rational;
mod registry;
mod validate;
pub use model::*;
pub use rational::*;
pub use registry::{attribute_type, is_text_attribute};
pub use validate::validate;
