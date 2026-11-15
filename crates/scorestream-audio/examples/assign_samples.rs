//! Explicit local-file example. No native loading in preflight-only mode.
use scorestream_audio::{
    FluidSynthBackend, RenderOptions,
    assignments::{LocalSampleProfile, PartSampleSelection, preflight_source},
};
use std::{error::Error, fs::File, io::Read, path::Path, sync::atomic::AtomicBool};

fn bounded_text(path: &str, limit: u64) -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err("Input file exceeds example byte budget".into());
    }
    Ok(text)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err("Usage: assign_samples source.sfs exact-source-sha256 profile.json selections.json [new.wav]".into());
    }
    let source = bounded_text(&args[0], 4 * 1024 * 1024)?;
    let profile: LocalSampleProfile =
        serde_json::from_str(&bounded_text(&args[2], 4 * 1024 * 1024)?)?;
    let selections: Vec<PartSampleSelection> =
        serde_json::from_str(&bounded_text(&args[3], 512 * 1024)?)?;
    let preflight = preflight_source(
        &source,
        &args[1],
        &profile,
        &selections,
        &RenderOptions::default(),
    )?;
    if args.len() == 4 || !preflight.report().contract_renderable {
        println!("{}", serde_json::to_string_pretty(preflight.report())?);
        if !preflight.report().contract_renderable {
            return Err(
                "Preflight blocked; source unchanged, no bank loaded, no audio written".into(),
            );
        }
        return Ok(());
    }
    let qualified = preflight.into_qualified()?;
    let backend = FluidSynthBackend::open_installed()?;
    let report = qualified.render_wav(&backend, &AtomicBool::new(false), Path::new(&args[4]))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
