//! Pure core discovery/timing probe. No audio bank is loaded and no PCM API exists here.
use scorestream_core::{
    fermata_timing::*, notation::*, repeat_traversal::UnclosedFinalEndingPolicy as Ending,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=5).contains(&args.len()) || (args.len() >= 4 && args[3] != "continue-to-movement-end") || (args.len()==5 && args[4]!="practice-principal-articulation") {
        return Err("probe_practice source.sfs selected.json [continue-to-movement-end [practice-principal-articulation]]".into());
    }
    let source = std::fs::read_to_string(&args[1])?;
    let selected: Value = serde_json::from_str(&std::fs::read_to_string(&args[2])?)?;
    let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
    if selected["source_sha256"] != hash {
        return Err("source hash mismatch".into());
    }
    let doc = scorestream_codec::parse(&source).map_err(|e| format!("{e:?}"))?;
    let routes: Vec<_> = selected["palette"]
        .as_array()
        .ok_or("palette missing")?
        .iter()
        .map(|v| {
            Ok(PracticeMidiSelection {
                part: v["part"].as_str().ok_or("part")?.into(),
                instrument: v["instrument"].as_str().ok_or("instrument")?.into(),
                bank: v["bank"]
                    .as_u64()
                    .and_then(|v| u16::try_from(v).ok())
                    .ok_or("bank")?,
                program_zero_based: v["program"]
                    .as_u64()
                    .and_then(|v| u8::try_from(v).ok())
                    .ok_or("program")?,
            })
        })
        .collect::<Result<_, &str>>()?;
    let grace = ExplicitGracePolicy {
        groups: selected["graces"]
            .as_array()
            .ok_or("graces")?
            .iter()
            .map(|v| -> Result<_, Box<dyn std::error::Error>> {
                if v["allocation"]["kind"] != "explicit_following_quarters" {
                    return Err("Unsupported probe allocation".into());
                }
                Ok(ExplicitGraceGroup {
                    principal: EventId(v["principal"].as_str().ok_or("principal")?.into()),
                    allocation: GraceAllocation::ExplicitFollowingQuarters {
                        members: v["allocation"]["members"]
                            .as_array()
                            .ok_or("members")?
                            .iter()
                            .map(|m| {
                                Ok(GraceQuarterAllocation {
                                    event: EventId(m["event"].as_str().ok_or("event")?.into()),
                                    quarters: m["quarters"].as_str().ok_or("quarters")?.parse()?,
                                })
                            })
                            .collect::<Result<_, Box<dyn std::error::Error>>>()?,
                    },
                })
            })
            .collect::<Result<_, _>>()?,
    };
    let holds = ExplicitSynchronizedFermataPolicy {
        groups: selected["holds"]
            .as_array()
            .ok_or("holds")?
            .iter()
            .map(|h| {
                Ok(ExplicitSynchronizedHoldGroup {
                    boundary: h["boundary"].as_str().ok_or("boundary")?.parse()?,
                    hold: ExplicitHoldGroup {
                        id: h["id"].as_str().ok_or("id")?.into(),
                        members: h["members"]
                            .as_array()
                            .ok_or("members")?
                            .iter()
                            .map(|m| m.as_str().map(|s| EventId(s.into())).ok_or("member"))
                            .collect::<Result<_, _>>()?,
                        added_seconds: h["added_seconds"].as_str().ok_or("seconds")?.parse()?,
                    },
                })
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()?,
    };
    let ending = if args.len() >= 4 {
        Ending::ContinueToMovementEnd
    } else {
        Ending::Reject
    };
    let policy = PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1;
    let art = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
    let discovery = match discover_document_practice_joint_with_final_ending(
        &doc, policy, art, &routes, ending,
    ) {
        Ok(d) => {
            json!({"graces":d.discovery().graces.occurrences.len(),"fermatas":d.discovery().fermatas.occurrences.len(),"terminal_dashes":d.terminal_dashes()})
        }
        Err(e) => json!({"errors":e}),
    };
    let timing = match perform_document_practice_joint_with_final_ending(
        &doc,
        policy,
        &routes,
        art,
        &grace,
        if args.len()==5 {GraceSoundPolicy::PracticePrincipalStaccatoAccentInAllocatedRemainderV1} else {GraceSoundPolicy::FullGraceThenGatedRemainder},
        &holds,
        ending,
    ) {
        Ok(q) => {
            json!({"grace_windows":q.joint().grace_windows().len(),"holds":q.joint().timing().holds().len(),"source_unchanged":q.source()==&doc,"windows":q.joint().grace_windows().iter().map(|w|json!({"id":w.id.0,"start":w.quarter_start.to_string(),"end":w.quarter_end.to_string(),"seconds_start":w.seconds_start.to_string(),"seconds_end":w.seconds_end.to_string()})).collect::<Vec<_>>()})
        }
        Err(e) => json!({"errors":e}),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"source_sha256":hash,"route_parts":routes.len(),"discovery":discovery,"pure_timing":timing,"principal_articulation_policy":if args.len()==5 {"practice_principal_staccato_accent_in_allocated_remainder_v1"} else {"not_selected"},"scope":"Core practice probe only; palette evidence is external, no sealed native bank/mixer receipt and no audio output"})
        )?
    );
    Ok(())
}
