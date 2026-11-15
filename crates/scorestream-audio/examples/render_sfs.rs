//! Run a genuine scorestream-app SFS compile through the native PCM adapter.
//! This example is not a desktop command or MusicXML importer.
use scorestream_audio::{FluidSynthBackend, RenderOptions, RouteAssignment, default_soundfont};
use std::{env, fs, io::Read, path::Path, sync::atomic::AtomicBool};

fn read_bounded(path: &str, limit: u64) -> Result<String, Box<dyn std::error::Error>> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err("Input exceeds example's explicit size bound".into());
    }
    Ok(text)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 && args.len() != 5 {
        return Err("Usage: render_sfs score.sfs routes.json new.wav [existing.sf2]".into());
    }
    let source = read_bounded(&args[1], 4 * 1024 * 1024)?;
    let plan = scorestream_app::playback::plan_source(&source)?;
    let routes: Vec<RouteAssignment> = serde_json::from_str(&read_bounded(&args[2], 1024 * 1024)?)?;
    let bank = if let Some(path) = args.get(4) {
        Path::new(path).to_path_buf()
    } else {
        default_soundfont()?
    };
    let backend = FluidSynthBackend::open_installed()?;
    let report = backend.render_wav(
        &plan,
        &routes,
        &bank,
        &RenderOptions::default(),
        &AtomicBool::new(false),
        Path::new(&args[3]),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
