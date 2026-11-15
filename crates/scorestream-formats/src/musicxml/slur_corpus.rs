//! Opt-in field-decoder receipts over pinned, untouched originals. These do not
//! claim that unrelated source semantics, or any whole original score, import.
use super::{slurs, xml};
use crate::{MappingReport, ScoreAdapter, sfs::SfsAdapter};
use scorestream_core::{
    Rational,
    notation::{EventId, PartId, Placement, RelationId},
    score::{ScoreDocument, SlurEndpoint, SlurOrientation},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::PathBuf,
};

const HASHES: [&str; 4] = [
    "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
    "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
    "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
    "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
];

fn original(movement: usize) -> Vec<u8> {
    let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
    let bytes = std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
    let archive =
        crate::mxl::open(File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap())
            .unwrap();
    assert_eq!(archive.root_path(), "score.xml");
    assert_eq!(archive.xml_bytes(), bytes.len() as u64);
    let mut paired = Vec::new();
    archive.into_reader().read_to_end(&mut paired).unwrap();
    assert_eq!(
        paired, bytes,
        "literal XML/MXL byte identity, movement {movement}"
    );
    bytes
}

fn pinned(scan: &xml::ScanSummary, movement: usize) {
    assert_eq!(
        scan.fingerprint
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        HASHES[movement - 1],
    );
}

// Independent exact scalar oracle for the pinned decimal vocabulary, not the
// production slur/decimal decoder and never a floating-point approximation.
fn raw_decimal(value: &str) -> Rational {
    let fractional = value.split_once('.').map_or(0, |(_, tail)| tail.len());
    let numerator = value.replace('.', "").parse::<i64>().unwrap();
    &Rational::from_integer(numerator) / &Rational::from_integer(10_i64.pow(fractional as u32))
}

fn raw_endpoint(n: &xml::Node, event: &EventId) -> SlurEndpoint {
    assert!(n.children.is_empty() && n.text.trim().is_empty());
    assert!(n.attrs.keys().all(|k| matches!(
        k.as_str(),
        "type" | "number" | "bezier-x" | "bezier-y" | "orientation" | "placement"
    )));
    assert!(n.attr("bezier-x").is_some() && n.attr("bezier-y").is_some());
    SlurEndpoint {
        event: event.clone(),
        bezier_x: n.attr("bezier-x").map(raw_decimal),
        bezier_y: n.attr("bezier-y").map(raw_decimal),
        orientation: match n.attr("orientation") {
            None => None,
            Some("under") => Some(SlurOrientation::Under),
            other => panic!("unexpected original orientation: {other:?}"),
        },
        placement: match n.attr("placement") {
            None => None,
            Some("below") => Some(Placement::Below),
            other => panic!("unexpected original placement: {other:?}"),
        },
    }
}

struct SourceStart {
    endpoint: SlurEndpoint,
    measure: String,
    onset: Rational,
    grace: bool,
}
type SourcePair = (SlurEndpoint, SlurEndpoint, u8);

