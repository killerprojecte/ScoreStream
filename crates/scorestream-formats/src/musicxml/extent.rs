//! Exact caller-selected exceptions, never an infer/max-length switch.
use super::{read::error, xml::Node};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Rational,
    score::{Meter, ScoreDocument},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// An explicit contract for one score-wide irregular bar. All declared parts and
/// all their staves must match. Ordinal is one-based; label is not an identity.
/// Only overfull bars are supported here. Short pickups keep the existing path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrregularMeasure {
    pub ordinal: usize,
    pub label: String,
    pub written_meter: Meter,
    pub actual_length: Rational,
}

/// Empty by default. A contract is exact, non-inheriting and consumed in every
/// part; stale labels, changed meters/extents, missing bars and duplicates fail.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicXmlOptions {
    pub irregular_measures: Vec<IrregularMeasure>,
    /// Explicit input-only regeneration of a finite set of geometric coordinates.
    #[serde(default, skip_serializing_if = "super::GeometryPolicy::is_strict")]
    pub geometry: super::GeometryPolicy,
    /// Input-only: opt-in authored v1 preserves ordered grouping and all typed choices but
    /// supplies no runtime realization. Legacy simple-sign import stays default.
    #[serde(default, skip_serializing_if = "super::ArticulationProfile::is_legacy")]
    pub articulations: super::ArticulationProfile,
}
impl MusicXmlOptions {
    pub(crate) fn at(&self, ordinal: usize) -> Option<&IrregularMeasure> {
        self.irregular_measures
            .iter()
            .find(|e| e.ordinal == ordinal)
    }
    pub(crate) fn validate(&self) -> Result<(), MappingReport> {
        let mut report = MappingReport::default();
        let mut ordinals = BTreeSet::new();
        for e in &self.irregular_measures {
            let meter = &e.written_meter;
            let valid_meter =
                !meter.beats.is_empty() && !meter.beats.contains(&0) && meter.beat_type > 0;
            if e.ordinal == 0
                || !ordinals.insert(e.ordinal)
                || e.label.trim().is_empty()
                || !valid_meter
                || !e.actual_length.is_positive()
                || (valid_meter && e.actual_length <= meter.quarters())
            {
                report.push(Format::MusicXml, MappingEffect::Invalid, "measure.extent_policy", "/options/irregular_measures", "Contract requires a unique positive ordinal, exact nonempty label, positive written meter and an actual length greater than that meter");
            }
        }
        if report.has_errors() {
            Err(report)
        } else {
            Ok(())
        }
    }
    pub(crate) fn complete(&self, score: &ScoreDocument, report: &mut MappingReport) {
        for e in &self.irregular_measures {
            if score.parts.is_empty() || score.parts.iter().any(|p| p.measures.len() < e.ordinal) {
                report.push(Format::MusicXml, MappingEffect::Invalid, "measure.extent_policy_unused", "/options/irregular_measures", format!("Contract for measure ordinal {} must match a measure in every declared part", e.ordinal));
            } else {
                let start = &score.parts[0].measures[e.ordinal - 1].start;
                if score
                    .parts
                    .iter()
                    .any(|p| p.measures[e.ordinal - 1].start != *start)
                {
                    report.push(Format::MusicXml, MappingEffect::Invalid, "measure.extent_policy_alignment", "/options/irregular_measures", "The contracted measure must begin at the same exact score position in every part");
                }
            }
        }
    }
}

fn matches(
    e: &IrregularMeasure,
    label: &str,
    implicit: bool,
    length: &Rational,
    staves: u32,
    meters: &BTreeMap<u32, Meter>,
) -> bool {
    !implicit
        && label == e.label
        && *length == e.actual_length
        && meters.len() == staves as usize
        && (1..=staves).all(|staff| meters.get(&staff) == Some(&e.written_meter))
}
fn policy_message(e: &IrregularMeasure) -> String {
    format!(
        "Explicit caller contract preserved actual length {} quarters at ordinal {} ({:?}); written meter {:?}/{} and numbering remain unchanged. This is not a timing inference or a fermata hold policy.",
        e.actual_length, e.ordinal, e.label, e.written_meter.beats, e.written_meter.beat_type
    )
}

