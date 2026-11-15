use scorestream_core::{Rational, notation::lower_document};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::MusicXml,
    sfs::{self, SfsAdapter},
};

const CLARINET: &str = include_str!("fixtures/transpose-bb-clarinet.musicxml");
const HORN: &str = include_str!("fixtures/transpose-eb-horn.musicxml");
const PICCOLO: &str = include_str!("fixtures/transpose-piccolo.musicxml");
const BASS: &str = include_str!("fixtures/transpose-contrabass.musicxml");

fn pitches(source: &str) -> Vec<Rational> {
    let doc = scorestream_codec::parse(source).unwrap();
    lower_document(&doc)
        .unwrap()
        .events
        .iter()
        .filter_map(|e| {
            e.sounding_pitch
                .as_ref()
                .map(|p| p + &Rational::from_integer(12))
        })
        .collect()
}

#[test]
fn staff_transposition_sfs_roundtrip_and_inheritance() {
    let source = "!score 1.1 unit=q\n!part p instrument=clarinet staves=1\n!voice v part=p staff=1\n@0\n| p m1 len=1 label=\"1\" implicit=true\n= p.s1 key=-1 transpose_chromatic=-2 transpose_diatonic=-1 transpose_octaves=0\nv A4:1 id=n1\n.\n@1\n| p m2 len=1 label=\"2\" implicit=true\nv A4:1 id=n2\n.\n@2\n| p m3 len=1 label=\"3\" implicit=true\n= p.s1 transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=0\nv A4:1 id=n3\n.\n!end at=3\n";
    let common = SfsAdapter.read(source).unwrap().value;
    assert!(common.parts[0].measures[1].states.is_empty());
    let output = SfsAdapter.write(&common).unwrap().value;
    let mut again = SfsAdapter.read(&output).unwrap().value;
    again.metadata.source.clear();
    assert_eq!(common, again);
    assert_eq!(
        pitches(&output),
        vec![
            Rational::from_integer(67),
            Rational::from_integer(67),
            Rational::from_integer(69)
        ]
    );
    assert_eq!(
        common.parts[0].measures[0].states[0].key.unwrap().fifths,
        -1
    );
}

#[test]
fn source_derived_orchestral_intervals_preserve_written_notes_and_keys() {
    for (xml, sounding, key, chromatic, diatonic, octaves) in [
        (CLARINET, 67, -1, -2, -1, 0),
        (HORN, 67, 0, -9, -5, 0),
        (PICCOLO, 84, 0, 0, 0, 1),
        (BASS, 43, -3, 0, 0, -1),
    ] {
        let common = MusicXml.read(xml).unwrap().value;
        let measure = &common.parts[0].measures[0];
        let state = &measure.states[0];
        let transpose = state.transpose.as_ref().unwrap();
        assert_eq!(transpose.chromatic, Rational::from_integer(chromatic));
        assert_eq!(transpose.diatonic, Some(diatonic));
        assert_eq!(transpose.octaves, octaves);
        assert_eq!(state.key.unwrap().fifths, key);
        let ast = sfs::to_document(&common).unwrap().value;
        let resolved = lower_document(&ast).unwrap();
        assert_eq!(resolved.events[0].written_pitch, measure.events[0].pitch);
        assert_eq!(
            resolved.events[0].sounding_pitch,
            Some(Rational::from_integer(sounding - 12))
        );
        let text = SfsAdapter.write(&common).unwrap().value;
        let mut again = SfsAdapter.read(&text).unwrap().value;
        again.parts[0].name = common.parts[0].name.clone();
        assert_eq!(common, again);
        let written = MusicXml.write(&common).unwrap().value;
        assert_eq!(common, MusicXml.read(&written).unwrap().value);
        assert_eq!(pitches(&text), vec![Rational::from_integer(sounding)]);
        let plan = scorestream_app::playback::plan_source(&text).unwrap();
        assert_eq!(plan.notes.len(), 1);
        assert_eq!(plan.notes[0].midi_note, sounding as f64);
    }
}

