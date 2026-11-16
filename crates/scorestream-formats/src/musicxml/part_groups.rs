//! Lossless boundary placement within part-list; no inferred group nesting.
use super::{
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::part_groups::*;
pub(super) fn read(list: &Node) -> Result<Vec<PartGroupBoundary>, MappingReport> {
    let mut out = vec![];
    let mut pending: Vec<PartGroupBoundary> = vec![];
    for n in &list.children {
        if n.name == "score-part" {
            let id = n
                .attr("id")
                .ok_or_else(|| error(n, "part_group.part", "Part identity required"))?;
            for mut g in pending.drain(..) {
                g.before_part = Some(id.to_owned());
                out.push(g);
            }
            continue;
        }
        if n.name != "part-group" {
            return Err(unsupported(
                n,
                "part_group.child",
                "Unsupported part-list child cannot be discarded",
            ));
        }
        let phase = n
            .attr("type")
            .and_then(GroupPhase::from_name)
            .ok_or_else(|| {
                error(
                    n,
                    "part_group.type",
                    "Expected authored start/stop boundary",
                )
            })?;
        if phase == GroupPhase::Stop && !n.children.is_empty() {
            return Err(unsupported(
                n,
                "part_group.stop_presentation",
                "Stop presentation is not discarded even though MusicXML consumers may ignore it",
            ));
        }
        if n.children.len() == 2 && n.children[0].name != "group-symbol" {
            return Err(unsupported(
                n,
                "part_group.order",
                "Symbol must precede common-barline presentation",
            ));
        }
        let symbol = n
            .child("group-symbol")
            .map(|s| {
                if s.text.trim() == "bracket" {
                    Ok(GroupSymbol::Bracket)
                } else {
                    Err(unsupported(
                        s,
                        "part_group.symbol",
                        "Only the source-backed bracket symbol is supported",
                    ))
                }
            })
            .transpose()?;
        let barline = n
            .child("group-barline")
            .map(|s| match s.text.trim() {
                "yes" => Ok(true),
                "no" => Ok(false),
                _ => Err(unsupported(
                    s,
                    "part_group.barline",
                    "Only authored yes/no common barline presentation is supported",
                )),
            })
            .transpose()?;
        pending.push(PartGroupBoundary {
            before_part: None,
            phase,
            number: n.attr("number").map(str::to_owned),
            symbol,
            barline,
        });
    }
    out.extend(pending);
    let parts: Vec<_> = list
        .children_named("score-part")
        .filter_map(|p| p.attr("id"))
        .collect();
    if !valid(&out, &parts) {
        return Err(unsupported(
            list,
            "part_group.fields",
            "Unsupported token/presentation or invalid part boundary",
        ));
    }
    Ok(out)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub(super) fn write(g: &PartGroupBoundary, out: &mut String) {
    out.push_str(&format!("    <part-group type=\"{}\"", g.phase.name()));
    if let Some(n) = &g.number {
        out.push_str(&format!(" number=\"{}\"", escape(n)));
    }
    if g.symbol.is_none() && g.barline.is_none() {
        out.push_str("/>\n");
        return;
    }
    out.push('>');
    if g.symbol.is_some() {
        out.push_str("<group-symbol>bracket</group-symbol>");
    }
    if let Some(b) = g.barline {
        out.push_str(if b {
            "<group-barline>yes</group-barline>"
        } else {
            "<group-barline>no</group-barline>"
        });
    }
    out.push_str("</part-group>\n");
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::{
        ScoreAdapter,
        musicxml::{MusicXml, xml},
        sfs::SfsAdapter,
    };
    use std::io::Read;
    fn emit(n: &Node, out: &mut String) {
        out.push('<');
        out.push_str(&n.name);
        for (k, v) in &n.attrs {
            out.push_str(&format!(" {k}=\"{}\"", escape(v)));
        }
        out.push('>');
        out.push_str(&escape(n.text.trim()));
        for c in &n.children {
            emit(c, out);
        }
        out.push_str(&format!("</{}>", n.name));
    }
    fn signature(n: &Node) -> Vec<String> {
        n.children
            .iter()
            .map(|c| {
                if c.name == "score-part" {
                    format!("part:{}", c.attr("id").unwrap())
                } else {
                    let mut s = String::new();
                    emit(c, &mut s);
                    s
                }
            })
            .collect()
    }
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_part_group_boundaries_and_presentation_are_exact() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = std::path::PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut counts = [0; 4];
        for movement in 1..=4 {
            let bytes =
                std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
            let archive = crate::mxl::open(
                std::fs::File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let mut paired = vec![];
            archive.into_reader().read_to_end(&mut paired).unwrap();
            assert_eq!(bytes, paired);
            let mut raw = None;
            let scan = xml::walk(bytes.as_slice(), |n, _, phase| {
                if phase == xml::Phase::Complete && n.name == "part-list" {
                    assert!(raw.is_none());
                    raw = Some(n.clone());
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
            let raw = raw.unwrap();
            for g in raw.children_named("part-group") {
                let mut report = MappingReport::default();
                xml::schema(g, &mut report);
                assert!(!report.has_errors());
            }
            let groups = read(&raw).unwrap();
            counts[movement - 1] = groups.len();
            if movement >= 3 {
                assert_eq!(groups.len(), 2);
                assert_eq!(
                    groups[0].before_part.as_deref(),
                    Some(if movement == 3 { "P14" } else { "P19" })
                );
                assert_eq!(groups[1].before_part, None);
                assert_eq!(
                    raw.children_named("score-part").last().unwrap().attr("id"),
                    Some(if movement == 3 { "P18" } else { "P23" })
                );
                assert_eq!(groups[0].symbol, Some(GroupSymbol::Bracket));
                assert_eq!(groups[0].barline, Some(true));
                assert!(groups.iter().all(|g| g.number.as_deref() == Some("1")));
            }
            let mut input = String::from("<score-partwise version=\"4.0\"><part-list>");
            let mut bodies = String::new();
            for c in &raw.children {
                if c.name == "part-group" {
                    emit(c, &mut input);
                } else {
                    let id = c.attr("id").unwrap();
                    input.push_str(&format!("<score-part id=\"{id}\"><part-name>Piano</part-name><score-instrument id=\"i_{id}\"><instrument-name>piano</instrument-name></score-instrument></score-part>"));
                    bodies.push_str(&format!("<part id=\"{id}\"><measure id=\"m_{id}\" number=\"1\"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id=\"e_{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part>"));
                }
            }
            input.push_str("</part-list>");
            input.push_str(&bodies);
            input.push_str("</score-partwise>");
            let score = MusicXml.read(&input).unwrap().value;
            assert_eq!(score.part_groups, groups);
            let native = SfsAdapter.write(&score).unwrap().value;
            let recovered = SfsAdapter.read(&native).unwrap().value;
            assert_eq!(score.part_groups, recovered.part_groups);
            let written = MusicXml.write(&recovered).unwrap().value;
            let again = MusicXml.read(&written).unwrap().value;
            assert_eq!(score.part_groups, again.part_groups);
            let mut output = None;
            xml::walk(written.as_bytes(), |n, _, phase| {
                if phase == xml::Phase::Complete && n.name == "part-list" {
                    output = Some(signature(n));
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(Some(signature(&raw)), output);
            let mut plain = score.clone();
            plain.part_groups.clear();
            assert_eq!(
                scorestream_app::playback::plan_source(&native).unwrap(),
                scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                    .unwrap()
            );
        }
        assert_eq!(counts, [0, 0, 2, 2]);
        eprintln!(
            "Original ordered part-group boundaries {counts:?}, III P14–P18 and IV P19–P23 bracket/common barline preserved; exact part-list family projection, not whole original import."
        );
    }
}