pub(super) fn read_length(
    n: &Node,
    ordinal: usize,
    extent: &Rational,
    staves: u32,
    meters: &BTreeMap<u32, Meter>,
    options: &MusicXmlOptions,
    report: &mut MappingReport,
) -> Result<Rational, MappingReport> {
    let implicit = super::read::bool_attr(n, "implicit")?;
    let nominal = meters.values().next().map(Meter::quarters);
    if nominal
        .as_ref()
        .is_some_and(|first| meters.values().any(|m| m.quarters() != *first))
    {
        return Err(super::read::unsupported(
            n,
            "meter.polymeter",
            "Unequal staff meter lengths require a later profile",
        ));
    }
    if let Some(e) = options.at(ordinal) {
        if !matches(
            e,
            n.attr("number").unwrap_or(""),
            implicit,
            extent,
            staves,
            meters,
        ) {
            return Err(error(
                n,
                "measure.extent_policy_mismatch",
                format!(
                    "Exact irregular-measure contract did not match label, written meter, implicit status or encoded extent (observed {extent} quarters); no measure is resized"
                ),
            ));
        }
        n.error(
            report,
            MappingEffect::PolicyApplied,
            "measure.irregular_extent",
            policy_message(e),
        );
        return Ok(extent.clone());
    }
    let length = if implicit {
        extent.clone()
    } else {
        nominal.clone().unwrap_or_else(|| extent.clone())
    };
    if *extent > length {
        return Err(error(
            n,
            "measure.overfull",
            format!(
                "Encoded extent {extent} quarters exceeds written meter length {length}; an exact per-measure contract is required to preserve an intentional irregular bar"
            ),
        ));
    }
    Ok(length)
}

