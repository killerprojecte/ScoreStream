//! Exact authored note appearance, never discarded as regenerable geometry.
use super::{read::error, xml::Node};
use crate::MappingReport;
use scorestream_core::score::StemDirection;
/// Direct note visibility only. Other print-* attributes retain their own scope.
pub(super) fn visible(note: &Node) -> Result<Option<bool>, MappingReport> {
    match note
        .attr("print-object")
        .map(|value| value.trim_matches([' ', '\t', '\r', '\n']))
    {
        None => Ok(None),
        Some("yes") => Ok(Some(true)),
        Some("no") => Ok(Some(false)),
        _ => Err(error(
            note,
            "note.print_object",
            "Note print-object must be yes or no",
        )),
    }
}
pub(super) fn visibility_attribute(visible: Option<bool>) -> &'static str {
    match visible {
        None => "",
        Some(true) => " print-object=\"yes\"",
        Some(false) => " print-object=\"no\"",
    }
}
pub(super) fn stem(note: &Node) -> Result<Option<StemDirection>, MappingReport> {
    note.child("stem")
        .map(|n| {
            StemDirection::from_name(n.text.trim_matches([' ', '\t', '\r', '\n']))
                .ok_or_else(|| error(n, "note.stem", "Stem must be up, down, none or double"))
        })
        .transpose()
}
pub(super) fn measure_rest(note: &Node) -> Result<Option<bool>, MappingReport> {
    let Some(rest) = note.child("rest") else {
        return Ok(None);
    };
    match rest.attr("measure") {
        None => Ok(None),
        Some("yes") => Ok(Some(true)),
        Some("no") => Ok(Some(false)),
        _ => Err(error(
            rest,
            "rest.measure",
            "Measure rest must be yes or no",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{geometry, read, xml};
    use super::*;
    use crate::{
        MappingEffect, ScoreAdapter,
        musicxml::{self, GeometryPolicy, MusicXml, MusicXmlOptions},
        sfs::SfsAdapter,
    };
    use scorestream_core::Rational;
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::PathBuf};

    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_stems_and_full_rests_are_exact_independent_fields() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for movement in 1..=4 {
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let mut stems = 0;
            let mut up = 0;
            let mut down = 0;
            let mut grace_stems = 0;
            let mut full_rests = 0;
            let mut full_rest_fermatas = 0;
            let mut hidden = 0;
            let mut divisions = BTreeMap::<String, Rational>::new();
            let mut meters = BTreeMap::<String, Rational>::new();
            let scan = xml::walk(
                BufReader::new(File::open(&path).unwrap()),
                |m, parent, phase| {
                    if phase != xml::Phase::Complete || m.name != "measure" {
                        return Ok(());
                    }
                    let part = parent.unwrap().attr("id").unwrap();
                    if let Some(a) = m.child("attributes") {
                        if let Some(d) = a.child("divisions") {
                            divisions.insert(part.into(), read::decimal(d)?);
                        }
                        if let Some(t) = a.child("time") {
                            let beats: Rational = t.value("beats").unwrap().parse().unwrap();
                            let unit: Rational = t.value("beat-type").unwrap().parse().unwrap();
                            meters.insert(
                                part.into(),
                                &(&beats * &Rational::from_integer(4)) / &unit,
                            );
                        }
                    }
                    let mut cursor = Rational::zero();
                    for n in &m.children {
                        if n.name == "backup" || n.name == "forward" {
                            let duration = read::decimal(n.child("duration").unwrap())?;
                            cursor = if n.name == "backup" {
                                &cursor - &duration
                            } else {
                                &cursor + &duration
                            };
                            continue;
                        }
                        if n.name != "note" {
                            continue;
                        }
                        if let Some(direction) = stem(n)? {
                            let leaf = n.child("stem").unwrap();
                            let mut report = MappingReport::default();
                            xml::schema(leaf, &mut report);
                            assert!(!report.has_errors());
                            assert!(leaf.attrs.is_empty());
                            assert!(leaf.children.is_empty());
                            assert!(n.child("rest").is_none());
                            stems += 1;
                            up += usize::from(direction == StemDirection::Up);
                            down += usize::from(direction == StemDirection::Down);
                            grace_stems += usize::from(n.child("grace").is_some());
                        }
                        if measure_rest(n)? == Some(true) {
                            full_rests += 1;
                            full_rest_fermatas += usize::from(
                                n.child("notations")
                                    .and_then(|x| x.child("fermata"))
                                    .is_some(),
                            );
                            let mut report = MappingReport::default();
                            xml::schema(n.child("rest").unwrap(), &mut report);
                            assert!(!report.has_errors());
                            assert_eq!(cursor, Rational::zero());
                            assert!(n.child("type").is_none());
                            assert!(n.child("chord").is_none());
                            assert!(n.child("grace").is_none());
                            let duration = read::decimal(n.child("duration").unwrap())?;
                            assert_eq!(&duration / &divisions[part], meters[part]);
                        }
                        if n.attr("print-object") == Some("no") {
                            hidden += 1;
                            assert!(n.child("rest").is_some());
                            assert!(n.attr("print-spacing").is_none());
                            let mut report = MappingReport::default();
                            xml::schema_with_geometry(
                                n,
                                GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
                                &mut geometry::Audit::default(),
                                &mut report,
                            );
                            assert_eq!(visible(n)?, Some(false));
                            assert!(
                                !report
                                    .diagnostics
                                    .iter()
                                    .any(|d| d.feature == "attribute.print-object"),
                                "{report:?}"
                            );
                            assert!(read::decimal(n.child("duration").unwrap())?.is_positive());
                        }
                        if n.child("chord").is_none() && n.child("grace").is_none() {
                            cursor = &cursor + &read::decimal(n.child("duration").unwrap())?;
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            assert_eq!(
                (stems, up, down, grace_stems, full_rests, hidden),
                [
                    (10029, 3636, 6393, 0, 3197, 73),
                    (7102, 3006, 4096, 13, 2152, 0),
                    (5939, 2110, 3829, 128, 3630, 7),
                    (21243, 8242, 13001, 96, 2861, 0)
                ][movement - 1]
            );
            assert_eq!(scan.notes, [15261, 10908, 11488, 30902][movement - 1]);
            assert_eq!(full_rest_fermatas, [32, 0, 20, 0][movement - 1]);
            let options = MusicXmlOptions {
                geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
                ..Default::default()
            };
            // No stripped-score acceptance claim: untouched originals still fail on other semantics.
            assert!(
                musicxml::read_reader_with_options(
                    BufReader::new(File::open(path).unwrap()),
                    &options
                )
                .is_err()
            );
        }
    }
    // The exact original I/P1/m1 note, with only an explicit small context wrapper.
    // Source file identity is pinned above. This tests the production end-to-end mapper,
    // not full-original acceptance, and retains source coordinates under explicit policy.
    #[test]
    fn original_full_rest_fragment_roundtrips_without_invented_whole_note_type() {
        let note = r#"<note default-x="151.94" default-y="-10">
        <rest measure="yes" />
        <duration>8</duration>
        <voice>1</voice>
        </note>"#;
        let xml = format!(
            r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{note}</measure></part></score-partwise>"#
        );
        assert!(MusicXml.read(&xml).is_err());
        let options = MusicXmlOptions {
            geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
            ..Default::default()
        };
        let mapped = musicxml::read_with_options(&xml, &options).unwrap();
        assert_eq!(
            mapped
                .report
                .diagnostics
                .iter()
                .filter(|d| d.effect == MappingEffect::PolicyApplied)
                .count(),
            2
        );
        let event = &mapped.value.parts[0].measures[0].events[0];
        assert_eq!(event.measure_rest, Some(true));
        assert!(event.written.is_none());
        assert_eq!(event.duration, Rational::from_integer(2));
        let common = SfsAdapter
            .read(&SfsAdapter.write(&mapped.value).unwrap().value)
            .unwrap()
            .value;
        assert_eq!(common, mapped.value);
        let exported = MusicXml.write(&common).unwrap().value;
        assert!(!exported.contains("<type>"));
        assert!(exported.contains("<rest measure=\"yes\"/>"));
        assert_eq!(MusicXml.read(&exported).unwrap().value, common);
    }
}
