//! Read-only metadata precursor, not a scanner that executes plug-ins or an audio
//! host. Every advertised identity/capability remains an unverified vendor claim.
#![forbid(unsafe_code)]

mod binary;
mod manifest;
mod roots;
mod scan;

pub use binary::{BinaryFormat, HeaderInfo, inspect_header};
pub use manifest::{ClassId, ClassInfo, Compatibility, ModuleInfo, parse_moduleinfo};
pub use roots::{Platform, RootContext, SuggestedRoot, suggested_roots};
pub use scan::{
    BinaryCandidate, Candidate, Diagnostic, DuplicateClass, ScanLimits, ScanReport, scan_roots,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fmt, io};

pub type Result<T> = std::result::Result<T, CatalogError>;
#[derive(Debug)]
pub enum CatalogError {
    Invalid(String),
    Limit(String),
    Io(io::Error),
}
impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "Invalid metadata: {message}"),
            Self::Limit(message) => write!(f, "Discovery limit: {message}"),
            Self::Io(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CatalogError {}
impl From<io::Error> for CatalogError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Header labels only. CPU type does not prove ABI/OS compatibility or loadability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    X86,
    X86_64,
    Arm,
    Arm64,
    Arm64Ec,
    Arm64X,
    Unknown(u32),
}
