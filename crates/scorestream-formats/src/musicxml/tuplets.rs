//! Closed authored endpoint bridge; no tuplet span or duration is inferred.
use super::{
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::{
    notation::{EventId, RelationId},
    notation_order::{NotationChild, NotationPhase},
    tuplets::{TupletChild, TupletNotation},
};
use std::collections::BTreeMap;
pub(super) struct Pending {
    event: EventId,
    children: Vec<Child>,
}
enum Child {
    Ready(TupletChild),
    Slur { path: String, phase: NotationPhase },
}
pub(super) fn endpoint(n: &Node) -> Result<TupletChild, MappingReport> {
    let p = match n.attr("type") {
        Some("start") => NotationPhase::Start,
        Some("stop") => NotationPhase::Stop,
        _ => return Err(error(n, "tuplet.phase", "Expected start/stop")),
    };
    let bracket = n
        .attr("bracket")
        .map(|s| match s {
            "yes" => Ok(true),
            "no" => Ok(false),
            _ => Err(error(n, "tuplet.bracket", "Expected yes/no bracket")),
        })
        .transpose()?;
    if p == NotationPhase::Stop && bracket.is_some() {
        return Err(unsupported(
            n,
            "tuplet.stop_fields",
            "This endpoint profile does not support stop bracket attributes",
        ));
    }
    Ok(TupletChild::Tuplet { phase: p, bracket })
}
pub(super) fn read(
    note: &Node,
    event: &EventId,
    authored: bool,
) -> Result<Option<Pending>, MappingReport> {
    let wrappers: Vec<_> = note.children_named("notations").collect();
    if !wrappers.iter().any(|w| w.child("tuplet").is_some()) {
        return Ok(None);
    }
    if note.child("grace").is_some() || wrappers.len() != 1 || !wrappers[0].attrs.is_empty() {
        return Err(unsupported(
            note,
            "tuplet.wrapper_scope",
            "Tuplet v1 retains one unadorned wrapper on a measured event; no hierarchy is flattened",
        ));
    }
    let mut children = vec![];
    let mut group_index = 0;
    for n in &wrappers[0].children {
        let phase = || match n.attr("type") {
            Some("start") => Ok(NotationPhase::Start),
            Some("stop") => Ok(NotationPhase::Stop),
            _ => Err(error(
                n,
                "tuplet.phase",
                "Expected authored start or stop; no pairing inferred",
            )),
        };
        let c = match n.name.as_str() {
            "tuplet" => Child::Ready(endpoint(n)?),
            "tied" => Child::Ready(TupletChild::Existing {
                notation: NotationChild::Tied { phase: phase()? },
            }),
            "slur" => Child::Slur {
                path: n.path.clone(),
                phase: phase()?,
            },
            "fermata" => Child::Ready(TupletChild::Existing {
                notation: NotationChild::Fermata {},
            }),
            "articulations" => {
                if !authored && !super::automatic::is_canonical_simple(note) {
                    return Err(unsupported(
                        n,
                        "tuplet.articulation_profile",
                        "Noncanonical tuplet/articulation hierarchy requires Automatic or authored mixed import; legacy groups cannot be flattened",
                    ));
                }
                let c = Child::Ready(TupletChild::Existing {
                    notation: NotationChild::Articulations { group_index },
                });
                group_index += 1;
                c
            }
            _ => {
                return Err(unsupported(
                    n,
                    "tuplet.sibling",
                    "Unmapped notation sibling cannot be discarded to retain tuplet endpoints",
                ));
            }
        };
        children.push(c);
    }
    Ok(Some(Pending {
        event: event.clone(),
        children,
    }))
}
pub(super) fn resolve(
    pending: Vec<Pending>,
    slurs: &BTreeMap<String, RelationId>,
) -> Vec<TupletNotation> {
    pending
        .into_iter()
        .map(|p| TupletNotation {
            event: p.event,
            children: p
                .children
                .into_iter()
                .map(|c| match c {
                    Child::Ready(v) => v,
                    Child::Slur { path, phase } => TupletChild::Existing {
                        notation: NotationChild::Slur {
                            relation: slurs[&path].clone(),
                            phase,
                        },
                    },
                })
                .collect(),
        })
        .collect()
}
pub(super) fn write(
    group: &TupletNotation,
    out: &mut String,
    mut other: impl FnMut(&NotationChild, &mut String),
) {
    out.push_str("        <notations>");
    for c in &group.children {
        match c {
            TupletChild::Tuplet { phase, bracket } => {
                out.push_str(if *phase == NotationPhase::Start {
                    "<tuplet type=\"start\""
                } else {
                    "<tuplet type=\"stop\""
                });
                if let Some(v) = bracket {
                    out.push_str(if *v {
                        " bracket=\"yes\""
                    } else {
                        " bracket=\"no\""
                    });
                }
                out.push_str("/>");
            }
            TupletChild::Existing { notation } => other(notation, out),
        }
    }
    out.push_str("</notations>\n");
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::{ScoreAdapter, musicxml::xml, sfs::SfsAdapter};
    use std::{fs::File, io::Read, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES; exact endpoint-family projection, not whole original acceptance"]
    fn original_tuplet_endpoint_projection_and_unsupported_siblings() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut counts = [0; 4];
        let mut brackets = [0; 3];
        let mut blocked = 0;
        // Explicit endpoint-family projection: one neutral measured note, no source sibling is claimed imported.
        let base = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note id="e"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
        let template = crate::musicxml::MusicXml.read(base).unwrap().value;
        for movement in 1..=4 {
            let bytes =
                std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
            let archive = crate::mxl::open(
                File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let mut paired = vec![];
            archive.into_reader().read_to_end(&mut paired).unwrap();
            assert_eq!(bytes, paired);
            let scan = xml::walk(bytes.as_slice(), |m, _, phase| {
                if phase != xml::Phase::Complete || m.name != "measure" {
                    return Ok(());
                }
                for note in m.children_named("note") {
                    let raw: Vec<_> = note
                        .children_named("notations")
                        .flat_map(|w| w.children_named("tuplet"))
                        .collect();
                    if raw.is_empty() {
                        continue;
                    }
                    if note
                        .children_named("notations")
                        .any(|w| w.child("ornaments").is_some())
                    {
                        assert!(read(note, &EventId("e".into()), true).is_err());
                        blocked += 1;
                    } else {
                        assert!(read(note, &EventId("e".into()), true)?.is_some());
                    }
                    let mut children = vec![];
                    for n in raw {
                        let mut report = MappingReport::default();
                        xml::schema(n, &mut report);
                        assert!(!report.has_errors());
                        let child = endpoint(n)?;
                        if let TupletChild::Tuplet { phase, bracket } = &child {
                            assert_eq!(
                                *phase,
                                if n.attr("type") == Some("start") {
                                    NotationPhase::Start
                                } else {
                                    NotationPhase::Stop
                                }
                            );
                            assert_eq!(*bracket, n.attr("bracket").map(|v| v == "yes"));
                            brackets[match bracket {
                                Some(true) => 0,
                                Some(false) => 1,
                                None => 2,
                            }] += 1;
                        }
                        children.push(child);
                        counts[movement - 1] += 1;
                    }
                    let mut score = template.clone();
                    score.tuplets = vec![TupletNotation {
                        event: EventId("e".into()),
                        children,
                    }];
                    let native = SfsAdapter.write(&score).unwrap().value;
                    let recovered = SfsAdapter.read(&native).unwrap().value;
                    assert_eq!(score.tuplets, recovered.tuplets);
                    let exported = crate::musicxml::MusicXml.write(&recovered).unwrap().value;
                    let again = crate::musicxml::MusicXml.read(&exported).unwrap().value;
                    assert_eq!(score.tuplets, again.tuplets);
                    assert_eq!(
                        score.parts[0].measures[0].events,
                        again.parts[0].measures[0].events
                    );
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
        }
        assert_eq!(counts, [0, 420, 0, 1722]);
        assert_eq!(brackets, [10, 1061, 1071]);
        assert_eq!(blocked, 103);
        eprintln!(
            "Original endpoint projection {counts:?}; bracket yes/no/absent {brackets:?}; {blocked} ornament-bearing hosts remain rejected by full bridge. No span/pairing claim."
        );
    }
}
