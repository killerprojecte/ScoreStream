//! Bounded, explicitly synthetic chord fixtures. Raw originals have separate receipts.
use scorestream_core::{Record, Scalar, score::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::MusicXml,
    sfs::{self, SfsAdapter},
};
fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Violin</part-name><score-instrument id="i"><instrument-name>violin</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1" implicit="yes"><attributes><divisions>24</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    )
}
#[allow(clippy::too_many_arguments)]
fn note(
    id: &str,
    pitch: &str,
    duration: u32,
    kind: &str,
    dots: u32,
    tm: bool,
    chord: bool,
    beam: &str,
    extra: &str,
) -> String {
    let flags = match kind {
        "eighth" => 1,
        "16th" => 2,
        "32nd" => 3,
        _ => 0,
    };
    let beams = if beam.is_empty() {
        String::new()
    } else {
        (1..=flags)
            .map(|level| format!("<beam number=\"{level}\">{beam}</beam>"))
            .collect()
    };
    format!(
        "<note id=\"{id}\">{}<pitch><step>{pitch}</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>{kind}</type>{}{}<stem>up</stem>{beams}{extra}</note>",
        if chord { "<chord/>" } else { "" },
        "<dot/>".repeat(dots as usize),
        if tm {
            "<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>"
        } else {
            ""
        }
    )
}
fn pair(kind: &str, duration: u32, dots: u32, tm: bool, triad: bool) -> String {
    let mut out = String::new();
    for (suffix, role) in [("1", "begin"), ("2", "end")] {
        // The head sorts after its members both lexically and by pitch.
        out += &note(
            &format!("z-head{suffix}"),
            "G",
            duration,
            kind,
            dots,
            tm,
            false,
            role,
            "",
        );
        out += &note(
            &format!("a-member{suffix}"),
            "C",
            duration,
            kind,
            dots,
            tm,
            true,
            "",
            "",
        );
        if triad {
            out += &note(
                &format!("b-member{suffix}"),
                "E",
                duration,
                kind,
                dots,
                tm,
                true,
                "",
                "",
            );
        }
    }
    out
}
fn normalized(mut score: ScoreDocument) -> ScoreDocument {
    for part in &mut score.parts {
        for m in &mut part.measures {
            // Stable sorting by time/voice permits existing independent-voice canonicalization
            // but deliberately does not reorder same-voice chord members by ID or pitch.
            m.events
                .sort_by(|a, b| (&a.offset, &a.voice).cmp(&(&b.offset, &b.voice)));
            let mut heads = std::collections::BTreeMap::new();
            for event in &mut m.events {
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
fn roundtrip(source: &str) -> ScoreDocument {
    let original = MusicXml.read(source).unwrap().value;
    let source_sfs = SfsAdapter.write(&original).unwrap().value;
    let native = scorestream_codec::parse(&source_sfs).unwrap();
    let native: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&native).unwrap()).unwrap();
    let canonical = scorestream_codec::canonical(&native);
    let from_sfs = SfsAdapter.read(&canonical).unwrap().value;
    assert_eq!(normalized(from_sfs.clone()), normalized(original.clone()));
    let exported = MusicXml.write(&from_sfs).unwrap().value;
    let recovered = MusicXml.read(&exported).unwrap().value;
    assert_eq!(normalized(recovered), normalized(original.clone()));
    let mut beam_notes = 0;
    for body in exported.split("<note id=\"").skip(1) {
        let id = body.split('"').next().unwrap();
        let body = body.split("</note>").next().unwrap();
        if body.contains("<beam") {
            beam_notes += 1;
            assert!(id.starts_with("z-head"));
            assert!(!body.contains("<chord/>"));
        }
        if id.starts_with("a-member") || id.starts_with("b-member") {
            assert!(body.contains("<chord/>"));
            assert!(!body.contains("<beam"));
        }
    }
    assert_eq!(beam_notes, 2);
    original
}
#[test]
fn seven_source_rhythm_classes_and_dyads_triads_preserve_order_without_new_fields() {
    for (kind, duration, dots, tm) in [
        ("eighth", 12, 0, false),
        ("16th", 6, 0, false),
        ("32nd", 3, 0, false),
        ("eighth", 8, 0, true),
        ("16th", 4, 0, true),
        ("eighth", 18, 1, false),
        ("16th", 9, 1, false),
    ] {
        for triad in [false, true] {
            let score = roundtrip(&xml(&pair(kind, duration, dots, tm, triad)));
            assert_eq!(
                score.parts[0].measures[0].events.len(),
                if triad { 6 } else { 4 }
            );
            let with = SfsAdapter.write(&score).unwrap().value;
            let mut plain = score.clone();
            plain.beams.clear();
            let plain = SfsAdapter.write(&plain).unwrap().value;
            assert_eq!(
                scorestream_app::playback::plan_source(&with).unwrap(),
                scorestream_app::playback::plan_source(&plain).unwrap()
            );
        }
    }
}
#[test]
fn each_member_keeps_its_own_accidental_and_tie_without_head_copying() {
    let mut notes = pair("eighth", 12, 0, false, false);
    for (id, role) in [("a-member1", "start"), ("a-member2", "stop")] {
        let start = notes.find(&format!("<note id=\"{id}\">")).unwrap();
        let end = start + notes[start..].find("</note>").unwrap();
        let body = notes[start..end]
            .replace("<voice>", &format!("<tie type=\"{role}\"/><voice>"))
            .replace(
                "<stem>",
                "<accidental cautionary=\"yes\" parentheses=\"no\">natural</accidental><stem>",
            );
        notes.replace_range(
            start..end,
            &format!("{body}<notations><tied type=\"{role}\"/></notations>"),
        );
    }
    let score = roundtrip(&xml(&notes));
    assert_eq!(score.ties.len(), 1);
    assert_eq!(
        score.ties[0]
            .events
            .iter()
            .map(|x| x.0.as_str())
            .collect::<Vec<_>>(),
        ["a-member1", "a-member2"]
    );
    for e in &score.parts[0].measures[0].events {
        assert_eq!(e.accidental.is_some(), e.id.0.starts_with("a-member"));
    }
    let with = SfsAdapter.write(&score).unwrap().value;
    let mut plain = score.clone();
    plain.beams.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&with).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
}
#[test]
fn reordered_native_json_and_member_beams_cannot_redefine_the_head() {
    let score = roundtrip(&xml(&pair("16th", 6, 0, false, false)));
    let doc = sfs::to_document(&score).unwrap().value;
    let mut changed = doc.clone();
    let frame = changed
        .frames
        .iter_mut()
        .find(|f| {
            f.records
                .iter()
                .filter(|r| matches!(r, Record::Event(_)))
                .count()
                == 2
        })
        .unwrap();
    let positions = frame
        .records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| matches!(r, Record::Event(_)).then_some(i))
        .collect::<Vec<_>>();
    frame.records.swap(positions[0], positions[1]);
    assert!(sfs::from_document(&changed).is_err());
    let mut changed = doc.clone();
    for frame in &mut changed.frames {
        for rec in &mut frame.records {
            match rec {
                Record::Mark(m) if m.kind == "scorestream.beam" && m.target == "#z-head1" => {
                    m.target = "#a-member1".into();
                }
                _ => {}
            }
        }
    }
    assert!(sfs::from_document(&changed).is_err());
    let changed: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&changed).unwrap()).unwrap();
    assert!(sfs::from_document(&changed).is_err());
}
#[test]
fn exact_normalized_head_targets_survive_independent_member_identity() {
    let mut score = MusicXml
        .read(&xml(&pair("eighth", 12, 0, false, true)))
        .unwrap()
        .value;
    score.parts[0].measures[0].events[0].id.0 = "head needing normalization".into();
    score.parts[0].measures[0].events[1].id.0 = "member needing normalization".into();
    for e in &mut score.parts[0].measures[0].events[..3] {
        e.chord = Some("foreign chord group".into());
    }
    for b in &mut score.beams {
        if b.event.0 == "z-head1" {
            b.event.0 = "head needing normalization".into();
        }
    }
    let doc = sfs::to_document(&score).unwrap().value;
    let canonical = scorestream_codec::canonical(&doc);
    let mapped = SfsAdapter.read(&canonical).unwrap().value;
    let events = &mapped.parts[0].measures[0].events;
    assert_eq!(mapped.beams[0].event, events[0].id);
    assert_ne!(mapped.beams[0].event, events[1].id);
    assert_eq!(events[0].chord, events[1].chord);
    assert_eq!(events[1].chord, events[2].chord);
}
#[test]
fn no_member_metadata_or_unsupported_layout_is_guessed() {
    let valid = pair("16th", 6, 0, false, false);
    for replacement in ["<stem>down</stem>", "", "<stem>none</stem>"] {
        let start = valid.find("<note id=\"a-member1\">").unwrap();
        let end = start + valid[start..].find("</note>").unwrap();
        let mut changed = valid.clone();
        changed.replace_range(
            start..end,
            &valid[start..end].replace("<stem>up</stem>", replacement),
        );
        let report = MusicXml.read(&xml(&changed)).unwrap_err();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.feature == "beam.chord_stem"
                    && d.effect == scorestream_formats::MappingEffect::Unsupported)
        );
    }
    for tag in [
        "<notations><slur type=\"start\"/></notations>",
        "<notehead filled=\"yes\">diamond</notehead>",
        "<lyric><text>x</text></lyric>",
    ] {
        assert!(
            MusicXml
                .read(&xml(&valid.replacen(
                    "</note>",
                    &format!("{tag}</note>"),
                    1
                )))
                .is_err()
        );
    }
    let changed = valid.replace("<chord/>", "<chord unknown=\"x\"/>");
    assert!(MusicXml.read(&xml(&changed)).is_err());
    let native = sfs::to_document(&MusicXml.read(&xml(&valid)).unwrap().value)
        .unwrap()
        .value;
    let mut changed = native;
    for f in &mut changed.frames {
        for rec in &mut f.records {
            match rec {
                Record::Event(e) if e.id() == Some("a-member1") => {
                    e.attributes.remove("w");
                    e.attributes.insert("dots".into(), Scalar::Integer(0));
                }
                _ => {}
            }
        }
    }
    assert!(sfs::from_document(&changed).is_err());
}

