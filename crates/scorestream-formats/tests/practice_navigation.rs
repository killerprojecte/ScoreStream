use scorestream_core::{
    fermata_timing::*, notation::*, repeat_traversal::UnclosedFinalEndingPolicy as Ending,
};
use scorestream_formats::sfs::{self, SfsOptions};
const PRACTICE: PracticeDiscoveryPolicy =
    PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1;
const ART: ExplicitArticulationPolicy = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
fn fixture() -> String {
    include_str!("fixtures/navigation-closed.musicxml")
      .replace("</score-part>","<midi-instrument id=\"I1\"><midi-program>1</midi-program></midi-instrument></score-part>")
      .replace("<barline location=\"right\"><ending number=\"2\" type=\"discontinue\"/></barline>","")
      .replace("<note id=\"a\">","<note id=\"g\"><grace/><pitch><step>B</step><octave>3</octave></pitch><voice>1</voice><type>16th</type></note><note id=\"a\">")
      .replace("<note id=\"d\"><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>","<note id=\"d\"><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><fermata/></notations></note>")
}
#[test]
fn explicit_practice_continuation_keeps_default_strict_and_source_unchanged() {
    let xml=fixture().replace("<note id=\"b\"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>","<note id=\"b\"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><articulations><staccato/></articulations></notations></note>");
    let model = scorestream_formats::musicxml::read_with_options(
        &xml,
        &scorestream_formats::musicxml::MusicXmlOptions {
            articulations: scorestream_formats::musicxml::ArticulationProfile::AuthoredMixedV1,
            ..Default::default()
        },
    )
    .unwrap()
    .value;
    let source = sfs::write_with_options(
        &model,
        &SfsOptions {
            unclosed_final_ending: Ending::ContinueToMovementEnd,
        },
    )
    .unwrap()
    .value;
    let doc = scorestream_codec::parse(&source).unwrap();
    let routes = vec![PracticeMidiSelection {
        part: doc.parts[0].id.clone(),
        instrument: doc.parts[0].instrument.clone(),
        bank: 0,
        program_zero_based: 0,
    }];
    assert!(discover_document_practice_joint(&doc, PRACTICE, ART, &routes).is_err());
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
    let discovered = discover_document_practice_joint_with_final_ending(
        &doc,
        PRACTICE,
        ART,
        &routes,
        Ending::ContinueToMovementEnd,
    )
    .unwrap();
    assert_eq!(
        discovered.final_ending_policy(),
        Ending::ContinueToMovementEnd
    );
    assert_eq!(discovered.source(), &doc);
    let d = discovered.discovery();
    assert_eq!(d.graces.occurrences.len(), 2);
    assert_eq!(d.fermatas.occurrences.len(), 1);
    let grace = ExplicitGracePolicy {
        groups: d
            .graces
            .occurrences
            .iter()
            .map(|g| ExplicitGraceGroup {
                principal: g.principal.clone(),
                allocation: GraceAllocation::ExplicitFollowingQuarters {
                    members: vec![GraceQuarterAllocation {
                        event: g.id.clone(),
                        quarters: "1/4".parse().unwrap(),
                    }],
                },
            })
            .collect(),
    };
    let f = &d.fermatas.occurrences[0];
    let holds = ExplicitSynchronizedFermataPolicy {
        groups: vec![ExplicitSynchronizedHoldGroup {
            boundary: f.quarter_end.clone(),
            hold: ExplicitHoldGroup {
                id: "final".into(),
                members: vec![f.id.clone()],
                added_seconds: "1/2".parse().unwrap(),
            },
        }],
    };
    assert!(
        perform_document_practice_joint(
            &doc,
            PRACTICE,
            &routes,
            ART,
            &grace,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &holds
        )
        .is_err()
    );
    let q = perform_document_practice_joint_with_final_ending(
        &doc,
        PRACTICE,
        &routes,
        ART,
        &grace,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &holds,
        Ending::ContinueToMovementEnd,
    )
    .unwrap();
    assert_eq!(q.source(), &doc);
    assert_eq!(q.final_ending_policy(), Ending::ContinueToMovementEnd);
    assert_eq!(q.joint().performed().score.end, "5".parse().unwrap());
    assert!(
        q.joint()
            .performed()
            .diagnostics
            .iter()
            .any(|d| d.code == "practice.navigation_unclosed_final_ending_continues")
    );
    assert_eq!(scorestream_codec::parse(&source).unwrap(), doc);
    assert!(
        discover_document_practice_joint_with_final_ending(
            &doc,
            PRACTICE,
            ART,
            &[],
            Ending::ContinueToMovementEnd
        )
        .is_err()
    );
    let mut invalid = doc.clone();
    invalid.required_extensions.push("unknown.sound@1".into());
    assert!(
        discover_document_practice_joint_with_final_ending(
            &invalid,
            PRACTICE,
            ART,
            &routes,
            Ending::ContinueToMovementEnd
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires the verified local rehearsal source receipts; no audio"]
fn actual_rehearsal_sources_reject_missing_explicit_routes() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(std::env::var("SCORESTREAM_REHEARSAL_WORKSPACE").unwrap());
    for (name, path, hash, ending) in [
        (
            "II",
            "ii-short-contract-evidence/musicxml.sfs",
            "bc8a68e71b144db1d486541dbf78e63e33da158101ad3cf88597cd2de2e7750d",
            Ending::Reject,
        ),
        (
            "III",
            "iii-short-contract-evidence/musicxml.sfs",
            "8e111fdf22f2bf82d891c62028e914172eaa0333ea35cc1678205eb71081c360",
            Ending::Reject,
        ),
        (
            "IV",
            "final-ending-IV.sfs",
            "39044fe2ed9a3d138b67b3c5d5db64794dfb64627d07258bc0854658017f71c9",
            Ending::ContinueToMovementEnd,
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(source.as_bytes())), hash);
        let doc = scorestream_codec::parse(&source).unwrap();
        let errors =
            discover_document_practice_joint_with_final_ending(&doc, PRACTICE, ART, &[], ending)
                .unwrap_err();
        assert_eq!(errors[0].code, "practice.midi_selection");
        println!("{name} {hash}: {}: {}", errors[0].code, errors[0].message);
    }
}
