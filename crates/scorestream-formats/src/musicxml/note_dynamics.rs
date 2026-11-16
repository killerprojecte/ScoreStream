//! Note-local MIDI NoteOn velocity percentage, separate from scope dynamics.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::error,
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::Rational;
pub(super) fn read(n: &Node) -> Result<Option<Rational>, MappingReport> {
    n.attr("dynamics")
        .map(|_| {
            let value = exact_decimal(&n.scalar_attribute("dynamics"))?;
            if value.is_negative() {
                return Err(error(
                    n,
                    "note_dynamics.value",
                    "Note dynamics must be nonnegative",
                ));
            }
            Ok(value)
        })
        .transpose()
}
pub(super) fn attribute(value: &Option<Rational>) -> String {
    value
        .as_ref()
        .map(|v| {
            format!(
                " dynamics=\"{}\"",
                decimal_text(v).expect("validated note dynamics")
            )
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod corpus {
    use super::super::{MusicXml, xml};
    use super::*;
    use crate::{ScoreAdapter, sfs::SfsAdapter};
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned original XML/MXL directory"]
    fn original_682_note_dynamics_values_retain_owner_and_tie_role() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for (i, hash) in hashes.iter().enumerate() {
            let mut counts = [0usize; 4];
            let scan=xml::walk(BufReader::new(File::open(root.join(format!("Beethoven_Op.67_{}.musicxml",i+1))).unwrap()),|m,_,phase| {
                if phase!=xml::Phase::Complete || m.name!="measure" {return Ok(())}
                for n in m.children_named("note") {
                    let Some(value)=read(n)? else {continue};
                    assert!(n.child("grace").is_none() && n.child("chord").is_none() && n.child("rest").is_none());
                    let start=n.children_named("tie").any(|t|t.attr("type")==Some("start"));
                    let stop=n.children_named("tie").any(|t|t.attr("type")==Some("stop"));
                    let role=match (start,stop) {(false,false)=>0,(true,false)=>1,(false,true)=>2,(true,true)=>3};counts[role]+=1;
                    let make=|id:&str,d:u8,attr:bool,start:bool,stop:bool| {
                        let mut ties=String::new();let mut notations=String::new();
                        for (present,phase) in [(stop,"stop"),(start,"start")] {if present {ties.push_str(&format!("<tie type=\"{phase}\"/>"));notations.push_str(&format!("<tied type=\"{phase}\"/>"));}}
                        format!("<note id=\"{id}\"{}><pitch><step>C</step><octave>4</octave></pitch><duration>{d}</duration>{ties}<voice>1</voice>{}</note>",if attr {attribute(&Some(value.clone()))}else{String::new()},if notations.is_empty(){String::new()}else{format!("<notations>{notations}</notations>")})
                    };
                    // A dedicated family fixture retains the original attribute's tie role.
                    // It does not assert original score timing/import is otherwise supported.
                    let body=match role {0=>make("owner",4,true,false,false),1=>format!("{}{}",make("owner",2,true,true,false),make("tail",2,false,false,true)),2=>format!("{}{}",make("head",2,false,true,false),make("owner",2,true,false,true)),_=>format!("{}{}{}",make("head",1,false,true,false),make("owner",2,true,true,true),make("tail",1,false,false,true))};
                    let source=format!(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"#);
                    let model=MusicXml.read(&source).unwrap().value;let owner=model.parts[0].measures[0].events.iter().find(|e|e.id.0=="owner").unwrap();assert_eq!(owner.attack_dynamics,Some(value));
                    let sfs=SfsAdapter.write(&model).unwrap().value;let restored=SfsAdapter.read(&sfs).unwrap().value;assert_eq!(restored,model);assert_eq!(MusicXml.read(&MusicXml.write(&restored).unwrap().value).unwrap().value,model);
                    let playback=scorestream_app::playback::plan_source(&sfs);if stop {assert!(playback.unwrap_err().contains("note_dynamics.tie_continuation_unsupported"));}else{assert!(playback.is_ok());}
                }
                Ok(())
            }).unwrap();
            assert_eq!(
                &scan
                    .fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hash
            );
            assert_eq!(
                counts,
                if i == 0 {
                    [640, 16, 19, 7]
                } else {
                    [0, 0, 0, 0]
                }
            );
            println!(
                "movement {} original note dynamics roles [untied,start,stop,stop+start]: {:?}; unchanged hash {}",
                i + 1,
                counts,
                hash
            );
        }
    }
}
