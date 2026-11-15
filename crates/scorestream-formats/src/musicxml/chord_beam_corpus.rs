//! Opt-in receipts over untouched, pinned originals, not whole-score import tests.
//! Source fields are inspected through the production XML walker and beam decoder.
use super::{appearance, beams, note_type, read, xml};
use crate::{MappingReport, ScoreAdapter, sfs::SfsAdapter};
use scorestream_core::{Rational, notation::EventId, score::*};
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};

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
    // Literal byte equality, not a reconstructed or markup-stripped score.
    assert_eq!(paired, bytes, "XML/MXL identity for movement {movement}");
    bytes
}

fn pinned(scan: &xml::ScanSummary, movement: usize) {
    assert_eq!(
        scan.fingerprint
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        HASHES[movement - 1]
    );
}

#[derive(Debug, PartialEq, Eq)]
struct Eligibility {
    raw_duration: Rational,
    divisions: Rational,
    duration: Rational,
    written: WrittenDuration,
    stem: StemDirection,
    voice: String,
    // Keep authored absence distinct; all these originals use implicit staff one.
    staff: Option<u32>,
}

fn eligible(n: &xml::Node, divisions: &Rational) -> Result<Eligibility, MappingReport> {
    assert_eq!(n.children_named("pitch").count(), 1, "{}", n.path);
    for excluded in [
        "rest",
        "grace",
        "unpitched",
        "cue",
        "notehead",
        "notehead-text",
    ] {
        assert!(n.child(excluded).is_none(), "{}: {excluded}", n.path);
    }
    let t = n.child("type").unwrap();
    assert!(t.attrs.is_empty());
    let time_modification = n.child("time-modification").map(|tm| TimeModification {
        actual: tm.value("actual-notes").unwrap().parse().unwrap(),
        normal: tm.value("normal-notes").unwrap().parse().unwrap(),
        normal_type: tm.value("normal-type").map(|v| note_type(v).unwrap()),
        normal_dots: tm.children_named("normal-dot").count() as u32,
    });
    if let Some(tm) = &time_modification {
        assert_eq!(
            (tm.actual, tm.normal, tm.normal_type, tm.normal_dots),
            (3, 2, None, 0)
        );
    }
    let written = WrittenDuration {
        note_type: note_type(t.text.trim()).unwrap(),
        size: read::note_size(n)?,
        // Dot coordinates differ legitimately between tones of the same chord.
        dots: n.children_named("dot").count() as u32,
        time_modification,
    };
    assert!(written.size.is_none());
    assert!(written.dots <= 1);
    let base: Rational = match written.note_type {
        NoteType::Eighth => "1/2",
        NoteType::N16 => "1/4",
        NoteType::N32 => "1/8",
        _ => panic!("unexpected beamed chord spelling at {}", n.path),
    }
    .parse()
    .unwrap();
    let mut sounding = if written.dots == 1 {
        &base * &"3/2".parse().unwrap()
    } else {
        base
    };
    if let Some(tm) = &written.time_modification {
        sounding = &(&sounding * &Rational::from_integer(i64::from(tm.normal)))
            / &Rational::from_integer(i64::from(tm.actual));
    }
    let raw_duration = read::decimal(n.child("duration").unwrap())?;
    assert!(raw_duration.is_positive() && divisions.is_positive());
    let duration = &raw_duration / divisions;
    assert_eq!(sounding, duration, "written/performed rhythm at {}", n.path);
    let stem = appearance::stem(n)?.unwrap();
    assert!(matches!(stem, StemDirection::Up | StemDirection::Down));
    assert!(n.child("stem").unwrap().attrs.is_empty());
    let voice = n.value("voice").unwrap().to_owned();
    let staff = n.value("staff").map(|v| v.parse::<u32>().unwrap());
    assert_eq!(voice, "1");
    assert_eq!(staff, None);
    Ok(Eligibility {
        raw_duration,
        divisions: divisions.clone(),
        duration,
        written,
        stem,
        voice,
        staff,
    })
}

