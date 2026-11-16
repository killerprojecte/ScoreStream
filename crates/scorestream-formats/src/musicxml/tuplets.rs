//! Closed ordered tuplet/ornament bridge; no span, ornament attacks or duration is inferred.
use super::{
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::{
    notation::{EventId, RelationId},
    notation_order::{NotationChild, NotationPhase},
    tuplets::{Ornament, TupletChild, TupletNotation},
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
pub(super) fn ornament(n: &Node) -> Result<Ornament, MappingReport> {
    let fail = || {
        unsupported(
            n,
            "ornaments.shape",
            "Supported authored ornaments are empty trill-mark, single tremolo 1..4, or empty wavy-line start/stop number absent/1; no performance or pairing is inferred",
        )
    };
    let value = match n.name.as_str() {
        "trill-mark" if n.attrs.is_empty() && n.children.is_empty() && n.text.trim().is_empty() => {
            Ornament::TrillMark {}
        }
        "tremolo"
            if n.attrs.len() == 1 && n.attr("type") == Some("single") && n.children.is_empty() =>
        {
            Ornament::Tremolo {
                strokes: n.text.trim().parse().map_err(|_| fail())?,
            }
        }
        "wavy-line"
            if n.attrs
                .keys()
                .all(|k| ["type", "number", "default-y"].contains(&k.as_str()))
                && n.children.is_empty()
                && n.text.trim().is_empty() =>
        {
            Ornament::WavyLine {
                phase: match n.attr("type") {
                    Some("start") => NotationPhase::Start,
                    Some("stop") => NotationPhase::Stop,
                    _ => return Err(fail()),
                },
                number: n.attr("number").map(str::to_owned),
                default_y: n
                    .attr("default-y")
                    .map(|_| super::geometry::exact_decimal(&n.scalar_attribute("default-y")))
                    .transpose()?,
            }
        }
        _ => return Err(fail()),
    };
    if !value.valid() {
        return Err(fail());
    }
    Ok(value)
}
pub(super) fn read(
    note: &Node,
    event: &EventId,
    authored: bool,
) -> Result<Option<Pending>, MappingReport> {
    let wrappers: Vec<_> = note.children_named("notations").collect();
    if !wrappers
        .iter()
        .any(|w| w.child("tuplet").is_some() || w.child("ornaments").is_some())
    {
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
    let mut ornament_group = 0;
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
        if n.name == "ornaments" {
            if !n.attrs.is_empty() || n.children.is_empty() || !n.text.trim().is_empty() {
                return Err(unsupported(
                    n,
                    "ornaments.wrapper",
                    "Only nonempty unadorned ornament groups are retained",
                ));
            }
            for child in &n.children {
                children.push(Child::Ready(TupletChild::Ornament {
                    group_index: ornament_group,
                    ornament: ornament(child)?,
                }));
            }
            ornament_group += 1;
            continue;
        }
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
    let mut open_group = None;
    for c in &group.children {
        if let TupletChild::Ornament { group_index, .. } = c {
            if open_group != Some(*group_index) {
                if open_group.is_some() {
                    out.push_str("</ornaments>");
                }
                out.push_str("<ornaments>");
                open_group = Some(*group_index);
            }
        } else if open_group.take().is_some() {
            out.push_str("</ornaments>");
        }
        match c {
            TupletChild::Ornament { ornament, .. } => match ornament {
                Ornament::TrillMark {} => out.push_str("<trill-mark/>"),
                Ornament::Tremolo { strokes } => {
                    out.push_str(&format!("<tremolo type=\"single\">{strokes}</tremolo>"))
                }
                Ornament::WavyLine {
                    phase,
                    number,
                    default_y,
                } => {
                    out.push_str(if *phase == NotationPhase::Start {
                        "<wavy-line type=\"start\""
                    } else {
                        "<wavy-line type=\"stop\""
                    });
                    if let Some(n) = number {
                        out.push_str(&format!(" number=\"{n}\""));
                    }
                    if let Some(y) = default_y {
                        out.push_str(&format!(
                            " default-y=\"{}\"",
                            super::geometry::decimal_text(y).expect("validated decimal")
                        ));
                    }
                    out.push_str("/>");
                }
            },
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
    if open_group.is_some() {
        out.push_str("</ornaments>");
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
    fn original_tuplet_endpoint_projection_and_ornament_siblings() {
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
                        assert!(read(note, &EventId("e".into()), true)?.is_some());
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
            "Original endpoint projection {counts:?}; bracket yes/no/absent {brackets:?}; {blocked} ornament-bearing hosts accepted by ordered source bridge. No span/pairing claim."
        );
    }
}

#[cfg(test)]
mod ornament_corpus {
    use super::*;
    use crate::{ScoreAdapter, musicxml::xml, sfs::SfsAdapter};
    use std::{fs::File, io::Read, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES; exact endpoint-family projection, not whole original acceptance"]
    fn original_ornament_projection_and_103_mixed_hosts() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut counts = [0; 4];
        let mut members = [0; 3];
        let mut mixed = 0;
        let mut sounding = 0;
        let mut authored = 0;
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
                        .flat_map(|w| w.children_named("ornaments"))
                        .collect();
                    if raw.is_empty() {
                        continue;
                    }
                    assert!(read(note, &EventId("e".into()), true)?.is_some());
                    let mut children = vec![];
                    for (group_index, w) in raw.iter().enumerate() {
                        let mut report = MappingReport::default();
                        xml::schema(w, &mut report);
                        assert!(!report.has_errors());
                        counts[movement - 1] += 1;
                        for n in &w.children {
                            let o = ornament(n)?;
                            members[match o {
                                Ornament::TrillMark {} => 0,
                                Ornament::Tremolo { .. } => 1,
                                Ornament::WavyLine { .. } => 2,
                            }] += 1;
                            children.push(TupletChild::Ornament {
                                group_index,
                                ornament: o,
                            });
                        }
                    }
                    let mut score = template.clone();
                    score.tuplets = vec![TupletNotation {
                        event: EventId("e".into()),
                        children,
                    }];
                    let recovered = SfsAdapter
                        .read(&SfsAdapter.write(&score).unwrap().value)
                        .unwrap()
                        .value;
                    assert_eq!(score.tuplets, recovered.tuplets);
                    let exported = crate::musicxml::MusicXml.write(&recovered).unwrap().value;
                    let again = crate::musicxml::MusicXml.read(&exported).unwrap().value;
                    assert_eq!(score.tuplets, again.tuplets);
                    // For all 103 original tuplet+ornament hosts retain the COMPLETE wrapper,
                    // including the one articulation sibling. Other note fields are not claimed.
                    if note
                        .children_named("notations")
                        .any(|w| w.child("tuplet").is_some())
                    {
                        mixed += 1;
                        fn emit(n: &Node, out: &mut String) {
                            out.push('<');
                            out.push_str(&n.name);
                            for (k, v) in &n.attrs {
                                out.push_str(&format!(" {k}=\"{v}\""));
                            }
                            out.push('>');
                            out.push_str(&n.text);
                            for c in &n.children {
                                emit(c, out);
                            }
                            out.push_str(&format!("</{}>", n.name));
                        }
                        let mut wrapper = String::new();
                        for w in note.children_named("notations") {
                            emit(w, &mut wrapper);
                        }
                        let input = base.replace("</note>", &format!("{wrapper}</note>"));
                        let opts = crate::musicxml::MusicXmlOptions {
                            articulations: crate::musicxml::ArticulationProfile::Automatic,
                            ..Default::default()
                        };
                        let source = crate::musicxml::read_with_options(&input, &opts)
                            .unwrap()
                            .value;
                        let recovered = SfsAdapter
                            .read(&SfsAdapter.write(&source).unwrap().value)
                            .unwrap()
                            .value;
                        let written = crate::musicxml::MusicXml.write(&recovered).unwrap().value;
                        let again = crate::musicxml::read_with_options(&written, &opts)
                            .unwrap()
                            .value;
                        assert_eq!(source.tuplets, again.tuplets);
                        assert_eq!(source.articulations, again.articulations);
                        assert_eq!(source.authored_articulations, again.authored_articulations);
                        authored += usize::from(source.authored_articulations.is_some());
                        let mut plain = source.clone();
                        plain.tuplets.clear();
                        let actual = scorestream_app::playback::plan_source(
                            &SfsAdapter.write(&source).unwrap().value,
                        );
                        let unornamented = scorestream_app::playback::plan_source(
                            &SfsAdapter.write(&plain).unwrap().value,
                        );
                        sounding += usize::from(actual.is_ok());
                        assert_eq!(actual.map_err(|_| ()), unornamented.map_err(|_| ()));
                    }
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
        assert_eq!(counts, [203, 162, 61, 1143]);
        assert_eq!(mixed, 103);
        assert_eq!(authored, 1);
        assert!(sounding >= 102);
        eprintln!(
            "Original ornament wrappers {counts:?}; trill/tremolo/wavy children {members:?}; {mixed} full mixed source wrappers roundtrip; {sounding} sounding equality and {authored} authored articulation host. Family projection, not full original import."
        );
    }
}
