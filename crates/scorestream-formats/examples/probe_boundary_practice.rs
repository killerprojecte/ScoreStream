//! Core timing and native plan probe. No audio bank is loaded or PCM rendered.
use scorestream_core::{fermata_timing::*, notation::*};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("probe_boundary_practice source.sfs selected.json".into());
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
    let policy = PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1;
    let art = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
    let discovery = json!({"scope":"source-retaining explicit boundary policy, no PCM"});
    let timing = match perform_document_practice_with_boundary_attacks(
        &doc,
        PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,
        policy,
        &routes,
        art,
        &grace,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &holds,
    ) {
        Ok(q) => {
            json!({"grace_windows":q.joint().grace_windows().len(),"holds":q.joint().timing().holds().len(),"source_unchanged":q.source()==&doc,"retained_statements":q.schedule().written_statements.len(),"paired_boundaries":q.schedule().boundaries.len()})
        }
        Err(e) => json!({"errors":e}),
    };
    let native_plan = if timing.get("errors").is_none() {
        use scorestream_app::playback::*;
        match plan_source_with_boundary_practice_joint_policies_and_pizzicato(
            &source,
            PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,
            PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
            PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
            PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
            policy,
            &routes,
            art,
            &grace,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &holds,
            FermataSoundPolicy::SustainSelectedAnchors,
            PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
        ) {
            Ok(p) => {
                json!({"notes":p.plan().notes.len(),"duration_seconds":p.plan().duration_seconds,"boundary":p.boundary_dynamics_receipt(),"note_dynamics":p.note_dynamics_receipts(),"diagnostics":p.plan().diagnostics})
            }
            Err(e) => json!({"error":e}),
        }
    } else {
        json!({"not_attempted":"core timing blocked"})
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"source_sha256":hash,"route_parts":routes.len(),"discovery":discovery,"pure_timing":timing,"native_plan":native_plan,"scope":"Core practice probe only; palette evidence is external, no sealed native bank/mixer receipt and no audio output"})
        )?
    );
    Ok(())
}