fn xml_score(body: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>clarinet</part-name><score-instrument id=\"i\"><instrument-name>clarinet</instrument-name></score-instrument></score-part></part-list><part id=\"p\">{body}</part></score-partwise>"
    )
}
const NOTE: &str =
    "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>";

#[test]
fn xml_measure_boundary_change_and_explicit_zero_reset_are_exact() {
    let xml = xml_score(&format!(
        "<measure number=\"1\" implicit=\"yes\"><attributes><divisions>1</divisions><key><fifths>2</fifths></key><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes>{NOTE}</measure>\
         <measure number=\"2\" implicit=\"yes\">{NOTE}</measure>\
         <measure number=\"3\" implicit=\"yes\"><attributes><transpose><diatonic>0</diatonic><chromatic>0</chromatic></transpose></attributes>{NOTE}</measure>"
    ));
    let common = MusicXml.read(&xml).unwrap().value;
    assert!(common.parts[0].measures[1].states.is_empty());
    assert_eq!(
        common.parts[0].measures[2].states[0]
            .transpose
            .as_ref()
            .unwrap()
            .chromatic,
        Rational::zero()
    );
    let sfs = SfsAdapter.write(&common).unwrap().value;
    assert_eq!(pitches(&sfs), [58, 58, 60].map(Rational::from_integer));
    let written = MusicXml.write(&common).unwrap().value;
    assert_eq!(common, MusicXml.read(&written).unwrap().value);
}

#[test]
fn staff_specific_and_all_staff_transposition_do_not_leak_or_double() {
    let original = include_str!("fixtures/two-staff.musicxml");
    for (number, expected) in [
        (" number=\"2\"", vec![60.0, 63.5, 29.0, 67.0]),
        ("", vec![48.0, 51.5, 29.0, 55.0]),
    ] {
        let xml = original.replacen("</attributes>", &format!("<transpose{number}><diatonic>0</diatonic><chromatic>0</chromatic><octave-change>-1</octave-change></transpose></attributes>"), 1);
        let common = MusicXml.read(&xml).unwrap().value;
        let sfs = SfsAdapter.write(&common).unwrap().value;
        let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
        assert_eq!(
            plan.notes.iter().map(|n| n.midi_note).collect::<Vec<_>>(),
            expected
        );
        let written = MusicXml.write(&common).unwrap().value;
        let again = MusicXml.read(&written).unwrap().value;
        let original_states = &common.parts[0].measures[0].states;
        assert_eq!(original_states, &again.parts[0].measures[0].states);
        assert_eq!(
            original_states
                .iter()
                .filter(|s| s.transpose.is_some())
                .count(),
            if number.is_empty() { 2 } else { 1 }
        );
    }
}

#[test]
fn absent_diatonic_is_preserved_not_invented_for_sfs() {
    let xml = CLARINET.replace("<diatonic>-1</diatonic>", "");
    let common = MusicXml.read(&xml).unwrap().value;
    assert_eq!(
        common.parts[0].measures[0].states[0]
            .transpose
            .as_ref()
            .unwrap()
            .diatonic,
        None
    );
    assert_eq!(
        common,
        MusicXml
            .read(&MusicXml.write(&common).unwrap().value)
            .unwrap()
            .value
    );
    assert!(
        SfsAdapter
            .write(&common)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "transpose.diatonic_unspecified")
    );
}

