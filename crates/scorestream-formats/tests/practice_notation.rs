use scorestream_core::{Document, Record, Scalar, fermata_timing::*, notation::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, ArticulationProfile, MusicXmlOptions},
    sfs::SfsAdapter,
};
const POLICY: PracticeDiscoveryPolicy =
    PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1;
const ART: ExplicitArticulationPolicy = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
fn source() -> Document {
    let xml=include_str!("fixtures/joint-articulation-playback.musicxml")
 .replace("<part-list>","<part-list><part-group type=\"start\" number=\"1\"><group-symbol>bracket</group-symbol><group-barline>yes</group-barline></part-group>")
 .replace("</part-list>","<part-group type=\"stop\" number=\"1\"/></part-list>")
 .replace("<instrument-name>piano</instrument-name></score-instrument></score-part>","<instrument-name>piano</instrument-name></score-instrument><midi-instrument id=\"ip\"><midi-program>1</midi-program></midi-instrument></score-part>")
 .replace("<score-instrument id=\"ir\"><instrument-name>piano</instrument-name></score-instrument><midi-instrument id=\"ip\">","<score-instrument id=\"ir\"><instrument-name>piano</instrument-name></score-instrument><midi-instrument id=\"ir\">")
 .replace("<note id=\"plain\"><pitch><step>D</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice></note>","<note id=\"plain\"><pitch><step>D</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice><type>quarter</type><dot/><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes><normal-type>eighth</normal-type></time-modification><notations><tuplet type=\"start\"/><tuplet type=\"stop\"/></notations></note>");
    let model = musicxml::read_with_options(
        &xml,
        &MusicXmlOptions {
            articulations: ArticulationProfile::AuthoredMixedV1,
            ..Default::default()
        },
    )
    .unwrap()
    .value;
    scorestream_codec::parse(&SfsAdapter.write(&model).unwrap().value).unwrap()
}
fn routes(doc: &Document) -> Vec<PracticeMidiSelection> {
    doc.parts
        .iter()
        .map(|p| PracticeMidiSelection {
            part: p.id.clone(),
            instrument: p.instrument.clone(),
            bank: 0,
            program_zero_based: 0,
        })
        .collect()
}
fn plan(doc: &Document) -> Result<QualifiedPracticePerformance, Vec<scorestream_core::Diagnostic>> {
    let routes = routes(doc);
    let d = discover_document_practice_joint(doc, POLICY, ART, &routes)?;
    let d = d.discovery();
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
    let holds = ExplicitSynchronizedFermataPolicy {
        groups: vec![ExplicitSynchronizedHoldGroup {
            boundary: "3".parse().unwrap(),
            hold: ExplicitHoldGroup {
                id: "hold".into(),
                members: d
                    .fermatas
                    .occurrences
                    .iter()
                    .map(|f| f.id.clone())
                    .collect(),
                added_seconds: "1/2".parse().unwrap(),
            },
        }],
    };
    perform_document_practice_joint(
        doc,
        POLICY,
        &routes,
        ART,
        &grace,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &holds,
    )
}
#[test]
fn typed_practice_notation_preserves_clock_events_and_source() {
    let doc = source();
    assert!(
        doc.required_extensions
            .contains(&"scorestream.tuplets@1".into())
    );
    assert!(
        doc.required_extensions
            .contains(&"scorestream.part_groups@1".into())
    );
    let mut bare = doc.clone();
    bare.required_extensions
        .retain(|e| e != "scorestream.tuplets@1" && e != "scorestream.part_groups@1");
    for f in &mut bare.frames {
        f.records.retain(|r|!matches!(r,Record::Mark(m) if m.kind==scorestream_core::tuplets::KIND || m.kind==scorestream_core::part_groups::KIND));
    }
    let a = plan(&doc).unwrap();
    let b = plan(&bare).unwrap();
    assert_eq!(a.source(), &doc);
    assert_eq!(
        a.joint().performed().score.events,
        b.joint().performed().score.events
    );
    assert_eq!(
        a.joint().performed().score.tempos,
        b.joint().performed().score.tempos
    );
    assert_eq!(
        a.joint().performed().score.end,
        b.joint().performed().score.end
    );
    assert_eq!(a.joint().timing().holds(), b.joint().timing().holds());
    assert_eq!(a.joint().grace_windows(), b.joint().grace_windows());
    let ta = trace_document_route(&doc).unwrap();
    let tb = trace_document_route(&bare).unwrap();
    assert_eq!(ta.end_quarter, tb.end_quarter);
    assert_eq!(ta.event_occurrences, tb.event_occurrences);
    assert!(discover_document_joint_performance(&doc, ART).is_err());
}
#[test]
fn malformed_or_inconsistent_practice_receipts_still_reject() {
    let doc = source();
    let mut bad = doc.clone();
    for f in &mut bad.frames {
        for r in &mut f.records {
            if let Record::Event(e) = r {
                if e.attributes.contains_key("tm") {
                    e.attributes
                        .insert("tm".into(), Scalar::String("4:2".into()));
                }
            }
        }
    }
    assert!(
        plan(&bad)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "practice.tuplet_clock_evidence")
    );
    for kind in [
        scorestream_core::tuplets::KIND,
        scorestream_core::part_groups::KIND,
    ] {
        let mut bad = doc.clone();
        let m = bad
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                Record::Mark(m) if m.kind == kind => Some(m),
                _ => None,
            })
            .unwrap();
        m.attributes.insert("unknown".into(), Scalar::Boolean(true));
        assert!(plan(&bad).is_err());
    }
    let mut bad = doc.clone();
    bad.required_extensions.push("scorestream.tuplets@2".into());
    assert!(plan(&bad).is_err());
}

