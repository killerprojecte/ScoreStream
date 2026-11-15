//! Only explicit beam level and role are mapped. No fan, repeater or style omission.
use super::{
    read::error,
    xml::{Node, schema_with_geometry},
};
use crate::MappingReport;
use scorestream_core::{notation::EventId, score::*};
use std::{collections::BTreeMap, fmt::Write};

pub(super) fn read(note: &Node, event: &EventId) -> Result<Vec<ScoreBeam>, MappingReport> {
    let mut out = Vec::new();
    for b in note.children_named("beam") {
        let mut report = MappingReport::default();
        schema_with_geometry(
            b,
            super::GeometryPolicy::Strict,
            &mut super::geometry::Audit::default(),
            &mut report,
        );
        if report.has_errors() {
            return Err(report);
        }
        let number = b
            .attr("number")
            .map(|v| {
                // The integer value is retained, including absent vs explicit level one.
                v.trim_matches([' ', '\t', '\r', '\n'])
                    .parse::<u8>()
                    .ok()
                    .filter(|n| (1..=8).contains(n))
                    .ok_or_else(|| {
                        error(
                            b,
                            "beam.number",
                            "Beam number must be an integer from 1 through 8",
                        )
                    })
            })
            .transpose()?;
        let value = BeamValue::from_name(b.text.trim_matches([' ', '\t', '\r', '\n'])).ok_or_else(
            || {
                error(
                    b,
                    "beam.value",
                    "Unknown beam role; no grouping is inferred",
                )
            },
        )?;
        out.push(ScoreBeam {
            event: event.clone(),
            number,
            value,
        });
    }
    Ok(out)
}

pub(super) struct Export<'a>(BTreeMap<&'a EventId, Vec<&'a ScoreBeam>>);
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument) -> Self {
        let mut grouped = BTreeMap::<_, Vec<_>>::new();
        for b in &score.beams {
            grouped.entry(&b.event).or_default().push(b);
        }
        for group in grouped.values_mut() {
            group.sort_by_key(|b| b.number.unwrap_or(1));
        }
        Self(grouped)
    }
    pub(super) fn write(&self, event: &EventId, out: &mut String) {
        for b in self.0.get(event).into_iter().flatten() {
            out.push_str("<beam");
            if let Some(n) = b.number {
                write!(out, " number=\"{n}\"").unwrap();
            }
            write!(out, ">{}</beam>", b.value.name()).unwrap();
        }
    }
}

#[cfg(test)]
mod corpus {
    use super::super::xml;
    use super::*;
    use std::{fs::File, io::BufReader, path::PathBuf};

    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the untouched original XML/MXL directory"]
    fn original_beam_field_decoding_is_exact_without_claiming_complete_import() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        // records, owning notes, rests, graces, begin, end, continue, hooks
        let expected = [
            [3855, 3851, 0, 0, 1212, 1212, 1431, 0],
            [10841, 4931, 36, 6, 3264, 3264, 3901, 412],
            [1246, 998, 0, 124, 621, 621, 4, 0],
            [11046, 9056, 298, 64, 3559, 3559, 3539, 389],
        ];
        fn inventory<R: std::io::BufRead>(
            input: R,
        ) -> (xml::ScanSummary, Vec<ScoreBeam>, [usize; 8]) {
            let mut marks = Vec::new();
            let mut counts = [0; 8];
            let mut notes = 0;
            let scan = xml::walk(input, |m, _, phase| {
                if phase != xml::Phase::Complete || m.name != "measure" {
                    return Ok(());
                }
                for n in m.children_named("note") {
                    notes += 1;
                    let decoded = read(n, &EventId(format!("inventory_n{notes}")))?;
                    if !decoded.is_empty() {
                        counts[1] += 1;
                        counts[2] += usize::from(n.child("rest").is_some());
                        counts[3] += usize::from(n.child("grace").is_some());
                    }
                    for beam in &decoded {
                        assert!(beam.number.is_some());
                        assert!((1..=3).contains(&beam.number.unwrap()));
                        let i = match beam.value {
                            BeamValue::Begin => 4,
                            BeamValue::End => 5,
                            BeamValue::Continue => 6,
                            BeamValue::ForwardHook | BeamValue::BackwardHook => 7,
                        };
                        counts[i] += 1;
                    }
                    counts[0] += decoded.len();
                    marks.extend(decoded);
                }
                Ok(())
            })
            .unwrap();
            (scan, marks, counts)
        }
        for movement in 1..=4 {
            let (scan, marks, counts) = inventory(BufReader::new(
                File::open(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap(),
            ));
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            assert_eq!(counts, expected[movement - 1]);
            let archive = crate::mxl::open(
                File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let (compressed, other, other_counts) = inventory(archive.into_reader());
            assert_eq!(compressed.fingerprint, scan.fingerprint);
            assert_eq!(other, marks);
            assert_eq!(other_counts, counts);
            // This only decodes direct beam fields on the raw source nodes.
            // It does not admit surrounding unsupported notation or return a partial score.
        }
    }
}