#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES to the untouched original XML/MXL directory"]
fn original_chord_head_inventory_is_exact_without_claiming_complete_import() {
    // Heads, attached members, head beam records, dyads, triads, dotted heads, tuplets.
    let expected = [
        [117, 117, 117, 117, 0, 0, 0],
        [196, 216, 397, 176, 20, 5, 24],
        [46, 48, 46, 44, 2, 0, 0],
        [264, 267, 371, 261, 3, 20, 107],
    ];
    let mut total = [0; 7];
    let mut rhythms = [0; 7];
    let mut stems = [0; 2];
    let mut dot_coordinate_differences = 0;
    for movement in 1..=4 {
        let bytes = original(movement);
        let mut counts = [0; 7];
        let mut divisions = BTreeMap::<String, Rational>::new();
        let scan = xml::walk(bytes.as_slice(), |m, parent, phase| {
            if phase != xml::Phase::Complete || m.name != "measure" {
                return Ok(());
            }
            let part = parent.unwrap().attr("id").unwrap();
            for (i, head) in m.children.iter().enumerate() {
                if head.name == "attributes"
                    && let Some(d) = head.child("divisions")
                {
                    divisions.insert(part.into(), read::decimal(d)?);
                }
                if head.name != "note" {
                    continue;
                }
                if head.child("chord").is_some() {
                    assert!(
                        beams::read(head, &EventId(head.path.clone()))?.is_empty(),
                        "No original chord member may carry beams: {}",
                        head.path
                    );
                    continue;
                }
                let decoded = beams::read(head, &EventId(head.path.clone()))?;
                if decoded.is_empty() {
                    continue;
                }
                let following = &m.children[i + 1..];
                // Discover members independently of adjacency, then prove adjacency.
                // A direction/backup/attributes interruption cannot be silently skipped.
                let members: Vec<_> = following
                    .iter()
                    .take_while(|n| n.name != "note" || n.child("chord").is_some())
                    .filter(|n| n.name == "note")
                    .collect();
                if members.is_empty() {
                    continue;
                }
                assert!((1..=2).contains(&members.len()));
                let anchor = eligible(head, &divisions[part])?;
                for (j, member) in members.iter().enumerate() {
                    assert_eq!(
                        following[j].path, member.path,
                        "interrupted chord attachment"
                    );
                    assert_eq!(member.children_named("chord").count(), 1);
                    assert_eq!(
                        eligible(member, &divisions[part])?,
                        anchor,
                        "{}",
                        member.path
                    );
                    assert!(beams::read(member, &EventId(member.path.clone()))?.is_empty());
                    // Same onset follows from contiguous <chord/> membership; members
                    // do not advance the source cursor. No timing is inferred from x/y.
                    if anchor.written.dots == 1 {
                        dot_coordinate_differences += usize::from(
                            head.child("dot").unwrap().attrs != member.child("dot").unwrap().attrs,
                        );
                    }
                }
                let class = match (
                    &anchor.written.note_type,
                    anchor.written.dots,
                    anchor.written.time_modification.is_some(),
                ) {
                    (NoteType::Eighth, 0, false) => 0,
                    (NoteType::N16, 0, false) => 1,
                    (NoteType::Eighth, 0, true) => 2,
                    (NoteType::N32, 0, false) => 3,
                    (NoteType::N16, 0, true) => 4,
                    (NoteType::Eighth, 1, false) => 5,
                    (NoteType::N16, 1, false) => 6,
                    _ => panic!("unexpected chord rhythm at {}", head.path),
                };
                rhythms[class] += 1;
                stems[usize::from(anchor.stem == StemDirection::Down)] += 1;
                for (level, b) in decoded.iter().enumerate() {
                    assert_eq!(b.event.0, head.path);
                    assert_eq!(b.number, Some((level + 1) as u8));
                    assert!(level < 3 && b.value != BeamValue::ForwardHook);
                }
                counts[0] += 1;
                counts[1] += members.len();
                counts[2] += decoded.len();
                counts[2 + members.len()] += 1;
                counts[5] += usize::from(anchor.written.dots == 1);
                counts[6] += usize::from(anchor.written.time_modification.is_some());
            }
            Ok(())
        })
        .unwrap();
        pinned(&scan, movement);
        assert_eq!(counts, expected[movement - 1], "movement {movement}");
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
    }
    assert_eq!(total, [623, 648, 931, 598, 25, 25, 131]);
    assert_eq!(rhythms, [253, 148, 107, 66, 24, 21, 4]);
    assert_eq!(stems, [401, 222]);
    assert_eq!(dot_coordinate_differences, 25);
}