fn terminal_source(start:bool)->Document {
    use scorestream_core::{dashes::*,words::WordsText,Frame,SourceRange};
    let mut doc=source();let part=doc.parts[0].id.clone();
    let measure=doc.frames.iter().flat_map(|f|f.records.iter().filter_map(move |r|match r {Record::Measure(m)=>Some((f,m)),_=>None})).find(|(f,m)|m.target==part && &f.time+&m.length==doc.end).unwrap().1;
    let endpoint=DashesEndpoint{words:if start {Some(WordsText{text:"cresc.".into(),font_family:None,font_size:None,font_style:None,font_weight:None,default_x:None,default_y:None,relative_x:None,relative_y:None})}else{None},system:None,part:part.clone(),measure:measure.id.clone(),offset:measure.length.clone(),staff:Some(1),placement:None,kind:if start {DashesType::Start}else{DashesType::Stop},number:Some(1),default_y:None,relative_x:None};
    let mark=encode(&endpoint,part,measure.id.clone(),0);
    doc.required_extensions.push(EXTENSION.into());
    doc.frames.push(Frame{time:doc.end.clone(),records:vec![Record::Mark(mark)],source:SourceRange::default()});doc
}
#[test]
fn exact_terminal_stop_retains_ownership_receipt_without_clock_change() {
    let base=source();let doc=terminal_source(false);let a=plan(&doc).unwrap();let b=plan(&base).unwrap();
    assert_eq!(a.source(),&doc);assert_eq!(a.joint().performed().score,b.joint().performed().score);assert_eq!(a.joint().grace_windows(),b.joint().grace_windows());assert_eq!(a.joint().timing().holds(),b.joint().timing().holds());
    let receipt=&a.terminal_dashes()[0];assert_eq!(a.terminal_dashes().len(),1);assert_eq!(receipt.boundary,doc.end);assert_eq!(receipt.measure_occurrence,1);assert_eq!(receipt.source_endpoint.offset,"2".parse().unwrap());assert_eq!(receipt.source_endpoint.kind,scorestream_core::dashes::DashesType::Stop);
    let discovery=discover_document_practice_joint(&doc,POLICY,ART,&routes(&doc)).unwrap();assert_eq!(discovery.terminal_dashes(),a.terminal_dashes());
    assert!(discover_document_joint_performance(&doc,ART).is_err());
}
#[test]
fn final_opening_unknown_fields_and_other_controls_are_not_admitted() {
    let opening=terminal_source(true);assert!(plan(&opening).unwrap_err().iter().any(|e|e.code=="practice.terminal_dashes"));
    let mut bad=terminal_source(false);if let Record::Mark(m)=&mut bad.frames.last_mut().unwrap().records[0] {m.attributes.insert("unknown".into(),Scalar::Boolean(true));}assert!(plan(&bad).is_err());
    let mut bad=terminal_source(false);if let Record::Mark(m)=&mut bad.frames.last_mut().unwrap().records[0] {m.kind="scorestream.dashes.unknown".into();}assert!(plan(&bad).is_err());
    let mut bad=terminal_source(false);bad.frames.last_mut().unwrap().records.push(Record::Mark(scorestream_core::Mark{target:bad.parts[0].id.clone(),kind:"dynamic".into(),attributes:std::collections::BTreeMap::from([("value".into(),Scalar::String("f".into())),("sound".into(),Scalar::Boolean(false))]),source:Default::default()}));
    assert!(plan(&bad).unwrap_err().iter().any(|e|e.code=="navigation.final_boundary_control"));
}