#[test]
fn fractional_chromatic_is_exact_and_unrepresentable_decimal_is_rejected() {
    let xml = CLARINET.replace("<chromatic>-2</chromatic>", "<chromatic>-2.5</chromatic>");
    let mut common = MusicXml.read(&xml).unwrap().value;
    let sfs = SfsAdapter.write(&common).unwrap().value;
    assert_eq!(pitches(&sfs), vec!["133/2".parse().unwrap()]);
    assert_eq!(
        common,
        MusicXml
            .read(&MusicXml.write(&common).unwrap().value)
            .unwrap()
            .value
    );
    common.parts[0].measures[0].states[0]
        .transpose
        .as_mut()
        .unwrap()
        .chromatic = "1/3".parse().unwrap();
    assert!(
        MusicXml
            .write(&common)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "transpose.chromatic")
    );
    assert!(SfsAdapter.write(&common).is_ok());
}

#[test]
fn malformed_unmapped_and_mid_measure_transposition_fail_without_output() {
    for replacement in [
        "<diatonic>-1</diatonic>",
        "<diatonic>-1</diatonic><chromatic>no</chromatic>",
        "<diatonic>-1.5</diatonic><chromatic>-2</chromatic>",
        "<diatonic>-1</diatonic><chromatic>-2</chromatic><chromatic>-2</chromatic>",
        "<diatonic>-1</diatonic><chromatic>-2</chromatic><octave-change>0.5</octave-change>",
        "<diatonic>-1</diatonic><chromatic>-2</chromatic><double/>",
    ] {
        let xml = xml_score(&format!(
            "<measure number=\"1\" implicit=\"yes\"><attributes><divisions>1</divisions><transpose>{replacement}</transpose></attributes>{NOTE}</measure>"
        ));
        assert!(MusicXml.read(&xml).is_err(), "{replacement}");
    }
    for number in ["0", "2", "-1"] {
        assert!(
            MusicXml
                .read(&CLARINET.replace("<transpose>", &format!("<transpose number=\"{number}\">")))
                .is_err()
        );
    }
    let mid = xml_score(&format!(
        "<measure number=\"1\" implicit=\"yes\"><attributes><divisions>1</divisions></attributes>{NOTE}<attributes><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes>{NOTE}</measure>"
    ));
    assert!(
        MusicXml
            .read(&mid)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "attributes.mid_measure")
    );
    let duplicate = CLARINET.replace(
        "</transpose>",
        "</transpose><transpose><chromatic>0</chromatic></transpose>",
    );
    assert!(
        MusicXml
            .read(&duplicate)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "attributes.conflict")
    );
}

#[test]
fn sfs_scope_and_mid_measure_limits_remain_explicit() {
    let source = "!score 1.1 unit=q\n!part p instrument=clarinet staves=1\n!voice v part=p staff=1\n@0\n| p m len=2 implicit=true\nv C4:2 id=n\n.\n@1\n= p.s1 transpose_chromatic=-2 transpose_diatonic=-1 transpose_octaves=0\n.\n!end at=2\n";
    assert!(
        SfsAdapter
            .read(source)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "state_inside_measure")
    );
    assert!(
        SfsAdapter
            .read(&source.replace("@1\n= p.s1", "@1\n= p"))
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "state_scope")
    );
    assert!(
        SfsAdapter
            .read(&source.replace(" transpose_diatonic=-1", ""))
            .is_err()
    );
}

#[test]
fn chromatic_export_obeys_the_same_numeric_precision_as_import() {
    let mut common = MusicXml.read(CLARINET).unwrap().value;
    for literal in [
        format!("1{}", "0".repeat(64)),
        format!("-1{}", "0".repeat(63)),
    ] {
        common.parts[0].measures[0].states[0]
            .transpose
            .as_mut()
            .unwrap()
            .chromatic = literal.parse().unwrap();
        assert!(SfsAdapter.write(&common).is_ok());
        assert!(
            MusicXml
                .write(&common)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "transpose.chromatic")
        );
    }
    common.parts[0].measures[0].states[0]
        .transpose
        .as_mut()
        .unwrap()
        .chromatic = format!("1{}", "0".repeat(63)).parse().unwrap();
    let xml = MusicXml.write(&common).unwrap().value;
    assert_eq!(common, MusicXml.read(&xml).unwrap().value);
}
