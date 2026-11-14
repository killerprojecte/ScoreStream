//! Headless plan inspection; never opens an audio device or modifies the score.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: inspect_playback <score.sfs>")?;
    let source = std::fs::read_to_string(path)?;
    let plan = scorestream_app::playback::plan_source(&source)?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}
