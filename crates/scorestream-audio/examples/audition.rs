//! Experimental Linux local harness, deliberately not a desktop command.
#[cfg(target_os = "linux")]
#[path = "audition/support.rs"]
mod support;
#[cfg(target_os = "linux")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let usage = "Usage: audition review-practice source.sfs written_notes_without_wedge_dashes_ornaments_slur_v1 | audition trace-route source.sfs | audition discover-joint source.sfs simple_staccato_accent_v1 | audition preflight source.sfs policy.json | audition render source.sfs policy.json existing.sf2 new.wav";
    let value = match args.get(1).map(String::as_str) {
        Some("review-practice")
            if args.len() == 4
                && args[3] == "written_notes_without_wedge_dashes_ornaments_slur_v1" =>
        {
            support::review_practice(
                &std::fs::read_to_string(&args[2])?,
                support::PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
            )?
        }
        Some("trace-route") if args.len() == 3 => {
            support::trace(&std::fs::read_to_string(&args[2])?)?
        }
        Some("discover-joint") if args.len() == 4 && args[3] == "simple_staccato_accent_v1" => {
            support::discover(&std::fs::read_to_string(&args[2])?)?
        }
        Some(command @ ("preflight" | "render"))
            if (command == "preflight" && args.len() == 4)
                || (command == "render" && args.len() == 6) =>
        {
            let source = std::fs::read_to_string(&args[2])?;
            let config = serde_json::from_str(&std::fs::read_to_string(&args[3])?)?;
            let audition = support::Audition::compile(&source, config)?;
            if command == "preflight" {
                audition.preflight()?
            } else {
                audition.render(
                    std::path::Path::new(&args[4]),
                    std::path::Path::new(&args[5]),
                )?
            }
        }
        _ => return Err(usage.into()),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    let result = run();
    #[cfg(not(target_os = "linux"))]
    let result: Result<(), Box<dyn std::error::Error>> =
        Err("The installed FluidSynth audition harness currently requires Linux".into());
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("audition: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
