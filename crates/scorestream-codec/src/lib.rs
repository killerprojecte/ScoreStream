//! Strict, versioned text codec for ScoreStream's core measured profile.
#![forbid(unsafe_code)]
mod lexer;
mod metadata;
mod parser;
mod writer;
pub use parser::{parse, parse_with_context, parse_with_prevalidation};
pub use scorestream_core::*;
pub use writer::canonical;
