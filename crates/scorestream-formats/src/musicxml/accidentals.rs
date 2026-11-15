//! Exact note accidental leaves. No broad style, SMuFL or unknown-field acceptance.
use super::{
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::score::{AccidentalGlyph, PrintedAccidental};
use std::fmt::Write;

pub(super) fn read(note: &Node) -> Result<Option<PrintedAccidental>, MappingReport> {
    note.child("accidental")
        .map(|n| {
            let glyph = AccidentalGlyph::from_name(
                n.text.trim_matches([' ', '\t', '\r', '\n']),
            ).ok_or_else(|| unsupported(
                n,
                "accidental.glyph",
                "Accidental glyph is outside the finite existing SFS registry; it is not replaced with a pitch-derived glyph",
            ))?;
            let flag = |key| match n.attr(key) {
                None => Ok(None),
                Some("yes") => Ok(Some(true)),
                Some("no") => Ok(Some(false)),
                _ => Err(error(
                    n,
                    "accidental.flag",
                    "Accidental cautionary and parentheses must be yes or no",
                )),
            };
            Ok(PrintedAccidental {
                glyph,
                cautionary: flag("cautionary")?,
                parentheses: flag("parentheses")?,
            })
        })
        .transpose()
}
pub(super) fn write(accidental: Option<&PrintedAccidental>, out: &mut String) {
    let Some(a) = accidental else { return };
    out.push_str("<accidental");
    for (key, value) in [("cautionary", a.cautionary), ("parentheses", a.parentheses)] {
        if let Some(value) = value {
            write!(out, " {key}=\"{}\"", if value { "yes" } else { "no" }).unwrap();
        }
    }
    write!(out, ">{}</accidental>", a.glyph.name()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::super::xml;
    use super::*;
    use scorestream_core::Rational;
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_accidental_glyphs_flags_and_absences_use_production_decoder() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let mut absent_altered = 0;
        let mut tied_starts = 0;
        for movement in 1..=4 {
            let mut counts = [0; 7];
            let mut absent = 0;
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let scan = xml::walk(BufReader::new(File::open(&path).unwrap()), |m, _, phase| {
                if phase != xml::Phase::Complete || m.name != "measure" {
                    return Ok(());
                }
                for n in m.children_named("note") {
                    let Some(a) = read(n)? else {
                        absent += 1;
                        absent_altered +=
                            usize::from(n.child("pitch").and_then(|p| p.child("alter")).is_some());
                        continue;
                    };
                    let leaf = n.child("accidental").unwrap();
                    let mut report = MappingReport::default();
                    xml::schema(leaf, &mut report);
                    assert!(!report.has_errors(), "{report:?}");
                    assert!(leaf.children.is_empty());
                    counts[0] += 1;
                    let (index, alter) = match a.glyph {
                        AccidentalGlyph::Natural => (1, "0"),
                        AccidentalGlyph::Flat => (2, "-1"),
                        AccidentalGlyph::Sharp => (3, "1"),
                        _ => panic!("new source glyph"),
                    };
                    counts[index] += 1;
                    let p = n.child("pitch").unwrap();
                    let written = p.value("alter").unwrap_or("0").parse::<Rational>().unwrap();
                    assert_eq!(written, alter.parse().unwrap());
                    if a.glyph == AccidentalGlyph::Natural {
                        assert!(p.child("alter").is_none());
                    }
                    match (a.cautionary, a.parentheses) {
                        (None, None) => assert!(leaf.attrs.is_empty()),
                        (Some(true), Some(false)) => {
                            counts[4] += 1;
                            assert_eq!(leaf.attrs.len(), 2);
                        }
                        _ => panic!("new source flags"),
                    }
                    counts[5] += usize::from(n.child("grace").is_some());
                    counts[6] += usize::from(n.child("chord").is_some());
                    assert!(
                        n.child("rest").is_none()
                            && n.child("cue").is_none()
                            && n.child("unpitched").is_none()
                    );
                    assert_ne!(n.attr("print-object"), Some("no"));
                    for t in n.children_named("tie") {
                        assert_eq!(t.attr("type"), Some("start"));
                        tied_starts += 1;
                    }
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(
                counts,
                [
                    [1134, 713, 153, 268, 70, 0, 51],
                    [700, 527, 144, 29, 66, 0, 25],
                    [534, 241, 170, 123, 42, 15, 16],
                    [1182, 168, 545, 469, 145, 0, 33]
                ][movement - 1]
            );
            assert_eq!(scan.notes, [15261, 10908, 11488, 30902][movement - 1]);
            assert_eq!(u64::try_from(absent + counts[0]).unwrap(), scan.notes);
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            let options = super::super::MusicXmlOptions {
                geometry: super::super::GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
                ..Default::default()
            };
            assert!(
                super::super::read_reader_with_options(
                    BufReader::new(File::open(path).unwrap()),
                    &options
                )
                .is_err()
            );
        }
        assert_eq!(absent_altered, 6554);
        assert_eq!(tied_starts, 229);
    }
}
