//! Metadata-only source route discovery. Never constructs a playback plan.
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 && !(args.len() == 3 && args[2] == "continue-to-movement-end") {
        return Err("Usage: trace_route source.sfs [continue-to-movement-end]".into());
    }
    let source = std::fs::read_to_string(&args[1])?;
    let doc = scorestream_codec::parse(&source).map_err(|e| format!("{e:?}"))?;
    let policy = if args.len() == 3 {
        scorestream_core::repeat_traversal::UnclosedFinalEndingPolicy::ContinueToMovementEnd
    } else {
        scorestream_core::repeat_traversal::UnclosedFinalEndingPolicy::Reject
    };
    let trace = scorestream_core::notation::trace_document_route_with_final_ending(&doc, policy)
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"source_sha256":format!("{:x}",Sha256::digest(source.as_bytes())),"trace":trace})
        )?
    );
    Ok(())
}
