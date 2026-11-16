use scorestream_formats::{musicxml::MusicXml, sfs::SfsAdapter, ScoreAdapter};
fn source(flag: &str, nested: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m1" number="217"><attributes><divisions>24</divisions><time><beats>3</beats><beat-type>8</beat-type></time></attributes><note id="a"><pitch><step>C</step><octave>4</octave></pitch><duration>36</duration><voice>1</voice></note><direction placement="below"><direction-type><dynamics default-x="-3.4" default-y="-40" relative-y="-25"><p/></dynamics></direction-type><offset{flag}>-20</offset><sound dynamics="44.44">{nested}</sound></direction></measure><measure id="m2" number="218"><direction placement="below"><direction-type><dynamics default-x="-3.62"><ff/></dynamics></direction-type><sound dynamics="106.67"/></direction><note id="b"><pitch><step>D</step><octave>4</octave></pitch><duration>36</duration><voice>1</voice></note></measure></part></score-partwise>"#
    )
}
#[test]
fn separate_authored_boundary_statements_roundtrip_and_require_explicit_performance() {
    for flag in ["", " sound=\"no\""] {
        let model = MusicXml.read(&source(flag, "")).unwrap().value;
        assert!(model.dynamics.is_empty());
        assert!(model.sound_dynamics.is_empty());
        let e = &model.authored_dynamics;
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].measure, "m1");
        assert_eq!(e[1].measure, "m2");
        assert_eq!(e[0].visual_at.to_string(), "2/3");
        assert_eq!(e[0].sounding_at.to_string(), "3/2");
        assert_eq!(e[1].sounding_at, e[0].sounding_at);
        assert_eq!(
            e[0].offset_sound,
            if flag.is_empty() { None } else { Some(false) }
        );
        let sfs = SfsAdapter.write(&model).unwrap().value;
        assert_eq!(SfsAdapter.read(&sfs).unwrap().value, model);
        let xml = MusicXml.write(&model).unwrap().value;
        assert_eq!(MusicXml.read(&xml).unwrap().value, model);
        assert_eq!(xml.matches("<sound dynamics=").count(), 2);
        let err = scorestream_app::playback::plan_source(&sfs).unwrap_err();
        assert!(
            err.contains("authored_dynamics.performance_required"),
            "{err}"
        );
    }
}
#[test]
fn sound_yes_is_not_reinterpreted_as_terminal_and_nested_zero_remains_authored() {
    let model = MusicXml.read(&source(" sound=\"yes\"", "")).unwrap().value;
    assert!(model.authored_dynamics.is_empty());
    assert_eq!(model.sound_dynamics[0].at.to_string(), "2/3");
    let model = MusicXml
        .read(&source(" sound=\"yes\"", "<offset>0</offset>"))
        .unwrap()
        .value;
    assert_eq!(model.authored_dynamics.len(), 2);
    assert_eq!(model.authored_dynamics[0].offset_sound, Some(true));
    assert_eq!(
        model.authored_dynamics[0].sound_offset,
        Some("0".parse().unwrap())
    );
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, model);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value,
        model
    );
}
#[test]
fn malformed_owner_exact_locations_and_wire_fail_without_discarding_values() {
    let model = MusicXml.read(&source("", "")).unwrap().value;
    for mode in 0..8 {
        let mut doc = scorestream_formats::sfs::to_document(&model).unwrap().value;
        if mode == 0 {
            doc.required_extensions.clear();
        }
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m)
                    if m.kind == scorestream_core::authored_dynamics::KIND =>
                {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        let s = |s: &str| scorestream_core::Scalar::String(s.into());
        match mode {
            1 => {
                mark.target = "#missing".into();
            }
            2 => {
                mark.attributes.insert("visual_at".into(), s("3/2"));
            }
            3 => {
                mark.attributes.insert("sounding_at".into(), s("2/3"));
            }
            4 => {
                mark.attributes.insert("cursor".into(), s("-1"));
            }
            5 => {
                mark.attributes
                    .insert("order".into(), scorestream_core::Scalar::Integer(2));
            }
            6 => {
                mark.attributes
                    .insert("silent".into(), scorestream_core::Scalar::Boolean(true));
            }
            7 => {
                mark.attributes.insert("percent".into(), s("-1"));
            }
            _ => {}
        }
        assert!(
            scorestream_core::authored_dynamics::decode(&doc).is_err(),
            "{mode}"
        );
    }
    let mut mixed = model.clone();
    mixed
        .sound_dynamics
        .push(scorestream_core::score::ScoreSoundDynamics {
            at: model.authored_dynamics[0].sounding_at.clone(),
            scope: scorestream_core::score::ScorePartScope {
                part: model.parts[0].id.clone(),
                staff: None,
            },
            percent: "1111/25".parse().unwrap(),
        });
    assert!(SfsAdapter
        .write(&mixed)
        .unwrap_err()
        .diagnostics
        .iter()
        .any(|d| d.feature == "authored_dynamics.mixed_owner"));
    let mut bad = model.clone();
    bad.authored_dynamics[0].offset = None;
    assert!(SfsAdapter.write(&bad).is_err());
}
#[test]
fn final_endpoint_is_retained_and_positive_overrun_is_not_clamped() {
    let source = source("", "");
    let one = source
        .split("<measure id=\"m2\"")
        .next()
        .unwrap()
        .to_owned()
        + "</part></score-partwise>";
    let m = MusicXml.read(&one).unwrap().value;
    assert_eq!(m.authored_dynamics.len(), 1);
    let sfs = SfsAdapter.write(&m).unwrap().value;
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, m);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&m).unwrap().value)
            .unwrap()
            .value,
        m
    );
    assert!(MusicXml
        .read(&one.replace(
            "<sound dynamics=\"44.44\">",
            "<sound dynamics=\"44.44\"><offset>1</offset>"
        ))
        .is_err());
}
#[test]
fn ordered_multiple_dynamic_wrappers_keep_each_cursor_and_shared_source_link() {
    let extra = r#"<direction><direction-type><dynamics><mp/></dynamics></direction-type><sound dynamics="60"/></direction>"#;
    let xml = source("", "").replace("<note id=\"a\">", &format!("{extra}<note id=\"a\">"));
    let m = MusicXml.read(&xml).unwrap().value;
    assert_eq!(m.authored_dynamics.len(), 3);
    assert_eq!(m.authored_dynamics[0].order, 0);
    assert_eq!(m.authored_dynamics[1].order, 1);
    let sfs = SfsAdapter.write(&m).unwrap().value;
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, m);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&m).unwrap().value)
            .unwrap()
            .value,
        m
    );
}
#[test]
fn repeat_mapping_cannot_silently_bypass_authored_boundary_requirement() {
    let xml=source("","").replace("<attributes>","<barline location=\"left\"><repeat direction=\"forward\"/></barline><attributes>").replace("</note></measure></part>","</note><barline location=\"right\"><repeat direction=\"backward\" times=\"2\"/></barline></measure></part>");
    let m = MusicXml.read(&xml).unwrap().value;
    let sfs = SfsAdapter.write(&m).unwrap().value;
    assert!(sfs.contains("scorestream.navigation@1"));
    let error = scorestream_app::playback::plan_source(&sfs).unwrap_err();
    assert!(
        error.contains("authored_dynamics.performance_required"),
        "{error}"
    );
}
