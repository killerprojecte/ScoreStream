use scorestream_core::{Rational, Record, Scalar, score::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::MusicXml,
    sfs::{self, SfsAdapter},
};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1" implicit="yes"><attributes><divisions>256</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, kind: &str, duration: u32, extra: &str, beams: &str) -> String {
    format!(
        r#"<note id="{id}">{extra}<pitch><step>C</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice><type>{kind}</type>{beams}</note>"#
    )
}
fn beam(number: Option<u8>, value: &str) -> String {
    format!(
        "<beam{}>{value}</beam>",
        number
            .map(|n| format!(" number=\"{n}\""))
            .unwrap_or_default()
    )
}
fn roundtrip(source: &str) -> ScoreDocument {
    // Existing adapters canonicalize independent voices by onset, not XML backup
    // traversal order. Compare musical ownership, not unrelated container order.
    fn comparable(mut score: ScoreDocument) -> ScoreDocument {
        for p in &mut score.parts {
            for m in &mut p.measures {
                m.events.sort_by(|a, b| {
                    (&a.offset, &a.voice, &a.id).cmp(&(&b.offset, &b.voice, &b.id))
                });
            }
        }
        score
    }
    let a = MusicXml.read(source).unwrap().value;
    let wire = SfsAdapter.write(&a).unwrap().value;
    let ast = scorestream_codec::parse(&wire).unwrap();
    assert_eq!(
        comparable(sfs::from_document(&ast).unwrap().value),
        comparable(a.clone())
    );
    let canonical = scorestream_codec::canonical(&ast);
    assert_eq!(
        comparable(SfsAdapter.read(&canonical).unwrap().value),
        comparable(a.clone())
    );
    let json: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&ast).unwrap()).unwrap();
    assert_eq!(
        comparable(sfs::from_document(&json).unwrap().value),
        comparable(a.clone())
    );
    let output = MusicXml.write(&a).unwrap().value;
    assert_eq!(
        comparable(MusicXml.read(&output).unwrap().value),
        comparable(a.clone())
    );
    a
}
#[test]
fn every_level_and_absent_default_survives_xml_sfs_json_canonical_roundtrip() {
    for count in 1..=8 {
        for first in [None, Some(1)] {
            let roles = |role| {
                (1..=count)
                    .map(|n| beam(if n == 1 { first } else { Some(n) }, role))
                    .collect::<String>()
            };
            let names = [
                "eighth", "16th", "32nd", "64th", "128th", "256th", "512th", "1024th",
            ];
            let d = 128 >> (count - 1);
            let source = xml(&format!(
                "{}{}",
                note("a", names[count as usize - 1], d, "", &roles("begin")),
                note("b", names[count as usize - 1], d, "", &roles("end"))
            ));
            let a = roundtrip(&source);
            assert_eq!(a.beams.len(), 2 * count as usize);
            assert_eq!(a.beams[0].number, first);
        }
    }
}
#[test]
fn hooks_rests_and_graphic_cue_size_do_not_change_time_or_pitch() {
    let a = note(
        "a",
        "16th",
        64,
        "",
        &(beam(Some(1), "begin") + &beam(Some(2), "forward hook")),
    )
    .replace("<type>", "<type size=\"cue\">");
    let b = note("b", "eighth", 128, "", &beam(None, "continue"))
        .replace("<pitch><step>C</step><octave>4</octave></pitch>", "<rest/>");
    let c = note(
        "c",
        "16th",
        64,
        "",
        &(beam(Some(1), "end") + &beam(Some(2), "backward hook")),
    );
    let score = roundtrip(&xml(&(a + &b + &c)));
    let events = &score.parts[0].measures[0].events;
    assert_eq!(events[0].duration, r("1/4"));
    assert_eq!(
        events[0].written.as_ref().unwrap().size,
        Some(NoteSize::Cue)
    );
    assert_eq!(events[1].pitch, scorestream_core::Pitch::Rest);
    let with = SfsAdapter.write(&score).unwrap().value;
    let mut without = score.clone();
    without.beams.clear();
    let without = SfsAdapter.write(&without).unwrap().value;
    assert_eq!(
        scorestream_app::playback::plan_source(&with).unwrap(),
        scorestream_app::playback::plan_source(&without).unwrap()
    );
}
#[test]
fn outward_third_level_hook_preserves_authored_beethoven_two_pattern() {
    // Exact beam pattern in II, P16/P17/P18, m237 note3 and m238 note2.
    let a = note(
        "a",
        "32nd",
        32,
        "",
        &(beam(Some(1), "begin") + &beam(Some(2), "begin") + &beam(Some(3), "forward hook")),
    );
    let b = note(
        "b",
        "32nd",
        32,
        "",
        &(beam(Some(1), "continue") + &beam(Some(2), "end") + &beam(Some(3), "forward hook")),
    );
    let c = note("c", "eighth", 128, "", &beam(Some(1), "end"));
    let score = roundtrip(&xml(&(a + &b + &c)));
    assert_eq!(score.beams[5].value, BeamValue::ForwardHook);
}
#[test]
fn independent_voices_and_grace_anchors_keep_beam_ownership() {
    let a = note("a", "eighth", 128, "", &beam(None, "begin"));
    let b = note("b", "eighth", 128, "", &beam(None, "end"));
    let ga = note(
        "ga",
        "16th",
        0,
        "<grace slash=\"yes\"/>",
        &(beam(Some(1), "begin") + &beam(Some(2), "begin")),
    )
    .replace("<duration>0</duration>", "");
    let gb = note(
        "gb",
        "16th",
        0,
        "<grace/>",
        &(beam(Some(1), "end") + &beam(Some(2), "end")),
    )
    .replace("<duration>0</duration>", "");
    let source = xml(&format!(
        "{ga}{gb}{a}{b}<backup><duration>256</duration></backup>{}",
        format!("{a}{b}")
            .replace("id=\"a\"", "id=\"c\"")
            .replace("id=\"b\"", "id=\"d\"")
            .replace("<voice>1</voice>", "<voice>2</voice>")
    ));
    let score = roundtrip(&source);
    assert_eq!(score.beams.len(), 8);
    assert_eq!(score.graces.len(), 2);
    assert_eq!(score.graces[0].anchor.0, "a");
}
#[test]
fn unsupported_fields_values_and_malformed_runs_fail_closed() {
    let valid = beam(Some(1), "begin");
    let a = note("a", "eighth", 128, "", &valid);
    let b = note("b", "eighth", 128, "", &beam(Some(1), "end"));
    for bad in [
        "<beam fan=\"none\">begin</beam>",
        "<beam color=\"#000000\">begin</beam>",
        "<beam repeater=\"no\">begin</beam>",
        "<beam id=\"x\">begin</beam>",
        "<beam number=\"0\">begin</beam>",
        "<beam number=\"9\">begin</beam>",
        "<beam number=\"NaN\">begin</beam>",
        "<beam><begin/></beam>",
        "<beam>other</beam>",
        "<beam>forward hook</beam>",
        "<beam>continue</beam>",
        "<beam>begin</beam><beam number=\"1\">begin</beam>",
        "<beam number=\"2\">begin</beam>",
    ] {
        assert!(
            MusicXml.read(&xml(&(a.replace(&valid, bad) + &b))).is_err(),
            "{bad}"
        );
    }
    assert!(MusicXml.read(&xml(&a)).is_err());
    let middle = note("middle", "eighth", 128, "", "");
    assert!(MusicXml.read(&xml(&(a.clone() + &middle + &b))).is_err());
    assert!(
        MusicXml
            .read(&xml(&(a.clone()
                + "<forward><duration>128</duration></forward>"
                + &b)))
            .is_err()
    );
    assert!(
        MusicXml
            .read(&xml(&(a
                + &note("chord", "16th", 64, "<chord/>", "")
                + &b)))
            .is_err()
    );
}
#[test]
fn native_marks_require_capability_exact_onset_and_strict_typed_fields() {
    let score = roundtrip(&xml(&(note("a", "eighth", 128, "", &beam(None, "begin"))
        + &note("b", "eighth", 128, "", &beam(None, "end")))));
    let doc = sfs::to_document(&score).unwrap().value;
    let mut missing = doc.clone();
    missing
        .required_extensions
        .retain(|e| e != "scorestream.beam@1");
    assert!(
        scorestream_core::validate(&missing, None)
            .iter()
            .any(|d| d.severity == scorestream_core::Severity::Error)
    );
    for (key, value) in [
        ("number", Scalar::String("1".into())),
        ("number", Scalar::Integer(9)),
        ("fan", Scalar::String("none".into())),
    ] {
        let mut changed = doc.clone();
        for f in &mut changed.frames {
            for rec in &mut f.records {
                match rec {
                    Record::Mark(m) if m.kind == "scorestream.beam" => {
                        m.attributes.insert(key.into(), value.clone());
                    }
                    _ => {}
                }
            }
        }
        assert!(sfs::from_document(&changed).is_err());
    }
}
#[test]
fn beam_targets_follow_sfs_identity_remapping_and_export_order() {
    let source = xml(&(note(
        "a",
        "eighth",
        128,
        "",
        &(beam(None, "begin") + "<notations><fermata/></notations>"),
    ) + &note("b", "eighth", 128, "", &beam(None, "end"))));
    let mut score = roundtrip(&source);
    let output = MusicXml.write(&score).unwrap().value;
    let a = output
        .split("<note id=\"a\"")
        .nth(1)
        .unwrap()
        .split("</note>")
        .next()
        .unwrap();
    assert!(a.find("<staff>").unwrap() < a.find("<beam>").unwrap());
    assert!(a.find("<beam>").unwrap() < a.find("<notations>").unwrap());
    score.parts[0].measures[0].events[0].id.0 = "source id with spaces".into();
    score.beams[0].event.0 = "source id with spaces".into();
    score.fermatas[0].event.0 = "source id with spaces".into();
    let doc = sfs::to_document(&score).unwrap().value;
    let mapped = sfs::from_document(&doc).unwrap().value;
    assert_eq!(
        mapped.beams[0].event,
        mapped.parts[0].measures[0].events[0].id
    );
    assert_eq!(mapped.beams[0].event, mapped.fermatas[0].event);
    assert_ne!(mapped.beams[0].event.0, "source id with spaces");
}
#[test]
fn unrelated_hidden_visibility_slurs_and_grace_notations_remain_blocked() {
    let a = note("a", "eighth", 128, "", &beam(None, "begin"));
    let b = note("b", "eighth", 128, "", &beam(None, "end"));
    for source in [
        xml(&(a.replace("<note id=\"a\"", "<note id=\"a\" print-object=\"no\"") + &b)),
        xml(&(a.replace(
            "</note>",
            "<notations><slur type=\"start\"/></notations></note>",
        ) + &b)),
        xml(&(a
            .replace("<duration>128</duration>", "")
            .replace("<pitch>", "<grace/><pitch>")
            .replace("</note>", "<notations><fermata/></notations></note>")
            + &b)),
    ] {
        assert!(MusicXml.read(&source).is_err());
    }
}
#[test]
fn normalized_grace_and_principal_ids_keep_distinct_beam_targets() {
    let ga = note(
        "ga",
        "16th",
        0,
        "<grace/>",
        &(beam(None, "begin") + &beam(Some(2), "begin")),
    )
    .replace("<duration>0</duration>", "");
    let gb = note(
        "gb",
        "16th",
        0,
        "<grace/>",
        &(beam(None, "end") + &beam(Some(2), "end")),
    )
    .replace("<duration>0</duration>", "");
    let a = note("a", "eighth", 128, "", &beam(None, "begin"));
    let b = note("b", "eighth", 128, "", &beam(None, "end"));
    let mut score = roundtrip(&xml(&(ga + &gb + &a + &b)));
    score.graces[0].id.0 = "grace id requiring normalization".into();
    score.parts[0].measures[0].events[0].id.0 = "principal id requiring normalization".into();
    for g in &mut score.graces {
        g.anchor.0 = "principal id requiring normalization".into();
    }
    for mark in &mut score.beams {
        match mark.event.0.as_str() {
            "ga" => mark.event.0 = "grace id requiring normalization".into(),
            "a" => mark.event.0 = "principal id requiring normalization".into(),
            _ => {}
        }
    }
    let doc = sfs::to_document(&score).unwrap().value;
    let output = scorestream_codec::canonical(&doc);
    let mapped = SfsAdapter.read(&output).unwrap().value;
    let grace = &mapped.graces[0];
    assert_ne!(grace.id, grace.anchor);
    assert_eq!(
        mapped.beams.iter().filter(|b| b.event == grace.id).count(),
        2
    );
    assert_eq!(
        mapped
            .beams
            .iter()
            .filter(|b| b.event == grace.anchor)
            .count(),
        1
    );
    assert_eq!(grace.anchor, mapped.parts[0].measures[0].events[0].id);
}
