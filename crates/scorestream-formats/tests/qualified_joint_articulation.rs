use scorestream_app::playback::{
    self, ExplicitArticulationPolicy, FermataSoundPolicy, GraceSoundPolicy,
};
use scorestream_core::{
    Rational,
    fermata_timing::{
        ExplicitHoldGroup, ExplicitSynchronizedFermataPolicy, ExplicitSynchronizedHoldGroup,
    },
    notation::{ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation, GraceQuarterAllocation},
};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXmlOptions},
    sfs::SfsAdapter,
};
const XML: &str = include_str!("fixtures/joint-articulation-playback.musicxml");
const POLICY: ExplicitArticulationPolicy = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn source(xml: &str) -> String {
    let model = musicxml::read_with_options(
        xml,
        &MusicXmlOptions {
            articulations: ArticulationProfile::AuthoredMixedV1,
            ..Default::default()
        },
    )
    .unwrap()
    .value;
    let s = SfsAdapter.write(&model).unwrap().value;
    let mut restored = SfsAdapter.read(&s).unwrap().value;
    restored.metadata.source.clear();
    assert_eq!(model, restored);
    s
}
fn policies(s: &str) -> (ExplicitGracePolicy, ExplicitSynchronizedFermataPolicy) {
    let d = playback::discover_source_joint_performance(s, POLICY).unwrap();
    let g = &d.graces.occurrences[0];
    (
        ExplicitGracePolicy {
            groups: vec![ExplicitGraceGroup {
                principal: g.principal.clone(),
                allocation: GraceAllocation::ExplicitFollowingQuarters {
                    members: vec![GraceQuarterAllocation {
                        event: g.id.clone(),
                        quarters: r("1/4"),
                    }],
                },
            }],
        },
        ExplicitSynchronizedFermataPolicy {
            groups: vec![ExplicitSynchronizedHoldGroup {
                hold: ExplicitHoldGroup {
                    id: "chosen".into(),
                    members: d.fermatas.occurrences.into_iter().map(|f| f.id).collect(),
                    added_seconds: r("1"),
                },
                boundary: r("3"),
            }],
        },
    )
}
fn play(s: &str) -> Result<playback::ExplicitCombinedPlayback, String> {
    let (g, f) = policies(s);
    playback::plan_source_with_explicit_joint_policies(
        s,
        POLICY,
        &g,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &f,
        FermataSoundPolicy::SustainSelectedAnchors,
    )
}
#[test]
fn separate_articulation_measure_combines_with_short_rest_hold_and_exact_graces() {
    let s = source(XML);
    let original = scorestream_codec::parse(&s).unwrap();
    let p = play(&s).unwrap();
    assert_eq!(p.articulations().unwrap().source(), &original);
    assert_eq!(p.articulations().unwrap().policy(), POLICY);
    let authored = p.articulations().unwrap().authored();
    assert_eq!(
        authored.events[0].notations[0].groups[0].items[0]
            .print_style
            .default_x,
        Some(r("5/4"))
    );
    let notes = &p.plan().notes;
    assert_eq!(notes.len(), 5);
    assert_eq!(notes[0].duration_seconds, 0.25);
    assert_eq!(notes[2].start_seconds, 1.0);
    assert_eq!(notes[2].duration_seconds, 1.5);
    assert_eq!(notes[3].start_seconds, 2.5);
    assert_eq!(notes[3].duration_seconds, 0.125);
    assert_eq!(notes[4].start_seconds, 2.625);
    assert_eq!(p.plan().duration_seconds, 3.0);
    assert_eq!(p.timing().holds().len(), 1);
    assert_eq!(p.timing().holds()[0].seconds_end, r("5/2"));
    assert_eq!(
        p.plan()
            .event_positions
            .iter()
            .find(|m| m.id == "short-rest")
            .unwrap()
            .end_seconds,
        1.25
    );
    assert_eq!(p.grace_windows()[0].seconds_start, r("5/2"));
    assert_eq!(scorestream_codec::parse(&s).unwrap(), original);
    let (g, f) = policies(&s);
    assert!(
        playback::plan_source_with_explicit_graces_and_synchronized_fermatas(
            &s,
            &g,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &f,
            FermataSoundPolicy::SustainSelectedAnchors
        )
        .unwrap_err()
        .contains("articulation.performance_unsupported")
    );
}
#[test]
fn numeric_staccato_and_separate_accent_keep_existing_sound_policies() {
    let numeric = XML.replacen(
        "<note id=\"art\">",
        "<sound dynamics=\"50\"/><note id=\"art\">",
        1,
    );
    let p = play(&source(&numeric)).unwrap();
    assert_eq!(p.plan().notes[0].velocity, 45);
    assert_eq!(p.plan().notes[0].duration_seconds, 0.25);
    let accented = XML.replace("<staccato ", "<accent ");
    assert_eq!(
        play(&source(&accented)).unwrap().plan().notes[0].velocity,
        96
    );
    assert!(
        play(&source(&numeric.replace("<staccato ", "<accent ")))
            .unwrap_err()
            .contains("attack-changing accents")
    );
}
#[test]
fn selected_hold_and_affected_grace_principal_do_not_accept_articulation_precedence() {
    let mark = "<articulations><staccato default-x=\"1.25\" default-y=\"-2.5\"/></articulations>";
    let held = XML.replace(
        "<fermata type=\"upright\"/>",
        &format!("<fermata type=\"upright\"/>{mark}"),
    );
    assert!(
        play(&source(&held))
            .unwrap_err()
            .contains("articulation.hold_interpretation")
    );
    let principal = XML.replace("<step>G</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note>", &format!("<step>G</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice><notations>{mark}</notations></note>"));
    assert!(
        play(&source(&principal))
            .unwrap_err()
            .contains("grace.articulation_unsupported")
    );
}
#[test]
fn tie_tail_articulation_away_from_holds_works_but_articulated_held_chain_rejects() {
    let mark = "<articulations><staccato default-x=\"1.25\" default-y=\"-2.5\"/></articulations>";
    let first = "<note id=\"art\"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice>";
    let plain = "<note id=\"plain\"><pitch><step>D</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note>";
    let independent = XML.replace(mark, "<tied type=\"start\"/>")
        .replace(first, &first.replace("<voice>", "<tie type=\"start\"/><voice>"))
        .replace(plain, &format!("<note id=\"plain\"><pitch><step>C</step><octave>4</octave></pitch><duration>8</duration><tie type=\"stop\"/><voice>1</voice><notations><tied type=\"stop\"/>{mark}</notations></note>"));
    let p = play(&source(&independent)).unwrap();
    assert_eq!(p.plan().notes.len(), 4);
    assert_eq!(p.plan().notes[0].duration_seconds, 0.75);
    assert_eq!(p.plan().notes[1].start_seconds, 1.0);
    assert_eq!(p.plan().notes[1].duration_seconds, 1.5);
    let cross_hold = XML.replace(plain, &format!("<note id=\"plain\"><pitch><step>E</step><octave>4</octave></pitch><duration>8</duration><tie type=\"start\"/><voice>1</voice><notations><tied type=\"start\"/>{}</notations></note>", mark.replace("staccato", "accent")))
        .replace("<note id=\"held\"><pitch><step>E</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice><notations>", "<note id=\"held\"><pitch><step>E</step><octave>4</octave></pitch><duration>8</duration><tie type=\"stop\"/><voice>1</voice><notations><tied type=\"stop\"/>");
    assert!(
        play(&source(&cross_hold))
            .unwrap_err()
            .contains("articulation.hold_interpretation")
    );
}
#[test]
fn explicit_native_pizzicato_receipts_use_the_qualified_joint_clock() {
    let xml=XML.replace("<instrument-name>piano</instrument-name>","<instrument-name>violin</instrument-name>")
        .replace("<note id=\"g\">","<direction><direction-type><words>pizz.</words></direction-type><sound pizzicato=\"yes\"/></direction><note id=\"g\">");
    let s=source(&xml);let (g,f)=policies(&s);
    assert!(play(&s).unwrap_err().contains("pizzicato.not_realized"));
    let p=playback::plan_source_with_joint_policies_and_pizzicato(&s,POLICY,&g,GraceSoundPolicy::FullGraceThenGatedRemainder,&f,FermataSoundPolicy::SustainSelectedAnchors,playback::PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation).unwrap();
    assert_eq!(p.pizzicato_attacks().len(),2);
    let baseline=play(&source(&xml.replace("pizzicato=\"yes\"","pizzicato=\"no\""))).unwrap();
    assert_eq!(p.plan().notes,baseline.plan().notes);assert_eq!(p.plan().duration_seconds,3.0);
    assert_eq!(p.plan().notes[3].start_seconds,2.5);assert_eq!(p.plan().notes[4].start_seconds,2.625);
}