// MusicXML carries ordered membership, not the internal chord-group string.
// Normalize only that identity; retain every event, its order and all other fields.
fn comparable(mut score: ScoreDocument) -> ScoreDocument {
    for part in &mut score.parts {
        for measure in &mut part.measures {
            let mut heads = BTreeMap::<String, String>::new();
            for event in &mut measure.events {
                if let Some(group) = &event.chord {
                    let head = heads
                        .entry(group.clone())
                        .or_insert_with(|| event.id.0.clone());
                    event.chord = Some(format!("head:{head}"));
                }
            }
        }
    }
    score
}

#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES; contextual fragment only, not whole-original acceptance"]
fn original_i_p14_m55_complete_measure_fragment_roundtrips_all_events_and_head_beams() {
    let bytes = original(1);
    pinned(&xml::scan(bytes.as_slice()).unwrap(), 1);
    let source = std::str::from_utf8(&bytes).unwrap();
    let part = source
        .split_once("<part id=\"P14\">")
        .unwrap()
        .1
        .split_once("</part>")
        .unwrap()
        .0;
    let start = part.find("<measure number=\"55\"").unwrap();
    let end = start + part[start..].find("</measure>").unwrap() + "</measure>".len();
    let raw_measure = &part[start..end];
    let first_measure = &part[..part.find("</measure>").unwrap()];
    let attributes_start = first_measure.find("<attributes>").unwrap();
    let attributes_end = attributes_start
        + first_measure[attributes_start..]
            .find("</attributes>")
            .unwrap()
        + "</attributes>".len();
    let inherited_attributes = &first_measure[attributes_start..attributes_end];
    // P14/m1 is the only attributes block before m55: divisions 4, key -3,
    // meter 2/4 and G clef line 2. Restore the entire raw block, not hand-picked
    // rhythmic fields. This does not claim every surrounding score field survives.
    assert!(first_measure.contains("<measure number=\"1\""));
    assert_eq!(part[..start].matches("<attributes>").count(), 1);
    // Borrow the complete original measure byte-for-byte, including width, note
    // coordinates, accidentals, stems, beam roles, whitespace and chord markers.
    // The wrapper and preceding full-rest measure are synthetic; their inherited
    // attributes above are copied unchanged from the original first measure.
    let prefix = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P14"><part-name>Violin 1</part-name><score-instrument id="i"><instrument-name>violin</instrument-name></score-instrument></score-part></part-list><part id="P14"><measure number="54-context">{inherited_attributes}<note><rest measure="yes"/><duration>8</duration><voice>1</voice></note></measure>"#
    );
    let contextual = format!("{prefix}{raw_measure}</part></score-partwise>");
    assert_eq!(
        &contextual.as_bytes()[prefix.len()..prefix.len() + raw_measure.len()],
        raw_measure.as_bytes()
    );
    assert!(super::MusicXml.read(&contextual).is_err());
    let options = super::MusicXmlOptions {
        geometry: super::GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
        ..Default::default()
    };
    let mapped = super::read_with_options(&contextual, &options)
        .unwrap()
        .value;
    assert_eq!(mapped.parts[0].measures.len(), 2);
    let events = &mapped.parts[0].measures[1].events;
    assert_eq!(events.len(), 7);
    assert_eq!(mapped.beams.len(), 3);
    for (index, role) in [
        (1, BeamValue::Begin),
        (3, BeamValue::Continue),
        (5, BeamValue::End),
    ] {
        assert_eq!(
            mapped
                .beams
                .iter()
                .filter(|b| b.event == events[index].id)
                .count(),
            1
        );
        assert!(
            mapped
                .beams
                .iter()
                .any(|b| b.event == events[index].id && b.number == Some(1) && b.value == role)
        );
        assert_eq!(events[index].chord, events[index + 1].chord);
        assert!(events[index].chord.is_some());
        assert!(mapped.beams.iter().all(|b| b.event != events[index + 1].id));
    }
    let native = SfsAdapter
        .read(&SfsAdapter.write(&mapped).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(comparable(native.clone()), comparable(mapped.clone()));
    let exported = super::MusicXml.write(&native).unwrap().value;
    let recovered = super::MusicXml.read(&exported).unwrap().value;
    assert_eq!(comparable(recovered), comparable(mapped));
}
