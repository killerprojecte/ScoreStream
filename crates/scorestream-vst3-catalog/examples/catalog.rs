//! Metadata only. Roots must be explicitly supplied; no automatic system scan.
use scorestream_vst3_catalog::{ScanLimits, scan_roots};
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let roots: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        return Err("Usage: catalog /explicit/approved/root [...] (metadata only)".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&scan_roots(&roots, &ScanLimits::default())?)?
    );
    Ok(())
}