#[test]
fn zero_authored_grace_uses_explicit_empty_policy_without_dummy_source_data() {
    let xml=XML.replace("<note id=\"g\"><grace slash=\"yes\"/><pitch><step>F</step><octave>4</octave></pitch><voice>1</voice><type>16th</type></note>", "");
    let s=source(&xml);assert!(!s.contains("!requires scorestream.grace@1"));
    let original=scorestream_codec::parse(&s).unwrap();
    let d=playback::discover_source_joint_performance(&s,POLICY).unwrap();assert!(d.graces.occurrences.is_empty());assert_eq!(d.fermatas.occurrences.len(),2);
    let (_,f)=policies(&source(XML));let empty=ExplicitGracePolicy {groups:vec![]};
    let p=playback::plan_source_with_explicit_joint_policies(&s,POLICY,&empty,GraceSoundPolicy::FullGraceThenGatedRemainder,&f,FermataSoundPolicy::SustainSelectedAnchors).unwrap();
    assert_eq!(p.plan().notes.len(),4);assert_eq!(p.plan().duration_seconds,3.0);assert!(p.grace_windows().is_empty());assert_eq!(scorestream_codec::parse(&s).unwrap(),original);
    let (nonempty,_)=policies(&source(XML));assert!(playback::plan_source_with_explicit_joint_policies(&s,POLICY,&nonempty,GraceSoundPolicy::FullGraceThenGatedRemainder,&f,FermataSoundPolicy::SustainSelectedAnchors).is_err());
    let trace=scorestream_core::notation::trace_document_route(&original).unwrap();assert_eq!(trace.written_graces,0);assert_eq!(trace.fermatas.len(),2);assert_eq!(trace.fermatas[0].end,r("3"));
}
#[test]
fn each_passive_family_is_typed_and_does_not_change_joint_notes() {
    let xml=XML.replace("<part-list>","<identification><encoding><software>fixture</software></encoding></identification><defaults><scaling><millimeters>7</millimeters><tenths>40</tenths></scaling></defaults><credit><credit-words>Fixture</credit-words></credit><part-list>")
        .replace("<part-name>Piano</part-name>","<part-name>Piano</part-name><part-abbreviation>Pno.</part-abbreviation>")
        .replace("<measure id=\"p1\" number=\"1\">","<measure id=\"p1\" number=\"1\"><print new-page=\"yes\"/>")
        .replace("<note id=\"art\">","<direction><direction-type><metronome parentheses=\"yes\"><beat-unit>quarter</beat-unit><per-minute>72</per-minute></metronome></direction-type></direction><note id=\"art\" default-x=\"10\">")
        .replacen("</notations></note>","</notations><lyric><syllabic>single</syllabic><text>la</text></lyric></note>",1);
    let model=musicxml::read_with_options(&xml,&MusicXmlOptions {articulations:ArticulationProfile::AuthoredMixedV1,geometry:musicxml::GeometryPolicy::PreserveNoteAndMeasureCoordinates,..Default::default()}).unwrap().value;
    let s=SfsAdapter.write(&model).unwrap().value;
    for family in ["provenance","defaults","credits","part_labels","print","metronome_display","geometry","lyrics"] {
        assert!(s.contains(&format!("!requires scorestream.{family}@1")),"{family}");
    }
    let p=play(&s).unwrap();let baseline=play(&source(XML)).unwrap();assert_eq!(p.plan().notes,baseline.plan().notes);assert_eq!(p.plan().routes,baseline.plan().routes);
    let mut malformed=scorestream_codec::parse(&s).unwrap();
    for f in &mut malformed.frames {for rec in &mut f.records {if let scorestream_core::Record::Mark(m)=rec {if m.kind=="scorestream.credit" {m.attributes.insert("unknown".into(),scorestream_core::Scalar::Boolean(true));}}}}
    assert!(scorestream_core::notation::discover_document_joint_performance(&malformed,POLICY).is_err());
}
#[test]
fn route_trace_keeps_sound_intent_without_authorizing_playback() {
    let xml=XML.replace("<note id=\"art\">","<direction><direction-type><wedge type=\"crescendo\" number=\"1\"/></direction-type></direction><note id=\"art\">")
        .replace("<note id=\"plain\">","<direction><direction-type><wedge type=\"stop\" number=\"1\"/></direction-type></direction><note id=\"plain\">");
    let s=source(&xml);let original=scorestream_codec::parse(&s).unwrap();
    let err=playback::discover_source_joint_performance(&s,POLICY).unwrap_err();assert!(err.contains("performance.unresolved_families"));assert!(err.contains("scorestream.wedge@1"));
    let trace=scorestream_core::notation::trace_document_route(&original).unwrap();assert_eq!(trace.measure_visits,2);assert_eq!(trace.fermatas.len(),2);assert!(trace.required_sound_review.iter().any(|e|e=="scorestream.wedge@1"));assert_eq!(trace.fermatas[0].event.0,"occ1:held");assert_eq!(scorestream_codec::parse(&s).unwrap(),original);
    let mut invalid=original.clone();invalid.required_extensions.push("unknown.sound@1".into());assert!(scorestream_core::notation::trace_document_route(&invalid).is_err());
}
#[test]
fn nonplayable_trace_matches_qualified_repeat_occurrence_coordinates() {
    let xml=XML.replacen("<measure id=\"p1\" number=\"1\">","<measure id=\"p1\" number=\"1\"><barline location=\"left\"><repeat direction=\"forward\"/></barline>",1)
        .replacen(" </measure>\n</part>","<barline location=\"right\"><repeat direction=\"backward\" times=\"2\"/></barline> </measure>\n</part>",1);
    let s=source(&xml);let doc=scorestream_codec::parse(&s).unwrap();let trace=scorestream_core::notation::trace_document_route(&doc).unwrap();let q=playback::discover_source_joint_performance(&s,POLICY).unwrap();
    assert_eq!(trace.measure_visits,4);assert_eq!(trace.event_occurrences,16);assert_eq!(trace.fermatas.len(),4);
    let a:std::collections::BTreeMap<_,_>=trace.fermatas.iter().map(|f|(&f.event,(&f.start,&f.end))).collect();
    let b:std::collections::BTreeMap<_,_>=q.fermatas.occurrences.iter().map(|f|(&f.id,(&f.quarter_start,&f.quarter_end))).collect();assert_eq!(a,b);
}
