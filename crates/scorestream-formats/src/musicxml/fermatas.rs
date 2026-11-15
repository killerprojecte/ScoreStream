//! Event-anchored fermatas. Source notation never becomes an inferred hold.
use super::{
    read::{decimal, error, unsupported},
    write,
    xml::Node,
};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{fermata_extension as ext, notation::EventId, score::*};
use std::{collections::BTreeMap, fmt::Write};
type Result<T> = std::result::Result<T, MappingReport>;
pub(super) fn read(note: &Node, event: &EventId) -> Result<Vec<ScoreFermata>> {
    let signs = note
        .children_named("notations")
        .flat_map(|n| n.children_named("fermata"))
        .collect::<Vec<_>>();
    if signs.len() > 1 {
        return Err(unsupported(
            note,
            "fermata.duplicate",
            "Multiple fermata signs on one event require a separate supported profile",
        ));
    }
    signs
        .into_iter()
        .map(|n| {
            let shape = match n.text.as_str() {
                "" => None,
                "normal" => Some(FermataShape::Normal),
                "angled" => Some(FermataShape::Angled),
                "square" => Some(FermataShape::Square),
                "double-angled" => Some(FermataShape::DoubleAngled),
                "double-square" => Some(FermataShape::DoubleSquare),
                "double-dot" => Some(FermataShape::DoubleDot),
                "half-curve" => Some(FermataShape::HalfCurve),
                "curlew" => Some(FermataShape::Curlew),
                _ => return Err(error(n, "fermata.shape", "Unknown MusicXML fermata shape")),
            };
            let orientation = match n.attr("type") {
                None => None,
                Some("upright") => Some(FermataOrientation::Upright),
                Some("inverted") => Some(FermataOrientation::Inverted),
                _ => {
                    return Err(error(
                        n,
                        "fermata.orientation",
                        "Fermata type must be upright or inverted",
                    ));
                }
            };
            let coordinate = |key| {
                n.attr(key)
                    .map(|s| {
                        let mut number = n.clone();
                        number.text = s.into();
                        decimal(&number)
                    })
                    .transpose()
            };
            Ok(ScoreFermata {
                event: event.clone(),
                shape,
                orientation,
                default_y: coordinate("default-y")?,
                relative_y: coordinate("relative-y")?,
            })
        })
        .collect()
}
pub(super) struct Export<'a> {
    by_event: BTreeMap<&'a EventId, &'a ScoreFermata>,
}
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument, report: &mut MappingReport) -> Self {
        for f in &score.fermatas {
            for coordinate in [&f.default_y, &f.relative_y].into_iter().flatten() {
                if write::decimal(coordinate).is_none_or(|v| v.len() > 64) {
                    report.push(Format::MusicXml,MappingEffect::Unsupported,"fermata.coordinate",&f.event.0,
                        "Exact fermata placement requires a finite decimal within supported precision; it is not rounded");
                }
            }
        }
        crate::fermatas::authored_only(score, Format::MusicXml, report);
        Self {
            by_event: score.fermatas.iter().map(|f| (&f.event, f)).collect(),
        }
    }
    pub(super) fn contains(&self, event: &EventId) -> bool {
        self.by_event.contains_key(event)
    }
    pub(super) fn write(&self, event: &EventId, out: &mut String) {
        let Some(f) = self.by_event.get(event) else {
            return;
        };
        out.push_str("<fermata");
        if let Some(o) = f.orientation {
            write!(out, " type=\"{}\"", ext::orientation_name(o)).unwrap();
        }
        for (key, value) in [("default-y", &f.default_y), ("relative-y", &f.relative_y)] {
            if let Some(v) = value {
                write!(out, " {key}=\"{}\"", write::decimal(v).unwrap()).unwrap();
            }
        }
        if let Some(s) = f.shape {
            write!(out, ">{}</fermata>", ext::shape_name(s)).unwrap();
        } else {
            out.push_str("/>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::xml;
    use super::*;
    use scorestream_core::Rational;
    use std::{fs::File, io::BufReader, path::PathBuf};
    /// Apply the actual fermata mapper to untouched source note fragments. Other
    /// note/score semantics still fail strict full import; no content is removed.
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_fermata_fragments_keep_all_written_anchors() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let mut totals = (0, 0, 0, 0);
        for movement in 1..=4 {
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let mut divisions = BTreeMap::<String, Rational>::new();
            let mut signs = vec![];
            let summary = xml::walk(
                BufReader::new(File::open(&path).unwrap()),
                |m, parent, phase| {
                    if phase != xml::Phase::Complete || m.name != "measure" {
                        return Ok(());
                    }
                    let part = parent.unwrap().attr("id").unwrap();
                    let measure = m.attr("number").unwrap();
                    if let Some(div) = m.child("attributes").and_then(|a| a.child("divisions")) {
                        divisions.insert(part.into(), decimal(div)?);
                    }
                    let div = &divisions[part];
                    let mut cursor = Rational::zero();
                    let mut last = Rational::zero();
                    for (i, node) in m.children.iter().enumerate() {
                        match node.name.as_str() {
                            "backup" => {
                                cursor =
                                    &cursor - &(&decimal(node.child("duration").unwrap())? / div)
                            }
                            "forward" => {
                                cursor =
                                    &cursor + &(&decimal(node.child("duration").unwrap())? / div)
                            }
                            "note" => {
                                let Some(duration) = node.child("duration") else {
                                    continue;
                                };
                                let duration = &decimal(duration)? / div;
                                let at = if node.child("chord").is_some() {
                                    last.clone()
                                } else {
                                    let at = cursor.clone();
                                    last = at.clone();
                                    cursor = &cursor + &duration;
                                    at
                                };
                                let id = EventId(format!("{part}_{measure}_{i}"));
                                let mapped = read(node, &id)?;
                                for symbol in node
                                    .children_named("notations")
                                    .flat_map(|n| n.children_named("fermata"))
                                {
                                    let mut report = MappingReport::default();
                                    xml::schema(symbol, &mut report);
                                    assert!(!report.has_errors(), "{report:?}");
                                }
                                for f in mapped {
                                    assert_eq!(f.event, id);
                                    assert_eq!(f.shape, None);
                                    assert_eq!(f.orientation, Some(FermataOrientation::Upright));
                                    assert!(f.relative_y.is_some());
                                    let coordinate =
                                        match (movement, part, measure, at.to_string().as_str()) {
                                            (1, "P3", "268", "0") => Some((Some("232/25"), "5")),
                                            (1, "P3", "268", "9") => Some((Some("79/10"), "5")),
                                            (2, "P1", "123", "0") => Some((Some("857/25"), "5")),
                                            (3, "P1", "9", "0") => Some((None, "393/100")),
                                            (3, "P2", "9", "0") => Some((Some("1/25"), "927/100")),
                                            (3, "P3", "9", "0") => Some((Some("1/25"), "607/100")),
                                            (3, "P3", "247", "0") => {
                                                Some((Some("1/25"), "713/100"))
                                            }
                                            (3, "P5", "9", "0") => Some((Some("107/25"), "41/5")),
                                            (3, "P15" | "P16", "247", "0") => {
                                                Some((None, "287/100"))
                                            }
                                            (4, "P1", "446", "0") => Some((Some("611/25"), "5")),
                                            (4, "P18", "446", "2") => Some((Some("1341/50"), "5")),
                                            _ => None,
                                        };
                                    if let Some((y, relative)) = coordinate {
                                        assert_eq!(f.default_y, y.map(|v| v.parse().unwrap()));
                                        assert_eq!(f.relative_y, Some(relative.parse().unwrap()));
                                    }
                                    totals.0 += 1;
                                    totals.1 += usize::from(node.child("rest").is_some());
                                    totals.2 += usize::from(
                                        node.children_named("tie")
                                            .any(|t| t.attr("type") == Some("stop")),
                                    );
                                    totals.3 += usize::from(f.default_y.is_some());
                                    signs.push((
                                        part.to_owned(),
                                        measure.to_owned(),
                                        at.clone(),
                                        duration.clone(),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                summary
                    .fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            assert_eq!(signs.len(), [177, 16, 80, 19][movement - 1]);
            let has = |p: &str, m: &str, at: &str, dur: &str| {
                signs.contains(&(
                    p.into(),
                    m.into(),
                    at.parse().unwrap(),
                    dur.parse().unwrap(),
                ))
            };
            match movement {
                1 => {
                    for m in ["2", "5"] {
                        assert_eq!(
                            signs
                                .iter()
                                .filter(|s| s.1 == m && s.2 == Rational::zero())
                                .count(),
                            18
                        );
                    }
                    assert!(has("P3", "268", "0", "2"));
                    assert!(has("P1", "268", "5", "1"));
                    assert!(has("P3", "268", "9", "1"));
                    assert!(has("P14", "21", "0", "2"));
                    assert!(has("P1", "21", "1", "1"));
                }
                2 => {
                    assert!(has("P1", "123", "0", "1"));
                    assert!(has("P9", "123", "0", "1/2"));
                }
                3 => {
                    assert!(has("P1", "257", "0", "3"));
                    assert!(has("P7", "257", "1", "1"));
                }
                4 => {
                    assert!(has("P1", "446", "0", "4"));
                    assert!(has("P18", "446", "2", "2"));
                }
                _ => unreachable!(),
            }
            assert!(
                super::super::read::read_reader(BufReader::new(File::open(path).unwrap())).is_err()
            );
        }
        assert_eq!(totals, (292, 113, 70, 78));
    }
}
