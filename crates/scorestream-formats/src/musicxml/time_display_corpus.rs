//! Exact time-signature family receipt; intentionally not complete original import.
use super::{MusicXml, xml};
use crate::{MappingReport, ScoreAdapter, sfs::SfsAdapter};
use scorestream_core::{Rational, score::TimeSymbol};
use std::{collections::BTreeMap, io::Read};
#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned originals"]
fn original_69_symbols_preserve_part_measure_staff_onset_and_meter() {
    let root = std::path::PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
    let hashes = [
        "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
        "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
        "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
        "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
    ];
    let mut counts = [0; 4];
    let mut signs = [0; 2];
    for movement in 1..=4 {
        let bytes =
            std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
        let archive = crate::mxl::open(
            std::fs::File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
        )
        .unwrap();
        let mut paired = vec![];
        archive.into_reader().read_to_end(&mut paired).unwrap();
        assert_eq!(bytes, paired);
        let mut bodies = BTreeMap::<String, String>::new();
        let mut meters = BTreeMap::<String, (u32, u32)>::new();
        let mut divisions = BTreeMap::<String, Rational>::new();
        let mut starts = BTreeMap::<String, Rational>::new();
        let mut expected = vec![];
        let scan=xml::walk(bytes.as_slice(),|m,part,phase|{
   if phase!=xml::Phase::Complete||m.name!="measure"{return Ok(());}
   let part=part.unwrap().attr("id").unwrap();
   for t in m.children_named("attributes").flat_map(|a|a.children_named("time")) {if t.attr("symbol").is_some(){counts[movement-1]+=1;}}
   if movement!=4{return Ok(());}
   let onset=starts.entry(part.to_owned()).or_insert_with(Rational::zero).clone();
   for d in m.children_named("attributes").filter_map(|a|a.value("divisions")) {divisions.insert(part.to_owned(),d.trim().parse().unwrap());}
   let label=m.attr("number").unwrap();let mut time=String::new();
   for t in m.children_named("attributes").flat_map(|a|a.children_named("time")) {
    let mut report=MappingReport::default();xml::schema(t,&mut report);assert!(!report.has_errors());
    assert!(t.attrs.keys().all(|k|k=="symbol"));
    let beats=t.value("beats").unwrap().trim().parse::<u32>().unwrap();let unit=t.value("beat-type").unwrap().trim().parse::<u32>().unwrap();let symbol=t.attr("symbol").map(|s|TimeSymbol::from_name(s).unwrap());
    if let Some(s)=symbol {signs[if s==TimeSymbol::Common {0}else{1}]+=1;assert!(s.matches(&[beats],unit));assert_eq!(onset.to_string(),match label {"1"=>"0","209"=>"778","364"=>"1398",_=>panic!("unexpected sign location")});}
    expected.push((part.to_owned(),label.to_owned(),onset.clone(),symbol,beats,unit));meters.insert(part.to_owned(),(beats,unit));
    time=format!("<time{}><beats>{beats}</beats><beat-type>{unit}</beat-type></time>",symbol.map(|s|format!(" symbol=\"{}\"",s.name())).unwrap_or_default());
   }
   let (beats,unit)=meters[part];let length=Rational::from_integer(i64::from(beats*4/unit));
   let mut cursor=Rational::zero();let mut extent=Rational::zero();let mut head=Rational::zero();
   for child in &m.children {
    if let Some(duration)=child.value("duration") {
     let duration=&duration.trim().parse::<Rational>().unwrap()/&divisions[part];
     match child.name.as_str() {
      "backup"=>cursor=&cursor-&duration,
      "forward"=>cursor=&cursor+&duration,
      "note" if child.child("chord").is_some()=>{let end=&head+&duration;if end>extent{extent=end;}},
      "note"=>{head=cursor.clone();cursor=&cursor+&duration;},
      _=>{}
     }
     if cursor>extent{extent=cursor.clone();}
    }
   }
   assert_eq!(extent,length,"Original measure {part}/{label} extent differs from projected numeric meter");

   let attributes=if label=="1" {format!("<attributes><divisions>1</divisions>{time}<clef><sign>G</sign><line>2</line></clef></attributes>")}else if !time.is_empty(){format!("<attributes>{time}</attributes>")}else{String::new()};
   bodies.entry(part.to_owned()).or_default().push_str(&format!("<measure id=\"m_{part}_{label}\" number=\"{label}\">{attributes}<note id=\"e_{part}_{label}\"><pitch><step>C</step><octave>4</octave></pitch><duration>{length}</duration></note></measure>"));starts.insert(part.to_owned(),&onset+&length);Ok(())
  }).unwrap();
        assert_eq!(
            scan.fingerprint
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            hashes[movement - 1]
        );
        if movement != 4 {
            continue;
        }
        let mut input = String::from("<score-partwise version=\"4.0\"><part-list>");
        for p in bodies.keys() {
            input.push_str(&format!("<score-part id=\"{p}\"><part-name>Piano</part-name><score-instrument id=\"i_{p}\"><instrument-name>piano</instrument-name></score-instrument></score-part>"));
        }
        input.push_str("</part-list>");
        for (p, body) in bodies {
            input.push_str(&format!("<part id=\"{p}\">{body}</part>"));
        }
        input.push_str("</score-partwise>");
        let source = MusicXml.read(&input).unwrap().value;
        assert_eq!(expected.len(), 92);
        for (p, label, onset, symbol, beats, unit) in expected {
            let m = source
                .parts
                .iter()
                .find(|v| v.id.0 == p)
                .unwrap()
                .measures
                .iter()
                .find(|m| m.label == label)
                .unwrap();
            assert_eq!(m.start, onset);
            assert_eq!(m.states[0].staff, 1);
            let meter = m.states[0].meter.as_ref().unwrap();
            assert_eq!(meter.symbol, symbol);
            assert_eq!(meter.beats, vec![beats]);
            assert_eq!(meter.beat_type, unit);
        }
        let native = SfsAdapter.write(&source).unwrap().value;
        let recovered = SfsAdapter.read(&native).unwrap().value;
        assert_eq!(source.parts, recovered.parts);
        let again = MusicXml
            .read(&MusicXml.write(&recovered).unwrap().value)
            .unwrap()
            .value;
        assert_eq!(source.parts, again.parts);
        let mut plain = source.clone();
        for p in &mut plain.parts {
            for m in &mut p.measures {
                for s in &mut m.states {
                    if let Some(m) = &mut s.meter {
                        m.symbol = None;
                    }
                }
            }
        }
        assert_eq!(
            scorestream_app::playback::plan_source(&native).unwrap(),
            scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                .unwrap()
        );
    }
    assert_eq!(counts, [0, 0, 0, 69]);
    assert_eq!(signs, [46, 23]);
    eprintln!(
        "Pinned symbols {counts:?}; common/cut {signs:?}; 92 source meter states across23 parts, exact onsets0/616/778/1398 in projected numeric-meter timeline. Not full original import."
    );
}
