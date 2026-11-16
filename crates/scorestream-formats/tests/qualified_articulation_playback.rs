//! Executable synthetic contracts for an explicitly selected authored-articulation
//! performer policy, not a cleaned original score or a full Beethoven import.
use scorestream_app::playback::{
    ExplicitArticulationPolicy, plan_source, plan_source_with_explicit_articulations,
};
use scorestream_core::score::ScoreDocument;
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXmlOptions},
    sfs::SfsAdapter,
};
fn source(xml: &str) -> (ScoreDocument, String) {
    let model = musicxml::read_with_options(
        xml,
        &MusicXmlOptions {
            articulations: ArticulationProfile::AuthoredMixedV1,
            ..Default::default()
        },
    )
    .unwrap()
    .value;
    let source = SfsAdapter.write(&model).unwrap().value;
    let mut restored = SfsAdapter.read(&source).unwrap().value;
    restored.metadata.source.clear();
    assert_eq!(restored, model);
    assert!(
        plan_source(&source)
            .unwrap_err()
            .contains("articulation.performance_unsupported")
    );
    (model, source)
}
fn xml(body: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>Piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\"><measure id=\"m\" number=\"1\"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"
    )
}
fn note(id: &str, tie: &str, notations: &str) -> String {
    let notations = if notations.is_empty() {
        String::new()
    } else {
        format!("<notations>{notations}</notations>")
    };
    format!(
        "<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>{tie}<voice>1</voice><type>quarter</type>{notations}</note>"
    )
}
fn sign(name: &str) -> String {
    format!(
        "<articulations><{name} default-x=\"1.25\" default-y=\"-2.5\" placement=\"above\"/></articulations>"
    )
}
fn play(s: &str) -> Result<scorestream_app::playback::ExplicitArticulationPlayback, String> {
    plan_source_with_explicit_articulations(s, ExplicitArticulationPolicy::SimpleStaccatoAccentV1)
}
#[test]
fn retained_coordinates_and_staccato_use_explicit_half_gate_with_numeric_dynamics() {
    let (model, s) = source(&xml(&format!(
        "<sound dynamics=\"50\"/>{}{}",
        note("a", "", &sign("staccato")),
        note("b", "", "")
    )));
    let original = scorestream_codec::parse(&s).unwrap();
    let p = play(&s).unwrap();
    assert_eq!(p.performance().source(), &original);
    assert_eq!(
        Some(p.performance().authored()),
        model.authored_articulations.as_ref()
    );
    assert_eq!(p.plan().notes.len(), 2);
    assert_eq!(p.plan().notes[0].duration_seconds, 0.25);
    assert_eq!(p.plan().notes[0].velocity, 45);
    assert!((p.plan().notes[1].duration_seconds - 0.46).abs() < 1e-12);
    assert_eq!(p.plan().event_positions[0].end_seconds, 0.5);
    assert_eq!(scorestream_codec::parse(&s).unwrap(), original);
}
#[test]
fn accent_is_an_explicit_velocity_policy_not_an_override_of_numeric_controls() {
    let body = format!("{}{}", note("a", "", &sign("accent")), note("b", "", ""));
    let (_, s) = source(&xml(&body));
    let p = play(&s).unwrap();
    assert_eq!(p.plan().notes[0].velocity, 96);
    assert_eq!(p.plan().notes[1].velocity, 80);
    let (_, controlled) = source(&xml(&format!("<sound dynamics=\"50\"/>{body}")));
    assert!(
        play(&controlled)
            .unwrap_err()
            .contains("attack-changing accents")
    );
}
#[test]
fn mixed_tie_and_articulation_records_preserve_one_attack_and_require_clear_positions() {
    let body = |first: &str, last: &str| {
        format!(
            "{}{}",
            note(
                "a",
                "<tie type=\"start\"/>",
                &format!("<tied type=\"start\"/>{first}")
            ),
            note(
                "b",
                "<tie type=\"stop\"/>",
                &format!("<tied type=\"stop\"/>{last}")
            )
        )
    };
    let (model, s) = source(&xml(&body("", &sign("staccato"))));
    assert!(model.notation_order.is_some());
    let p = play(&s).unwrap();
    assert_eq!(p.plan().notes.len(), 1);
    assert_eq!(p.plan().notes[0].duration_seconds, 0.75);
    assert_eq!(p.plan().event_positions.len(), 2);
    for (first, last) in [
        (sign("staccato"), String::new()),
        (String::new(), sign("accent")),
    ] {
        let (_, s) = source(&xml(&body(&first, &last)));
        assert!(
            play(&s)
                .unwrap_err()
                .contains("articulation.tie_interpretation")
        );
    }
}
#[test]
fn other_signs_duplicate_stacks_and_unselected_hold_policy_never_fall_back() {
    for (notations, expected) in [
        (sign("tenuto"), "sign_interpretation"),
        (
            format!("{}{}", sign("staccato"), sign("staccato")),
            "stacked_interpretation",
        ),
    ] {
        let (_, s) = source(&xml(&format!(
            "{}{}",
            note("a", "", &notations),
            note("b", "", "")
        )));
        assert!(play(&s).unwrap_err().contains(expected));
    }
    let (_, s) = source(&xml(&format!(
        "{}{}",
        note("a", "", &sign("staccato")),
        note("b", "", "<fermata/>")
    )));
    assert!(
        play(&s)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
}
#[test]
fn authored_staccato_is_projected_for_each_real_repeat_visit() {
    let xml = include_str!("fixtures/navigation-performed.musicxml").replace(
        "<type>quarter</type></note>",
        &format!(
            "<type>quarter</type><notations>{}</notations></note>",
            sign("staccato")
        ),
    );
    let (_, s) = source(&xml);
    let p = play(&s).unwrap();
    assert_eq!(p.plan().notes.len(), 6);
    assert_eq!(p.plan().notes[1].written_id, p.plan().notes[3].written_id);
    assert_ne!(p.plan().notes[1].id, p.plan().notes[3].id);
    assert_eq!(p.plan().notes[1].duration_seconds, 0.5);
    assert_eq!(p.plan().notes[3].duration_seconds, 0.5);
}