#[test]
fn legal_shorter_musicxml_chord_members_report_unsupported_not_repair() {
    let valid = pair("16th", 6, 0, false, false);
    let start = valid.find("<note id=\"a-member1\">").unwrap();
    let end = start + valid[start..].find("</note>").unwrap();
    let mut changed = valid.clone();
    changed.replace_range(
        start..end,
        &valid[start..end]
            .replace("<duration>6</duration>", "<duration>3</duration>")
            .replace("<type>16th</type>", "<type>32nd</type>"),
    );
    let report = MusicXml.read(&xml(&changed)).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "beam.chord_rhythm"
                && d.effect == scorestream_formats::MappingEffect::Unsupported)
    );
}

#[test]
fn native_chord_group_identifiers_remain_strict_after_common_normalization() {
    let score = MusicXml
        .read(&xml(&pair("eighth", 12, 0, false, false)))
        .unwrap()
        .value;
    let mut doc = sfs::to_document(&score).unwrap().value;
    for frame in &mut doc.frames {
        for rec in &mut frame.records {
            match rec {
                Record::Event(e) if matches!(e.id(), Some("z-head1" | "a-member1")) => {
                    e.attributes.insert(
                        "chord".into(),
                        Scalar::String("invalid native group".into()),
                    );
                }
                _ => {}
            }
        }
    }
    assert!(sfs::from_document(&doc).is_err());
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == scorestream_core::Severity::Error
                && d.code == "field_value"
                && d.message == "Invalid chord: expected identifier")
    );
}
