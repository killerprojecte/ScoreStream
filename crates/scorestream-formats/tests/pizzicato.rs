use scorestream_core::{Rational, words::PizzicatoMode};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Violin</part-name><score-instrument id="i"><instrument-name>violin</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>2</divisions></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn note() -> &'static str {
    r#"<note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>"#
}
fn control(mode: &str, offset: &str) -> String {
    format!(
        r#"<direction><direction-type><words>arco</words></direction-type><offset sound="no">0</offset><sound pizzicato="{mode}"><offset>{offset}</offset></sound></direction>"#
    )
}
fn wire(xml: &str) -> String {
    let model = MusicXml.read(xml).unwrap().value;
    let s = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(SfsAdapter.read(&s).unwrap().value, model);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value,
        model
    );
    s
}
#[test]
fn actual_notes_retain_independent_state_and_text_without_phantom_events() {
    let xml = score(&(control("yes", "0") + note()));
    let model = MusicXml.read(&xml).unwrap().value;
    assert_eq!(model.words[0].words.text, "arco");
    assert_eq!(
        model.words[0].pizzicato.as_ref().unwrap().mode,
        PizzicatoMode::Pizzicato
    );
    assert_eq!(model.parts[0].measures[0].events.len(), 1);
    let s = wire(&xml);
    assert!(s.contains("scorestream.pizzicato@1"));
    assert!(
        scorestream_app::playback::plan_source(&s)
            .unwrap_err()
            .contains("pizzicato.not_realized")
    );
    let s = wire(&score(&(control("no", "0") + note())));
    assert_eq!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .notes
            .len(),
        1
    );
}
#[test]
fn sound_offset_is_exact_and_does_not_move_words() {
    let xml = score(&(control("yes", "1") + note()));
    let mut model = MusicXml.read(&xml).unwrap().value;
    let w = &mut model.words[0];
    assert_eq!(w.offset, Rational::zero());
    assert_eq!(w.pizzicato.as_ref().unwrap().at, "1/2".parse().unwrap());
    w.pizzicato.as_mut().unwrap().at = "1/3".parse().unwrap();
    let xml = MusicXml.write(&model).unwrap().value;
    assert_eq!(MusicXml.read(&xml).unwrap().value, model);
    let s = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .notes
            .len(),
        1
    ); // attack precedes yes
}
#[test]
fn rest_only_state_and_terminal_control_do_not_block_notes() {
    let rest = r#"<note><rest/><duration>2</duration><voice>1</voice></note>"#;
    let s = wire(&score(
        &(control("yes", "0") + rest + &control("no", "0") + note()),
    ));
    assert_eq!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .notes
            .len(),
        1
    );
    let s = wire(&score(&(note().to_owned() + &control("yes", "0"))));
    assert_eq!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .notes
            .len(),
        1
    );
}
#[test]
fn closed_authorization_rejects_unknown_modes_owners_and_conflicts() {
    for mode in ["true", "false", "Yes", "1", ""] {
        assert!(
            MusicXml
                .read(&score(&(control(mode, "0") + note())))
                .is_err()
        );
    }
    assert!(
        MusicXml
            .read(&score(&(r#"<sound pizzicato="yes"/>"#.to_owned() + note())))
            .is_err()
    );
    assert!(
        MusicXml
            .read(&score(
                &(control("yes", "0")
                    .replace("pizzicato=\"yes\"", "pizzicato=\"yes\" tempo=\"100\"")
                    + note())
            ))
            .is_err()
    );
    assert!(
        MusicXml
            .read(&score(
                &(control("yes", "0") + &control("no", "0") + note())
            ))
            .is_err()
    );
    let s = wire(&score(&(control("no", "0") + note())));
    assert!(
        SfsAdapter
            .read(&s.replace("scorestream.pizzicato@1", "scorestream.pizzicato@2"))
            .is_err()
    );
}
#[test]
fn text_alone_does_not_authorize_sound() {
    let xml = score(
        &(r#"<direction><direction-type><words>pizz.</words></direction-type></direction>"#
            .to_owned()
            + note()),
    );
    let model = MusicXml.read(&xml).unwrap().value;
    assert!(model.words[0].pizzicato.is_none());
    assert_eq!(
        scorestream_app::playback::plan_source(&wire(&xml))
            .unwrap()
            .notes
            .len(),
        1
    );
}
#[test]
fn repeat_restores_unspecified_before_prior_pass_pizzicato() {
    let xml = score(
        &(r#"<barline location="left"><repeat direction="forward"/></barline>"#.to_owned()
            + note()
            + r#"</measure><measure id="m2" number="2">"#
            + &control("yes", "0")
            + r#"<note><rest/><duration>2</duration><voice>1</voice></note><barline location="right"><repeat direction="backward"/></barline>"#),
    );
    let xml=xml.replace("<attributes><divisions>2</divisions></attributes><barline location=\"left\"><repeat direction=\"forward\"/></barline>", "<barline location=\"left\"><repeat direction=\"forward\"/></barline><attributes><divisions>2</divisions></attributes>");
    let s = wire(&xml);
    assert_eq!(
        scorestream_app::playback::plan_source(&s)
            .unwrap()
            .notes
            .len(),
        2
    );
    let with_notes = xml.replace("<note><rest/><duration>2</duration><voice>1</voice></note>", "<note id=\"plucked\"><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>");
    let p = scorestream_app::playback::plan_source_with_pizzicato(&wire(&with_notes), scorestream_app::playback::PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation).unwrap();
    assert_eq!(p.plan().notes.len(),4); assert_eq!(p.pizzicato_attacks().len(),2);
    let arco = xml.replace("<note id=\"n\">", &(control("no", "0") + "<note id=\"n\">"));
    assert_eq!(scorestream_app::playback::plan_source(&wire(&arco)).unwrap().notes.len(), 2);
    let terminal = xml.replace(&control("yes", "0"), "").replace("<barline location=\"right\">", &(control("yes", "0") + "<barline location=\"right\">"));
    assert_eq!(scorestream_app::playback::plan_source(&wire(&terminal)).unwrap().notes.len(), 2);

}
#[test]
fn separate_tempo_numeric_dynamics_and_part_scope_are_unchanged() {
    let xml = score(&(control("no", "0") + r#"<sound tempo="60" dynamics="50"/>"# + note()));
    let model = MusicXml.read(&xml).unwrap().value;
    assert_eq!(model.tempos.len(), 1);
    assert_eq!(model.sound_dynamics.len(), 1);
    let s = wire(&xml);
    let plan = scorestream_app::playback::plan_source(&s).unwrap();
    assert_eq!(plan.notes.len(), 1);
    let mut plain = model.clone();
    plain.words.clear();
    let baseline =
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap();
    assert_eq!(plan.notes, baseline.notes);
    let xml = score(
        &(control("yes", "0") + r#"<note><rest/><duration>2</duration><voice>1</voice></note>"#),
    );
    let xml=xml.replace("</part-list>",r#"<score-part id="q"><part-name>Cello</part-name><score-instrument id="j"><instrument-name>cello</instrument-name></score-instrument></score-part></part-list>"#).replace("</score-partwise>",&format!(r#"<part id="q"><measure id="mq" number="1"><attributes><divisions>2</divisions></attributes>{}</measure></part></score-partwise>"#,note().replace("id=\"n\"","id=\"other\"")));
    assert_eq!(
        scorestream_app::playback::plan_source(&wire(&xml))
            .unwrap()
            .notes
            .len(),
        1
    );
}
#[test]
fn native_capability_and_payload_are_required_together() {
    let s = wire(&score(&(control("no", "0") + note())));
    let mut doc = scorestream_codec::parse(&s).unwrap();
    doc.required_extensions
        .retain(|e| e != "scorestream.pizzicato@1");
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == scorestream_core::Severity::Error)
    );
    let mut doc = scorestream_codec::parse(&s).unwrap();
    for f in &mut doc.frames {
        for r in &mut f.records {
            if let scorestream_core::Record::Mark(m) = r {
                if m.kind == "scorestream.words" {
                    m.attributes.remove("pizzicato");
                }
            }
        }
    }
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == scorestream_core::Severity::Error)
    );
}
