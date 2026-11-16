//! Explicit local experiment; computes/discards PCM and writes only JSON stdout.
#[path = "audition/support.rs"]
mod support;
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().collect();
    if !(args.len()==5 || (args.len()==6 && args[5]=="until_native_idle")) || args[4]!="hold_last_gain_at_noteoff_v1" {return Err("Usage: dry_admission_probe SOURCE POLICY BANK hold_last_gain_at_noteoff_v1; retains selected joint practice policies, never writes PCM".into());}
    let source=std::fs::read_to_string(&args[1])?;
    let config=serde_json::from_str(&std::fs::read_to_string(&args[2])?)?;
    let audition=support::Audition::compile_practice(&source,
        scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        config,support::PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        scorestream_audio::midi_mixer::LocalMidiPolicy::InitialGmCc7CenteredV1)?;
    println!("{}",serde_json::to_string_pretty(&if args.len()==6 { audition.dry_admission_until_idle(&source,std::path::Path::new(&args[3]))? } else { audition.dry_admission_probe(&source,std::path::Path::new(&args[3]))? })?);
    Ok(())
}
