use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn xml(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>3</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>{body}</measure></part></score-partwise>"#
    )
}
const CLEF: &str = "<attributes><clef><sign>F</sign><line>4</line></clef></attributes>";
fn note(d: u32) -> String {
    format!("<note><pitch><step>C</step><octave>4</octave></pitch><duration>{d}</duration></note>")
}
fn receipt(input: &str, offset: &str) {
    let score = MusicXml.read(input).unwrap().value;
    assert_eq!(
        score.parts[0].measures[0].clef_changes[0].offset,
        offset.parse().unwrap()
    );
    let sfs = SfsAdapter.write(&score).unwrap().value;
    let restored = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(score.parts, restored.parts);
    let output = MusicXml.write(&restored).unwrap().value;
    assert_eq!(score.parts, MusicXml.read(&output).unwrap().value.parts);
    let mut plain = score.clone();
    plain.parts[0].measures[0].clef_changes.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&sfs).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
}
#[test]
fn exact_offsets_native_lane_and_passive_audio() {
    for (d, offset) in [(1, "1/3"), (3, "1"), (7, "7/3")] {
        receipt(&xml(&format!("{}{CLEF}{}", note(d), note(12 - d))), offset);
    }
}
#[test]
fn closed_cursor_excursion_does_not_split_sustained_event() {
    receipt(
        &xml(&format!(
            "{}<backup><duration>5</duration></backup>{CLEF}<forward><duration>5</duration></forward>",
            note(12)
        )),
        "7/3",
    );
}
#[test]
fn ambiguous_or_other_timed_states_reject() {
    for body in [
        format!("{}<backup><duration>5</duration></backup>{CLEF}", note(12)),
        format!("{}{CLEF}", note(12)),
        format!(
            "{}<attributes><divisions>6</divisions></attributes>{}",
            note(3),
            note(9)
        ),
        format!(
            "{}{}{CLEF}{}",
            note(3),
            "<backup><duration>3</duration></backup>",
            note(12)
        ),
        format!(
            "{}{CLEF}{}",
            note(3),
            note(9).replace("</note>", "<voice>2</voice></note>")
        ),
        format!(
            "{}{}{}",
            note(3),
            CLEF.replace(
                "<line>4</line>",
                "<line>4</line><clef-octave-change>1</clef-octave-change>"
            ),
            note(9)
        ),
    ] {
        assert!(MusicXml.read(&xml(&body)).is_err(), "{body}");
    }
}
#[test]
fn invalid_model_offsets_and_duplicate_states_reject() {
    let mut score = MusicXml
        .read(&xml(&format!("{}{CLEF}{}", note(3), note(9))))
        .unwrap()
        .value;
    let c = score.parts[0].measures[0].clef_changes[0].clone();
    for offset in ["0", "4", "5", "-1"] {
        score.parts[0].measures[0].clef_changes[0].offset = offset.parse().unwrap();
        assert!(MusicXml.write(&score).is_err());
    }
    score.parts[0].measures[0].clef_changes = vec![c.clone(), c];
    assert!(SfsAdapter.write(&score).is_err());
}
#[test]
#[ignore = "requires hash-pinned SCORESTREAM_MXL_FIXTURES; source family projection, not full import"]
fn all_62_original_timing_pitch_rest_clef_projections() {
    let root = std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap();
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/audit_beethoven_timed_clefs.py"
        ))
        .arg(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.len(), 62);
    for row in rows {
        let input = row["xml"].as_str().unwrap();
        let score = MusicXml
            .read(input)
            .unwrap_or_else(|e| panic!("{row}\n{e:?}"))
            .value;
        assert_eq!(score.parts[0].id.0, row["part"].as_str().unwrap());
        let changes = &score.parts[0].measures[0].clef_changes;
        assert_eq!(changes.len(), row["changes"].as_array().unwrap().len());
        for (actual, expected) in changes.iter().zip(row["changes"].as_array().unwrap()) {
            assert_eq!(
                actual.offset,
                expected["offset"].as_str().unwrap().parse().unwrap()
            );
            assert_eq!(actual.clef.line, expected["line"].as_u64().unwrap() as u8);
            assert_eq!(
                format!("{:?}", actual.clef.sign),
                expected["sign"].as_str().unwrap()
            );
        }
        receipt(input, row["changes"][0]["offset"].as_str().unwrap());
    }
}
#[test]
fn multiple_changes_and_tied_continuation_do_not_retrigger() {
    let a = note(6)
        .replace(
            "</note>",
            "<notations><tied type=\"start\"/></notations></note>",
        )
        .replace("</duration>", "</duration><tie type=\"start\"/>");
    let b = note(6)
        .replace(
            "</note>",
            "<notations><tied type=\"stop\"/></notations></note>",
        )
        .replace("</duration>", "</duration><tie type=\"stop\"/>");
    let input = xml(&format!(
        "{a}{CLEF}{b}<backup><duration>3</duration></backup>{CLEF}<forward><duration>3</duration></forward>"
    ));
    receipt(&input, "2");
    let score = MusicXml.read(&input).unwrap().value;
    assert_eq!(score.ties.len(), 1);
    assert_eq!(score.parts[0].measures[0].events.len(), 2);
    let native = SfsAdapter.write(&score).unwrap().value;
    assert_eq!(
        scorestream_app::playback::plan_source(&native)
            .unwrap()
            .notes
            .len(),
        1
    );
    let re = MusicXml
        .read(&MusicXml.write(&score).unwrap().value)
        .unwrap()
        .value;
    assert_eq!(score.ties, re.ties);
}
#[test]
fn native_mixed_timed_state_remains_rejected() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv C4:4 id=n\n.\n@1\n= p.s1 clef=F4\n.\n!end at=4\n";
    assert_eq!(
        SfsAdapter.read(source).unwrap().value.parts[0].measures[0]
            .clef_changes
            .len(),
        1
    );
    for extra in [" key=0", " meter=4/4"] {
        assert!(
            SfsAdapter
                .read(&source.replace("clef=F4", &format!("clef=F4{extra}")))
                .is_err()
        );
    }
}
