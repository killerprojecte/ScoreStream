//! Narrow boundary barline mapping. Unmapped attributes and children remain errors.
use super::{
    read, write,
    xml::{self, Node},
};
use crate::{MappingEffect, MappingReport};
use scorestream_core::{notation::PartId, score::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
type Result<T> = std::result::Result<T, MappingReport>;
fn error(n: &Node, feature: &str, message: &str) -> MappingReport {
    let mut r = MappingReport::default();
    n.error(&mut r, MappingEffect::Invalid, feature, message);
    r
}
fn unsupported(n: &Node, feature: &str, message: &str) -> MappingReport {
    let mut r = MappingReport::default();
    n.error(&mut r, MappingEffect::Unsupported, feature, message);
    r
}
fn style(value: &str) -> Option<BarlineStyle> {
    Some(match value {
        "regular" => BarlineStyle::Regular,
        "dotted" => BarlineStyle::Dotted,
        "dashed" => BarlineStyle::Dashed,
        "heavy" => BarlineStyle::Heavy,
        "light-light" => BarlineStyle::LightLight,
        "light-heavy" => BarlineStyle::LightHeavy,
        "heavy-light" => BarlineStyle::HeavyLight,
        "heavy-heavy" => BarlineStyle::HeavyHeavy,
        "tick" => BarlineStyle::Tick,
        "short" => BarlineStyle::Short,
        "none" => BarlineStyle::None,
        _ => return None,
    })
}
fn style_name(value: BarlineStyle) -> &'static str {
    match value {
        BarlineStyle::Regular => "regular",
        BarlineStyle::Dotted => "dotted",
        BarlineStyle::Dashed => "dashed",
        BarlineStyle::Heavy => "heavy",
        BarlineStyle::LightLight => "light-light",
        BarlineStyle::LightHeavy => "light-heavy",
        BarlineStyle::HeavyLight => "heavy-light",
        BarlineStyle::HeavyHeavy => "heavy-heavy",
        BarlineStyle::Tick => "tick",
        BarlineStyle::Short => "short",
        BarlineStyle::None => "none",
    }
}
pub(super) fn read_measure(measure: &Node, part: &PartId, id: &str) -> Result<Vec<ScoreBarline>> {
    let mut result = vec![];
    for (i, n) in measure
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name == "barline")
    {
        let mut previous = 0;
        for child in &n.children {
            let rank = match child.name.as_str() {
                "bar-style" => 0,
                "ending" => 1,
                "repeat" => 2,
                _ => continue,
            };
            if rank < previous {
                return Err(error(
                    n,
                    "barline.child_order",
                    "Barline children must be style, ending, then repeat",
                ));
            }
            previous = rank;
        }
        let location = match n.attr("location").unwrap_or("right") {
            "left" => BarlineLocation::Left,
            "right" => BarlineLocation::Right,
            "middle" => {
                return Err(unsupported(
                    n,
                    "barline.middle",
                    "Middle barline placement requires an explicit within-measure position",
                ));
            }
            _ => {
                return Err(error(
                    n,
                    "barline.location",
                    "Expected left, right, or middle",
                ));
            }
        };
        let other = if location == BarlineLocation::Left {
            &measure.children[..i]
        } else {
            &measure.children[i + 1..]
        };
        if other
            .iter()
            .any(|x| !matches!(x.name.as_str(), "print" | "bookmark" | "link"))
        {
            return Err(error(
                n,
                "barline.position",
                "Barline location disagrees with its position among measure content",
            ));
        }
        let style = n
            .child("bar-style")
            .map(|s| {
                style(s.text.trim())
                    .ok_or_else(|| error(s, "barline.style", "Invalid MusicXML bar style"))
            })
            .transpose()?;
        let repeat = n
            .child("repeat")
            .map(|r| {
                let direction = match r.attr("direction") {
                    Some("forward") => RepeatDirection::Forward,
                    Some("backward") => RepeatDirection::Backward,
                    _ => {
                        return Err(error(
                            r,
                            "repeat.direction",
                            "Required direction must be forward or backward",
                        ));
                    }
                };
                let times = r
                    .attr("times")
                    .map(|s| {
                        s.trim().parse::<u32>().map_err(|_| {
                            error(
                                r,
                                "repeat.times",
                                "Repeat count must be a bounded nonnegative integer",
                            )
                        })
                    })
                    .transpose()?;
                if direction == RepeatDirection::Forward && times.is_some() {
                    return Err(error(
                        r,
                        "repeat.times_direction",
                        "Only backward repeats can carry a play count",
                    ));
                }
                Ok(RepeatSign { direction, times })
            })
            .transpose()?;
        let ending = n
            .child("ending")
            .map(|e| {
                let phase = match e.attr("type") {
                    Some("start") => EndingPhase::Start,
                    Some("stop") => EndingPhase::Stop,
                    Some("discontinue") => EndingPhase::Discontinue,
                    _ => {
                        return Err(error(
                            e,
                            "ending.phase",
                            "Required type must be start, stop, or discontinue",
                        ));
                    }
                };
                let number = e
                    .attr("number")
                    .ok_or_else(|| error(e, "ending.passes", "Missing required ending number"))?;
                if number.len() > 16384 {
                    return Err(unsupported(
                        e,
                        "ending.passes_limit",
                        "Ending pass list exceeds supported lexical size",
                    ));
                }
                if number.trim().is_empty() {
                    return Err(unsupported(
                        e,
                        "ending.unknown_passes",
                        "Unknown ending passes cannot be routed without guessing",
                    ));
                }
                let passes = number
                    .split(',')
                    .map(|s| s.trim().parse::<u32>().ok().filter(|n| *n > 0))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        error(
                            e,
                            "ending.passes",
                            "Expected comma-separated positive integer passes",
                        )
                    })?;
                if passes.len() > 1024 {
                    return Err(unsupported(
                        e,
                        "ending.passes_limit",
                        "At most 1024 explicit ending passes",
                    ));
                }
                if passes.iter().collect::<BTreeSet<_>>().len() != passes.len() {
                    return Err(error(
                        e,
                        "ending.passes",
                        "Ending pass numbers must be distinct",
                    ));
                }
                let default_y = e
                    .attr("default-y")
                    .map(|v| {
                        let mut number = e.clone();
                        number.text = v.into();
                        read::decimal(&number)
                    })
                    .transpose()?;
                Ok(EndingSign {
                    passes,
                    phase,
                    text: e.text.clone(),
                    default_y,
                })
            })
            .transpose()?;
        result.push(ScoreBarline {
            part: part.clone(),
            measure: id.into(),
            location,
            id: n.attr("id").map(str::to_owned),
            style,
            repeat,
            ending,
        });
    }
    Ok(result)
}
pub(super) struct Output<'a> {
    bars: BTreeMap<(&'a str, BarlineLocation), &'a ScoreBarline>,
}
pub(super) fn output<'a>(
    score: &'a ScoreDocument,
    xml_ids: &mut BTreeSet<String>,
    report: &mut MappingReport,
) -> Output<'a> {
    let mut bars = BTreeMap::new();
    for b in &score.barlines {
        let path = format!("/barlines/{}/{}", b.part, b.measure);
        if let Some(id) = &b.id
            && (!write::xml_id(id) || !xml_ids.insert(id.clone()))
        {
            report.push(
                crate::Format::MusicXml,
                MappingEffect::Invalid,
                "barline.identity",
                &path,
                "Invalid or duplicate XML ID",
            );
        }
        if let Some(e) = &b.ending {
            if e.text.chars().any(|c| !xml::valid_char(c)) {
                report.push(
                    crate::Format::MusicXml,
                    MappingEffect::Invalid,
                    "xml.character",
                    &path,
                    "Ending text contains an invalid XML character",
                );
            }
            if e.passes.len() > 1024 {
                report.push(
                    crate::Format::MusicXml,
                    MappingEffect::Unsupported,
                    "ending.passes_limit",
                    &path,
                    "At most 1024 explicit ending passes",
                );
            }
            if e.default_y
                .as_ref()
                .is_some_and(|v| write::decimal(v).is_none_or(|s| s.len() > 64))
            {
                report.push(
                    crate::Format::MusicXml,
                    MappingEffect::Unsupported,
                    "ending.default_y",
                    &path,
                    "Ending placement requires an exact decimal of at most 32 places and 64 characters",
                );
            }
        }
        bars.insert((b.measure.as_str(), b.location), b);
    }
    Output { bars }
}
impl Output<'_> {
    pub(super) fn emit(&self, out: &mut String, measure: &str, location: BarlineLocation) {
        let Some(b) = self.bars.get(&(measure, location)) else {
            return;
        };
        write!(
            out,
            "      <barline location=\"{}\"",
            if location == BarlineLocation::Left {
                "left"
            } else {
                "right"
            }
        )
        .unwrap();
        if let Some(id) = &b.id {
            write!(out, " id=\"{}\"", xml::escape(id)).unwrap();
        }
        out.push('>');
        if let Some(style) = b.style {
            write!(out, "<bar-style>{}</bar-style>", style_name(style)).unwrap();
        }
        if let Some(e) = &b.ending {
            write!(
                out,
                "<ending number=\"{}\" type=\"{}\"",
                e.passes
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                match e.phase {
                    EndingPhase::Start => "start",
                    EndingPhase::Stop => "stop",
                    EndingPhase::Discontinue => "discontinue",
                }
            )
            .unwrap();
            if let Some(y) = &e.default_y {
                write!(out, " default-y=\"{}\"", write::decimal(y).unwrap()).unwrap();
            }
            write!(out, ">{}</ending>", xml::escape(&e.text)).unwrap();
        }
        if let Some(r) = &b.repeat {
            write!(
                out,
                "<repeat direction=\"{}\"",
                if r.direction == RepeatDirection::Forward {
                    "forward"
                } else {
                    "backward"
                }
            )
            .unwrap();
            if let Some(times) = r.times {
                write!(out, " times=\"{times}\"").unwrap();
            }
            out.push_str("/>");
        }
        out.push_str("</barline>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::BufReader, path::PathBuf};
    /// Exercises this production boundary mapper on untouched originals only.
    /// It does NOT bypass other strict mappings or claim full-score conversion.
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_barline_fragments_use_production_mapper() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let expected_counts = [(36, 18, 0), (18, 0, 0), (93, 36, 4), (140, 46, 3)];
        for movement in 1..=4 {
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let mut input = BufReader::new(File::open(&path).unwrap());
            let (mut bars, mut repeats, mut endings) = (vec![], BTreeSet::new(), BTreeSet::new());
            let mut indices: BTreeMap<String, usize> = BTreeMap::new();
            let summary = xml::walk(&mut input, |m, parent, phase| {
                if phase != xml::Phase::Complete
                    || m.name != "measure"
                    || !parent.is_some_and(|p| p.name == "part")
                {
                    return Ok(());
                }
                let pid = parent.unwrap().attr("id").unwrap();
                let index = indices.entry(pid.into()).or_default();
                let read = read_measure(m, &PartId(pid.into()), m.attr("number").unwrap())?;
                for b in &read {
                    let at = *index + usize::from(b.location == BarlineLocation::Right);
                    if let Some(r) = &b.repeat {
                        repeats.insert((at, r.direction, r.times));
                    }
                    if let Some(e) = &b.ending {
                        assert_eq!(pid, "P1");
                        endings.insert((at, e.phase, e.passes.clone()));
                    }
                }
                for node in m.children_named("barline") {
                    let mut report = MappingReport::default();
                    xml::schema(node, &mut report);
                    assert!(!report.has_errors(), "{report:?}");
                }
                bars.extend(read);
                *index += 1;
                Ok(())
            })
            .unwrap();
            let hash: String = summary
                .fingerprint
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(hash, hashes[movement - 1]);
            assert_eq!(
                (
                    bars.len(),
                    bars.iter().filter(|b| b.repeat.is_some()).count(),
                    bars.iter().filter(|b| b.ending.is_some()).count()
                ),
                expected_counts[movement - 1]
            );
            let expected_repeats = match movement {
                1 => vec![(124, RepeatDirection::Backward, None)],
                2 => vec![],
                3 => vec![
                    (141, RepeatDirection::Forward, None),
                    (162, RepeatDirection::Backward, None),
                ],
                4 => vec![
                    (1, RepeatDirection::Forward, None),
                    (86, RepeatDirection::Backward, None),
                ],
                _ => unreachable!(),
            };
            assert_eq!(repeats, expected_repeats.into_iter().collect());
            let expected_endings = match movement {
                3 => vec![
                    (161, EndingPhase::Start, vec![1]),
                    (162, EndingPhase::Stop, vec![1]),
                    (162, EndingPhase::Start, vec![2]),
                    (163, EndingPhase::Discontinue, vec![2]),
                ],
                4 => vec![
                    (84, EndingPhase::Start, vec![1]),
                    (86, EndingPhase::Stop, vec![1]),
                    (86, EndingPhase::Start, vec![2]),
                ],
                _ => vec![],
            };
            assert_eq!(endings, expected_endings.into_iter().collect());
            // Other score meaning is still correctly blocked, with no partial model.
            assert!(
                super::super::read::read_reader(BufReader::new(File::open(path).unwrap())).is_err()
            );
        }
    }
}
