use scorestream_core::part_groups::{GroupPhase, GroupSymbol};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn xml(boundaries: [&str; 4]) -> String {
    let mut declarations = String::new();
    let mut bodies = String::new();
    for i in 1..=3 {
        declarations.push_str(boundaries[i - 1]);
        declarations.push_str(&format!("<score-part id=\"P{i}\"><part-name>P{i}</part-name><score-instrument id=\"i{i}\"><instrument-name>piano</instrument-name></score-instrument></score-part>"));
        bodies.push_str(&format!("<part id=\"P{i}\"><measure number=\"1\"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id=\"n{i}\"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part>"));
    }
    declarations.push_str(boundaries[3]);
    format!(
        "<score-partwise version=\"4.0\"><part-list>{declarations}</part-list>{bodies}</score-partwise>"
    )
}
fn roundtrip(source: &str) -> scorestream_core::score::ScoreDocument {
    let imported = MusicXml.read(source).unwrap();
    let score = imported.value;
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "part_groups.authored")
    );
    let native = SfsAdapter.write(&score).unwrap().value;
    let recovered = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(score.part_groups, recovered.part_groups);
    let output = MusicXml.write(&recovered).unwrap().value;
    assert_eq!(
        score.part_groups,
        MusicXml.read(&output).unwrap().value.part_groups
    );
    let mut plain = score.clone();
    plain.part_groups.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&native).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    score
}
#[test]
fn ordered_overlapping_boundaries_and_optional_authored_fields() {
    let source = xml([
        "<part-group type=\"start\"><group-symbol>bracket</group-symbol><group-barline>yes</group-barline></part-group>",
        "<part-group type=\"start\" number=\"A B\"><group-barline>no</group-barline></part-group>",
        "<part-group type=\"stop\" number=\"1\"/>",
        "<part-group type=\"stop\" number=\"A B\"/>",
    ]);
    let s = roundtrip(&source);
    assert_eq!(
        s.part_groups
            .iter()
            .map(|g| g.before_part.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("P1"), Some("P2"), Some("P3"), None]
    );
    assert_eq!(s.part_groups[0].number, None);
    assert_eq!(s.part_groups[2].number.as_deref(), Some("1"));
    assert_eq!(s.part_groups[0].symbol, Some(GroupSymbol::Bracket));
    assert_eq!(s.part_groups[1].barline, Some(false));
}
#[test]
fn same_boundary_order_and_unresolved_sequences_are_never_repaired() {
    let s = roundtrip(&xml([
        "<part-group type=\"start\" number=\"\"/>",
        "<part-group type=\"stop\" number=\"unmatched\"/><part-group type=\"start\" number=\"1\"/>",
        "",
        "",
    ]));
    assert_eq!(s.part_groups.len(), 3);
    assert_eq!(s.part_groups[0].number.as_deref(), Some(""));
    assert_eq!(s.part_groups[1].phase, GroupPhase::Stop);
    assert_eq!(s.part_groups[2].phase, GroupPhase::Start);
    assert_eq!(s.part_groups[1].before_part, s.part_groups[2].before_part);
}
#[test]
fn unsupported_presentation_is_atomic_and_invalid_native_is_rejected() {
    for g in [
        "<part-group type=\"start\"><group-name>Name</group-name></part-group>",
        "<part-group type=\"start\"><group-symbol>brace</group-symbol></part-group>",
        "<part-group type=\"start\"><group-barline>Mensurstrich</group-barline></part-group>",
        "<part-group type=\"start\"><group-symbol color=\"#000000\">bracket</group-symbol></part-group>",
        "<part-group type=\"start\"><group-barline>yes</group-barline><group-symbol>bracket</group-symbol></part-group>",
        "<part-group type=\"stop\"><group-symbol>bracket</group-symbol></part-group>",
        "<part-group type=\"start\" number=\" leading\"/>",
        "<part-group type=\"start\" number=\"a  b\"/>",
        "<part-group type=\"continue\"/>",
    ] {
        assert!(MusicXml.read(&xml([g, "", "", ""])).is_err(), "{g}");
    }
    assert!(
        MusicXml
            .read(&xml(["", "", "", "<part-group type=\"start\"/>"]))
            .is_err()
    );
    let s = roundtrip(&xml([
        "<part-group type=\"start\" number=\"1\"><group-symbol>bracket</group-symbol></part-group>",
        "",
        "",
        "<part-group type=\"stop\" number=\"1\"/>",
    ]));
    let n = SfsAdapter.write(&s).unwrap().value;
    for bad in [
        n.replace("scorestream.part_groups@1", "scorestream.unknown@1"),
        n.replace("index=0", "index=2"),
        n.replace("before_part=P1", "before_part=missing"),
        n.replace("symbol=bracket", "symbol=brace"),
        n.replace("phase=stop", "phase=stop barline=true"),
        n.replace(
            "+ score scorestream.part_group",
            "+ P1 scorestream.part_group",
        ),
    ] {
        assert_ne!(bad, n);
        assert!(SfsAdapter.read(&bad).is_err(), "{bad}");
    }
    let mut bad = s;
    bad.part_groups.swap(0, 1);
    assert!(SfsAdapter.write(&bad).is_err());
}
#[test]
fn direct_wire_validation_checks_time_zero_complete_score_and_closed_fields() {
    use scorestream_core::{DocumentKind, Record, Scalar};
    let s = MusicXml
        .read(&xml([
            "<part-group type=\"start\"/>",
            "",
            "",
            "<part-group type=\"stop\"/>",
        ]))
        .unwrap()
        .value;
    let doc = scorestream_formats::sfs::to_document(&s).unwrap().value;
    assert_eq!(
        scorestream_core::part_groups::decode(&doc).unwrap(),
        s.part_groups
    );
    let mut late = doc.clone();
    for f in &mut late.frames {
        f.time = "1".parse().unwrap();
    }
    assert!(scorestream_core::part_groups::decode(&late).is_err());
    let mut fragment = doc.clone();
    fragment.document_kind = DocumentKind::Fragment;
    assert!(scorestream_core::part_groups::decode(&fragment).is_err());
    let mut unknown = doc.clone();
    for f in &mut unknown.frames {
        for r in &mut f.records {
            if let Record::Mark(m) = r {
                if m.kind == scorestream_core::part_groups::KIND {
                    m.attributes.insert("unknown".into(), Scalar::Boolean(true));
                }
            }
        }
    }
    assert!(scorestream_core::part_groups::decode(&unknown).is_err());
    let mut missing = doc;
    missing.frames.iter_mut().for_each(|f| {
        f.records
            .retain(|r| !matches!(r,Record::Mark(m)if m.kind==scorestream_core::part_groups::KIND))
    });
    assert!(scorestream_core::part_groups::decode(&missing).is_err());
}
