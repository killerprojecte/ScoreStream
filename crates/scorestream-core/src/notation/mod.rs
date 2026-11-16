//! Shared semantic boundary: wire AST -> typed, resolved notation -> intervals.
//! Import adapters may construct this model; renderers and players must not parse
//! foreign markup or independently reinterpret covered wire attributes.
mod articulations;
pub use articulations::*;
mod combined;
mod fermata;
mod grace;
mod intervals;
mod lower;
mod model;
mod perform;
mod passive;
mod trace;
pub use trace::*;
pub use combined::*;
pub use fermata::*;
pub use grace::*;
pub use intervals::*;
pub use lower::lower_document;
pub use model::*;

pub use perform::*;

mod authored_dynamic_schedule;
pub use authored_dynamic_schedule::*;
