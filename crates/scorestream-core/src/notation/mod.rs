//! Shared semantic boundary: wire AST -> typed, resolved notation -> intervals.
//! Import adapters may construct this model; renderers and players must not parse
//! foreign markup or independently reinterpret covered wire attributes.
mod fermata;
mod intervals;
mod lower;
mod model;
mod perform;
pub use fermata::*;
pub use intervals::*;
pub use lower::lower_document;
pub use model::*;

pub use perform::*;