pub(super) fn validate_export(
    score: &ScoreDocument,
    options: &MusicXmlOptions,
    report: &mut MappingReport,
) {
    options.complete(score, report);
    for part in &score.parts {
        let mut meters = BTreeMap::new();
        for (index, m) in part.measures.iter().enumerate() {
            for state in &m.states {
                if let Some(meter) = &state.meter {
                    meters.insert(state.staff, meter.clone());
                }
            }
            if let Some(e) = options.at(index + 1) {
                if matches(e, &m.label, m.implicit, &m.length, part.staves, &meters) {
                    report.push(
                        Format::MusicXml,
                        MappingEffect::PolicyApplied,
                        "measure.irregular_extent",
                        format!("/parts/{}/measures/{}", part.id, m.id),
                        policy_message(e),
                    );
                } else {
                    report.push(Format::MusicXml, MappingEffect::Invalid, "measure.extent_policy_mismatch", &m.id, "Export requires the exact label, written meter, actual length and non-implicit status in every part");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        read::{decimal, note_size},
        xml,
    };
    use super::*;
    use scorestream_core::score::NoteSize;
    use std::{fs::File, io::BufReader, path::PathBuf};

    /// Apply the production extent-policy and graphic-size decoders to untouched
    /// original fragments. This is an evidence check, not full original import.
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_cadenza_extent_and_sounding_size_are_exact() {
        let path = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap())
            .join("Beethoven_Op.67_1.musicxml");
        let meter = Meter {
            beats: vec![2],
            beat_type: 4,
        };
        let options = MusicXmlOptions {
            geometry: super::super::GeometryPolicy::Strict,
            articulations: Default::default(),
            irregular_measures: vec![IrregularMeasure {
                ordinal: 268,
                label: "268".into(),
                written_meter: meter.clone(),
                actual_length: Rational::from_integer(10),
            }],
        };
        let mut divisions = BTreeMap::new();
        let mut meters = BTreeMap::new();
        let mut ordinals = BTreeMap::<String, usize>::new();
        let mut checked = BTreeSet::new();
        let mut oboe = vec![];
        let mut report = MappingReport::default();
        let summary = xml::walk(
            BufReader::new(File::open(&path).unwrap()),
            |m, parent, phase| {
                if phase != xml::Phase::Complete || m.name != "measure" {
                    return Ok(());
                }
                let part = parent.unwrap().attr("id").unwrap();
                let ordinal = ordinals.entry(part.into()).or_default();
                *ordinal += 1;
                if let Some(a) = m.child("attributes") {
                    if let Some(d) = a.child("divisions") {
                        divisions.insert(part.to_owned(), decimal(d)?);
                    }
                    if let Some(t) = a.child("time") {
                        meters.insert(
                            part.to_owned(),
                            Meter {
                                beats: t
                                    .value("beats")
                                    .unwrap()
                                    .split('+')
                                    .map(|s| s.parse().unwrap())
                                    .collect(),
                                beat_type: t.value("beat-type").unwrap().parse().unwrap(),
                            },
                        );
                    }
                }
                if m.attr("number") != Some("268") {
                    return Ok(());
                }
                assert_eq!(*ordinal, 268);
                assert_eq!(m.attr("implicit"), None);
                assert_eq!(meters[part], meter);
                assert!(checked.insert(part.to_owned()));
                let div = &divisions[part];
                assert_eq!(*div, Rational::from_integer(4));
                let mut cursor = Rational::zero();
                let mut extent = Rational::zero();
                let mut last = Rational::zero();
                for n in &m.children {
                    match n.name.as_str() {
                        "backup" => {
                            cursor = &cursor - &(&decimal(n.child("duration").unwrap())? / div)
                        }
                        "forward" => {
                            cursor = &cursor + &(&decimal(n.child("duration").unwrap())? / div);
                            if cursor > extent {
                                extent = cursor.clone();
                            }
                        }
                        "note" => {
                            assert!(n.child("cue").is_none());
                            assert!(n.child("grace").is_none());
                            let duration = &decimal(n.child("duration").unwrap())? / div;
                            let at = if n.child("chord").is_some() {
                                last.clone()
                            } else {
                                last = cursor.clone();
                                cursor = &cursor + &duration;
                                last.clone()
                            };
                            let end = &at + &duration;
                            if end > extent {
                                extent = end;
                            }
                            if part == "P3" {
                                let t = n.child("type").unwrap();
                                let mut schema = MappingReport::default();
                                xml::schema(t, &mut schema);
                                assert!(!schema.has_errors());
                                oboe.push((at.to_string(), duration.to_string(), note_size(n)?));
                            }
                        }
                        _ => {}
                    }
                }
                let active = BTreeMap::from([(1, meter.clone())]);
                assert!(
                    read_length(
                        m,
                        *ordinal,
                        &extent,
                        1,
                        &active,
                        &MusicXmlOptions::default(),
                        &mut MappingReport::default()
                    )
                    .is_err()
                );
                assert_eq!(
                    read_length(m, *ordinal, &extent, 1, &active, &options, &mut report)?,
                    Rational::from_integer(10)
                );
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
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b"
        );
        assert_eq!(summary.notes, 15261);
        assert_eq!(checked.len(), 18);
        assert_eq!(report.diagnostics.len(), 18);
        assert!(
            report
                .diagnostics
                .iter()
                .all(|d| d.effect == MappingEffect::PolicyApplied && d.source.is_some())
        );
        let expected = [
            ("0", "2", None),
            ("2", "1", Some(NoteSize::Cue)),
            ("3", "1", Some(NoteSize::Cue)),
            ("4", "2", Some(NoteSize::Cue)),
            ("6", "1/4", Some(NoteSize::Cue)),
            ("25/4", "1/4", Some(NoteSize::Cue)),
            ("13/2", "1/4", Some(NoteSize::Cue)),
            ("27/4", "1/4", Some(NoteSize::Cue)),
            ("7", "1", Some(NoteSize::Cue)),
            ("8", "1", Some(NoteSize::Cue)),
            ("9", "1", Some(NoteSize::Cue)),
        ];
        assert_eq!(
            oboe,
            expected
                .into_iter()
                .map(|(a, d, s)| (a.into(), d.into(), s))
                .collect::<Vec<_>>()
        );
        // No layout/symbol filtering is used to make the whole source pass.
        let failed = super::super::read_reader_with_options(
            BufReader::new(File::open(path).unwrap()),
            &options,
        )
        .unwrap_err();
        assert!(failed.has_errors());
        assert!(
            failed
                .diagnostics
                .iter()
                .any(|d| d.effect == MappingEffect::Unsupported)
        );
    }
}