#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES to untouched original XML/MXL files; field-decoder receipt only"]
fn original_slur_fields_and_all_2533_pairs_are_preserved_without_claiming_whole_import() {
    // Pairs, same-note parallel groups, genuine grace starts, equal-onset pairs,
    // start-only under/below pairs. No complete ScoreDocument is synthesized.
    let expected = [
        [489, 114, 0, 0, 0],
        [582, 55, 3, 3, 0],
        [462, 94, 66, 66, 0],
        [1000, 62, 61, 61, 3],
    ];
    let mut total = [0; 5];
    let mut parallel_ids = 0;
    for movement in 1..=4 {
        let bytes = original(movement);
        let mut encounters = Vec::new();
        let mut open = BTreeMap::<(String, u8), SourceStart>::new();
        let mut source_pairs = BTreeMap::<(String, String), Vec<SourcePair>>::new();
        let mut divisions = BTreeMap::<String, Rational>::new();
        let mut serial = 0;
        let mut counts = [0; 5];
        let mut phases = [0; 2];
        let mut vocabulary = BTreeMap::<String, usize>::new();
        let scan = xml::walk(bytes.as_slice(), |m, parent, phase| {
            if phase != xml::Phase::Complete || m.name != "measure" {
                return Ok(());
            }
            let part = parent.unwrap().attr("id").unwrap();
            let mut cursor = Rational::zero();
            let mut previous_onset = Rational::zero();
            // Preserve actual part/measure/note and per-note notation order.
            for n in &m.children {
                if n.name == "attributes" {
                    if let Some(d) = n.value("divisions") {
                        divisions.insert(part.into(), raw_decimal(d));
                    }
                    continue;
                }
                if matches!(n.name.as_str(), "backup" | "forward") {
                    let duration = &raw_decimal(n.value("duration").unwrap()) / &divisions[part];
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
                serial += 1;
                let event = EventId(format!("original{movement}_note{serial}"));
                let onset = if n.child("chord").is_some() {
                    previous_onset.clone()
                } else {
                    cursor.clone()
                };
                let raw: Vec<_> = n
                    .children_named("notations")
                    .flat_map(|notations| notations.children_named("slur"))
                    .collect();
                let decoded = slurs::read(n, &PartId(part.into()), &event)?;
                assert_eq!(decoded.len(), raw.len(), "{}", n.path);
                encounters.extend(decoded);
                for endpoint in raw {
                    // Context receipts, not fabricated common-model event anchors.
                    assert_eq!(n.value("voice"), Some("1"));
                    assert!(n.child("staff").is_none());
                    assert!(n.child("rest").is_none() && n.child("chord").is_none());
                    assert_eq!(n.children_named("pitch").count(), 1);
                    let grace = n.child("grace").is_some();
                    if grace {
                        assert!(n.child("duration").is_none());
                    }
                    for key in endpoint.attrs.keys() {
                        *vocabulary.entry(key.clone()).or_default() += 1;
                    }
                    let number: u8 = endpoint.attr("number").unwrap().parse().unwrap();
                    assert!((1..=2).contains(&number));
                    let value = raw_endpoint(endpoint, &event);
                    match endpoint.attr("type") {
                        Some("start") => {
                            phases[0] += 1;
                            counts[2] += usize::from(grace);
                            assert!(
                                open.insert(
                                    (part.into(), number),
                                    SourceStart {
                                        endpoint: value,
                                        measure: m.path.clone(),
                                        onset: onset.clone(),
                                        grace,
                                    }
                                )
                                .is_none(),
                                "unexpected original pairing collision at {}",
                                endpoint.path
                            );
                        }
                        Some("stop") => {
                            phases[1] += 1;
                            assert!(!grace, "no original stop anchors a grace note");
                            let start = open
                                .remove(&(part.into(), number))
                                .expect("original stop follows its start");
                            assert_ne!(start.endpoint.event, value.event);
                            let same_onset = start.measure == m.path && start.onset == onset;
                            assert_eq!(same_onset, start.grace, "{}", endpoint.path);
                            counts[3] += usize::from(same_onset);
                            if start.endpoint.orientation.is_some()
                                || start.endpoint.placement.is_some()
                            {
                                assert_eq!(movement, 4);
                                assert_eq!(
                                    start.endpoint.orientation,
                                    Some(SlurOrientation::Under)
                                );
                                assert_eq!(start.endpoint.placement, Some(Placement::Below));
                                counts[4] += 1;
                            }
                            assert!(value.orientation.is_none() && value.placement.is_none());
                            source_pairs
                                .entry((start.endpoint.event.0.clone(), value.event.0.clone()))
                                .or_default()
                                .push((start.endpoint, value, number));
                        }
                        other => panic!("unexpected original phase: {other:?}"),
                    }
                }
                if n.child("chord").is_none() {
                    previous_onset = onset;
                    if let Some(duration) = n.value("duration") {
                        cursor = &cursor + &(&raw_decimal(duration) / &divisions[part]);
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        pinned(&scan, movement);
        assert!(open.is_empty());
        let mut report = MappingReport::default();
        let resolved = slurs::resolve(encounters, &mut report).unwrap();
        assert!(!report.has_errors());
        counts[0] = resolved.len();
        assert_eq!(phases, [resolved.len(); 2]);
        assert_eq!(
            resolved
                .iter()
                .map(|s| &s.id)
                .collect::<BTreeSet<_>>()
                .len(),
            resolved.len()
        );
        for pairs in source_pairs.values() {
            assert!((1..=2).contains(&pairs.len()));
            if pairs.len() == 2 {
                assert_eq!(
                    pairs.iter().map(|p| p.2).collect::<BTreeSet<_>>(),
                    BTreeSet::from([1, 2])
                );
                assert_eq!((&pairs[0].0, &pairs[0].1), (&pairs[1].0, &pairs[1].1));
                counts[1] += 1;
                parallel_ids += 2;
            }
        }
        // Consume every raw pair exactly once. Distinct IDs with identical fields
        // cannot be merged, and endpoint-specific coordinate/absence swaps fail.
        for slur in &resolved {
            let pairs = source_pairs
                .get_mut(&(slur.start.event.0.clone(), slur.end.event.0.clone()))
                .unwrap();
            let index = pairs
                .iter()
                .position(|(a, b, _)| a == &slur.start && b == &slur.end)
                .unwrap();
            pairs.remove(index);
        }
        assert!(source_pairs.values().all(Vec::is_empty));
        for key in ["type", "number", "bezier-x", "bezier-y"] {
            assert_eq!(vocabulary.remove(key), Some(resolved.len() * 2));
        }
        for key in ["orientation", "placement"] {
            assert_eq!(vocabulary.remove(key).unwrap_or(0), counts[4]);
        }
        assert!(vocabulary.is_empty());
        assert_eq!(counts, expected[movement - 1], "movement {movement}");
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
    }
    assert_eq!(total, [2533, 325, 130, 130, 3]);
    assert_eq!(parallel_ids, 650);
}

// XML local numbering is not the native persistent relation ID. Only that
// identity is normalized for comparison; no endpoint or other score field is.
fn comparable(mut score: ScoreDocument) -> ScoreDocument {
    for slur in &mut score.slurs {
        slur.id = RelationId(String::new());
    }
    score
        .slurs
        .sort_by_key(|s| serde_json::to_string(s).unwrap());
    score
}

#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES; intact contextual fragment, not whole-original acceptance"]
fn original_i_p1_m449_intact_measure_preserves_both_parallel_slurs() {
    let bytes = original(1);
    pinned(&xml::scan(bytes.as_slice()).unwrap(), 1);
    let source = std::str::from_utf8(&bytes).unwrap();
    let part = source
        .split_once("<part id=\"P1\">")
        .unwrap()
        .1
        .split_once("</part>")
        .unwrap()
        .0;
    let start = part.find("<measure number=\"449\"").unwrap();
    let end = start + part[start..].find("</measure>").unwrap() + "</measure>".len();
    let measure = &part[start..end];
    let first = &part[..part.find("</measure>").unwrap()];
    let a = first.find("<attributes>").unwrap();
    let b = a + first[a..].find("</attributes>").unwrap() + "</attributes>".len();
    let inherited = &first[a..b];
    assert!(first.contains("<measure number=\"1\""));
    assert_eq!(part[..start].matches("<attributes>").count(), 1);
    // Entire inherited block: divisions 4, key -3, meter 2/4, G clef line 2.
    // Wrapper/context rest are synthetic; the source measure and attributes are
    // byte-for-byte borrowed, including all widths, coordinates and notations.
    let prefix = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Flute 1</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="P1"><measure number="448-context">{inherited}<note><rest measure="yes"/><duration>8</duration><voice>1</voice></note></measure>"#
    );
    let contextual = format!("{prefix}{measure}</part></score-partwise>");
    assert_eq!(
        &contextual.as_bytes()[prefix.len()..prefix.len() + measure.len()],
        measure.as_bytes()
    );
    assert!(super::MusicXml.read(&contextual).is_err());
    let mapped = super::read_with_options(
        &contextual,
        &super::MusicXmlOptions {
            geometry: super::GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
            ..Default::default()
        },
    )
    .unwrap()
    .value;
    assert_eq!(mapped.parts[0].measures.len(), 2);
    let events = &mapped.parts[0].measures[1].events;
    assert_eq!(events.len(), 2);
    assert_eq!(mapped.slurs.len(), 2);
    assert_ne!(mapped.slurs[0].id, mapped.slurs[1].id);
    for slur in &mapped.slurs {
        assert_eq!(
            slur.start,
            SlurEndpoint {
                event: events[0].id.clone(),
                bezier_x: Some(raw_decimal("10.389214")),
                bezier_y: Some(raw_decimal("5.980902")),
                orientation: None,
                placement: None,
            }
        );
        assert_eq!(
            slur.end,
            SlurEndpoint {
                event: events[1].id.clone(),
                bezier_x: Some(raw_decimal("-4.730152")),
                bezier_y: Some(raw_decimal("11.015108")),
                orientation: None,
                placement: None,
            }
        );
    }
    let native = SfsAdapter
        .read(&SfsAdapter.write(&mapped).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(native, mapped);
    let exported = super::MusicXml.write(&native).unwrap().value;
    let recovered = super::MusicXml.read(&exported).unwrap().value;
    assert_eq!(comparable(recovered), comparable(mapped));
}
